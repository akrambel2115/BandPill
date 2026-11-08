use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
};

use crate::utils::to_wide_chars;

pub const DWMWCP_ROUND: u32 = 2;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub is_dark: bool,
    pub bg: COLORREF,
    pub card_bg: COLORREF,
    pub card_border: COLORREF,
    pub text_primary: COLORREF,
    pub text_secondary: COLORREF,
    pub accent: COLORREF,
    pub alert_amber: COLORREF,
    pub alert_red: COLORREF,
    pub hover_bg: COLORREF,
    pub bar_bg: COLORREF,
    pub bar_fill: COLORREF,
}

pub fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

pub fn is_system_dark_theme() -> bool {
    let subkey = to_wide_chars(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let val_name = to_wide_chars("SystemUsesLightTheme");

    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY(ptr::null_mut());
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_ok()
        {
            let mut val = 0u32;
            let mut len = std::mem::size_of::<u32>() as u32;

            let res = RegQueryValueExW(
                hkey,
                PCWSTR(val_name.as_ptr()),
                None,
                None,
                Some(&mut val as *mut u32 as *mut u8),
                Some(&mut len),
            );
            let _ = RegCloseKey(hkey);

            if res.is_ok() {
                return val == 0;
            }
        }
    }

    true // Default to dark on Windows 11
}

#[allow(dead_code)]
pub fn get_theme_colors(is_dark: bool) -> ThemeColors {
    if is_dark {
        ThemeColors {
            is_dark: true,
            bg: rgb(31, 31, 31),
            card_bg: rgb(39, 39, 39),
            card_border: rgb(56, 56, 56),
            text_primary: rgb(255, 255, 255),
            text_secondary: rgb(158, 158, 158),
            accent: rgb(96, 205, 255), // Windows 11 Cyan Accent
            alert_amber: rgb(255, 185, 0),
            alert_red: rgb(232, 17, 35),
            hover_bg: rgb(50, 50, 50),
            bar_bg: rgb(55, 55, 55),
            bar_fill: rgb(96, 205, 255),
        }
    } else {
        ThemeColors {
            is_dark: false,
            bg: rgb(243, 243, 243),
            card_bg: rgb(255, 255, 255),
            card_border: rgb(224, 224, 224),
            text_primary: rgb(17, 17, 17),
            text_secondary: rgb(97, 97, 97),
            accent: rgb(0, 103, 192), // Windows 11 Deep Blue Accent
            alert_amber: rgb(216, 131, 0),
            alert_red: rgb(209, 52, 56),
            hover_bg: rgb(229, 229, 229),
            bar_bg: rgb(230, 230, 230),
            bar_fill: rgb(0, 103, 192),
        }
    }
}

pub fn apply_fluent_window_styling(hwnd: HWND, is_dark: bool) {
    unsafe {
        let dark_val: i32 = if is_dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_val as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        let corner_val: u32 = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_val as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}
