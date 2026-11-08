use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush,
    DeleteDC, DeleteObject, FillRect, InvalidateRect, RoundRect, SelectObject, SetBkMode,
    SetTextColor, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_BOLD, HDC, PS_SOLID,
    SRCCOPY, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DestroyMenu, GetCursorPos, GetSystemMetrics,
    SetWindowPos, ShowWindow, TrackPopupMenuEx, HWND_TOPMOST, MF_POPUP, MF_SEPARATOR, MF_STRING,
    SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::config::Config;
use crate::ui::theme::{is_system_dark_theme, rgb};
use crate::utils::{format_usage, gdi_draw_text, to_wide_chars};

pub const PILL_CLASS_NAME: &str = "BandPillWidgetClass";

pub const PILL_CMD_LOCK: u32 = 2001;
pub const PILL_CMD_DASHBOARD: u32 = 2002;
pub const PILL_CMD_DATASAVER: u32 = 2003;
pub const PILL_CMD_APPLY_ALL: u32 = 2004;
pub const PILL_CMD_STOP_UPDATE: u32 = 2005;
pub const PILL_CMD_METERED: u32 = 2006;
pub const PILL_CMD_BITS: u32 = 2007;
pub const PILL_CMD_RESET: u32 = 2008;
pub const PILL_CMD_HIDE: u32 = 2009;
pub const PILL_CMD_EXIT: u32 = 2010;

pub struct TaskbarPillWidget {
    pub hwnd: HWND,
    pub width: i32,
    pub height: i32,
    pub is_dragging: bool,
    pub is_hovered: bool,
    pub locked: bool,
    pub current_text: String,
    pub drag_start_cursor: POINT,
    pub drag_start_win: POINT,
}

impl TaskbarPillWidget {
    pub fn new(h_instance: windows::Win32::Foundation::HINSTANCE, config: &Config) -> Self {
        let width = 100;
        let height = 34;

        let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        let x = if config.pill_x > 0 && config.pill_x < sw {
            config.pill_x
        } else {
            sw - 260
        };

        let y = if config.pill_y > 0 && config.pill_y < sh {
            config.pill_y
        } else {
            sh - 85
        };

        let wide_class = to_wide_chars(PILL_CLASS_NAME);
        let wide_title = to_wide_chars("BandPill");

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
            .unwrap_or(HWND(ptr::null_mut()))
        };

        let widget = Self {
            hwnd,
            width,
            height,
            is_dragging: false,
            is_hovered: false,
            locked: config.pill_locked,
            current_text: "0 MB".to_string(),
            drag_start_cursor: POINT::default(),
            drag_start_win: POINT::default(),
        };

        if config.show_taskbar_pill && !hwnd.is_invalid() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }

        widget
    }

    pub fn set_visible(&self, visible: bool) {
        if !self.hwnd.is_invalid() {
            unsafe {
                let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
            }
        }
    }

    pub fn update_usage(&mut self, mb_val: f64, unit_mode: &str) {
        let (_, _, full) = format_usage(mb_val, unit_mode);
        if full != self.current_text {
            self.current_text = full;

            let required_width = if self.current_text.len() >= 8 {
                114
            } else if self.current_text.len() >= 6 {
                102
            } else {
                94
            };

            if required_width != self.width {
                self.width = required_width;
                if !self.hwnd.is_invalid() {
                    unsafe {
                        let _ = SetWindowPos(
                            self.hwnd,
                            HWND_TOPMOST,
                            0,
                            0,
                            self.width,
                            self.height,
                            SWP_NOACTIVATE | SWP_NOMOVE,
                        );
                    }
                }
            }

            self.repaint();
        }
    }

    pub fn repaint(&self) {
        if !self.hwnd.is_invalid() {
            unsafe {
                let _ = InvalidateRect(self.hwnd, None, false);
            }
        }
    }

    pub fn render_pill(&self, hdc: HDC, _style: &str) {
        let is_dark = is_system_dark_theme();
        let bg_color = if is_dark {
            if self.is_hovered {
                rgb(46, 46, 46)
            } else {
                rgb(32, 32, 32)
            }
        } else {
            if self.is_hovered {
                rgb(220, 220, 220)
            } else {
                rgb(240, 240, 240)
            }
        };

        let border_color = if is_dark {
            if self.is_hovered {
                rgb(96, 205, 255)
            } else {
                rgb(60, 60, 60)
            }
        } else {
            if self.is_hovered {
                rgb(0, 103, 192)
            } else {
                rgb(190, 190, 190)
            }
        };

        let text_color = if is_dark {
            rgb(255, 255, 255)
        } else {
            rgb(15, 15, 15)
        };

        unsafe {
            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bmp = CreateCompatibleBitmap(hdc, self.width, self.height);
            let old_bmp = SelectObject(mem_dc, mem_bmp);

            // Transparent key background
            let key_brush = CreateSolidBrush(rgb(0, 0, 1));
            let r_full = RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            };
            FillRect(mem_dc, &r_full, key_brush);
            let _ = DeleteObject(key_brush);

            // Draw rounded pill
            let pill_brush = CreateSolidBrush(bg_color);
            let pill_pen = CreatePen(PS_SOLID, 1, border_color);
            let old_brush = SelectObject(mem_dc, pill_brush);
            let old_pen = SelectObject(mem_dc, pill_pen);

            let _ = RoundRect(mem_dc, 2, 2, self.width - 2, self.height - 2, 16, 16);

            let _ = SelectObject(mem_dc, old_brush);
            let _ = SelectObject(mem_dc, old_pen);
            let _ = DeleteObject(pill_brush);
            let _ = DeleteObject(pill_pen);

            // Draw Text
            SetBkMode(mem_dc, TRANSPARENT);
            SetTextColor(mem_dc, text_color);

            let font_name = to_wide_chars("Segoe UI Variable Display");
            let font = CreateFontW(
                -14,
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
            let old_font = SelectObject(mem_dc, font);

            let mut text_rect = RECT {
                left: 6,
                top: 0,
                right: self.width - 6,
                bottom: self.height,
            };

            gdi_draw_text(
                mem_dc,
                &self.current_text,
                &mut text_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );

            let _ = SelectObject(mem_dc, old_font);
            let _ = DeleteObject(font);

            // Blit to screen DC
            let _ = BitBlt(hdc, 0, 0, self.width, self.height, mem_dc, 0, 0, SRCCOPY);

            let _ = SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(mem_bmp);
            let _ = DeleteDC(mem_dc);
        }
    }

    pub fn show_menu(&mut self) -> u32 {
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_err() {
                return 0;
            }
            let hmenu = menu.unwrap();

            let lock_label = if self.locked {
                "✔ Position Locked"
            } else {
                "🔓 Lock Position"
            };
            let t_lock = to_wide_chars(lock_label);
            let t_dash = to_wide_chars("📊 Open Details Dashboard");
            let t_ds = to_wide_chars("🛡️ Open Data Saver Tools");

            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                PILL_CMD_LOCK as usize,
                PCWSTR(t_lock.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                PILL_CMD_DASHBOARD as usize,
                PCWSTR(t_dash.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                PILL_CMD_DATASAVER as usize,
                PCWSTR(t_ds.as_ptr()),
            );

            // Submenu
            let sub = CreatePopupMenu().unwrap();
            let t_sub_all = to_wide_chars("⚡ Apply All 3 Protections");
            let t_sub_p2p = to_wide_chars("🛑 Stop Update Uploads (Delivery Optimization)");
            let t_sub_met = to_wide_chars("📶 Toggle Metered Connection");
            let t_sub_bits = to_wide_chars("🛑 Kill BITS Background Service");

            let _ = AppendMenuW(
                sub,
                MF_STRING,
                PILL_CMD_APPLY_ALL as usize,
                PCWSTR(t_sub_all.as_ptr()),
            );
            let _ = AppendMenuW(
                sub,
                MF_STRING,
                PILL_CMD_STOP_UPDATE as usize,
                PCWSTR(t_sub_p2p.as_ptr()),
            );
            let _ = AppendMenuW(
                sub,
                MF_STRING,
                PILL_CMD_METERED as usize,
                PCWSTR(t_sub_met.as_ptr()),
            );
            let _ = AppendMenuW(
                sub,
                MF_STRING,
                PILL_CMD_BITS as usize,
                PCWSTR(t_sub_bits.as_ptr()),
            );

            let t_sub = to_wide_chars("🛡️ Quick Data Saver Actions");
            let _ = AppendMenuW(hmenu, MF_POPUP, sub.0 as usize, PCWSTR(t_sub.as_ptr()));

            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR(ptr::null()));

            let t_reset = to_wide_chars("🔄 Reset Today's Usage");
            let t_hide = to_wide_chars("👁 Hide Taskbar Pill");
            let t_exit = to_wide_chars("❌ Exit");

            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                PILL_CMD_RESET as usize,
                PCWSTR(t_reset.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                PILL_CMD_HIDE as usize,
                PCWSTR(t_hide.as_ptr()),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                PILL_CMD_EXIT as usize,
                PCWSTR(t_exit.as_ptr()),
            );

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.hwnd);

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
}
