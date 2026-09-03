use oxiroute_core::{ChatRequest, Result};

pub mod anthropic;
pub mod antigravity;
pub mod gemini;
pub mod openai;

pub use antigravity::AntigravityTranslator;

pub trait RequestTranslator: Send + Sync {
    fn translate_request(&self, request: &ChatRequest, target_model: &str) -> Result<serde_json::Value>;
}

pub trait ResponseChunkTranslator: Send + Sync {
    fn translate_chunk(&self, chunk_data: &str) -> Result<Option<oxiroute_core::ChatChunk>>;
}
