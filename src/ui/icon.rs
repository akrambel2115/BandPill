use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush, DeleteDC,
    DeleteObject, DrawTextW, FillRect, GetDC, ReleaseDC, SelectObject, SetBkMode, SetTextColor,
    DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_BOLD, FW_SEMIBOLD, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

use crate::ui::theme::{is_system_dark_theme, rgb};
use crate::utils::{format_usage, to_wide_chars};

pub fn create_tray_icon(mb_val: f64, unit_mode: &str) -> Option<HICON> {
    let is_dark = is_system_dark_theme();
    let (num_str, unit_str, _) = format_usage(mb_val, unit_mode);

    let size: i32 = 64; // High-DPI icon canvas size
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return None;
        }

        let mem_dc = CreateCompatibleDC(screen_dc);
        let color_bmp = CreateCompatibleBitmap(screen_dc, size, size);
        let mask_bmp = CreateCompatibleBitmap(screen_dc, size, size);

        let old_bmp = SelectObject(mem_dc, color_bmp);

        // Fill background
        let bg_color = if is_dark {
            rgb(24, 24, 24)
        } else {
            rgb(240, 240, 240)
        };
        let bg_brush = CreateSolidBrush(bg_color);
        let rect = RECT {
            left: 0,
            top: 0,
            right: size,
            bottom: size,
        };
        FillRect(mem_dc, &rect, bg_brush);
        let _ = DeleteObject(bg_brush);

        SetBkMode(mem_dc, TRANSPARENT);

        // Colors
        let num_color = if is_dark {
            rgb(255, 255, 255)
        } else {
            rgb(20, 20, 20)
        };
        let unit_color = if is_dark {
            rgb(96, 205, 255)
        } else {
            rgb(0, 103, 192)
        };

        // Font scaling based on number length
        let len = num_str.len();
        let (num_font_size, unit_font_size, num_y_rect, unit_y_rect) = if len <= 2 {
            (
                (size as f32 * 0.46) as i32,
                (size as f32 * 0.24) as i32,
                RECT {
                    left: 0,
                    top: 4,
                    right: size,
                    bottom: (size as f32 * 0.58) as i32,
                },
                RECT {
                    left: 0,
                    top: (size as f32 * 0.58) as i32,
                    right: size,
                    bottom: size - 4,
                },
            )
        } else if len == 3 {
            (
                (size as f32 * 0.38) as i32,
                (size as f32 * 0.22) as i32,
                RECT {
                    left: 0,
                    top: 6,
                    right: size,
                    bottom: (size as f32 * 0.58) as i32,
                },
                RECT {
                    left: 0,
                    top: (size as f32 * 0.58) as i32,
                    right: size,
                    bottom: size - 4,
                },
            )
        } else {
            (
                (size as f32 * 0.32) as i32,
                (size as f32 * 0.20) as i32,
                RECT {
                    left: 0,
                    top: 8,
                    right: size,
                    bottom: (size as f32 * 0.58) as i32,
                },
                RECT {
                    left: 0,
                    top: (size as f32 * 0.58) as i32,
                    right: size,
                    bottom: size - 4,
                },
            )
        };

        let font_name = to_wide_chars("Segoe UI Variable Display");
        let num_font = CreateFontW(
            -num_font_size,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            1, // DEFAULT_CHARSET
            0,
            0,
            0,
            0,
            windows::core::PCWSTR(font_name.as_ptr()),
        );

        let unit_font = CreateFontW(
            -unit_font_size,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            windows::core::PCWSTR(font_name.as_ptr()),
        );

        // Draw Number
        let mut r_num = num_y_rect;
        let old_font = SelectObject(mem_dc, num_font);
        SetTextColor(mem_dc, num_color);
        let mut wide_num = to_wide_chars(&num_str);
        DrawTextW(
            mem_dc,
            &mut wide_num[..num_str.len()],
            &mut r_num,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        // Draw Unit
        let mut r_unit = unit_y_rect;
        let _ = SelectObject(mem_dc, unit_font);
        SetTextColor(mem_dc, unit_color);
        let mut wide_unit = to_wide_chars(&unit_str);
        DrawTextW(
            mem_dc,
            &mut wide_unit[..unit_str.len()],
            &mut r_unit,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        let _ = SelectObject(mem_dc, old_font);
        let _ = SelectObject(mem_dc, old_bmp);

        let _ = DeleteObject(num_font);
        let _ = DeleteObject(unit_font);
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);

        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bmp,
            hbmColor: color_bmp,
        };

        let icon = CreateIconIndirect(&icon_info);

        let _ = DeleteObject(color_bmp);
        let _ = DeleteObject(mask_bmp);

        icon.ok()
    }
}
