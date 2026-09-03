use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    AuthenticationError,
    PermissionError,
    RateLimit,
    QuotaExhausted,
    ModelCapacityExhausted,
    Timeout,
    NetworkError,
    Provider5xx,
    InvalidRequest,
    ModelUnavailable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedFailure {
    pub kind: FailureKind,
    pub retryable: bool,
    pub status_code: Option<u16>,
    pub message: String,
    pub retry_after_secs: Option<u64>,
}

impl ClassifiedFailure {
    pub fn classify(status_code: Option<u16>, message: &str) -> Self {
        let msg_lower = message.to_lowercase();
        let mut kind = FailureKind::Unknown;
        let mut retryable = false;

        if let Some(status) = status_code {
            match status {
                401 => {
                    kind = FailureKind::AuthenticationError;
                }
                403 => {
                    kind = FailureKind::PermissionError;
                }
                404 => {
                    kind = FailureKind::ModelUnavailable;
                }
                408 | 504 => {
                    kind = FailureKind::Timeout;
                    retryable = true;
                }
                429 => {
                    if msg_lower.contains("quota")
                        || msg_lower.contains("insufficient")
                        || msg_lower.contains("exhausted")
                    {
                        kind = FailureKind::QuotaExhausted;
                    } else {
                        kind = FailureKind::RateLimit;
                    }
                    retryable = true;
                }
                503 => {
                    if msg_lower.contains("capacity") || msg_lower.contains("model_capacity_exhausted") {
                        kind = FailureKind::ModelCapacityExhausted;
                    } else {
                        kind = FailureKind::Provider5xx;
                    }
                    retryable = true;
                }
                500..=599 => {
                    kind = FailureKind::Provider5xx;
                    retryable = true;
                }
                400 => {
                    kind = FailureKind::InvalidRequest;
                }
                _ => {}
            }
        }

        if kind == FailureKind::Unknown {
            if msg_lower.contains("econnreset")
                || msg_lower.contains("econnrefused")
                || msg_lower.contains("network")
                || msg_lower.contains("socket")
            {
                kind = FailureKind::NetworkError;
                retryable = true;
            } else if msg_lower.contains("timed out") || msg_lower.contains("timeout") {
                kind = FailureKind::Timeout;
                retryable = true;
            } else if msg_lower.contains("unauthorized") || msg_lower.contains("invalid token") {
                kind = FailureKind::AuthenticationError;
            }
        }

        Self {
            kind,
            retryable,
            status_code,
            message: message.to_string(),
            retry_after_secs: None,
        }
    }
}
