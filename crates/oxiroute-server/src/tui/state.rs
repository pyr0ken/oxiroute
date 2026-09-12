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
];

pub const KNOWN_MODELS: &[(&str, &str)] = &[
    ("antigravity/claude-sonnet-4-6", "Google Antigravity / Claude 3.7 Sonnet (Agentic)"),
    ("antigravity/claude-opus-4-6-thinking", "Google Antigravity / Claude 3.7 Opus Thinking"),
    ("antigravity/gemini-3.7-flash-low", "Google Antigravity / Gemini 2.5 Flash (Low Tier)"),
    ("antigravity/gemini-3.7-flash-tiered", "Google Antigravity / Gemini 2.5 Flash (Tiered)"),
    ("antigravity/gemini-2.5-flash", "Google Antigravity / Gemini 2.5 Flash"),
    ("antigravity/gemini-3.6-flash-high", "Google Antigravity / Gemini 2.5 Pro (High Tier)"),
    ("antigravity/gemini-pro-agent", "Google Antigravity / Gemini 2.5 Pro Agent"),
    ("deepseek/deepseek-r1", "DeepSeek R1 Full Reasoning Model"),
    ("deepseek/deepseek-v3", "DeepSeek V3 671B MoE General Chat"),
    ("openrouter/auto", "OpenRouter Smart Auto Router"),
    ("openrouter/anthropic/claude-3.7-sonnet", "OpenRouter Anthropic Claude 3.7 Sonnet"),
    ("openrouter/deepseek/deepseek-r1", "OpenRouter DeepSeek R1 Reasoning"),
    ("openai/gpt-4o", "OpenAI GPT-4o Multimodal Flagship"),
    ("openai/gpt-4o-mini", "OpenAI GPT-4o Mini Fast & Efficient"),
    ("openai/o3-mini", "OpenAI o3 Mini Fast Reasoning Model"),
    ("anthropic/claude-3-5-sonnet-20241022", "Anthropic Claude 3.5 Sonnet v2"),
    ("anthropic/claude-3-7-sonnet", "Anthropic Claude 3.7 Sonnet Hybrid"),
    ("groq/llama-3.3-70b-versatile", "Groq LPU LLaMA 3.3 70B (Fast Inference)"),
    ("groq/deepseek-r1-distill-llama-70b", "Groq LPU DeepSeek R1 Distill 70B"),
];

pub const PROVIDER_TYPES: &[(&str, &str)] = &[
    ("antigravity", "Google Antigravity OAuth (Cloud Code IDE)"),
    ("agy", "Google Antigravity CLI OAuth (agy)"),
    ("openrouter", "OpenRouter Aggregator API (openrouter.ai)"),
    ("opencode", "OpenCode API Key (opencode.ai)"),
    ("opencode-zen", "OpenCode Zen Free Models API"),
    ("deepseek-web", "DeepSeek Web Session Key"),
    ("qwen-web", "Qwen Web Session Key"),
    ("tokenrouter", "TokenRouter Aggregator API"),
    ("deepseek", "DeepSeek Platform API (api.deepseek.com)"),
    ("groq", "Groq Ultra-Fast LPU (api.groq.com)"),
    ("openai", "OpenAI Official API (api.openai.com)"),
    ("anthropic", "Anthropic Official API (api.anthropic.com)"),
    ("google", "Google Gemini API (generativelanguage.googleapis.com)"),
    ("kiro", "Kiro Gateway API"),
    ("bai", "BAI / Free Provider Endpoint"),
    ("custom", "Custom OpenAI-compatible Endpoint"),
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
    pub fn next(self) -> Self {
        match self {
            ActiveTab::Overview => ActiveTab::Combos,
            ActiveTab::Combos => ActiveTab::Providers,
            ActiveTab::Providers => ActiveTab::Proxies,
            ActiveTab::Proxies => ActiveTab::Timeline,
            ActiveTab::Timeline => ActiveTab::Overview,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            ActiveTab::Overview => ActiveTab::Timeline,
            ActiveTab::Combos => ActiveTab::Overview,
            ActiveTab::Providers => ActiveTab::Combos,
            ActiveTab::Proxies => ActiveTab::Providers,
            ActiveTab::Timeline => ActiveTab::Proxies,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModalState {
    None,
    Help,
    AddCombo {
        name: String,
        strategy_idx: usize,
        models: Vec<String>,
        proxy_idx: usize,
        active_field: usize,
    },
    EditCombo {
        combo_id: String,
        name: String,
        strategy_idx: usize,
        models: Vec<String>,
        proxy_idx: usize,
        active_field: usize,
    },
    ModelPicker {
        return_to_edit: bool,
        combo_id: Option<String>,
        combo_name: String,
        strategy_idx: usize,
        proxy_idx: usize,
        search_query: String,
        selected_models: Vec<String>,
        cursor_idx: usize,
    },
    AddProxy {
        name: String,
        protocol_idx: usize,
        host: String,
        port: String,
        active_field: usize,
    },
    EditProxy {
        proxy_id: String,
        name: String,
        protocol_idx: usize,
        host: String,
        port: String,
        active_field: usize,
    },
    AddProvider {
        provider_type_idx: usize,
        name: String,
        api_key: String,
        priority: usize,
        active_field: usize,
    },
    EditProvider {
        account_id: String,
        provider_type_idx: usize,
        name: String,
        api_key: String,
        priority: usize,
        active_field: usize,
    },
}

#[derive(Debug, Clone)]
pub struct ComboDisplay {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub models: Vec<String>,
    pub proxy_name: Option<String>,
    pub status_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AccountDisplay {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub email: String,
    pub priority: u32,
    pub is_active: bool,
    pub status_text: String,
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

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: &'static str,
    pub message: String,
}

pub struct TuiState {
    pub port: u16,
    pub proxy: Option<String>,
    pub storage: Option<std::sync::Arc<oxiroute_auth::StorageManager>>,
    pub active_tab: RwLock<ActiveTab>,
    pub selected_index: RwLock<usize>,
    pub modal: RwLock<ModalState>,
    pub total_requests: AtomicU64,
    pub active_requests: AtomicU64,
    pub total_tokens: AtomicU64,
    pub historical_burned_tokens: AtomicU64,
    pub start_time: Instant,
    pub logs: RwLock<VecDeque<LogEntry>>,
    pub combos: RwLock<Vec<ComboDisplay>>,
    pub accounts: RwLock<Vec<AccountDisplay>>,
    pub proxies: RwLock<Vec<ProxyDisplay>>,
    pub timeline: RwLock<VecDeque<TimelineItem>>,
    pub recent_latencies: RwLock<Vec<u64>>,
}

impl TuiState {
    pub fn new(
        port: u16,
        proxy: Option<String>,
        storage: Option<std::sync::Arc<oxiroute_auth::StorageManager>>,
    ) -> Self {
        Self {
            port,
            proxy,
            storage,
            active_tab: RwLock::new(ActiveTab::Overview),
            selected_index: RwLock::new(0),
            modal: RwLock::new(ModalState::None),
            total_requests: AtomicU64::new(0),
            active_requests: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            historical_burned_tokens: AtomicU64::new(0),
            start_time: Instant::now(),
            logs: RwLock::new(VecDeque::with_capacity(150)),
            combos: RwLock::new(Vec::new()),
            accounts: RwLock::new(Vec::new()),
            proxies: RwLock::new(Vec::new()),
            timeline: RwLock::new(VecDeque::with_capacity(50)),
            recent_latencies: RwLock::new(Vec::new()),
        }
    }

    pub fn set_historical_burned_tokens(&self, tokens: u64) {
        self.historical_burned_tokens.store(tokens, Ordering::Relaxed);
    }

    pub fn set_accounts(&self, accs: Vec<AccountDisplay>) {
        if let Ok(mut accounts) = self.accounts.write() {
            *accounts = accs;
        }
    }

    pub fn set_combos(&self, cmbs: Vec<ComboDisplay>) {
        if let Ok(mut combos) = self.combos.write() {
            *combos = cmbs;
        }
    }

    pub fn set_proxies(&self, prxs: Vec<ProxyDisplay>) {
        if let Ok(mut proxies) = self.proxies.write() {
            *proxies = prxs;
        }
    }

    pub fn set_tab(&self, tab: ActiveTab) {
        if let Ok(mut t) = self.active_tab.write() {
            *t = tab;
        }
        if let Ok(mut sel) = self.selected_index.write() {
            *sel = 0;
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
        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level,
            message: message.into(),
        };

        if let Ok(mut logs) = self.logs.write() {
            if logs.len() >= 150 {
                logs.pop_front();
            }
            logs.push_back(entry);
        }
    }

    pub fn record_request_start(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.active_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_request_end(
        &self,
        model: &str,
        account: &str,
        tokens: u64,
        latency_ms: u64,
        status_code: u16,
    ) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.total_tokens.fetch_add(tokens, Ordering::Relaxed);

        if let Ok(mut latencies) = self.recent_latencies.write() {
            if latencies.len() >= 40 {
                latencies.remove(0);
            }
            latencies.push(latency_ms);
        }

        let item = TimelineItem {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
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

    pub fn record_historical_timeline_item(
        &self,
        id: String,
        timestamp: String,
        model: String,
        account: String,
        tokens: u64,
        latency_ms: u64,
        status_code: u16,
    ) {
        let item = TimelineItem {
            id,
            timestamp,
            model,
            account,
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

    pub fn uptime_formatted(&self) -> String {
        let secs = self.start_time.elapsed().as_secs();
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, s)
    }
}
