use bandpill::storage::{DayState, HistoryEntry};
use std::collections::HashMap;

#[test]
fn test_day_state_serde() {
    let mut apps = HashMap::new();
    apps.insert("Google Chrome".to_string(), 1024 * 1024 * 50);
    apps.insert("Spotify".to_string(), 1024 * 1024 * 20);

    let state = DayState {
        date: "2026-11-07".to_string(),
        today_sent: 1024 * 1024 * 10,
        today_recv: 1024 * 1024 * 60,
        apps,
        updated_at: 1700000000.0,
    };

    let json = serde_json::to_string(&state).expect("Failed to serialize DayState");
    let loaded: DayState = serde_json::from_str(&json).expect("Failed to deserialize DayState");

    assert_eq!(loaded.date, "2026-11-07");
    assert_eq!(loaded.today_sent, 1024 * 1024 * 10);
    assert_eq!(loaded.today_recv, 1024 * 1024 * 60);
    assert_eq!(loaded.apps.get("Google Chrome"), Some(&(1024 * 1024 * 50)));
}

#[test]
fn test_history_entry_calculations() {
    let entry = HistoryEntry {
        date: "2026-11-06".to_string(),
        sent: 500,
        recv: 1500,
        total: 2000,
    };

    assert_eq!(entry.total, entry.sent + entry.recv);
}
