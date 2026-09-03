use serde::{Deserialize, Serialize};
use super::message::Message;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallChunk>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallChunk {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub kind: Option<String>,
    pub function: FunctionCallChunk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCallChunk {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

impl ChatChunk {
    pub fn new_text(id: &str, model: &str, text: &str) -> Self {
        Self {
            id: id.to_string(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    content: Some(text.to_string()),
                    ..Default::default()
                },
                finish_reason: None,
            }],
            usage: None,
        }
    }
}
