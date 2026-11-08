#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::ptr;
use std::sync::mpsc::{channel, Receiver};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowRect,
    LoadCursorW, PostQuitMessage, RegisterClassExW, SetTimer, SetWindowPos, TranslateMessage,
    IDC_ARROW, MSG, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, WM_DESTROY, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW,
    WS_OVERLAPPED,
};

mod config;
mod datasaver;
mod ipc;
mod net;
mod storage;
mod tracker;
mod ui;
mod utils;

use config::{load_config, save_config, set_autostart, Config};
use datasaver::DataSaverManager;
use ipc::{check_or_wake_instance, start_ipc_server, IpcCommand};
use tracker::{AlertData, NetworkTracker};
use ui::flyout::{FluentFlyout, FlyoutAction, FlyoutTab, FLYOUT_CLASS_NAME};
use ui::pill::{
    TaskbarPillWidget, PILL_CLASS_NAME, PILL_CMD_APPLY_ALL, PILL_CMD_BITS, PILL_CMD_DASHBOARD,
    PILL_CMD_DATASAVER, PILL_CMD_EXIT, PILL_CMD_HIDE, PILL_CMD_LOCK, PILL_CMD_METERED,
    PILL_CMD_RESET, PILL_CMD_STOP_UPDATE,
};
use ui::toast::{AlertToastWindow, ToastAction, TOAST_CLASS_NAME};
use ui::tray::{
    TrayManager, CMD_APPLY_ALL_PROTECTIONS, CMD_EXIT, CMD_KILL_BITS, CMD_OPEN_DASHBOARD,
    CMD_OPEN_DATASAVER, CMD_RESET_TODAY, CMD_STOP_UPDATE_UPLOADS, CMD_TOGGLE_METERED,
    CMD_TOGGLE_PILL, WM_TRAY_ICON,
};
use utils::to_wide_chars;

const MSG_CLASS_NAME: &str = "BandPillMsgClass";

thread_local! {
    static APP_INSTANCE: RefCell<Option<*mut App>> = const { RefCell::new(None) };
}

pub struct App {
    pub config: Config,
    pub tracker: NetworkTracker,
    pub msg_hwnd: HWND,
    pub tray: TrayManager,
    pub pill: TaskbarPillWidget,
    pub flyout: FluentFlyout,
    pub active_toast: Option<AlertToastWindow>,
    pub ipc_rx: Receiver<IpcCommand>,
    pub cycle_count: u64,
    pub h_instance: HINSTANCE,
}

impl App {
    pub fn new(h_instance: HINSTANCE, ipc_rx: Receiver<IpcCommand>) -> Box<Self> {
        let config = load_config();
        let tracker = NetworkTracker::new(config.clone());

        // Create message-only receiver window
        let wide_class = to_wide_chars(MSG_CLASS_NAME);
        let wide_title = to_wide_chars("MsgReceiver");
        let msg_hwnd = unsafe {
            CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
                PCWSTR(wide_class.as_ptr()),
                PCWSTR(wide_title.as_ptr()),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                h_instance,
                None,
            )
            .unwrap_or(HWND(ptr::null_mut()))
        };

        let mut tray = TrayManager::new(msg_hwnd);
        tray.init(&config.unit_mode);

        let pill = TaskbarPillWidget::new(h_instance, &config);
        let flyout = FluentFlyout::new(h_instance);

        let mut app = Box::new(Self {
            config,
            tracker,
            msg_hwnd,
            tray,
            pill,
            flyout,
            active_toast: None,
            ipc_rx,
            cycle_count: 0,
            h_instance,
        });

        // Set 1-second update timer
        unsafe {
            let _ = SetTimer(msg_hwnd, 1, 1000, None);
        }

        // Show flyout on initial start so user immediately sees the tracker
        app.flyout.show(Some(FlyoutTab::Overview));
        app.tick();

        app
    }

    pub fn tick(&mut self) {
        // 1. Process pending IPC commands
        while let Ok(cmd) = self.ipc_rx.try_recv() {
            match cmd {
                IpcCommand::Wake | IpcCommand::Show => {
                    self.flyout.show(Some(FlyoutTab::Overview));
                }
                IpcCommand::ShowTab(tab_name) => {
                    let tab = if tab_name == "datasaver" {
                        FlyoutTab::DataSaver
                    } else {
                        FlyoutTab::Overview
                    };
                    self.flyout.show(Some(tab));
                }
                IpcCommand::TestAlert => {
                    let test_alert = AlertData {
                        pid: 18452,
                        name: "Steam Client (Test)".to_string(),
                        exe: "steam.exe".to_string(),
                        mb_downloaded: 104.8,
                        threshold_mb: 100,
                        duration_sec: 42,
                        speed: 4800000.0,
                        speed_str: "4.8 MB/s".to_string(),
                        is_critical: false,
                        is_svchost: false,
                        timestamp: crate::storage::current_epoch_seconds(),
                    };
                    self.show_toast(test_alert);
                }
                IpcCommand::Reset => {
                    self.reset_today();
                }
                IpcCommand::Exit => {
                    self.exit_app();
                }
            }
        }

        // 2. Network tracking cycle
        let alerts = self.tracker.update_cycle();
        let stats = self.tracker.get_stats();

        // 3. Update System Tray
        self.tray.update(&stats, &self.config.unit_mode);

        // 4. Update Taskbar Pill
        if self.config.show_taskbar_pill {
            self.pill
                .update_usage(stats.mb_total, &self.config.unit_mode);
        }

        // 5. Update Flyout if open
        if self.flyout.is_open {
            self.flyout.repaint();
        }

        // 6. Fire newly triggered alerts
        for alert in alerts {
            self.show_toast(alert);
        }

        // 7. Periodic state save every 10 seconds
        self.cycle_count += 1;
        if self.cycle_count.is_multiple_of(10) {
            self.tracker.save_state();
        }
    }

    pub fn show_toast(&mut self, alert: AlertData) {
        if let Some(t) = self.active_toast.take() {
            t.close();
        }
        self.active_toast = Some(AlertToastWindow::new(self.h_instance, alert));
    }

    pub fn reset_today(&mut self) {
        self.tracker.reset_today();
        let stats = self.tracker.get_stats();
        self.tray.update(&stats, &self.config.unit_mode);
        self.pill
            .update_usage(stats.mb_total, &self.config.unit_mode);
        self.flyout.set_feedback("Reset today's counter to 0 MB.");
    }

    pub fn toggle_pill(&mut self) {
        self.config.show_taskbar_pill = !self.config.show_taskbar_pill;
        self.pill.set_visible(self.config.show_taskbar_pill);
        let _ = save_config(&self.config);
        self.flyout.repaint();
    }

    pub fn exit_app(&mut self) {
        self.tracker.save_state();
        self.tray.cleanup();
        if let Some(t) = self.active_toast.take() {
            t.close();
        }
        self.pill.set_visible(false);
        self.flyout.close();
        unsafe {
            PostQuitMessage(0);
        }
    }
}

// ----------------------------------------------------------------------------
// Window Procedures
// ----------------------------------------------------------------------------

unsafe extern "system" fn msg_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    (*app_ptr).tick();
                }
            });
            LRESULT(0)
        }
        WM_TRAY_ICON => {
            let event = (lparam.0 & 0xffff) as u32;
            if event == windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP {
                APP_INSTANCE.with(|cell| {
                    if let Some(app_ptr) = *cell.borrow() {
                        (*app_ptr).flyout.toggle();
                    }
                });
            } else if event == windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP {
                APP_INSTANCE.with(|cell| {
                    if let Some(app_ptr) = *cell.borrow() {
                        let pill_vis = (*app_ptr).config.show_taskbar_pill;
                        let cmd = (*app_ptr).tray.show_menu(pill_vis);
                        handle_tray_command(app_ptr, cmd);
                    }
                });
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn handle_tray_command(app: *mut App, cmd: u32) {
    match cmd {
        CMD_OPEN_DASHBOARD => (*app).flyout.show(Some(FlyoutTab::Overview)),
        CMD_OPEN_DATASAVER => (*app).flyout.show(Some(FlyoutTab::DataSaver)),
        CMD_APPLY_ALL_PROTECTIONS => {
            let ok = DataSaverManager::apply_all_protections();
            let msg = if ok {
                "Applied all 3 bandwidth protections!"
            } else {
                "Applied protections (check Admin prompt)."
            };
            (*app).tray.show_notification("Data Saver", msg);
            (*app).flyout.repaint();
        }
        CMD_STOP_UPDATE_UPLOADS => {
            let ok = DataSaverManager::disable_delivery_optimization();
            let msg = if ok {
                "Disabled Delivery Optimization P2P uploads!"
            } else {
                "Delivery Optimization update initiated."
            };
            (*app).tray.show_notification("Data Saver", msg);
            (*app).flyout.repaint();
        }
        CMD_TOGGLE_METERED => {
            let (is_met, _, _) = DataSaverManager::get_metered_status(true);
            let _ = DataSaverManager::toggle_metered_connection(!is_met);
            let state_str = if !is_met { "METERED" } else { "UNMETERED" };
            (*app)
                .tray
                .show_notification("Data Saver", &format!("Connection set to {}!", state_str));
            (*app).flyout.repaint();
        }
        CMD_KILL_BITS => {
            let _ = DataSaverManager::kill_and_restrict_bits();
            (*app)
                .tray
                .show_notification("Data Saver", "BITS service stopped & set to Manual!");
            (*app).flyout.repaint();
        }
        CMD_RESET_TODAY => (*app).reset_today(),
        CMD_TOGGLE_PILL => (*app).toggle_pill(),
        CMD_EXIT => (*app).exit_app(),
        _ => {}
    }
}

unsafe extern "system" fn pill_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    let style = (*app_ptr).config.pill_style.clone();
                    (*app_ptr).pill.render_pill(hdc, &style);
                }
            });
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEMOVE => {
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    if !(*app_ptr).pill.is_hovered {
                        (*app_ptr).pill.is_hovered = true;
                        (*app_ptr).pill.repaint();
                    }
                    if (*app_ptr).pill.is_dragging && !(*app_ptr).pill.locked {
                        let mut pt = POINT::default();
                        let _ = GetCursorPos(&mut pt);
                        let dx = pt.x - (*app_ptr).pill.drag_start_cursor.x;
                        let dy = pt.y - (*app_ptr).pill.drag_start_cursor.y;
                        let new_x = (*app_ptr).pill.drag_start_win.x + dx;
                        let new_y = (*app_ptr).pill.drag_start_win.y + dy;
                        let _ = SetWindowPos(
                            hwnd,
                            windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST,
                            new_x,
                            new_y,
                            0,
                            0,
                            SWP_NOACTIVATE | SWP_NOSIZE | SWP_SHOWWINDOW,
                        );
                    }
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let _ = SetCapture(hwnd);
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let mut rect = RECT::default();
                    let _ = GetWindowRect(hwnd, &mut rect);

                    (*app_ptr).pill.drag_start_cursor = pt;
                    (*app_ptr).pill.drag_start_win = POINT {
                        x: rect.left,
                        y: rect.top,
                    };
                    (*app_ptr).pill.is_dragging = false;
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let dx = (pt.x - (*app_ptr).pill.drag_start_cursor.x).abs();
                    let dy = (pt.y - (*app_ptr).pill.drag_start_cursor.y).abs();

                    if dx > 4 || dy > 4 {
                        let mut rect = RECT::default();
                        let _ = GetWindowRect(hwnd, &mut rect);
                        (*app_ptr).config.pill_x = rect.left;
                        (*app_ptr).config.pill_y = rect.top;
                        let _ = save_config(&(*app_ptr).config);
                    } else {
                        (*app_ptr).flyout.toggle();
                    }
                    (*app_ptr).pill.is_dragging = false;
                }
            });
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    let cmd = (*app_ptr).pill.show_menu();
                    match cmd {
                        PILL_CMD_LOCK => {
                            (*app_ptr).pill.locked = !(*app_ptr).pill.locked;
                            (*app_ptr).config.pill_locked = (*app_ptr).pill.locked;
                            let _ = save_config(&(*app_ptr).config);
                        }
                        PILL_CMD_DASHBOARD => (*app_ptr).flyout.show(Some(FlyoutTab::Overview)),
                        PILL_CMD_DATASAVER => (*app_ptr).flyout.show(Some(FlyoutTab::DataSaver)),
                        PILL_CMD_APPLY_ALL => {
                            let _ = DataSaverManager::apply_all_protections();
                            (*app_ptr)
                                .tray
                                .show_notification("Data Saver", "Applied all 3 protections!");
                            (*app_ptr).flyout.repaint();
                        }
                        PILL_CMD_STOP_UPDATE => {
                            let _ = DataSaverManager::disable_delivery_optimization();
                            (*app_ptr)
                                .tray
                                .show_notification("Data Saver", "Disabled P2P update uploads!");
                            (*app_ptr).flyout.repaint();
                        }
                        PILL_CMD_METERED => {
                            let (is_met, _, _) = DataSaverManager::get_metered_status(true);
                            let _ = DataSaverManager::toggle_metered_connection(!is_met);
                            let s = if !is_met { "METERED" } else { "UNMETERED" };
                            (*app_ptr)
                                .tray
                                .show_notification("Data Saver", &format!("Network set to {}!", s));
                            (*app_ptr).flyout.repaint();
                        }
                        PILL_CMD_BITS => {
                            let _ = DataSaverManager::kill_and_restrict_bits();
                            (*app_ptr).tray.show_notification(
                                "Data Saver",
                                "BITS service killed & restricted!",
                            );
                            (*app_ptr).flyout.repaint();
                        }
                        PILL_CMD_RESET => (*app_ptr).reset_today(),
                        PILL_CMD_HIDE => (*app_ptr).toggle_pill(),
                        PILL_CMD_EXIT => (*app_ptr).exit_app(),
                        _ => {}
                    }
                }
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn flyout_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    let tracker_ref = &(*app_ptr).tracker;
                    (*app_ptr).flyout.render(hdc, tracker_ref);
                }
            });
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    let action = (*app_ptr).flyout.handle_click(x, y);
                    handle_flyout_action(app_ptr, action);
                }
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn handle_flyout_action(app: *mut App, action: FlyoutAction) {
    match action {
        FlyoutAction::None => {}
        FlyoutAction::Close => (*app).flyout.close(),
        FlyoutAction::SwitchTab(tab) => {
            (*app).flyout.active_tab = tab;
            (*app).flyout.repaint();
        }
        FlyoutAction::SetTopAppsLimit(limit) => {
            (*app).config.top_apps_limit = limit;
            let _ = save_config(&(*app).config);
            (*app).flyout.repaint();
        }
        FlyoutAction::StopAndBlockApp(name) => {
            let (_, msg) = (*app).tracker.stop_and_block_app(&name);
            (*app).flyout.set_feedback(&msg);
            (*app).flyout.repaint();
        }
        FlyoutAction::UnblockApp(name) => {
            let (_, msg) = (*app).tracker.unblock_app(&name);
            (*app).flyout.set_feedback(&msg);
            (*app).flyout.repaint();
        }
        FlyoutAction::UnblockAll => {
            let blocked: Vec<String> = (*app).tracker.blocked_apps.iter().cloned().collect();
            for b in blocked {
                let _ = (*app).tracker.unblock_app(&b);
            }
            (*app).flyout.set_feedback("Unblocked all applications.");
            (*app).flyout.repaint();
        }
        FlyoutAction::KillProcess(pid) => {
            let (_ok, msg) = (*app).tracker.kill_process(pid);
            (*app).flyout.set_feedback(&msg);
            (*app).flyout.repaint();
        }
        FlyoutAction::SnoozeAlert(pid) => {
            (*app).tracker.snooze_process(pid, None, 900);
            (*app).flyout.set_feedback("Alert snoozed for 15 minutes.");
            (*app).flyout.repaint();
        }
        FlyoutAction::ToggleAlertEnabled => {
            (*app).config.heavy_download_alert_enabled =
                !(*app).config.heavy_download_alert_enabled;
            (*app).tracker.config.heavy_download_alert_enabled =
                (*app).config.heavy_download_alert_enabled;
            let _ = save_config(&(*app).config);
            (*app).flyout.repaint();
        }
        FlyoutAction::SetAlertThreshold(mb) => {
            (*app).config.heavy_download_threshold_mb = mb;
            (*app).tracker.config.heavy_download_threshold_mb = mb;
            let _ = save_config(&(*app).config);
            (*app).flyout.repaint();
        }
        FlyoutAction::SetUnitMode(mode) => {
            (*app).config.unit_mode = mode;
            let _ = save_config(&(*app).config);
            let stats = (*app).tracker.get_stats();
            (*app).tray.update(&stats, &(*app).config.unit_mode);
            (*app)
                .pill
                .update_usage(stats.mb_total, &(*app).config.unit_mode);
            (*app).flyout.repaint();
        }
        FlyoutAction::TogglePill(show) => {
            (*app).config.show_taskbar_pill = show;
            (*app).pill.set_visible(show);
            let _ = save_config(&(*app).config);
            (*app).flyout.repaint();
        }
        FlyoutAction::ToggleAutostart(enable) => {
            let exe_path = std::env::current_exe().unwrap_or_default();
            let cmd = exe_path.to_str().unwrap_or("");
            let ok = set_autostart(enable, Some(cmd));
            if ok {
                (*app).config.start_with_windows = enable;
                let _ = save_config(&(*app).config);
                let msg = if enable {
                    "Autostart enabled."
                } else {
                    "Autostart disabled."
                };
                (*app).flyout.set_feedback(msg);
            }
            (*app).flyout.repaint();
        }
        FlyoutAction::ResetToday => (*app).reset_today(),
        FlyoutAction::ExitApp => (*app).exit_app(),
        FlyoutAction::ApplyAllProtections => {
            let _ = DataSaverManager::apply_all_protections();
            (*app)
                .flyout
                .set_feedback("Applied all 3 bandwidth leak protections!");
            (*app).flyout.repaint();
        }
        FlyoutAction::StopDeliveryOptimization => {
            let _ = DataSaverManager::disable_delivery_optimization();
            (*app)
                .flyout
                .set_feedback("Disabled Delivery Optimization P2P uploads.");
            (*app).flyout.repaint();
        }
        FlyoutAction::OpenDeliveryOptimizationSettings => {
            DataSaverManager::open_delivery_optimization_settings();
        }
        FlyoutAction::ToggleMetered => {
            let (is_met, _, _) = DataSaverManager::get_metered_status(true);
            let _ = DataSaverManager::toggle_metered_connection(!is_met);
            let state_str = if !is_met { "Metered" } else { "Unrestricted" };
            (*app)
                .flyout
                .set_feedback(&format!("Set active network to {}.", state_str));
            (*app).flyout.repaint();
        }
        FlyoutAction::OpenNetworkSettings => {
            DataSaverManager::open_network_settings();
        }
        FlyoutAction::KillBits => {
            let _ = DataSaverManager::kill_and_restrict_bits();
            (*app)
                .flyout
                .set_feedback("Terminated BITS service and set startup to Manual.");
            (*app).flyout.repaint();
        }
        FlyoutAction::OpenServicesMsc => {
            DataSaverManager::open_services_msc();
        }
    }
}

unsafe extern "system" fn toast_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    if let Some(toast) = (*app_ptr).active_toast.as_ref() {
                        toast.render(hdc);
                    }
                }
            });
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            APP_INSTANCE.with(|cell| {
                if let Some(app_ptr) = *cell.borrow() {
                    let mut action = ToastAction::None;
                    if let Some(toast) = (*app_ptr).active_toast.as_ref() {
                        action = toast.handle_click(x, y);
                    }
                    match action {
                        ToastAction::Kill(pid) => {
                            let (_ok, msg) = (*app_ptr).tracker.kill_process(pid);
                            (*app_ptr).flyout.set_feedback(&msg);
                            if let Some(t) = (*app_ptr).active_toast.take() {
                                t.close();
                            }
                        }
                        ToastAction::StopBits => {
                            let _ = DataSaverManager::kill_and_restrict_bits();
                            (*app_ptr).flyout.show(Some(FlyoutTab::DataSaver));
                            if let Some(t) = (*app_ptr).active_toast.take() {
                                t.close();
                            }
                        }
                        ToastAction::Snooze(pid) => {
                            (*app_ptr).tracker.snooze_process(pid, None, 900);
                            if let Some(t) = (*app_ptr).active_toast.take() {
                                t.close();
                            }
                        }
                        ToastAction::Close => {
                            if let Some(t) = (*app_ptr).active_toast.take() {
                                t.close();
                            }
                        }
                        ToastAction::None => {}
                    }
                }
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn register_classes(h_instance: HINSTANCE) {
    unsafe {
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();

        // 1. Message-only window class
        let w_msg = to_wide_chars(MSG_CLASS_NAME);
        let wc_msg = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(msg_wnd_proc),
            hInstance: h_instance,
            lpszClassName: PCWSTR(w_msg.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc_msg);

        // 2. Pill window class
        let w_pill = to_wide_chars(PILL_CLASS_NAME);
        let wc_pill = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(pill_wnd_proc),
            hInstance: h_instance,
            hCursor: cursor,
            lpszClassName: PCWSTR(w_pill.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc_pill);

        // 3. Flyout window class
        let w_flyout = to_wide_chars(FLYOUT_CLASS_NAME);
        let wc_flyout = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(flyout_wnd_proc),
            hInstance: h_instance,
            hCursor: cursor,
            lpszClassName: PCWSTR(w_flyout.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc_flyout);

        // 4. Toast window class
        let w_toast = to_wide_chars(TOAST_CLASS_NAME);
        let wc_toast = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(toast_wnd_proc),
            hInstance: h_instance,
            hCursor: cursor,
            lpszClassName: PCWSTR(w_toast.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc_toast);
    }
}

fn main() {
    // 1. Set Per-Monitor V2 DPI awareness for crisp Windows 11 rendering
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    // 2. Check if an existing instance is already running
    if check_or_wake_instance() {
        println!("Existing instance notified and brought to foreground.");
        return;
    }

    // 3. Start local IPC server for single-instance wakeups & control
    let (ipc_tx, ipc_rx) = channel::<IpcCommand>();
    start_ipc_server(ipc_tx);

    let h_instance = HINSTANCE(ptr::null_mut());
    register_classes(h_instance);

    // 4. Initialize application state
    let app = App::new(h_instance, ipc_rx);
    let app_raw = Box::into_raw(app);

    APP_INSTANCE.with(|cell| {
        *cell.borrow_mut() = Some(app_raw);
    });

    // 5. Windows standard message loop
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Clean up
    APP_INSTANCE.with(|cell| {
        if let Some(ptr) = cell.borrow_mut().take() {
            unsafe {
                let _ = Box::from_raw(ptr);
            }
        }
    });
}
