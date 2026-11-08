use bandpill::config::Config;

#[test]
fn test_default_config_values() {
    let cfg = Config::default();
    assert_eq!(cfg.unit_mode, "MB");
    assert!(cfg.show_taskbar_pill);
    assert!(cfg.show_tray_icon);
    assert_eq!(cfg.pill_style, "pill");
    assert_eq!(cfg.pill_x, -1);
    assert_eq!(cfg.pill_y, -1);
    assert!(!cfg.pill_locked);
    assert_eq!(cfg.track_mode, "physical");
    assert_eq!(cfg.daily_budget_mb, 0);
    assert!(cfg.heavy_download_alert_enabled);
    assert_eq!(cfg.heavy_download_threshold_mb, 100);
    assert_eq!(cfg.heavy_download_window_sec, 60);
    assert_eq!(cfg.top_apps_limit, 5);
    assert!(cfg.blocked_apps.is_empty());
    assert!(cfg.blocked_exes.is_empty());
}

#[test]
fn test_config_json_roundtrip() {
    let original = Config {
        unit_mode: "AUTO".to_string(),
        heavy_download_threshold_mb: 250,
        top_apps_limit: 10,
        blocked_apps: vec!["Epic Games Launcher".to_string()],
        blocked_exes: vec!["epicgameslauncher.exe".to_string()],
        ..Default::default()
    };

    let json = serde_json::to_string(&original).expect("Failed to serialize config");
    let deserialized: Config = serde_json::from_str(&json).expect("Failed to deserialize config");

    assert_eq!(deserialized.unit_mode, "AUTO");
    assert_eq!(deserialized.heavy_download_threshold_mb, 250);
    assert_eq!(deserialized.top_apps_limit, 10);
    assert_eq!(deserialized.blocked_apps, vec!["Epic Games Launcher"]);
    assert_eq!(deserialized.blocked_exes, vec!["epicgameslauncher.exe"]);
}

#[test]
fn test_config_fallback_from_empty_json() {
    let empty_json = "{}";
    let cfg: Config = serde_json::from_str(empty_json).expect("Failed to deserialize empty json");
    assert_eq!(cfg.unit_mode, "MB");
    assert!(cfg.show_taskbar_pill);
    assert_eq!(cfg.heavy_download_threshold_mb, 100);
    assert_eq!(cfg.top_apps_limit, 5);
}
