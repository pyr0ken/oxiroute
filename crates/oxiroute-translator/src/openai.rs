use oxiroute_core::{ChatRequest, Result};
use crate::RequestTranslator;

pub struct OpenAiTranslator;

impl RequestTranslator for OpenAiTranslator {
    fn translate_request(&self, request: &ChatRequest, target_model: &str) -> Result<serde_json::Value> {
        let mut val = serde_json::to_value(request)?;
        if let Some(obj) = val.as_object_mut() {
            obj.insert("model".to_string(), serde_json::Value::String(target_model.to_string()));
        }
        Ok(val)
    }
}
