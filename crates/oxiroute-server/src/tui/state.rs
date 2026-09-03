use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::Instant;

pub const COMBO_STRATEGIES: &[(&str, &str)] = &[
    ("priority", "Strict priority fallback in order"),
    ("round-robin", "Even round-robin rotation across active models"),
    ("least-used", "Route to model with fewest requests"),
    ("p2c", "Power of Two Choices randomized load balancing"),
    ("cost-optimized", "Prioritize cheapest eligible models"),
    ("reset-aware", "Rotate based on quota reset windows"),
    ("reset-window", "Account for daily/weekly quota renewal"),
    ("fill-first", "Exhaust primary budget before fallback"),
    ("weighted", "Weighted percentage distribution"),
    ("fusion", "Fan-out to multiple models & synthesize (Ensemble)"),
    ("context-optimized", "Optimize routing by context window length"),
    ("cache-optimized", "Cache affinity routing (vLLM/SGLang/Prefix)"),
    ("context-relay", "Relay conversation context across models"),
    ("headroom", "Reserve buffer headroom before rate limits"),
    ("strict-random", "True uniform random selection"),
    ("auto", "16-factor automatic score-based routing"),
    ("lkgp", "Last Known Good Provider fast-path"),
    ("pipeline", "Sequential multi-model processing pipeline"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Overview = 0,
    Combos = 1,
    Providers = 2,
    Proxies = 3,
    Timeline = 4,
}

impl ActiveTab {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => ActiveTab::Overview,
            1 => ActiveTab::Combos,
            2 => ActiveTab::Providers,
            3 => ActiveTab::Proxies,
            4 => ActiveTab::Timeline,
            _ => ActiveTab::Overview,
        }
    }

    pub fn next(&self) -> Self {
        Self::from_index((*self as usize + 1) % 5)
    }

    pub fn prev(&self) -> Self {
        Self::from_index((*self as usize + 4) % 5)
    }
}

#[derive(Debug, Clone)]
pub enum ModalState {
    None,
    AddCombo {
        name: String,
        strategy_idx: usize,
        models_input: String,
        active_field: usize,
    },
    AddProxy {
        name: String,
        protocol_idx: usize,
        host: String,
        port: String,
        active_field: usize,
    },
    Help,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AccountDisplay {
    pub provider: String,
    pub email: String,
    pub priority: u32,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct ComboDisplay {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub models: Vec<String>,
    pub proxy_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProxyDisplay {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct TimelineItem {
    pub id: String,
    pub timestamp: String,
    pub model: String,
    pub account: String,
    pub tokens: u64,
    pub latency_ms: u64,
    pub status_code: u16,
}

pub struct TuiState {
    pub port: u16,
    pub proxy: Option<String>,
    pub active_tab: RwLock<ActiveTab>,
    pub modal: RwLock<ModalState>,
    pub selected_index: RwLock<usize>,

    pub total_requests: AtomicU64,
    pub active_requests: AtomicU64,
    pub total_tokens: AtomicU64,
    pub prompt_tokens: AtomicU64,
    pub completion_tokens: AtomicU64,
    pub compacted_tokens_saved: AtomicU64,
    pub start_time: Instant,

    pub logs: RwLock<VecDeque<LogEntry>>,
    pub accounts: RwLock<Vec<AccountDisplay>>,
    pub combos: RwLock<Vec<ComboDisplay>>,
    pub proxies: RwLock<Vec<ProxyDisplay>>,
    pub timeline: RwLock<VecDeque<TimelineItem>>,
    pub recent_latencies: RwLock<Vec<u64>>,
}

impl TuiState {
    pub fn new(port: u16, proxy: Option<String>) -> Self {
        let initial_combos = vec![
            ComboDisplay {
                id: "1".to_string(),
                name: "claude".to_string(),
                strategy: "least-used".to_string(),
                models: vec![
                    "antigravity/claude-sonnet-4-6".to_string(),
                    "antigravity/claude-opus-4-6-thinking".to_string(),
                ],
                proxy_name: Some("socks5h://127.0.0.1:5454".to_string()),
            },
            ComboDisplay {
                id: "2".to_string(),
                name: "antigravity".to_string(),
                strategy: "priority".to_string(),
                models: vec![
                    "antigravity/gemini-3.7-flash-low".to_string(),
                    "antigravity/gemini-2.5-flash".to_string(),
                ],
                proxy_name: Some("socks5h://127.0.0.1:5454".to_string()),
            },
        ];

        let initial_proxies = vec![ProxyDisplay {
            id: "1".to_string(),
            name: "Default SOCKS5".to_string(),
            protocol: "socks5h".to_string(),
            host: "127.0.0.1".to_string(),
            port: 5454,
            status: "Active ●".to_string(),
        }];

        Self {
            port,
            proxy,
            active_tab: RwLock::new(ActiveTab::Overview),
            modal: RwLock::new(ModalState::None),
            selected_index: RwLock::new(0),
            total_requests: AtomicU64::new(0),
            active_requests: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            prompt_tokens: AtomicU64::new(0),
            completion_tokens: AtomicU64::new(0),
            compacted_tokens_saved: AtomicU64::new(0),
            start_time: Instant::now(),
            logs: RwLock::new(VecDeque::with_capacity(120)),
            accounts: RwLock::new(Vec::new()),
            combos: RwLock::new(initial_combos),
            proxies: RwLock::new(initial_proxies),
            timeline: RwLock::new(VecDeque::with_capacity(60)),
            recent_latencies: RwLock::new(vec![120, 140, 110, 180, 160, 130, 150, 140, 125, 135]),
        }
    }

    pub fn set_tab(&self, tab: ActiveTab) {
        if let Ok(mut t) = self.active_tab.write() {
            *t = tab;
        }
        if let Ok(mut s) = self.selected_index.write() {
            *s = 0;
        }
    }

    pub fn open_modal(&self, modal: ModalState) {
        if let Ok(mut m) = self.modal.write() {
            *m = modal;
        }
    }

    pub fn close_modal(&self) {
        if let Ok(mut m) = self.modal.write() {
            *m = ModalState::None;
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

    pub fn record_request_end(&self, model: &str, account: &str, tokens: u64, latency_ms: u64, status_code: u16) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.total_tokens.fetch_add(tokens, Ordering::Relaxed);

        if let Ok(mut latencies) = self.recent_latencies.write() {
            if latencies.len() >= 30 {
                latencies.remove(0);
            }
            latencies.push(latency_ms);
        }

        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        let item = TimelineItem {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            timestamp: now,
            model: model.to_string(),
            account: account.to_string(),
            tokens,
            latency_ms,
            status_code,
        };

        if let Ok(mut tl) = self.timeline.write() {
            if tl.len() >= 50 {
                tl.pop_front();
            }
            tl.push_back(item);
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
