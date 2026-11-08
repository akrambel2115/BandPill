use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush,
    DeleteDC, DeleteObject, FillRect, GetStockObject, InvalidateRect, RoundRect, SelectObject,
    SetBkMode, SetTextColor, DT_CENTER, DT_LEFT, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK,
    FW_BOLD, FW_NORMAL, HDC, HFONT, PS_SOLID, SRCCOPY, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetSystemMetrics, SetWindowPos, ShowWindow, HWND_TOPMOST, SM_CXSCREEN,
    SM_CYSCREEN, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::datasaver::DataSaverManager;
use crate::tracker::{NetworkStats, NetworkTracker};
use crate::ui::theme::{apply_fluent_window_styling, is_system_dark_theme, rgb};
use crate::utils::{format_usage, gdi_draw_text, to_wide_chars};

pub const FLYOUT_CLASS_NAME: &str = "BandPillFlyoutClass";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlyoutTab {
    Overview,
    DataSaver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlyoutAction {
    None,
    Close,
    SwitchTab(FlyoutTab),
    SetTopAppsLimit(i32),
    StopAndBlockApp(String),
    UnblockApp(String),
    UnblockAll,
    KillProcess(u32),
    SnoozeAlert(u32),
    ToggleAlertEnabled,
    SetAlertThreshold(u64),
    SetUnitMode(String),
    TogglePill(bool),
    ToggleAutostart(bool),
    ResetToday,
    ExitApp,
    // Data Saver Actions
    ApplyAllProtections,
    StopDeliveryOptimization,
    OpenDeliveryOptimizationSettings,
    ToggleMetered,
    OpenNetworkSettings,
    KillBits,
    OpenServicesMsc,
}

pub struct FluentFlyout {
    pub hwnd: HWND,
    pub is_open: bool,
    pub active_tab: FlyoutTab,
    pub width: i32,
    pub height: i32,
    pub feedback_message: String,
    pub hit_targets: Vec<(RECT, FlyoutAction)>,
}

impl FluentFlyout {
    pub fn new(h_instance: windows::Win32::Foundation::HINSTANCE) -> Self {
        let width = 410;
        let height = 780;

        let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        let x = sw - width - 20;
        let y = (sh - height - 52).max(10);

        let wide_class = to_wide_chars(FLYOUT_CLASS_NAME);
        let wide_title = to_wide_chars("NetworkUsageDashboard");

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                PCWSTR(wide_class.as_ptr()),
                PCWSTR(wide_title.as_ptr()),
                WS_POPUP,
                x,
                y,
                width,
                height,
                None,
                None,
                h_instance,
                None,
            )
            .unwrap_or(HWND(std::ptr::null_mut()))
        };

        if !hwnd.is_invalid() {
            apply_fluent_window_styling(hwnd, is_system_dark_theme());
        }

        Self {
            hwnd,
            is_open: false,
            active_tab: FlyoutTab::Overview,
            width,
            height,
            feedback_message: String::new(),
            hit_targets: Vec::new(),
        }
    }

    pub fn show(&mut self, tab: Option<FlyoutTab>) {
        if let Some(t) = tab {
            self.active_tab = t;
        }
        self.is_open = true;

        let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        let x = sw - self.width - 20;
        let y = (sh - self.height - 52).max(10);

        if !self.hwnd.is_invalid() {
            apply_fluent_window_styling(self.hwnd, is_system_dark_theme());
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    x,
                    y,
                    self.width,
                    self.height,
                    SWP_SHOWWINDOW,
                );
                let _ = ShowWindow(self.hwnd, SW_SHOW);
                let _ = InvalidateRect(self.hwnd, None, false);
            }
        }
    }

    pub fn close(&mut self) {
        self.is_open = false;
        if !self.hwnd.is_invalid() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.show(None);
        }
    }

    pub fn set_feedback(&mut self, msg: &str) {
        self.feedback_message = msg.to_string();
        self.repaint();
    }

    pub fn repaint(&self) {
        if !self.hwnd.is_invalid() {
            unsafe {
                let _ = InvalidateRect(self.hwnd, None, false);
            }
        }
    }

    pub fn render(&mut self, hdc: HDC, tracker: &NetworkTracker) {
        self.hit_targets.clear();

        let is_dark = is_system_dark_theme();
        let bg_color = if is_dark {
            rgb(26, 26, 26)
        } else {
            rgb(243, 243, 243)
        };
        let card_bg = if is_dark {
            rgb(36, 36, 36)
        } else {
            rgb(255, 255, 255)
        };
        let card_border = if is_dark {
            rgb(52, 52, 52)
        } else {
            rgb(224, 224, 224)
        };
        let text_color = if is_dark {
            rgb(255, 255, 255)
        } else {
            rgb(17, 17, 17)
        };
        let sub_color = if is_dark {
            rgb(155, 155, 155)
        } else {
            rgb(105, 105, 105)
        };
        let accent_color = if is_dark {
            rgb(96, 205, 255)
        } else {
            rgb(0, 103, 192)
        };
        let amber_color = rgb(255, 185, 0);
        let red_color = rgb(232, 17, 35);

        unsafe {
            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bmp = CreateCompatibleBitmap(hdc, self.width, self.height);
            let old_bmp = SelectObject(mem_dc, mem_bmp);

            // Fill background
            let bg_brush = CreateSolidBrush(bg_color);
            let r_full = RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            };
            FillRect(mem_dc, &r_full, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Outer border
            let out_pen = CreatePen(PS_SOLID, 1, card_border);
            let old_pen = SelectObject(mem_dc, out_pen);
            let old_br = SelectObject(
                mem_dc,
                GetStockObject(windows::Win32::Graphics::Gdi::GET_STOCK_OBJECT_FLAGS(5)),
            );
            let _ = RoundRect(mem_dc, 0, 0, self.width, self.height, 12, 12);
            let _ = SelectObject(mem_dc, old_br);
            let _ = SelectObject(mem_dc, old_pen);
            let _ = DeleteObject(out_pen);

            SetBkMode(mem_dc, TRANSPARENT);

            let font_name = to_wide_chars("Segoe UI Variable Display");
            let f_title = CreateFontW(
                -16,
                0,
                0,
                0,
                FW_BOLD.0 as i32,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                PCWSTR(font_name.as_ptr()),
            );
            let f_h2 = CreateFontW(
                -13,
                0,
                0,
                0,
                FW_BOLD.0 as i32,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                PCWSTR(font_name.as_ptr()),
            );
            let f_body = CreateFontW(
                -12,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                PCWSTR(font_name.as_ptr()),
            );
            let f_body_bold = CreateFontW(
                -12,
                0,
                0,
                0,
                FW_BOLD.0 as i32,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                PCWSTR(font_name.as_ptr()),
            );
            let f_small = CreateFontW(
                -10,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                PCWSTR(font_name.as_ptr()),
            );

            let mut cur_y = 14;

            // 1. Top Header: Title + Adapter Badge + Close '✕'
            let old_f = SelectObject(mem_dc, f_title);
            SetTextColor(mem_dc, text_color);
            let mut r_title = RECT {
                left: 18,
                top: cur_y,
                right: 180,
                bottom: cur_y + 24,
            };
            gdi_draw_text(
                mem_dc,
                "Network Usage",
                &mut r_title,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Active Adapter Badge
            let stats = tracker.get_stats();
            let adapter_text = format!("● {}", stats.active_adapter);
            let _ = SelectObject(mem_dc, f_small);
            SetTextColor(mem_dc, rgb(76, 194, 255));
            let mut r_adapt = RECT {
                left: 160,
                top: cur_y + 2,
                right: self.width - 45,
                bottom: cur_y + 24,
            };
            gdi_draw_text(
                mem_dc,
                &adapter_text,
                &mut r_adapt,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Close '✕'
            let _ = SelectObject(mem_dc, f_h2);
            SetTextColor(mem_dc, sub_color);
            let r_close = RECT {
                left: self.width - 36,
                top: cur_y - 2,
                right: self.width - 12,
                bottom: cur_y + 22,
            };
            self.hit_targets.push((r_close, FlyoutAction::Close));
            let mut r_close_mut = r_close;
            gdi_draw_text(
                mem_dc,
                "✕",
                &mut r_close_mut,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            cur_y += 32;

            // 2. Segmented Tab Bar: [ 📊 Usage Overview ] [ 🛡 Data Saver ]
            let tab_h = 32;
            let tab_w = (self.width - 36) / 2;
            let r_tab1 = RECT {
                left: 18,
                top: cur_y,
                right: 18 + tab_w - 2,
                bottom: cur_y + tab_h,
            };
            let r_tab2 = RECT {
                left: 18 + tab_w + 2,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + tab_h,
            };

            self.hit_targets
                .push((r_tab1, FlyoutAction::SwitchTab(FlyoutTab::Overview)));
            self.hit_targets
                .push((r_tab2, FlyoutAction::SwitchTab(FlyoutTab::DataSaver)));

            let is_ov = self.active_tab == FlyoutTab::Overview;
            let tab1_bg = if is_ov {
                if is_dark {
                    rgb(56, 56, 56)
                } else {
                    rgb(220, 220, 220)
                }
            } else {
                card_bg
            };
            let tab2_bg = if !is_ov {
                if is_dark {
                    rgb(56, 56, 56)
                } else {
                    rgb(220, 220, 220)
                }
            } else {
                card_bg
            };

            draw_rounded_rect(mem_dc, r_tab1, 6, tab1_bg, card_border);
            draw_rounded_rect(mem_dc, r_tab2, 6, tab2_bg, card_border);

            let _ = SelectObject(mem_dc, if is_ov { f_body_bold } else { f_body });
            SetTextColor(mem_dc, if is_ov { text_color } else { sub_color });
            let mut t1_m = r_tab1;
            gdi_draw_text(
                mem_dc,
                "📊 Usage Overview",
                &mut t1_m,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(mem_dc, if !is_ov { f_body_bold } else { f_body });
            SetTextColor(mem_dc, if !is_ov { accent_color } else { sub_color });
            let mut t2_m = r_tab2;
            gdi_draw_text(
                mem_dc,
                "🛡 Data Saver",
                &mut t2_m,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            cur_y += tab_h + 10;

            // Optional Feedback Banner
            if !self.feedback_message.is_empty() {
                let r_fb = RECT {
                    left: 18,
                    top: cur_y,
                    right: self.width - 18,
                    bottom: cur_y + 24,
                };
                let fb_bg = if is_dark {
                    rgb(24, 48, 64)
                } else {
                    rgb(230, 245, 255)
                };
                draw_rounded_rect(mem_dc, r_fb, 4, fb_bg, accent_color);
                let _ = SelectObject(mem_dc, f_small);
                SetTextColor(mem_dc, accent_color);
                let mut fb_m = r_fb;
                gdi_draw_text(
                    mem_dc,
                    &self.feedback_message,
                    &mut fb_m,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                cur_y += 30;
            }

            match self.active_tab {
                FlyoutTab::Overview => {
                    self.render_overview_tab(
                        mem_dc,
                        tracker,
                        &stats,
                        cur_y,
                        is_dark,
                        card_bg,
                        card_border,
                        text_color,
                        sub_color,
                        accent_color,
                        amber_color,
                        red_color,
                        f_h2,
                        f_body_bold,
                        f_small,
                    );
                }
                FlyoutTab::DataSaver => {
                    self.render_datasaver_tab(
                        mem_dc,
                        cur_y,
                        is_dark,
                        card_bg,
                        card_border,
                        text_color,
                        sub_color,
                        accent_color,
                        f_h2,
                        f_body_bold,
                        f_small,
                    );
                }
            }

            let _ = SelectObject(mem_dc, old_f);
            let _ = DeleteObject(f_title);
            let _ = DeleteObject(f_h2);
            let _ = DeleteObject(f_body);
            let _ = DeleteObject(f_body_bold);
            let _ = DeleteObject(f_small);

            let _ = BitBlt(hdc, 0, 0, self.width, self.height, mem_dc, 0, 0, SRCCOPY);
            let _ = SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(mem_bmp);
            let _ = DeleteDC(mem_dc);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_overview_tab(
        &mut self,
        dc: HDC,
        tracker: &NetworkTracker,
        stats: &NetworkStats,
        mut cur_y: i32,
        is_dark: bool,
        card_bg: COLORREF,
        card_border: COLORREF,
        text_color: COLORREF,
        sub_color: COLORREF,
        accent_color: COLORREF,
        amber_color: COLORREF,
        red_color: COLORREF,
        f_h2: HFONT,
        f_body_bold: HFONT,
        f_small: HFONT,
    ) {
        unsafe {
            // 1. Active Heavy Download Banner (if any)
            if let Some((&pid, alert)) = tracker.active_alerts.iter().next() {
                let alert_h = 58;
                let r_al = RECT {
                    left: 18,
                    top: cur_y,
                    right: self.width - 18,
                    bottom: cur_y + alert_h,
                };
                let al_bg = if is_dark {
                    rgb(61, 29, 29)
                } else {
                    rgb(255, 240, 240)
                };
                draw_rounded_rect(dc, r_al, 6, al_bg, red_color);

                let _ = SelectObject(dc, f_body_bold);
                SetTextColor(
                    dc,
                    if is_dark {
                        rgb(255, 102, 119)
                    } else {
                        rgb(209, 52, 56)
                    },
                );
                let mut r_altxt = RECT {
                    left: 26,
                    top: cur_y + 6,
                    right: self.width - 150,
                    bottom: cur_y + 26,
                };
                let alert_title = format!("⚠️ Heavy Download: {}", alert.name);
                gdi_draw_text(
                    dc,
                    &alert_title,
                    &mut r_altxt,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );

                let _ = SelectObject(dc, f_small);
                SetTextColor(dc, sub_color);
                let mut r_alsub = RECT {
                    left: 26,
                    top: cur_y + 28,
                    right: self.width - 150,
                    bottom: cur_y + 50,
                };
                let alert_sub = format!(
                    "⚡ {:.1} MB in {}s • {}",
                    alert.mb_downloaded, alert.duration_sec, alert.speed_str
                );
                gdi_draw_text(
                    dc,
                    &alert_sub,
                    &mut r_alsub,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );

                // Kill Button
                let r_al_kill = RECT {
                    left: self.width - 138,
                    top: cur_y + 12,
                    right: self.width - 76,
                    bottom: cur_y + 44,
                };
                self.hit_targets
                    .push((r_al_kill, FlyoutAction::KillProcess(pid)));
                draw_rounded_rect(dc, r_al_kill, 4, red_color, red_color);
                let _ = SelectObject(dc, f_small);
                SetTextColor(dc, rgb(255, 255, 255));
                let mut r_km = r_al_kill;
                gdi_draw_text(
                    dc,
                    "🛑 Kill",
                    &mut r_km,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                // Snooze Button
                let r_al_snz = RECT {
                    left: self.width - 72,
                    top: cur_y + 12,
                    right: self.width - 24,
                    bottom: cur_y + 44,
                };
                self.hit_targets
                    .push((r_al_snz, FlyoutAction::SnoozeAlert(pid)));
                draw_rounded_rect(
                    dc,
                    r_al_snz,
                    4,
                    if is_dark {
                        rgb(60, 60, 60)
                    } else {
                        rgb(220, 220, 220)
                    },
                    card_border,
                );
                SetTextColor(dc, text_color);
                let mut r_sm = r_al_snz;
                gdi_draw_text(
                    dc,
                    "✔ Let",
                    &mut r_sm,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                cur_y += alert_h + 10;
            }

            // 2. Large Today's Data Card
            let data_card_h = 78;
            let r_dc = RECT {
                left: 18,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + data_card_h,
            };
            draw_rounded_rect(dc, r_dc, 8, card_bg, card_border);

            let (_, _, full_usage) = format_usage(stats.mb_total, &tracker.config.unit_mode);
            let _ = SelectObject(dc, f_h2);
            SetTextColor(dc, accent_color);
            let mut r_u = RECT {
                left: 30,
                top: cur_y + 12,
                right: 210,
                bottom: cur_y + 42,
            };
            gdi_draw_text(
                dc,
                &full_usage,
                &mut r_u,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(dc, f_small);
            SetTextColor(dc, sub_color);
            let mut r_lbl = RECT {
                left: 30,
                top: cur_y + 44,
                right: 210,
                bottom: cur_y + 66,
            };
            gdi_draw_text(
                dc,
                "Today's Total Consumption",
                &mut r_lbl,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Speeds
            let _ = SelectObject(dc, f_body_bold);
            SetTextColor(dc, text_color);
            let speed_dn_txt = format!("⬇ {}", stats.speed_down_str);
            let mut r_sdn = RECT {
                left: 220,
                top: cur_y + 14,
                right: self.width - 30,
                bottom: cur_y + 36,
            };
            gdi_draw_text(
                dc,
                &speed_dn_txt,
                &mut r_sdn,
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(dc, f_small);
            SetTextColor(dc, sub_color);
            let speed_up_txt = format!("⬆ {} | {:.1} MB", stats.speed_up_str, stats.mb_recv);
            let mut r_sup = RECT {
                left: 220,
                top: cur_y + 42,
                right: self.width - 30,
                bottom: cur_y + 64,
            };
            gdi_draw_text(
                dc,
                &speed_up_txt,
                &mut r_sup,
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
            );

            cur_y += data_card_h + 12;

            // 3. Applications Bandwidth Monitor Section
            let _ = SelectObject(dc, f_h2);
            SetTextColor(dc, text_color);
            let mut r_apphdr = RECT {
                left: 20,
                top: cur_y,
                right: 220,
                bottom: cur_y + 22,
            };
            gdi_draw_text(
                dc,
                "Active Applications",
                &mut r_apphdr,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Filter Buttons: [ 5 ] [ 10 ] [ All ]
            let filters = [(5, "Top 5"), (10, "Top 10"), (-1, "All")];
            let mut btn_x = self.width - 150;
            for (limit, label) in filters {
                let is_sel = tracker.config.top_apps_limit == limit;
                let r_flt = RECT {
                    left: btn_x,
                    top: cur_y,
                    right: btn_x + 38,
                    bottom: cur_y + 20,
                };
                self.hit_targets
                    .push((r_flt, FlyoutAction::SetTopAppsLimit(limit)));

                let flt_bg = if is_sel { accent_color } else { card_bg };
                draw_rounded_rect(dc, r_flt, 4, flt_bg, card_border);

                let _ = SelectObject(dc, f_small);
                SetTextColor(
                    dc,
                    if is_sel {
                        rgb(255, 255, 255)
                    } else {
                        sub_color
                    },
                );
                let mut r_fm = r_flt;
                gdi_draw_text(dc, label, &mut r_fm, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                btn_x += 42;
            }

            cur_y += 26;

            // App Rows
            let apps = tracker.get_top_apps(tracker.config.top_apps_limit);
            if apps.is_empty() {
                let r_empty = RECT {
                    left: 18,
                    top: cur_y,
                    right: self.width - 18,
                    bottom: cur_y + 36,
                };
                draw_rounded_rect(dc, r_empty, 6, card_bg, card_border);
                let _ = SelectObject(dc, f_small);
                SetTextColor(dc, sub_color);
                let mut r_em = r_empty;
                gdi_draw_text(
                    dc,
                    "No application traffic recorded yet today.",
                    &mut r_em,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                cur_y += 42;
            } else {
                for app in &apps {
                    let row_h = 44;
                    let r_row = RECT {
                        left: 18,
                        top: cur_y,
                        right: self.width - 18,
                        bottom: cur_y + row_h,
                    };
                    draw_rounded_rect(dc, r_row, 6, card_bg, card_border);

                    // App Name
                    let _ = SelectObject(dc, f_body_bold);
                    SetTextColor(dc, text_color);
                    let mut r_an = RECT {
                        left: 26,
                        top: cur_y + 6,
                        right: 190,
                        bottom: cur_y + 24,
                    };
                    gdi_draw_text(
                        dc,
                        &app.name,
                        &mut r_an,
                        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                    );

                    // Usage text & speed
                    let _ = SelectObject(dc, f_small);
                    SetTextColor(dc, sub_color);
                    let sub_txt = if app.is_active {
                        format!("{} • ⚡ {}", app.day_str, app.speed_str)
                    } else {
                        format!("{} • Idle", app.day_str)
                    };
                    let mut r_asub = RECT {
                        left: 26,
                        top: cur_y + 24,
                        right: 190,
                        bottom: cur_y + 40,
                    };
                    gdi_draw_text(
                        dc,
                        &sub_txt,
                        &mut r_asub,
                        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                    );

                    // Usage Progress Bar
                    let bar_x = 196;
                    let bar_w = 78;
                    let bar_h = 6;
                    let bar_y = cur_y + 19;
                    let r_bar_bg = RECT {
                        left: bar_x,
                        top: bar_y,
                        right: bar_x + bar_w,
                        bottom: bar_y + bar_h,
                    };
                    let bg_br = CreateSolidBrush(if is_dark {
                        rgb(55, 55, 55)
                    } else {
                        rgb(225, 225, 225)
                    });
                    FillRect(dc, &r_bar_bg, bg_br);
                    let _ = DeleteObject(bg_br);

                    let fill_w = ((app.percent / 100.0) * (bar_w as f64)).round() as i32;
                    if fill_w > 0 {
                        let r_bar_fill = RECT {
                            left: bar_x,
                            top: bar_y,
                            right: bar_x + fill_w,
                            bottom: bar_y + bar_h,
                        };
                        let fill_br = CreateSolidBrush(accent_color);
                        FillRect(dc, &r_bar_fill, fill_br);
                        let _ = DeleteObject(fill_br);
                    }

                    // Action Button (Stop & Block / Blocked / System)
                    let btn_rect = RECT {
                        left: self.width - 110,
                        top: cur_y + 10,
                        right: self.width - 26,
                        bottom: cur_y + 34,
                    };
                    if app.is_critical {
                        let _ = SelectObject(dc, f_small);
                        SetTextColor(dc, sub_color);
                        draw_rounded_rect(
                            dc,
                            btn_rect,
                            4,
                            if is_dark {
                                rgb(48, 48, 48)
                            } else {
                                rgb(235, 235, 235)
                            },
                            card_border,
                        );
                        let mut bm = btn_rect;
                        gdi_draw_text(
                            dc,
                            "🛡️ System",
                            &mut bm,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                        );
                    } else if app.is_blocked {
                        self.hit_targets
                            .push((btn_rect, FlyoutAction::UnblockApp(app.name.clone())));
                        let _ = SelectObject(dc, f_small);
                        SetTextColor(dc, rgb(255, 255, 255));
                        draw_rounded_rect(
                            dc,
                            btn_rect,
                            4,
                            if is_dark {
                                rgb(120, 30, 30)
                            } else {
                                rgb(180, 50, 50)
                            },
                            red_color,
                        );
                        let mut bm = btn_rect;
                        gdi_draw_text(
                            dc,
                            "⛔ Blocked",
                            &mut bm,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                        );
                    } else {
                        self.hit_targets
                            .push((btn_rect, FlyoutAction::StopAndBlockApp(app.name.clone())));
                        let _ = SelectObject(dc, f_small);
                        SetTextColor(dc, text_color);
                        draw_rounded_rect(
                            dc,
                            btn_rect,
                            4,
                            if is_dark {
                                rgb(50, 50, 50)
                            } else {
                                rgb(228, 228, 228)
                            },
                            card_border,
                        );
                        let mut bm = btn_rect;
                        gdi_draw_text(
                            dc,
                            "🛑 Block",
                            &mut bm,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                        );
                    }

                    cur_y += row_h + 4;
                }
            }

            // Unblock All link if any blocked apps
            if !tracker.blocked_apps.is_empty() {
                let r_unblk = RECT {
                    left: self.width - 120,
                    top: cur_y,
                    right: self.width - 20,
                    bottom: cur_y + 18,
                };
                self.hit_targets.push((r_unblk, FlyoutAction::UnblockAll));
                let _ = SelectObject(dc, f_small);
                SetTextColor(dc, accent_color);
                let mut um = r_unblk;
                gdi_draw_text(
                    dc,
                    "(Unblock All)",
                    &mut um,
                    DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
                );
                cur_y += 20;
            } else {
                cur_y += 6;
            }

            // 4. Download Alert Configuration Card
            let alert_cfg_h = 56;
            let r_ac = RECT {
                left: 18,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + alert_cfg_h,
            };
            draw_rounded_rect(dc, r_ac, 6, card_bg, card_border);

            let _ = SelectObject(dc, f_body_bold);
            SetTextColor(dc, text_color);
            let en_str = if tracker.config.heavy_download_alert_enabled {
                "☑ High Download Alerts"
            } else {
                "☐ High Download Alerts"
            };
            let r_en = RECT {
                left: 26,
                top: cur_y + 8,
                right: 200,
                bottom: cur_y + 26,
            };
            self.hit_targets
                .push((r_en, FlyoutAction::ToggleAlertEnabled));
            let mut r_en_m = r_en;
            gdi_draw_text(
                dc,
                en_str,
                &mut r_en_m,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Threshold pills: 50MB, 100MB, 250MB, 500MB
            let thresholds = [50u64, 100u64, 250u64, 500u64];
            let mut th_x = 26;
            for th in thresholds {
                let is_cur = tracker.config.heavy_download_threshold_mb == th;
                let r_th = RECT {
                    left: th_x,
                    top: cur_y + 28,
                    right: th_x + 54,
                    bottom: cur_y + 48,
                };
                self.hit_targets
                    .push((r_th, FlyoutAction::SetAlertThreshold(th)));

                let th_bg = if is_cur {
                    amber_color
                } else {
                    if is_dark {
                        rgb(48, 48, 48)
                    } else {
                        rgb(230, 230, 230)
                    }
                };
                draw_rounded_rect(dc, r_th, 4, th_bg, card_border);

                let _ = SelectObject(dc, f_small);
                SetTextColor(dc, if is_cur { rgb(0, 0, 0) } else { text_color });
                let mut th_m = r_th;
                let th_lbl = format!("{} MB", th);
                gdi_draw_text(
                    dc,
                    &th_lbl,
                    &mut th_m,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                th_x += 60;
            }

            cur_y += alert_cfg_h + 10;

            // 5. Preferences & Actions Card
            let pref_h = 70;
            let r_pr = RECT {
                left: 18,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + pref_h,
            };
            draw_rounded_rect(dc, r_pr, 6, card_bg, card_border);

            // Pill toggle & Autostart toggle
            let pill_txt = if tracker.config.show_taskbar_pill {
                "☑ Taskbar Pill"
            } else {
                "☐ Taskbar Pill"
            };
            let r_pt = RECT {
                left: 26,
                top: cur_y + 8,
                right: 140,
                bottom: cur_y + 26,
            };
            self.hit_targets.push((
                r_pt,
                FlyoutAction::TogglePill(!tracker.config.show_taskbar_pill),
            ));
            let _ = SelectObject(dc, f_small);
            SetTextColor(dc, text_color);
            let mut r_pt_m = r_pt;
            gdi_draw_text(
                dc,
                pill_txt,
                &mut r_pt_m,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            let auto_txt = if tracker.config.start_with_windows {
                "☑ Run at Startup"
            } else {
                "☐ Run at Startup"
            };
            let r_at = RECT {
                left: 150,
                top: cur_y + 8,
                right: 280,
                bottom: cur_y + 26,
            };
            self.hit_targets.push((
                r_at,
                FlyoutAction::ToggleAutostart(!tracker.config.start_with_windows),
            ));
            let mut r_at_m = r_at;
            gdi_draw_text(
                dc,
                auto_txt,
                &mut r_at_m,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Reset Today & Exit App Buttons
            let r_res = RECT {
                left: 26,
                top: cur_y + 36,
                right: 180,
                bottom: cur_y + 60,
            };
            self.hit_targets.push((r_res, FlyoutAction::ResetToday));
            draw_rounded_rect(
                dc,
                r_res,
                4,
                if is_dark {
                    rgb(48, 48, 48)
                } else {
                    rgb(230, 230, 230)
                },
                card_border,
            );
            let _ = SelectObject(dc, f_small);
            SetTextColor(dc, text_color);
            let mut r_res_m = r_res;
            gdi_draw_text(
                dc,
                "🔄 Reset Usage",
                &mut r_res_m,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            let r_exit = RECT {
                left: 190,
                top: cur_y + 36,
                right: self.width - 26,
                bottom: cur_y + 60,
            };
            self.hit_targets.push((r_exit, FlyoutAction::ExitApp));
            draw_rounded_rect(
                dc,
                r_exit,
                4,
                if is_dark {
                    rgb(48, 48, 48)
                } else {
                    rgb(230, 230, 230)
                },
                card_border,
            );
            SetTextColor(dc, red_color);
            let mut r_exit_m = r_exit;
            gdi_draw_text(
                dc,
                "❌ Exit App",
                &mut r_exit_m,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_datasaver_tab(
        &mut self,
        dc: HDC,
        mut cur_y: i32,
        is_dark: bool,
        card_bg: COLORREF,
        card_border: COLORREF,
        text_color: COLORREF,
        sub_color: COLORREF,
        accent_color: COLORREF,
        f_h2: HFONT,
        f_body_bold: HFONT,
        f_small: HFONT,
    ) {
        unsafe {
            // 1. Master Action Card: Apply All 3 Protections
            let master_h = 100;
            let r_mc = RECT {
                left: 18,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + master_h,
            };
            let mc_bg = if is_dark {
                rgb(20, 44, 60)
            } else {
                rgb(230, 246, 255)
            };
            draw_rounded_rect(dc, r_mc, 8, mc_bg, accent_color);

            let _ = SelectObject(dc, f_h2);
            SetTextColor(dc, accent_color);
            let mut r_mt = RECT {
                left: 30,
                top: cur_y + 10,
                right: self.width - 30,
                bottom: cur_y + 30,
            };
            gdi_draw_text(
                dc,
                "🛡️ Master Background Anti-Leak Guard",
                &mut r_mt,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(dc, f_small);
            SetTextColor(dc, text_color);
            let mut r_md = RECT {
                left: 30,
                top: cur_y + 32,
                right: self.width - 30,
                bottom: cur_y + 54,
            };
            gdi_draw_text(
                dc,
                "Stops Windows update peering, enables metered mode, and disables BITS.",
                &mut r_md,
                DT_LEFT | DT_WORDBREAK,
            );

            // Master Button
            let r_mbtn = RECT {
                left: 30,
                top: cur_y + 60,
                right: self.width - 30,
                bottom: cur_y + 90,
            };
            self.hit_targets
                .push((r_mbtn, FlyoutAction::ApplyAllProtections));
            draw_rounded_rect(dc, r_mbtn, 6, accent_color, accent_color);

            let _ = SelectObject(dc, f_body_bold);
            SetTextColor(dc, rgb(255, 255, 255));
            let mut r_mbm = r_mbtn;
            gdi_draw_text(
                dc,
                "⚡ Apply All 3 Protections Now",
                &mut r_mbm,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            cur_y += master_h + 14;

            // 2. Card 1: Delivery Optimization (P2P uploads)
            let (p2p_blocked, p2p_status) = DataSaverManager::get_delivery_optimization_status();
            let c1_h = 92;
            let r_c1 = RECT {
                left: 18,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + c1_h,
            };
            draw_rounded_rect(dc, r_c1, 8, card_bg, card_border);

            let _ = SelectObject(dc, f_body_bold);
            SetTextColor(dc, text_color);
            let mut r_c1t = RECT {
                left: 28,
                top: cur_y + 10,
                right: self.width - 100,
                bottom: cur_y + 28,
            };
            gdi_draw_text(
                dc,
                "1. Windows Update P2P Peering",
                &mut r_c1t,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(dc, f_small);
            SetTextColor(
                dc,
                if p2p_blocked {
                    rgb(76, 194, 255)
                } else {
                    rgb(232, 17, 35)
                },
            );
            let mut r_c1s = RECT {
                left: self.width - 160,
                top: cur_y + 10,
                right: self.width - 28,
                bottom: cur_y + 28,
            };
            gdi_draw_text(
                dc,
                &p2p_status,
                &mut r_c1s,
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
            );

            SetTextColor(dc, sub_color);
            let mut r_c1d = RECT {
                left: 28,
                top: cur_y + 30,
                right: self.width - 28,
                bottom: cur_y + 50,
            };
            gdi_draw_text(
                dc,
                "Windows uploads update files to other PCs over the internet.",
                &mut r_c1d,
                DT_LEFT | DT_WORDBREAK,
            );

            // Button + Settings link
            let r_btn_p2p = RECT {
                left: 28,
                top: cur_y + 56,
                right: 180,
                bottom: cur_y + 82,
            };
            self.hit_targets
                .push((r_btn_p2p, FlyoutAction::StopDeliveryOptimization));
            draw_rounded_rect(
                dc,
                r_btn_p2p,
                4,
                if is_dark {
                    rgb(50, 50, 50)
                } else {
                    rgb(228, 228, 228)
                },
                card_border,
            );
            let _ = SelectObject(dc, f_small);
            SetTextColor(dc, text_color);
            let mut r_pm = r_btn_p2p;
            gdi_draw_text(
                dc,
                "🛑 Stop Uploads",
                &mut r_pm,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            let r_set_p2p = RECT {
                left: 190,
                top: cur_y + 56,
                right: 300,
                bottom: cur_y + 82,
            };
            self.hit_targets
                .push((r_set_p2p, FlyoutAction::OpenDeliveryOptimizationSettings));
            SetTextColor(dc, accent_color);
            let mut r_sm = r_set_p2p;
            gdi_draw_text(
                dc,
                "⚙ Settings",
                &mut r_sm,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            cur_y += c1_h + 12;

            // 3. Card 2: Metered Connection
            let (is_metered, profile, _) = DataSaverManager::get_metered_status(false);
            let c2_h = 92;
            let r_c2 = RECT {
                left: 18,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + c2_h,
            };
            draw_rounded_rect(dc, r_c2, 8, card_bg, card_border);

            let _ = SelectObject(dc, f_body_bold);
            SetTextColor(dc, text_color);
            let mut r_c2t = RECT {
                left: 28,
                top: cur_y + 10,
                right: self.width - 100,
                bottom: cur_y + 28,
            };
            gdi_draw_text(
                dc,
                "2. Metered Connection Mode",
                &mut r_c2t,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(dc, f_small);
            SetTextColor(
                dc,
                if is_metered {
                    rgb(76, 194, 255)
                } else {
                    sub_color
                },
            );
            let met_lbl = if is_metered {
                "Metered (Restricted)"
            } else {
                "Unrestricted"
            };
            let mut r_c2s = RECT {
                left: self.width - 160,
                top: cur_y + 10,
                right: self.width - 28,
                bottom: cur_y + 28,
            };
            gdi_draw_text(
                dc,
                met_lbl,
                &mut r_c2s,
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
            );

            SetTextColor(dc, sub_color);
            let met_desc = format!(
                "Profile: {} • Halts automatic OS downloads & telemetry.",
                profile
            );
            let mut r_c2d = RECT {
                left: 28,
                top: cur_y + 30,
                right: self.width - 28,
                bottom: cur_y + 50,
            };
            gdi_draw_text(dc, &met_desc, &mut r_c2d, DT_LEFT | DT_WORDBREAK);

            // Toggle Button + Settings link
            let r_btn_met = RECT {
                left: 28,
                top: cur_y + 56,
                right: 180,
                bottom: cur_y + 82,
            };
            self.hit_targets
                .push((r_btn_met, FlyoutAction::ToggleMetered));
            draw_rounded_rect(
                dc,
                r_btn_met,
                4,
                if is_dark {
                    rgb(50, 50, 50)
                } else {
                    rgb(228, 228, 228)
                },
                card_border,
            );
            SetTextColor(dc, text_color);
            let btn_met_lbl = if is_metered {
                "📶 Unmeter Network"
            } else {
                "📶 Set as Metered"
            };
            let mut r_mm = r_btn_met;
            gdi_draw_text(
                dc,
                btn_met_lbl,
                &mut r_mm,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            let r_set_net = RECT {
                left: 190,
                top: cur_y + 56,
                right: 300,
                bottom: cur_y + 82,
            };
            self.hit_targets
                .push((r_set_net, FlyoutAction::OpenNetworkSettings));
            SetTextColor(dc, accent_color);
            let mut r_nm = r_set_net;
            gdi_draw_text(
                dc,
                "⚙ Network",
                &mut r_nm,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            cur_y += c2_h + 12;

            // 4. Card 3: BITS Service
            let (bits_restricted, bits_status, _) = DataSaverManager::get_bits_status();
            let c3_h = 92;
            let r_c3 = RECT {
                left: 18,
                top: cur_y,
                right: self.width - 18,
                bottom: cur_y + c3_h,
            };
            draw_rounded_rect(dc, r_c3, 8, card_bg, card_border);

            let _ = SelectObject(dc, f_body_bold);
            SetTextColor(dc, text_color);
            let mut r_c3t = RECT {
                left: 28,
                top: cur_y + 10,
                right: self.width - 100,
                bottom: cur_y + 28,
            };
            gdi_draw_text(
                dc,
                "3. Background Transfer Service (BITS)",
                &mut r_c3t,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(dc, f_small);
            SetTextColor(
                dc,
                if bits_restricted {
                    rgb(76, 194, 255)
                } else {
                    rgb(232, 17, 35)
                },
            );
            let mut r_c3s = RECT {
                left: self.width - 160,
                top: cur_y + 10,
                right: self.width - 28,
                bottom: cur_y + 28,
            };
            gdi_draw_text(
                dc,
                &bits_status,
                &mut r_c3s,
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
            );

            SetTextColor(dc, sub_color);
            let mut r_c3d = RECT {
                left: 28,
                top: cur_y + 30,
                right: self.width - 28,
                bottom: cur_y + 50,
            };
            gdi_draw_text(
                dc,
                "Stops background file transfers and prevents auto-start.",
                &mut r_c3d,
                DT_LEFT | DT_WORDBREAK,
            );

            // Kill BITS Button + Services.msc link
            let r_btn_bits = RECT {
                left: 28,
                top: cur_y + 56,
                right: 180,
                bottom: cur_y + 82,
            };
            self.hit_targets.push((r_btn_bits, FlyoutAction::KillBits));
            draw_rounded_rect(
                dc,
                r_btn_bits,
                4,
                if is_dark {
                    rgb(50, 50, 50)
                } else {
                    rgb(228, 228, 228)
                },
                card_border,
            );
            SetTextColor(dc, text_color);
            let mut r_bm = r_btn_bits;
            gdi_draw_text(
                dc,
                "🛑 Kill & Manual",
                &mut r_bm,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            let r_set_svc = RECT {
                left: 190,
                top: cur_y + 56,
                right: 300,
                bottom: cur_y + 82,
            };
            self.hit_targets
                .push((r_set_svc, FlyoutAction::OpenServicesMsc));
            SetTextColor(dc, accent_color);
            let mut r_svm = r_set_svc;
            gdi_draw_text(
                dc,
                "⚙ services.msc",
                &mut r_svm,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }

    pub fn handle_click(&mut self, x: i32, y: i32) -> FlyoutAction {
        for (rect, action) in &self.hit_targets {
            if x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom {
                return action.clone();
            }
        }
        FlyoutAction::None
    }
}

fn draw_rounded_rect(dc: HDC, rect: RECT, radius: i32, bg_color: COLORREF, border_color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(bg_color);
        let pen = CreatePen(PS_SOLID, 1, border_color);
        let old_br = SelectObject(dc, brush);
        let old_pn = SelectObject(dc, pen);

        let _ = RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius * 2,
            radius * 2,
        );

        let _ = SelectObject(dc, old_pn);
        let _ = SelectObject(dc, old_br);
        let _ = DeleteObject(brush);
        let _ = DeleteObject(pen);
    }
}
