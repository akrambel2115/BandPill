use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, NOTIFY_ICON_INFOTIP_FLAGS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos, SetForegroundWindow,
    TrackPopupMenuEx, HICON, MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, TPM_RETURNCMD, WM_USER,
};

use crate::tracker::NetworkStats;
use crate::ui::icon::create_tray_icon;
use crate::utils::{format_usage, to_wide_chars};

pub const WM_TRAY_ICON: u32 = WM_USER + 101;

// Menu Command IDs
pub const CMD_OPEN_DASHBOARD: u32 = 1001;
pub const CMD_OPEN_DATASAVER: u32 = 1002;
pub const CMD_APPLY_ALL_PROTECTIONS: u32 = 1003;
pub const CMD_STOP_UPDATE_UPLOADS: u32 = 1004;
pub const CMD_TOGGLE_METERED: u32 = 1005;
pub const CMD_KILL_BITS: u32 = 1006;
pub const CMD_RESET_TODAY: u32 = 1007;
pub const CMD_TOGGLE_PILL: u32 = 1008;
pub const CMD_EXIT: u32 = 1009;

pub struct TrayManager {
    hwnd: HWND,
    current_icon: Option<HICON>,
    last_rounded_mb: i64,
    last_unit_mode: String,
}

impl TrayManager {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            current_icon: None,
            last_rounded_mb: -1,
            last_unit_mode: String::new(),
        }
    }

    pub fn init(&mut self, unit_mode: &str) {
        let icon = create_tray_icon(0.0, unit_mode);
        self.current_icon = icon;

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_TIP | NIF_ICON,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: icon.unwrap_or_default(),
            ..Default::default()
        };

        let tip = to_wide_chars("BandPill - Loading...");
        let copy_len = tip.len().min(nid.szTip.len());
        nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
    }

    pub fn update(&mut self, stats: &NetworkStats, unit_mode: &str) {
        let rounded_mb = stats.mb_total.round() as i64;
        let mut flags = NIF_TIP;

        if rounded_mb != self.last_rounded_mb || unit_mode != self.last_unit_mode {
            self.last_rounded_mb = rounded_mb;
            self.last_unit_mode = unit_mode.to_string();

            if let Some(old_hicon) = self.current_icon.take() {
                unsafe {
                    let _ = DestroyIcon(old_hicon);
                }
            }

            let new_icon = create_tray_icon(stats.mb_total, unit_mode);
            self.current_icon = new_icon;
            flags |= NIF_ICON;
        }

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: flags,
            hIcon: self.current_icon.unwrap_or_default(),
            ..Default::default()
        };

        let (_, _, full_usage) = format_usage(stats.mb_total, unit_mode);
        let tip_text = format!(
            "Today: {}\n⬇ {:.1} MB | ⬆ {:.1} MB\nSpeed: ⬇ {} ⬆ {}\nAdapter: {}",
            full_usage,
            stats.mb_recv,
            stats.mb_sent,
            stats.speed_down_str,
            stats.speed_up_str,
            stats.active_adapter
        );

        let wide_tip = to_wide_chars(&tip_text);
        let copy_len = wide_tip.len().min(nid.szTip.len() - 1);
        nid.szTip[..copy_len].copy_from_slice(&wide_tip[..copy_len]);

        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn show_notification(&self, title: &str, message: &str) {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_INFO,
            dwInfoFlags: NOTIFY_ICON_INFOTIP_FLAGS(1), // NIIF_INFO
            ..Default::default()
        };

        let wide_title = to_wide_chars(title);
        let t_len = wide_title.len().min(nid.szInfoTitle.len() - 1);
        nid.szInfoTitle[..t_len].copy_from_slice(&wide_title[..t_len]);

        let wide_msg = to_wide_chars(message);
        let m_len = wide_msg.len().min(nid.szInfo.len() - 1);
        nid.szInfo[..m_len].copy_from_slice(&wide_msg[..m_len]);

        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn show_menu(&self, pill_visible: bool) -> u32 {
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_err() {
                return 0;
            }
            let hmenu = menu.unwrap();

            let t_dash = to_wide_chars("📊 Open Details Dashboard");
            let t_ds = to_wide_chars("🛡️ Open Data Saver Tools");
            let t_apply = to_wide_chars("⚡ Apply All 3 Data Protections");
            let t_p2p = to_wide_chars("🛑 Stop Windows Update Uploads (Delivery Optimization)");
            let t_metered = to_wide_chars("📶 Toggle Metered Connection");
            let t_bits = to_wide_chars("🛑 Kill BITS Background Service");
            let t_reset = to_wide_chars("🔄 Reset Today's Counter");
            let t_pill = to_wide_chars("Show Taskbar Pill Widget");
            let t_exit = to_wide_chars("❌ Exit");

            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_OPEN_DASHBOARD as usize,
                PCWSTR(t_dash.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_OPEN_DATASAVER as usize,
                PCWSTR(t_ds.as_ptr()),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR(ptr::null()));

            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_APPLY_ALL_PROTECTIONS as usize,
                PCWSTR(t_apply.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_STOP_UPDATE_UPLOADS as usize,
                PCWSTR(t_p2p.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_TOGGLE_METERED as usize,
                PCWSTR(t_metered.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_KILL_BITS as usize,
                PCWSTR(t_bits.as_ptr()),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR(ptr::null()));

            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_RESET_TODAY as usize,
                PCWSTR(t_reset.as_ptr()),
            );

            let pill_check = if pill_visible {
                MF_CHECKED
            } else {
                MF_UNCHECKED
            };
            let _ = AppendMenuW(
                hmenu,
                MF_STRING | pill_check,
                CMD_TOGGLE_PILL as usize,
                PCWSTR(t_pill.as_ptr()),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR(ptr::null()));

            let _ = AppendMenuW(hmenu, MF_STRING, CMD_EXIT as usize, PCWSTR(t_exit.as_ptr()));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);

            let cmd = TrackPopupMenuEx(
                hmenu,
                (TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD).0,
                pt.x,
                pt.y,
                self.hwnd,
                None,
            );

            let _ = DestroyMenu(hmenu);
            cmd.0 as u32
        }
    }

    pub fn cleanup(&mut self) {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            ..Default::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            if let Some(hicon) = self.current_icon.take() {
                let _ = DestroyIcon(hicon);
            }
        }
    }
}
