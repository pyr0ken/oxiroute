use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use oxiroute_core::{ChatChunk, ChatRequest, OxiError, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;

pub struct UniversalExecutor {
    client: reqwest::Client,
}

impl UniversalExecutor {
    pub fn new(proxy_url: Option<&str>) -> Result<Self> {
        let mut builder = reqwest::Client::builder();

        if let Some(proxy_str) = proxy_url {
            let proxy = reqwest::Proxy::all(proxy_str)
                .map_err(|e| OxiError::Network(format!("Failed to configure proxy: {}", e)))?;
            builder = builder.proxy(proxy);
        }

        let client = builder
            .build()
            .map_err(|e| OxiError::Network(format!("Failed to build reqwest client: {}", e)))?;

        Ok(Self { client })
    }

    pub async fn execute_stream(
        &self,
        base_url: &str,
        api_key: &str,
        request: &ChatRequest,
        model_override: Option<&str>,
    ) -> Result<impl Stream<Item = Result<ChatChunk>>> {
        let endpoint = if base_url.ends_with("/chat/completions") {
            base_url.to_string()
        } else {
            format!("{}/chat/completions", base_url.trim_end_matches('/'))
        };

        let mut payload: Value = serde_json::to_value(request)?;
        if let Some(target) = model_override {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("model".to_string(), Value::String(target.to_string()));
            }
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| OxiError::Auth(format!("Invalid auth key: {}", e)))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));

        let response = self
            .client
            .post(&endpoint)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| OxiError::Network(format!("Universal provider dispatch error: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();
            return Err(OxiError::Provider {
                status,
                message: body_text,
                is_rate_limited: status == 429,
            });
        }

        let byte_stream = response.bytes_stream().eventsource();
        let mapped_stream = byte_stream.filter_map(|event_res| async move {
            match event_res {
                Ok(event) => {
                    let data = event.data.trim();
                    if data.is_empty() || data == "[DONE]" {
                        return None;
                    }
                    match serde_json::from_str::<ChatChunk>(data) {
                        Ok(chunk) => Some(Ok(chunk)),
                        Err(e) => {
                            tracing::debug!("Non-chunk SSE event: {} ({})", data, e);
                            None
                        }
                    }
                }
                Err(e) => Some(Err(OxiError::Network(format!("SSE event error: {}", e)))),
            }
        });

        Ok(mapped_stream)
    }
}
