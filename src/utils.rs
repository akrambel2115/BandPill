use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;
use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{DrawTextW, DRAW_TEXT_FORMAT, HDC};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn gdi_draw_text(hdc: HDC, text: &str, rect: &mut RECT, flags: DRAW_TEXT_FORMAT) -> i32 {
    let mut wide = to_wide_chars(text);
    let len = wide.len().saturating_sub(1);
    unsafe { DrawTextW(hdc, &mut wide[..len], rect, flags) }
}

pub fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1024.0 {
        format!("{:.0} B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / (1024.0 * 1024.0))
    }
}

pub fn format_usage(mb_val: f64, mode: &str) -> (String, String, String) {
    match mode {
        "AUTO" => {
            if mb_val >= 1024.0 {
                let gb_val = mb_val / 1024.0;
                let num = if gb_val < 10.0 {
                    format!("{:.1}", gb_val)
                } else {
                    format!("{:.0}", gb_val)
                };
                let full = format!("{} GB", num);
                (num, "GB".to_string(), full)
            } else {
                let num = format!("{:.0}", mb_val.round());
                let full = format!("{} MB", num);
                (num, "MB".to_string(), full)
            }
        }
        "MB_COMPACT" => {
            let num = format!("{:.0}", mb_val.round());
            let full = format!("{}MB", num);
            (num, "MB".to_string(), full)
        }
        _ => {
            // "MB" default
            let num = format!("{:.0}", mb_val.round());
            let full = format!("{} MB", num);
            (num, "MB".to_string(), full)
        }
    }
}

pub fn is_admin() -> bool {
    let output = Command::new("net").arg("session").output();
    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

pub fn run_elevated_powershell(script: &str) -> bool {
    if is_admin() {
        let res = Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", script])
            .output();
        match res {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    } else {
        let verb = to_wide_chars("runas");
        let file = to_wide_chars("powershell.exe");
        let params = to_wide_chars(&format!(
            "-NoProfile -WindowStyle Hidden -Command \"{}\"",
            script
        ));

        unsafe {
            let ret = ShellExecuteW(
                HWND(ptr::null_mut()),
                PCWSTR(verb.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR(params.as_ptr()),
                PCWSTR(ptr::null()),
                SW_HIDE,
            );
            (ret.0 as usize) > 32
        }
    }
}

pub fn open_url_or_command(target: &str) {
    let verb = to_wide_chars("open");
    let file = to_wide_chars(target);
    unsafe {
        ShellExecuteW(
            HWND(ptr::null_mut()),
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(ptr::null()),
            PCWSTR(ptr::null()),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}
