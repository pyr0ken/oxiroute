use oxiroute_core::{ChatRequest, Result};
use crate::RequestTranslator;

pub struct GeminiTranslator;

impl RequestTranslator for GeminiTranslator {
    fn translate_request(&self, request: &ChatRequest, _target_model: &str) -> Result<serde_json::Value> {
        let mut contents = Vec::new();

        for msg in &request.messages {
            let role = match msg.role {
                oxiroute_core::Role::User => "user",
                oxiroute_core::Role::Assistant => "model",
                _ => "user",
            };

            contents.push(serde_json::json!({
                "role": role,
                "parts": [{ "text": msg.content.as_text() }]
            }));
        }

        let payload = serde_json::json!({
            "contents": contents,
            "generationConfig": {
                "temperature": request.temperature,
                "maxOutputTokens": request.max_tokens
            }
        });

        Ok(payload)
    }
}
