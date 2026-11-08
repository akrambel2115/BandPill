use chrono::Local;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::config::{save_config, Config};
use crate::net::adapter::{get_network_adapters, is_relevant_adapter};
use crate::net::process::{
    get_active_network_pids, get_process_info, get_process_io, kill_process_by_pid,
    list_all_pids_for_names, CRITICAL_PROCESSES,
};
use crate::storage::{archive_day, current_epoch_seconds, load_state, save_state, DayState};
use crate::utils::format_speed;

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub date: String,
    pub bytes_total: u64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub mb_total: f64,
    pub mb_sent: f64,
    pub mb_recv: f64,
    pub speed_up: f64,
    pub speed_down: f64,
    pub speed_up_str: String,
    pub speed_down_str: String,
    pub active_adapter: String,
}

#[derive(Debug, Clone)]
pub struct AppUsageInfo {
    pub name: String,
    pub day_mb: f64,
    pub day_str: String,
    pub speed: f64,
    pub speed_str: String,
    pub is_active: bool,
    pub percent: f64,
    pub pids: Vec<u32>,
    pub exes: Vec<String>,
    pub is_critical: bool,
    pub is_blocked: bool,
    pub total_count: usize,
}

#[derive(Debug, Clone)]
pub struct AlertData {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub mb_downloaded: f64,
    pub threshold_mb: u64,
    pub duration_sec: u64,
    pub speed: f64,
    pub speed_str: String,
    pub is_critical: bool,
    pub is_svchost: bool,
    pub timestamp: f64,
}

pub struct NetworkTracker {
    pub config: Config,
    pub today_date: String,
    pub today_sent: u64,
    pub today_recv: u64,
    pub speed_up: f64,
    pub speed_down: f64,
    pub active_adapter: String,

    pub app_day_bytes: HashMap<String, u64>,
    pub app_live_speeds: HashMap<String, f64>,
    pub app_active_pids: HashMap<String, HashSet<u32>>,
    pub app_active_exes: HashMap<String, HashSet<String>>,
    pub last_proc_io: HashMap<u32, u64>,
    pub name_cache: HashMap<String, String>,

    pub proc_recent_history: HashMap<u32, VecDeque<(Instant, u64)>>,
    pub snoozed_alerts: HashMap<String, Instant>,
    pub active_alerts: HashMap<u32, AlertData>,

    pub last_adapter_io: HashMap<String, (u64, u64)>,
    pub last_poll_time: Instant,

    pub blocked_apps: HashSet<String>,
    pub blocked_exes: HashSet<String>,
}

impl NetworkTracker {
    pub fn new(config: Config) -> Self {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let mut tracker = Self {
            blocked_apps: config.blocked_apps.iter().cloned().collect(),
            blocked_exes: config
                .blocked_exes
                .iter()
                .map(|e| e.to_lowercase())
                .collect(),
            config,
            today_date: today,
            today_sent: 0,
            today_recv: 0,
            speed_up: 0.0,
            speed_down: 0.0,
            active_adapter: "None".to_string(),
            app_day_bytes: HashMap::new(),
            app_live_speeds: HashMap::new(),
            app_active_pids: HashMap::new(),
            app_active_exes: HashMap::new(),
            last_proc_io: HashMap::new(),
            name_cache: HashMap::new(),
            proc_recent_history: HashMap::new(),
            snoozed_alerts: HashMap::new(),
            active_alerts: HashMap::new(),
            last_adapter_io: HashMap::new(),
            last_poll_time: Instant::now(),
        };

        tracker.load_initial_state();
        tracker
    }

    fn load_initial_state(&mut self) {
        if let Some(state) = load_state() {
            if state.date == self.today_date {
                self.today_sent = state.today_sent;
                self.today_recv = state.today_recv;
                self.app_day_bytes = state.apps;
            } else if !state.date.is_empty() && (state.today_sent > 0 || state.today_recv > 0) {
                archive_day(&state.date, state.today_sent, state.today_recv);
            }
        }
    }

    pub fn save_state(&self) {
        let state = DayState {
            date: self.today_date.clone(),
            today_sent: self.today_sent,
            today_recv: self.today_recv,
            apps: self.app_day_bytes.clone(),
            updated_at: current_epoch_seconds(),
        };
        save_state(&state);
    }

    pub fn reset_today(&mut self) {
        self.today_sent = 0;
        self.today_recv = 0;
        self.speed_up = 0.0;
        self.speed_down = 0.0;
        self.app_day_bytes.clear();
        self.app_live_speeds.clear();
        self.last_proc_io.clear();
        self.proc_recent_history.clear();
        self.active_alerts.clear();
        self.save_state();
    }

    pub fn update_cycle(&mut self) -> Vec<AlertData> {
        let now_instant = Instant::now();
        let elapsed = now_instant
            .duration_since(self.last_poll_time)
            .as_secs_f64()
            .max(0.2);
        self.last_poll_time = now_instant;

        let current_date = Local::now().format("%Y-%m-%d").to_string();

        // Check midnight rollover
        if current_date != self.today_date {
            archive_day(&self.today_date, self.today_sent, self.today_recv);
            self.today_date = current_date;
            self.today_sent = 0;
            self.today_recv = 0;
            self.app_day_bytes.clear();
            self.app_live_speeds.clear();
            self.last_proc_io.clear();
            self.proc_recent_history.clear();
            self.active_alerts.clear();
        }

        // Enforce blocked apps (kills respawns)
        self.enforce_blocked_apps();

        // 1. Adapter network I/O
        let adapters = get_network_adapters();
        let mut total_d_sent = 0u64;
        let mut total_d_recv = 0u64;
        let mut active_candidate = "None".to_string();
        let mut max_traffic = 0u64;

        for adapter in &adapters {
            if !is_relevant_adapter(&adapter.name, &adapter.description, &self.config.track_mode) {
                continue;
            }
            if !adapter.is_up {
                continue;
            }

            if let Some(&(last_sent, last_recv)) = self.last_adapter_io.get(&adapter.name) {
                let d_sent = adapter.bytes_sent.saturating_sub(last_sent);
                let d_recv = adapter.bytes_recv.saturating_sub(last_recv);

                total_d_sent += d_sent;
                total_d_recv += d_recv;

                let traffic = d_sent + d_recv;
                if traffic >= max_traffic {
                    max_traffic = traffic;
                    active_candidate = adapter.name.clone();
                }
            }

            self.last_adapter_io.insert(
                adapter.name.clone(),
                (adapter.bytes_sent, adapter.bytes_recv),
            );
        }

        self.today_sent += total_d_sent;
        self.today_recv += total_d_recv;
        self.speed_up = (total_d_sent as f64) / elapsed;
        self.speed_down = (total_d_recv as f64) / elapsed;

        if active_candidate != "None" {
            self.active_adapter = active_candidate;
        } else if self.active_adapter == "None" {
            for adapter in &adapters {
                if adapter.is_up
                    && is_relevant_adapter(
                        &adapter.name,
                        &adapter.description,
                        &self.config.track_mode,
                    )
                {
                    self.active_adapter = adapter.name.clone();
                    break;
                }
            }
        }

        // 2. Per-process network I/O
        let net_pids = get_active_network_pids();
        let mut current_proc_io = HashMap::new();
        let mut app_deltas: HashMap<String, u64> = HashMap::new();
        let mut current_app_pids: HashMap<String, HashSet<u32>> = HashMap::new();
        let mut current_app_exes: HashMap<String, HashSet<String>> = HashMap::new();
        let mut newly_triggered_alerts = Vec::new();

        let window_sec = Duration::from_secs(self.config.heavy_download_window_sec);
        let threshold_bytes = self.config.heavy_download_threshold_mb * 1024 * 1024;
        let alert_enabled = self.config.heavy_download_alert_enabled;

        for &pid in &net_pids {
            if let Some((read_bytes, write_bytes)) = get_process_io(pid) {
                let (app_name, proc_exe) = get_process_info(pid, &mut self.name_cache);

                current_app_pids
                    .entry(app_name.clone())
                    .or_default()
                    .insert(pid);
                current_app_exes
                    .entry(app_name.clone())
                    .or_default()
                    .insert(proc_exe.clone());

                let total_bytes = read_bytes + write_bytes;
                current_proc_io.insert(pid, total_bytes);

                if let Some(&last_total) = self.last_proc_io.get(&pid) {
                    let d = total_bytes.saturating_sub(last_total);
                    if d > 0 {
                        *app_deltas.entry(app_name.clone()).or_insert(0) += d;
                    }
                }

                // Rolling download history for heavy download detection
                let history = self.proc_recent_history.entry(pid).or_default();
                history.push_back((now_instant, read_bytes));

                while history.len() > 1
                    && now_instant.duration_since(history.front().unwrap().0) > window_sec
                {
                    history.pop_front();
                }

                if history.len() >= 2 && alert_enabled {
                    let (oldest_t, oldest_b) = history.front().unwrap();
                    let dl_in_window = read_bytes.saturating_sub(*oldest_b);
                    let dur = now_instant.duration_since(*oldest_t).as_secs_f64().max(1.0);

                    if dl_in_window >= threshold_bytes {
                        let pid_key = pid.to_string();
                        let is_snoozed = self
                            .snoozed_alerts
                            .get(&pid_key)
                            .is_some_and(|&until| now_instant < until)
                            || self
                                .snoozed_alerts
                                .get(&app_name)
                                .is_some_and(|&until| now_instant < until);

                        if !is_snoozed {
                            let speed = (dl_in_window as f64) / dur;
                            let lower_exe = proc_exe.to_lowercase();
                            let is_critical = CRITICAL_PROCESSES.contains(&lower_exe.as_str());
                            let is_svchost = lower_exe.contains("svchost");

                            let alert = AlertData {
                                pid,
                                name: app_name.clone(),
                                exe: proc_exe.clone(),
                                mb_downloaded: (dl_in_window as f64) / (1024.0 * 1024.0),
                                threshold_mb: self.config.heavy_download_threshold_mb,
                                duration_sec: dur.round() as u64,
                                speed,
                                speed_str: format_speed(speed),
                                is_critical,
                                is_svchost,
                                timestamp: current_epoch_seconds(),
                            };

                            self.active_alerts.insert(pid, alert.clone());
                            // Temporarily snooze for 20 seconds to prevent alert flooding
                            self.snoozed_alerts
                                .insert(pid_key, now_instant + Duration::from_secs(20));
                            newly_triggered_alerts.push(alert);
                        }
                    }
                }
            }
        }

        self.last_proc_io = current_proc_io;

        // Clean dead PIDs
        self.proc_recent_history
            .retain(|pid, _| net_pids.contains(pid));
        self.active_alerts.retain(|pid, _| net_pids.contains(pid));

        // Update application data
        self.app_active_pids = current_app_pids;
        self.app_active_exes = current_app_exes;

        for speed in self.app_live_speeds.values_mut() {
            *speed = 0.0;
        }

        for (app, d_bytes) in app_deltas {
            *self.app_day_bytes.entry(app.clone()).or_insert(0) += d_bytes;
            self.app_live_speeds.insert(app, (d_bytes as f64) / elapsed);
        }

        newly_triggered_alerts
    }

    pub fn enforce_blocked_apps(&mut self) {
        if self.blocked_apps.is_empty() && self.blocked_exes.is_empty() {
            return;
        }

        let targets: Vec<String> = self.blocked_apps.iter().cloned().collect();
        let exes: Vec<String> = self.blocked_exes.iter().cloned().collect();
        let to_kill = list_all_pids_for_names(&targets, &exes);

        for (pid, exe) in to_kill {
            let _ = kill_process_by_pid(pid, &exe);
        }
    }

    pub fn kill_process(&mut self, pid: u32) -> (bool, String) {
        let exe = self
            .active_alerts
            .get(&pid)
            .map(|a| a.exe.clone())
            .unwrap_or_default();
        let (ok, msg) = kill_process_by_pid(pid, &exe);
        if ok {
            self.active_alerts.remove(&pid);
        }
        (ok, msg)
    }

    pub fn kill_app(&mut self, app_name: &str) -> (bool, String) {
        let pids = self
            .app_active_pids
            .get(app_name)
            .cloned()
            .unwrap_or_default();
        let exes = self
            .app_active_exes
            .get(app_name)
            .cloned()
            .unwrap_or_default();

        let targets = vec![app_name.to_string()];
        let exes_vec: Vec<String> = exes.into_iter().collect();

        let mut pids_to_kill = pids;
        if pids_to_kill.is_empty() {
            let found = list_all_pids_for_names(&targets, &exes_vec);
            for (p, _) in found {
                pids_to_kill.insert(p);
            }
        }

        if pids_to_kill.is_empty() {
            return (false, format!("No active process found for {}.", app_name));
        }

        let mut killed_pids = Vec::new();
        let mut last_err = String::new();

        for pid in pids_to_kill {
            let (ok, msg) = self.kill_process(pid);
            if ok {
                killed_pids.push(pid);
            } else {
                last_err = msg;
            }
        }

        self.app_live_speeds.insert(app_name.to_string(), 0.0);
        self.app_active_pids.remove(app_name);

        if !killed_pids.is_empty() {
            let pid_strs: Vec<String> = killed_pids.iter().map(|p| p.to_string()).collect();
            (
                true,
                format!(
                    "Successfully stopped {} (PID {}).",
                    app_name,
                    pid_strs.join(", ")
                ),
            )
        } else if !last_err.is_empty() {
            (false, last_err)
        } else {
            (false, format!("Could not stop {}.", app_name))
        }
    }

    pub fn stop_and_block_app(&mut self, app_name: &str) -> (bool, String) {
        let _ = self.kill_app(app_name);

        self.blocked_apps.insert(app_name.to_string());
        if let Some(exes) = self.app_active_exes.get(app_name) {
            for exe in exes {
                self.blocked_exes.insert(exe.to_lowercase());
            }
        }

        self.config.blocked_apps = self.blocked_apps.iter().cloned().collect();
        self.config.blocked_exes = self.blocked_exes.iter().cloned().collect();
        let _ = save_config(&self.config);

        self.enforce_blocked_apps();
        (
            true,
            format!("Stopped & Blocked {}. Prevented from reopening.", app_name),
        )
    }

    pub fn unblock_app(&mut self, app_name: &str) -> (bool, String) {
        self.blocked_apps
            .retain(|a| a.to_lowercase() != app_name.to_lowercase());
        if let Some(exes) = self.app_active_exes.get(app_name) {
            for exe in exes {
                self.blocked_exes.remove(&exe.to_lowercase());
            }
        }

        self.config.blocked_apps = self.blocked_apps.iter().cloned().collect();
        self.config.blocked_exes = self.blocked_exes.iter().cloned().collect();
        let _ = save_config(&self.config);

        (
            true,
            format!("Unblocked {}. It can now run normally.", app_name),
        )
    }

    pub fn is_app_blocked(&self, app_name: &str) -> bool {
        let lower = app_name.to_lowercase();
        self.blocked_apps.iter().any(|a| a.to_lowercase() == lower)
    }

    pub fn snooze_process(&mut self, pid: u32, app_name: Option<&str>, duration_sec: u64) {
        let exp = Instant::now() + Duration::from_secs(duration_sec);
        self.snoozed_alerts.insert(pid.to_string(), exp);
        if let Some(name) = app_name {
            self.snoozed_alerts.insert(name.to_string(), exp);
        }
        self.active_alerts.remove(&pid);
    }

    pub fn get_stats(&self) -> NetworkStats {
        let total = self.today_sent + self.today_recv;
        NetworkStats {
            date: self.today_date.clone(),
            bytes_total: total,
            bytes_sent: self.today_sent,
            bytes_recv: self.today_recv,
            mb_total: (total as f64) / (1024.0 * 1024.0),
            mb_sent: (self.today_sent as f64) / (1024.0 * 1024.0),
            mb_recv: (self.today_recv as f64) / (1024.0 * 1024.0),
            speed_up: self.speed_up,
            speed_down: self.speed_down,
            speed_up_str: format_speed(self.speed_up),
            speed_down_str: format_speed(self.speed_down),
            active_adapter: self.active_adapter.clone(),
        }
    }

    pub fn get_top_apps(&self, limit: i32) -> Vec<AppUsageInfo> {
        let total_day = self.today_sent + self.today_recv;
        let mut apps: Vec<String> = self.app_day_bytes.keys().cloned().collect();

        apps.sort_by(|a, b| {
            let speed_a = self.app_live_speeds.get(a).copied().unwrap_or(0.0);
            let speed_b = self.app_live_speeds.get(b).copied().unwrap_or(0.0);
            let day_a = self.app_day_bytes.get(a).copied().unwrap_or(0);
            let day_b = self.app_day_bytes.get(b).copied().unwrap_or(0);

            let act_a = if speed_a > 1024.0 { 1 } else { 0 };
            let act_b = if speed_b > 1024.0 { 1 } else { 0 };

            act_b
                .cmp(&act_a)
                .then(
                    speed_b
                        .partial_cmp(&speed_a)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(day_b.cmp(&day_a))
        });

        let total_count = apps.len();
        let selected = if limit <= 0 {
            apps
        } else {
            apps.into_iter().take(limit as usize).collect()
        };

        let mut results = Vec::new();
        for app in selected {
            let day_b = self.app_day_bytes.get(&app).copied().unwrap_or(0);
            let day_mb = (day_b as f64) / (1024.0 * 1024.0);
            let speed = self.app_live_speeds.get(&app).copied().unwrap_or(0.0);
            let pct = if total_day > 0 {
                ((day_b as f64) / (total_day as f64) * 100.0).clamp(2.0, 100.0)
            } else {
                0.0
            };

            let pids: Vec<u32> = self
                .app_active_pids
                .get(&app)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            let exes: Vec<String> = self
                .app_active_exes
                .get(&app)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();

            let is_crit = exes
                .iter()
                .any(|e| CRITICAL_PROCESSES.contains(&e.to_lowercase().as_str()))
                || CRITICAL_PROCESSES.contains(&app.to_lowercase().as_str())
                || app.to_lowercase().contains("windows system");

            let day_str = if day_mb < 1024.0 {
                format!("{:.1} MB", day_mb)
            } else {
                format!("{:.2} GB", day_mb / 1024.0)
            };

            results.push(AppUsageInfo {
                name: app.clone(),
                day_mb,
                day_str,
                speed,
                speed_str: format_speed(speed),
                is_active: speed > 1024.0,
                percent: pct,
                pids,
                exes,
                is_critical: is_crit,
                is_blocked: self.is_app_blocked(&app),
                total_count,
            });
        }

        results
    }
}
