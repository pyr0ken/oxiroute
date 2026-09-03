use serde::{Deserialize, Serialize};
use super::request::FormatType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    Bearer,
    ApiKeyHeader(String),
    OAuth2,
    Custom,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub vision: bool,
    pub tools: bool,
    pub thinking: bool,
    pub stream: bool,
    pub json_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub format: FormatType,
    pub auth_type: AuthType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Vec<ModelSpec>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
}

fn default_true() -> bool {
    true
}
