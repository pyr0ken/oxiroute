use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use oxiroute_auth::AntigravityAccount;
use oxiroute_core::{ChatChunk, ChatRequest, OxiError, Result};
use oxiroute_translator::AntigravityTranslator;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use uuid::Uuid;

pub const ENDPOINTS: &[&str] = &[
    "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse",
    "https://daily-cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse",
];

pub struct AntigravityExecutor {
    client: reqwest::Client,
}

impl AntigravityExecutor {
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

    async fn try_execute_endpoint(
        &self,
        endpoint: &str,
        request: &ChatRequest,
        target_model: &str,
        account: &AntigravityAccount,
    ) -> Result<reqwest::Response> {
        let envelope = AntigravityTranslator::build_envelope(
            request,
            target_model,
            &account.project_id,
        )?;

        let mut headers = HeaderMap::new();
        let auth_val = format!("Bearer {}", account.access_token.load());
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_val)
                .map_err(|e| OxiError::Auth(format!("Invalid auth header: {}", e)))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "antigravity/cli/1.1.5 (aidev_client; os_type=darwin; arch=arm64; auth_method=consumer)",
            ),
        );

        let response = self
            .client
            .post(endpoint)
            .headers(headers)
            .json(&envelope)
            .send()
            .await
            .map_err(|e| OxiError::Network(format!("Network dispatch error: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error body".to_string());
            return Err(OxiError::Provider {
                status,
                message: body_text,
                is_rate_limited: status == 429 || status == 503,
            });
        }

        Ok(response)
    }

    pub async fn execute_stream_with_failover(
        &self,
        request: &ChatRequest,
        target_model: &str,
        accounts: &[AntigravityAccount],
    ) -> Result<(impl Stream<Item = Result<ChatChunk>>, String)> {
        if accounts.is_empty() {
            return Err(OxiError::Auth("No available Antigravity accounts".to_string()));
        }

        let mut last_error = None;

        for (acc_idx, account) in accounts.iter().enumerate() {
            for (ep_idx, &endpoint) in ENDPOINTS.iter().enumerate() {
                tracing::debug!(
                    "Attempting account #{}: {} on endpoint #{}: {}",
                    acc_idx + 1,
                    account.email,
                    ep_idx + 1,
                    endpoint
                );

                match self.try_execute_endpoint(endpoint, request, target_model, account).await {
                    Ok(response) => {
                        let chunk_id = format!("chatcmpl-{}", Uuid::new_v4());
                        let model_name = target_model.to_string();
                        let active_email = account.email.clone();

                        let byte_stream = response.bytes_stream().eventsource();
                        let mapped_stream = byte_stream.filter_map(move |event_res| {
                            let cid = chunk_id.clone();
                            let mname = model_name.clone();
                            async move {
                                match event_res {
                                    Ok(event) => {
                                        let data = event.data.trim();
                                        if data.is_empty() || data == "[DONE]" {
                                            return None;
                                        }
                                        match AntigravityTranslator::parse_chunk(data, &mname, &cid) {
                                            Ok(Some(chunk)) => Some(Ok(chunk)),
                                            Ok(None) => None,
                                            Err(e) => Some(Err(e)),
                                        }
                                    }
                                    Err(e) => Some(Err(OxiError::Network(format!("SSE event error: {}", e)))),
                                }
                            }
                        });

                        return Ok((mapped_stream, active_email));
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failover trigger: Account {} on {} failed: {}. Trying next...",
                            account.email,
                            endpoint,
                            e
                        );
                        last_error = Some(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or(OxiError::AllCombosFailed))
    }
}
