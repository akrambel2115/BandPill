use bandpill::config::Config;
use bandpill::tracker::NetworkTracker;

#[test]
fn test_tracker_initial_stats() {
    let cfg = Config::default();
    let tracker = NetworkTracker::new(cfg);

    let stats = tracker.get_stats();
    assert_eq!(stats.bytes_sent, tracker.today_sent);
    assert_eq!(stats.bytes_recv, tracker.today_recv);
    assert_eq!(stats.bytes_total, tracker.today_sent + tracker.today_recv);
}

#[test]
fn test_tracker_reset_today() {
    let cfg = Config::default();
    let mut tracker = NetworkTracker::new(cfg);

    tracker.today_sent = 50 * 1024 * 1024;
    tracker.today_recv = 100 * 1024 * 1024;
    tracker.speed_up = 25000.0;
    tracker.speed_down = 80000.0;
    tracker
        .app_day_bytes
        .insert("Steam".to_string(), 150 * 1024 * 1024);

    tracker.reset_today();

    let stats = tracker.get_stats();
    assert_eq!(stats.bytes_sent, 0);
    assert_eq!(stats.bytes_recv, 0);
    assert_eq!(stats.bytes_total, 0);
    assert_eq!(stats.mb_total, 0.0);
    assert!(tracker.app_day_bytes.is_empty());
}

#[test]
fn test_tracker_stop_and_block_lifecycle() {
    let cfg = Config::default();
    let mut tracker = NetworkTracker::new(cfg);

    assert!(!tracker.is_app_blocked("Epic Games"));

    let (ok, msg) = tracker.stop_and_block_app("Epic Games");
    assert!(ok);
    assert!(msg.contains("Stopped & Blocked"));
    assert!(tracker.is_app_blocked("Epic Games"));
    // Case-insensitivity check
    assert!(tracker.is_app_blocked("epic games"));
    assert!(tracker.is_app_blocked("EPIC GAMES"));

    let (unblock_ok, unblock_msg) = tracker.unblock_app("Epic Games");
    assert!(unblock_ok);
    assert!(unblock_msg.contains("Unblocked"));
    assert!(!tracker.is_app_blocked("Epic Games"));
}

#[test]
fn test_tracker_top_apps_sorting() {
    let cfg = Config::default();
    let mut tracker = NetworkTracker::new(cfg);
    tracker.reset_today();

    tracker.today_sent = 200 * 1024 * 1024;
    tracker.today_recv = 300 * 1024 * 1024;

    tracker
        .app_day_bytes
        .insert("App1".to_string(), 50 * 1024 * 1024);
    tracker.app_live_speeds.insert("App1".to_string(), 0.0);

    tracker
        .app_day_bytes
        .insert("App2".to_string(), 10 * 1024 * 1024);
    tracker.app_live_speeds.insert("App2".to_string(), 500000.0); // Active app!

    tracker
        .app_day_bytes
        .insert("App3".to_string(), 100 * 1024 * 1024);
    tracker.app_live_speeds.insert("App3".to_string(), 0.0);

    let top = tracker.get_top_apps(2);
    assert_eq!(top.len(), 2);
    // Active app (App2 with 500KB/s) should be sorted first!
    assert_eq!(top[0].name, "App2");
    assert!(top[0].is_active);
    // Next should be highest daily usage (App3 with 100MB)
    assert_eq!(top[1].name, "App3");
}
