use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};

use crate::utils::to_wide_chars;

pub const APP_NAME: &str = "BandPill";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_unit_mode")]
    pub unit_mode: String, // "MB", "MB_COMPACT", "AUTO"

    #[serde(default = "default_true")]
    pub show_taskbar_pill: bool,

    #[serde(default = "default_true")]
    pub show_tray_icon: bool,

    #[serde(default = "default_pill_style")]
    pub pill_style: String, // "pill", "text_only", "with_icon"

    #[serde(default = "default_negative_one")]
    pub pill_x: i32,

    #[serde(default = "default_negative_one")]
    pub pill_y: i32,

    #[serde(default = "default_false")]
    pub pill_locked: bool,

    #[serde(default = "default_opacity")]
    pub pill_bg_opacity: f32,

    #[serde(default = "default_accent_color")]
    pub accent_color: String,

    #[serde(default = "default_text_color")]
    pub text_color: String,

    #[serde(default = "default_track_mode")]
    pub track_mode: String, // "physical", "all"

    #[serde(default = "default_zero")]
    pub daily_budget_mb: u64,

    #[serde(default = "default_false")]
    pub start_with_windows: bool,

    #[serde(default = "default_true")]
    pub heavy_download_alert_enabled: bool,

    #[serde(default = "default_threshold_mb")]
    pub heavy_download_threshold_mb: u64,

    #[serde(default = "default_window_sec")]
    pub heavy_download_window_sec: u64,

    #[serde(default = "default_top_apps_limit")]
    pub top_apps_limit: i32,

    #[serde(default)]
    pub blocked_apps: Vec<String>,

    #[serde(default)]
    pub blocked_exes: Vec<String>,
}

fn default_unit_mode() -> String {
    "MB".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_pill_style() -> String {
    "pill".to_string()
}
fn default_negative_one() -> i32 {
    -1
}
fn default_opacity() -> f32 {
    0.85
}
fn default_accent_color() -> String {
    "#60cdff".to_string()
}
fn default_text_color() -> String {
    "#ffffff".to_string()
}
fn default_track_mode() -> String {
    "physical".to_string()
}
fn default_zero() -> u64 {
    0
}
fn default_threshold_mb() -> u64 {
    100
}
fn default_window_sec() -> u64 {
    60
}
fn default_top_apps_limit() -> i32 {
    5
}

impl Default for Config {
    fn default() -> Self {
        Self {
            unit_mode: default_unit_mode(),
            show_taskbar_pill: default_true(),
            show_tray_icon: default_true(),
            pill_style: default_pill_style(),
            pill_x: default_negative_one(),
            pill_y: default_negative_one(),
            pill_locked: default_false(),
            pill_bg_opacity: default_opacity(),
            accent_color: default_accent_color(),
            text_color: default_text_color(),
            track_mode: default_track_mode(),
            daily_budget_mb: default_zero(),
            start_with_windows: default_false(),
            heavy_download_alert_enabled: default_true(),
            heavy_download_threshold_mb: default_threshold_mb(),
            heavy_download_window_sec: default_window_sec(),
            top_apps_limit: default_top_apps_limit(),
            blocked_apps: Vec::new(),
            blocked_exes: Vec::new(),
        }
    }
}

pub fn get_config_dir() -> PathBuf {
    let base = if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data)
    } else if let Ok(userprofile) = env::var("USERPROFILE") {
        PathBuf::from(userprofile)
    } else {
        PathBuf::from(".")
    };

    let new_dir = base.join("BandPill");
    let legacy_dir = base.join("DataUsageTracker");
    if !new_dir.exists() && legacy_dir.exists() {
        return legacy_dir;
    }
    new_dir
}

pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.json")
}

pub fn get_state_path() -> PathBuf {
    get_config_dir().join("data_state.json")
}

pub fn get_history_path() -> PathBuf {
    get_config_dir().join("history.json")
}

pub fn load_config() -> Config {
    let dir = get_config_dir();
    let _ = fs::create_dir_all(&dir);

    let path = get_config_path();
    let mut cfg = if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str::<Config>(&content).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    } else {
        Config::default()
    };

    cfg.start_with_windows = is_autostart_enabled();
    cfg
}

pub fn save_config(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let dir = get_config_dir();
    fs::create_dir_all(&dir)?;

    let path = get_config_path();
    let json = serde_json::to_string_pretty(cfg)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn is_autostart_enabled() -> bool {
    let subkey = to_wide_chars(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let val_name = to_wide_chars(APP_NAME);

    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY(ptr::null_mut());
        let res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );

        if res.is_err() {
            return false;
        }

        let mut data_len = 0u32;
        let query_res = RegQueryValueExW(
            hkey,
            PCWSTR(val_name.as_ptr()),
            None,
            None,
            None,
            Some(&mut data_len),
        );

        let _ = RegCloseKey(hkey);
        query_res.is_ok()
    }
}

pub fn set_autostart(enable: bool, command: Option<&str>) -> bool {
    let subkey = to_wide_chars(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let val_name = to_wide_chars(APP_NAME);

    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY(ptr::null_mut());
        let res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );

        if res.is_err() {
            return false;
        }

        let ok = if enable {
            let cmd = command.unwrap_or("");
            let wide_cmd = to_wide_chars(cmd);
            let bytes_len = (wide_cmd.len() * 2) as u32;
            RegSetValueExW(
                hkey,
                PCWSTR(val_name.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    wide_cmd.as_ptr() as *const u8,
                    bytes_len as usize,
                )),
            )
            .is_ok()
        } else {
            let del_res = RegDeleteValueW(hkey, PCWSTR(val_name.as_ptr()));
            del_res.is_ok() || del_res.0 == 2 // ERROR_FILE_NOT_FOUND is considered success for delete
        };

        let _ = RegCloseKey(hkey);
        ok
    }
}
