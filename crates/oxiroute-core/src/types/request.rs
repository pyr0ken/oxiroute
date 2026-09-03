use serde::{Deserialize, Serialize};
use super::message::{Message, ToolDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatType {
    OpenAI,
    Anthropic,
    Gemini,
    Kiro,
    Codex,
}

impl std::fmt::Display for FormatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatType::OpenAI => write!(f, "openai"),
            FormatType::Anthropic => write!(f, "anthropic"),
            FormatType::Gemini => write!(f, "gemini"),
            FormatType::Kiro => write!(f, "kiro"),
            FormatType::Codex => write!(f, "codex"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

fn default_true() -> bool {
    true
}
