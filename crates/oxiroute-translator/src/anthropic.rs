use oxiroute_core::{ChatRequest, Result};
use crate::RequestTranslator;

pub struct AnthropicTranslator;

impl RequestTranslator for AnthropicTranslator {
    fn translate_request(&self, request: &ChatRequest, target_model: &str) -> Result<serde_json::Value> {
        // Translation from OpenAI ChatRequest to Anthropic Messages API
        let mut system_prompt = String::new();
        let mut messages = Vec::new();

        for msg in &request.messages {
            match msg.role {
                oxiroute_core::Role::System => {
                    if !system_prompt.is_empty() {
                        system_prompt.push('\n');
                    }
                    system_prompt.push_str(&msg.content.as_text());
                }
                oxiroute_core::Role::User => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": msg.content.as_text()
                    }));
                }
                oxiroute_core::Role::Assistant => {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": msg.content.as_text()
                    }));
                }
                _ => {}
            }
        }

        let mut payload = serde_json::json!({
            "model": target_model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "stream": request.stream
        });

        if !system_prompt.is_empty() {
            payload["system"] = serde_json::Value::String(system_prompt);
        }

        if let Some(temp) = request.temperature {
            payload["temperature"] = serde_json::json!(temp);
        }

        Ok(payload)
    }
}
