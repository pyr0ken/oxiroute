use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::Instant;

pub struct LogEntry {
    pub timestamp: String,
    pub level: &'static str,
    pub message: String,
}

pub struct AccountDisplay {
    pub provider: String,
    pub email: String,
    pub priority: u32,
    pub is_active: bool,
}

pub struct TuiState {
    pub port: u16,
    pub proxy: Option<String>,
    pub total_requests: AtomicU64,
    pub active_requests: AtomicU64,
    pub total_tokens: AtomicU64,
    pub start_time: Instant,
    pub logs: RwLock<VecDeque<LogEntry>>,
    pub accounts: RwLock<Vec<AccountDisplay>>,
    pub recent_latencies: RwLock<Vec<u64>>,
}

impl TuiState {
    pub fn new(port: u16, proxy: Option<String>) -> Self {
        Self {
            port,
            proxy,
            total_requests: AtomicU64::new(0),
            active_requests: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            start_time: Instant::now(),
            logs: RwLock::new(VecDeque::with_capacity(120)),
            accounts: RwLock::new(Vec::new()),
            recent_latencies: RwLock::new(vec![120, 140, 110, 180, 160, 130, 150, 140, 125, 135]),
        }
    }

    pub fn add_log(&self, level: &'static str, message: impl Into<String>) {
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        let entry = LogEntry {
            timestamp: now,
            level,
            message: message.into(),
        };

        if let Ok(mut logs) = self.logs.write() {
            if logs.len() >= 100 {
                logs.pop_front();
            }
            logs.push_back(entry);
        }
    }

    pub fn record_request_start(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.active_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_request_end(&self, tokens: u64, latency_ms: u64) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.total_tokens.fetch_add(tokens, Ordering::Relaxed);

        if let Ok(mut latencies) = self.recent_latencies.write() {
            if latencies.len() >= 30 {
                latencies.remove(0);
            }
            latencies.push(latency_ms);
        }
    }

    pub fn set_accounts(&self, accs: Vec<AccountDisplay>) {
        if let Ok(mut accounts) = self.accounts.write() {
            *accounts = accs;
        }
    }

    pub fn uptime_formatted(&self) -> String {
        let elapsed = self.start_time.elapsed().as_secs();
        let hours = elapsed / 3600;
        let mins = (elapsed % 3600) / 60;
        let secs = elapsed % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}
