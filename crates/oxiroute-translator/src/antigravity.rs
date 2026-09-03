use oxiroute_core::{ChatChunk, ChatRequest, ChunkChoice, Delta, OxiError, Result, Role};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityContent {
    pub role: String,
    pub parts: Vec<AntigravityPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravitySafetySetting {
    pub category: String,
    pub threshold: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxOutputTokens")]
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityInnerRequest {
    pub contents: Vec<AntigravityContent>,
    #[serde(rename = "generationConfig")]
    pub generation_config: AntigravityGenerationConfig,
    #[serde(rename = "safetySettings")]
    pub safety_settings: Vec<AntigravitySafetySetting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityRequestEnvelope {
    pub project: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub model: String,
    #[serde(rename = "userAgent")]
    pub user_agent: String,
    #[serde(rename = "requestType")]
    pub request_type: String,
    pub request: AntigravityInnerRequest,
}

pub struct AntigravityTranslator;

impl AntigravityTranslator {
    pub fn build_envelope(
        request: &ChatRequest,
        target_model: &str,
        project_id: &str,
    ) -> Result<AntigravityRequestEnvelope> {
        let mut contents = Vec::new();

        for msg in &request.messages {
            let role = match msg.role {
                Role::User => "user".to_string(),
                Role::Assistant => "model".to_string(),
                Role::System => "user".to_string(), // In Antigravity Gemini format, system prompts map to user turn or prefix
                Role::Tool | Role::Function => "user".to_string(),
            };

            let text = msg.content.as_text();
            if !text.is_empty() {
                contents.push(AntigravityContent {
                    role,
                    parts: vec![AntigravityPart { text: Some(text) }],
                });
            }
        }

        // Clean model name (strip "antigravity/" prefix if present)
        let clean_model = target_model
            .strip_prefix("antigravity/")
            .unwrap_or(target_model);

        let safety_settings = vec![
            AntigravitySafetySetting {
                category: "HARM_CATEGORY_HARASSMENT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            AntigravitySafetySetting {
                category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            AntigravitySafetySetting {
                category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            AntigravitySafetySetting {
                category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
        ];

        let envelope = AntigravityRequestEnvelope {
            project: project_id.to_string(),
            request_id: format!("agent-{}", Uuid::new_v4()),
            model: clean_model.to_string(),
            user_agent: "antigravity".to_string(),
            request_type: "agent".to_string(),
            request: AntigravityInnerRequest {
                contents,
                generation_config: AntigravityGenerationConfig {
                    temperature: request.temperature.or(Some(0.2)),
                    top_p: request.top_p,
                    max_output_tokens: request.max_tokens.or(Some(8192)),
                },
                safety_settings,
            },
        };

        Ok(envelope)
    }

    pub fn parse_chunk(raw_json: &str, model: &str, chunk_id: &str) -> Result<Option<ChatChunk>> {
        let val: serde_json::Value = serde_json::from_str(raw_json)?;

        // Check if there is an error block
        if let Some(err) = val.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown Google API error");
            return Err(OxiError::Provider {
                status: err.get("code").and_then(|c| c.as_u64()).unwrap_or(500) as u16,
                message: msg.to_string(),
                is_rate_limited: msg.contains("quota") || msg.contains("RESOURCE_EXHAUSTED"),
            });
        }

        let resp = match val.get("response") {
            Some(r) => r,
            None => return Ok(None),
        };

        let candidate = resp.get("candidates").and_then(|c| c.get(0));
        let parts = candidate
            .and_then(|c| c.get("content"))
            .and_then(|cnt| cnt.get("parts"));

        let mut accumulated_text = String::new();
        if let Some(parts_arr) = parts.and_then(|p| p.as_array()) {
            for part in parts_arr {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    accumulated_text.push_str(text);
                }
            }
        }

        let finish_reason = candidate
            .and_then(|c| c.get("finishReason"))
            .and_then(|f| f.as_str())
            .map(|f| match f {
                "STOP" => "stop".to_string(),
                "MAX_TOKENS" => "length".to_string(),
                other => other.to_lowercase(),
            });

        if accumulated_text.is_empty() && finish_reason.is_none() {
            return Ok(None);
        }

        Ok(Some(ChatChunk {
            id: chunk_id.to_string(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: Some("assistant".to_string()),
                    content: if accumulated_text.is_empty() {
                        None
                    } else {
                        Some(accumulated_text)
                    },
                    ..Default::default()
                },
                finish_reason,
            }],
            usage: None,
        }))
    }
}
