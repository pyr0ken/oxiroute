use thiserror::Error;

#[derive(Error, Debug)]
pub enum OxiError {
    #[error("Serialization / Deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Format translation error: {0}")]
    Translation(String),

    #[error("Provider error ({status}): {message}")]
    Provider {
        status: u16,
        message: String,
        is_rate_limited: bool,
    },

    #[error("Authentication / Token error: {0}")]
    Auth(String),

    #[error("Combo routing error: all candidate providers failed")]
    AllCombosFailed,

    #[error("Model not found or unsupported: {0}")]
    ModelNotFound(String),

    #[error("Network / IO error: {0}")]
    Network(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, OxiError>;
