use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{get_history_path, get_state_path};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DayState {
    pub date: String,
    pub today_sent: u64,
    pub today_recv: u64,
    pub apps: HashMap<String, u64>,
    pub updated_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub date: String,
    pub sent: u64,
    pub recv: u64,
    pub total: u64,
}

pub fn current_epoch_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn load_state() -> Option<DayState> {
    let path = get_state_path();
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<DayState>(&content).ok(),
        Err(_) => None,
    }
}

pub fn save_state(state: &DayState) {
    let path = get_state_path();
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, json);
    }
}

pub fn load_history() -> Vec<HistoryEntry> {
    let path = get_history_path();
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<Vec<HistoryEntry>>(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn archive_day(date_str: &str, sent: u64, recv: u64) {
    if sent == 0 && recv == 0 {
        return;
    }
    let mut history = load_history();
    let mut updated = false;

    for item in &mut history {
        if item.date == date_str {
            item.sent = sent;
            item.recv = recv;
            item.total = sent + recv;
            updated = true;
            break;
        }
    }

    if !updated {
        history.push(HistoryEntry {
            date: date_str.to_string(),
            sent,
            recv,
            total: sent + recv,
        });
    }

    if history.len() > 30 {
        let excess = history.len() - 30;
        history.drain(0..excess);
    }

    let path = get_history_path();
    if let Ok(json) = serde_json::to_string_pretty(&history) {
        let _ = fs::write(path, json);
    }
}
