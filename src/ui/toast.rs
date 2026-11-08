use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush,
    DeleteDC, DeleteObject, FillRect, GetStockObject, RoundRect, SelectObject, SetBkMode,
    SetTextColor, DT_CENTER, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, FW_BOLD, FW_NORMAL,
    PS_SOLID, SRCCOPY, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetSystemMetrics, SetWindowPos, ShowWindow, HWND_TOPMOST,
    SM_CXSCREEN, SM_CYSCREEN, SWP_SHOWWINDOW, SW_SHOW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

#[link(name = "user32")]
extern "system" {
    fn MessageBeep(u_type: u32) -> windows::Win32::Foundation::BOOL;
}

use crate::tracker::AlertData;
use crate::ui::theme::{apply_fluent_window_styling, is_system_dark_theme, rgb};
use crate::utils::{gdi_draw_text, to_wide_chars};

pub const TOAST_CLASS_NAME: &str = "BandPillToastClass";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastAction {
    None,
    Kill(u32),
    StopBits,
    Snooze(u32),
    Close,
}

pub struct AlertToastWindow {
    pub hwnd: HWND,
    pub alert: AlertData,
    pub width: i32,
    pub height: i32,
    pub hover_btn: i32, // 0 = none, 1 = primary (kill/bits), 2 = snooze, 3 = close
}

impl AlertToastWindow {
    pub fn new(h_instance: windows::Win32::Foundation::HINSTANCE, alert: AlertData) -> Self {
        unsafe {
            let _ = MessageBeep(0x00000030); // MB_ICONEXCLAMATION
        }

        let width = 390;
        let height = 230;

        let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        let x = sw - width - 24;
        let y = sh - height - 56;

        let wide_class = to_wide_chars(TOAST_CLASS_NAME);
        let wide_title = to_wide_chars("HighBandwidthAlert");

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

        if !hwnd.is_invalid() {
            apply_fluent_window_styling(hwnd, is_system_dark_theme());
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_SHOWWINDOW);
            }
        }

        Self {
            hwnd,
            alert,
            width,
            height,
            hover_btn: 0,
        }
    }

    pub fn render(&self, hdc: windows::Win32::Graphics::Gdi::HDC) {
        let is_dark = is_system_dark_theme();
        let bg_color = if is_dark {
            rgb(32, 32, 32)
        } else {
            rgb(249, 249, 249)
        };
        let card_bg = if is_dark {
            rgb(41, 41, 41)
        } else {
            rgb(255, 255, 255)
        };
        let text_color = if is_dark {
            rgb(255, 255, 255)
        } else {
            rgb(17, 17, 17)
        };
        let sub_color = if is_dark {
            rgb(160, 160, 160)
        } else {
            rgb(100, 100, 100)
        };
        let amber_color = rgb(255, 185, 0);

        unsafe {
            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bmp = CreateCompatibleBitmap(hdc, self.width, self.height);
            let old_bmp = SelectObject(mem_dc, mem_bmp);

            // Fill background with subtle border
            let bg_brush = CreateSolidBrush(bg_color);
            let r_full = RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            };
            FillRect(mem_dc, &r_full, bg_brush);
            let _ = DeleteObject(bg_brush);

            let border_pen = CreatePen(PS_SOLID, 1, amber_color);
            let old_pen = SelectObject(mem_dc, border_pen);
            let old_brush = SelectObject(
                mem_dc,
                GetStockObject(windows::Win32::Graphics::Gdi::GET_STOCK_OBJECT_FLAGS(
                    5, /* NULL_BRUSH */
                )),
            );
            let _ = RoundRect(mem_dc, 1, 1, self.width - 1, self.height - 1, 12, 12);
            let _ = SelectObject(mem_dc, old_brush);
            let _ = SelectObject(mem_dc, old_pen);
            let _ = DeleteObject(border_pen);

            SetBkMode(mem_dc, TRANSPARENT);

            let font_name = to_wide_chars("Segoe UI Variable Display");
            let title_font = CreateFontW(
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
            let regular_font = CreateFontW(
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
            let bold_font = CreateFontW(
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

            // 1. Header: ⚠️ High Network Download Detected
            let old_font = SelectObject(mem_dc, title_font);
            SetTextColor(mem_dc, amber_color);
            let mut header_rect = RECT {
                left: 16,
                top: 12,
                right: self.width - 36,
                bottom: 32,
            };
            gdi_draw_text(
                mem_dc,
                "⚠️ High Network Download Detected",
                &mut header_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Close 'X'
            SetTextColor(
                mem_dc,
                if self.hover_btn == 3 {
                    text_color
                } else {
                    sub_color
                },
            );
            let mut x_rect = RECT {
                left: self.width - 32,
                top: 10,
                right: self.width - 12,
                bottom: 32,
            };
            gdi_draw_text(
                mem_dc,
                "✕",
                &mut x_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            // 2. Middle Card with details
            let card_rect = RECT {
                left: 14,
                top: 38,
                right: self.width - 14,
                bottom: 175,
            };
            let card_brush = CreateSolidBrush(card_bg);
            let c_border = CreatePen(
                PS_SOLID,
                1,
                if is_dark {
                    rgb(60, 60, 60)
                } else {
                    rgb(220, 220, 220)
                },
            );
            let old_c_brush = SelectObject(mem_dc, card_brush);
            let old_c_pen = SelectObject(mem_dc, c_border);
            let _ = RoundRect(
                mem_dc,
                card_rect.left,
                card_rect.top,
                card_rect.right,
                card_rect.bottom,
                8,
                8,
            );
            let _ = SelectObject(mem_dc, old_c_brush);
            let _ = SelectObject(mem_dc, old_c_pen);
            let _ = DeleteObject(card_brush);
            let _ = DeleteObject(c_border);

            // Process Name & PID
            let _ = SelectObject(mem_dc, bold_font);
            SetTextColor(mem_dc, text_color);
            let mut name_rect = RECT {
                left: 24,
                top: 46,
                right: self.width - 24,
                bottom: 66,
            };
            gdi_draw_text(
                mem_dc,
                &self.alert.name,
                &mut name_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            let _ = SelectObject(mem_dc, regular_font);
            SetTextColor(mem_dc, sub_color);
            let sub_info = format!("PID: {} • {}", self.alert.pid, self.alert.exe);
            let mut sub_rect = RECT {
                left: 24,
                top: 66,
                right: self.width - 24,
                bottom: 84,
            };
            gdi_draw_text(
                mem_dc,
                &sub_info,
                &mut sub_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Download Stats Banner
            let banner_rect = RECT {
                left: 22,
                top: 88,
                right: self.width - 22,
                bottom: 122,
            };
            let banner_bg = CreateSolidBrush(if is_dark {
                rgb(51, 36, 0)
            } else {
                rgb(255, 244, 206)
            });
            FillRect(mem_dc, &banner_rect, banner_bg);
            let _ = DeleteObject(banner_bg);

            let _ = SelectObject(mem_dc, bold_font);
            SetTextColor(
                mem_dc,
                if is_dark {
                    amber_color
                } else {
                    rgb(128, 92, 0)
                },
            );
            let stats_line = format!(
                "⚡ Downloaded: {:.1} MB in {}s (Limit: {}MB) • {}",
                self.alert.mb_downloaded,
                self.alert.duration_sec,
                self.alert.threshold_mb,
                self.alert.speed_str
            );
            let mut st_rect = RECT {
                left: 28,
                top: 88,
                right: self.width - 28,
                bottom: 122,
            };
            gdi_draw_text(
                mem_dc,
                &stats_line,
                &mut st_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Hint Text
            let _ = SelectObject(mem_dc, regular_font);
            SetTextColor(mem_dc, sub_color);
            let hint = if self.alert.is_critical {
                "Protected system process. Terminating may affect OS stability."
            } else if self.alert.is_svchost {
                "Windows Service Host. May be Windows Update or BITS."
            } else {
                "Actively downloading data. Kill process to preserve your bandwidth."
            };
            let mut hint_rect = RECT {
                left: 24,
                top: 128,
                right: self.width - 24,
                bottom: 168,
            };
            gdi_draw_text(mem_dc, hint, &mut hint_rect, DT_LEFT | DT_WORDBREAK);

            // 3. Action Buttons
            let btn1_rect = RECT {
                left: 16,
                top: 184,
                right: 180,
                bottom: 218,
            };
            let btn1_hover = self.hover_btn == 1;
            let (btn1_bg, btn1_text) = if self.alert.is_critical {
                (
                    if btn1_hover {
                        rgb(0, 120, 215)
                    } else {
                        rgb(0, 103, 192)
                    },
                    "🛡️ Stop Background BITS",
                )
            } else {
                (
                    if btn1_hover {
                        rgb(232, 17, 35)
                    } else {
                        rgb(209, 52, 56)
                    },
                    "🛑 Kill Process",
                )
            };
            let b1_brush = CreateSolidBrush(btn1_bg);
            let old_b1 = SelectObject(mem_dc, b1_brush);
            let _ = RoundRect(
                mem_dc,
                btn1_rect.left,
                btn1_rect.top,
                btn1_rect.right,
                btn1_rect.bottom,
                6,
                6,
            );
            let _ = SelectObject(mem_dc, old_b1);
            let _ = DeleteObject(b1_brush);

            let _ = SelectObject(mem_dc, bold_font);
            SetTextColor(mem_dc, rgb(255, 255, 255));
            let mut b1_mut_rect = btn1_rect;
            gdi_draw_text(
                mem_dc,
                btn1_text,
                &mut b1_mut_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            // Button 2 (Right): Let Continue
            let btn2_rect = RECT {
                left: self.width - 140,
                top: 184,
                right: self.width - 16,
                bottom: 218,
            };
            let btn2_hover = self.hover_btn == 2;
            let btn2_bg = if is_dark {
                if btn2_hover {
                    rgb(69, 69, 69)
                } else {
                    rgb(56, 56, 56)
                }
            } else {
                if btn2_hover {
                    rgb(215, 215, 215)
                } else {
                    rgb(229, 229, 229)
                }
            };
            let b2_brush = CreateSolidBrush(btn2_bg);
            let old_b2 = SelectObject(mem_dc, b2_brush);
            let _ = RoundRect(
                mem_dc,
                btn2_rect.left,
                btn2_rect.top,
                btn2_rect.right,
                btn2_rect.bottom,
                6,
                6,
            );
            let _ = SelectObject(mem_dc, old_b2);
            let _ = DeleteObject(b2_brush);

            let _ = SelectObject(mem_dc, regular_font);
            SetTextColor(mem_dc, text_color);
            let mut b2_mut_rect = btn2_rect;
            gdi_draw_text(
                mem_dc,
                "✔ Let Continue",
                &mut b2_mut_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            // Clean fonts
            let _ = SelectObject(mem_dc, old_font);
            let _ = DeleteObject(title_font);
            let _ = DeleteObject(regular_font);
            let _ = DeleteObject(bold_font);

            // Copy to screen
            let _ = BitBlt(hdc, 0, 0, self.width, self.height, mem_dc, 0, 0, SRCCOPY);
            let _ = SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(mem_bmp);
            let _ = DeleteDC(mem_dc);
        }
    }

    pub fn handle_click(&self, x: i32, y: i32) -> ToastAction {
        if (self.width - 34..=self.width - 10).contains(&x) && (8..=34).contains(&y) {
            return ToastAction::Close;
        }

        if (16..=180).contains(&x) && (184..=218).contains(&y) {
            if self.alert.is_critical {
                return ToastAction::StopBits;
            } else {
                return ToastAction::Kill(self.alert.pid);
            }
        }

        if (self.width - 140..=self.width - 16).contains(&x) && (184..=218).contains(&y) {
            return ToastAction::Snooze(self.alert.pid);
        }

        ToastAction::None
    }

    pub fn close(&self) {
        if !self.hwnd.is_invalid() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}
