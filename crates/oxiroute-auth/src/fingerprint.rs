use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone)]
pub struct Fingerprint {
    pub id: &'static str,
    pub user_agent: &'static str,
    pub sec_ch_ua: &'static str,
    pub sec_ch_ua_platform: &'static str,
    pub sec_ch_ua_mobile: &'static str,
    pub accept_language: &'static str,
}

pub static PROFILES: &[Fingerprint] = &[
    Fingerprint {
        id: "chrome-mac",
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Not-A.Brand\";v=\"99\", \"Chromium\";v=\"149\", \"Google Chrome\";v=\"149\"",
        sec_ch_ua_platform: "\"macOS\"",
        sec_ch_ua_mobile: "?0",
        accept_language: "en-US,en;q=0.9",
    },
    Fingerprint {
        id: "chrome-linux",
        user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Not-A.Brand\";v=\"99\", \"Chromium\";v=\"149\", \"Google Chrome\";v=\"149\"",
        sec_ch_ua_platform: "\"Linux\"",
        sec_ch_ua_mobile: "?0",
        accept_language: "en-US,en;q=0.9",
    },
    Fingerprint {
        id: "chrome-win",
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Not-A.Brand\";v=\"99\", \"Chromium\";v=\"149\", \"Google Chrome\";v=\"149\"",
        sec_ch_ua_platform: "\"Windows\"",
        sec_ch_ua_mobile: "?0",
        accept_language: "en-US,en;q=0.9",
    },
    Fingerprint {
        id: "safari-mac",
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.4 Safari/605.1.15",
        sec_ch_ua: "\"Not-A.Brand\";v=\"99\", \"Safari\";v=\"18.4\"",
        sec_ch_ua_platform: "\"macOS\"",
        sec_ch_ua_mobile: "?0",
        accept_language: "en-US,en;q=0.9",
    },
];

pub struct FingerprintRotator {
    current: AtomicUsize,
}

impl FingerprintRotator {
    pub fn new() -> Self {
        Self {
            current: AtomicUsize::new(0),
        }
    }

    pub fn next_profile(&self) -> &'static Fingerprint {
        let idx = self.current.fetch_add(1, Ordering::Relaxed) % PROFILES.len();
        &PROFILES[idx]
    }
}

impl Default for FingerprintRotator {
    fn default() -> Self {
        Self::new()
    }
}
