use std::collections::HashMap;
use oxiroute_core::{
    ChatChunk, ChatRequest, ChunkChoice, Delta, FunctionCallChunk, OxiError, Result, Role,
    ToolCallChunk,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityFunctionCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityFunctionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub response: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "thoughtSignature")]
    pub thought_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "functionCall")]
    pub function_call: Option<AntigravityFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "functionResponse")]
    pub function_response: Option<AntigravityFunctionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityContent {
    pub role: String,
    pub parts: Vec<AntigravityPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityFunctionDeclaration {
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityTool {
    #[serde(skip_serializing_if = "Option::is_none", rename = "functionDeclarations")]
    pub function_declarations: Option<Vec<AntigravityFunctionDeclaration>>,
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
    #[serde(skip_serializing_if = "Option::is_none", rename = "sessionId")]
    pub session_id: Option<String>,
    pub contents: Vec<AntigravityContent>,
    #[serde(rename = "generationConfig")]
    pub generation_config: AntigravityGenerationConfig,
    #[serde(rename = "safetySettings")]
    pub safety_settings: Vec<AntigravitySafetySetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AntigravityTool>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "toolConfig")]
    pub tool_config: Option<serde_json::Value>,
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

pub fn clean_schema_for_gemini(schema: &mut serde_json::Value) {
    if let serde_json::Value::Object(map) = schema {
        const UNSUPPORTED: &[&str] = &[
            "minLength",
            "maxLength",
            "exclusiveMinimum",
            "exclusiveMaximum",
            "multipleOf",
            "strict",
            "encrypted",
            "minItems",
            "maxItems",
            "format",
            "default",
            "examples",
            "$schema",
            "$id",
            "$anchor",
            "additionalProperties",
        ];
        for key in UNSUPPORTED {
            map.remove(*key);
        }

        if let Some(any_of) = map.remove("anyOf").or_else(|| map.remove("oneOf")) {
            if let serde_json::Value::Array(arr) = any_of {
                for mut item in arr {
                    if let serde_json::Value::Object(ref item_map) = item {
                        if item_map.get("type").and_then(|t| t.as_str()) != Some("null") {
                            clean_schema_for_gemini(&mut item);
                            if let serde_json::Value::Object(item_obj) = item {
                                for (k, v) in item_obj {
                                    map.entry(k).or_insert(v);
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        if let Some(serde_json::Value::Object(props)) = map.get_mut("properties") {
            for (_, prop_val) in props.iter_mut() {
                clean_schema_for_gemini(prop_val);
            }
        }

        if let Some(items) = map.get_mut("items") {
            clean_schema_for_gemini(items);
        }

        if let Some(props) = map.get("properties").and_then(|p| p.as_object()).cloned() {
            if let Some(serde_json::Value::Array(reqs)) = map.get_mut("required") {
                reqs.retain(|r| {
                    r.as_str().map(|s| props.contains_key(s)).unwrap_or(false)
                });
            }
        }

        if map.get("type").and_then(|t| t.as_str()) == Some("object") {
            let is_empty = match map.get("properties") {
                Some(serde_json::Value::Object(p)) => p.is_empty(),
                _ => true,
            };
            if is_empty {
                let mut props = serde_json::Map::new();
                let mut reason_field = serde_json::Map::new();
                reason_field.insert("type".to_string(), serde_json::json!("string"));
                reason_field.insert(
                    "description".to_string(),
                    serde_json::json!("Brief explanation of why you are calling this tool"),
                );
                props.insert("reason".to_string(), serde_json::Value::Object(reason_field));
                map.insert("properties".to_string(), serde_json::Value::Object(props));
                map.insert("required".to_string(), serde_json::json!(["reason"]));
            }
        }
    }
}

fn merge_consecutive_contents(contents: Vec<AntigravityContent>) -> Vec<AntigravityContent> {
    let mut merged: Vec<AntigravityContent> = Vec::new();
    for content in contents {
        if content.parts.is_empty() {
            continue;
        }
        if let Some(last) = merged.last_mut() {
            if last.role == content.role {
                last.parts.extend(content.parts);
                continue;
            }
        }
        merged.push(content);
    }
    merged
}

impl AntigravityTranslator {
    pub fn build_envelope(
        request: &ChatRequest,
        target_model: &str,
        project_id: &str,
    ) -> Result<AntigravityRequestEnvelope> {
        let mut raw_contents = Vec::new();

        // 1. Build map of tool_call_id -> function_name
        let mut tc_id_to_name: HashMap<String, String> = HashMap::new();
        for msg in &request.messages {
            if let Some(ref tcs) = msg.tool_calls {
                for tc in tcs {
                    tc_id_to_name.insert(tc.id.clone(), tc.function.name.clone());
                }
            }
        }

        for msg in &request.messages {
            match msg.role {
                Role::User | Role::System => {
                    let text = msg.content.as_text();
                    if !text.is_empty() {
                        raw_contents.push(AntigravityContent {
                            role: "user".to_string(),
                            parts: vec![AntigravityPart {
                                text: Some(text),
                                thought: None,
                                thought_signature: None,
                                function_call: None,
                                function_response: None,
                            }],
                        });
                    }
                }
                Role::Assistant => {
                    let mut parts = Vec::new();
                    let text = msg.content.as_text();
                    if !text.is_empty() {
                        parts.push(AntigravityPart {
                            text: Some(text),
                            thought: None,
                            thought_signature: None,
                            function_call: None,
                            function_response: None,
                        });
                    }
                    if let Some(ref tcs) = msg.tool_calls {
                        for tc in tcs {
                            let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                                .unwrap_or_else(|_| serde_json::json!({}));
                            parts.push(AntigravityPart {
                                text: None,
                                thought: None,
                                thought_signature: Some("skip_thought_signature_validator".to_string()),
                                function_call: Some(AntigravityFunctionCall {
                                    id: Some(tc.id.clone()),
                                    name: tc.function.name.clone(),
                                    args,
                                }),
                                function_response: None,
                            });
                        }
                    }
                    if !parts.is_empty() {
                        raw_contents.push(AntigravityContent {
                            role: "model".to_string(),
                            parts,
                        });
                    }
                }
                Role::Tool | Role::Function => {
                    let name = msg
                        .name
                        .clone()
                        .or_else(|| {
                            msg.tool_call_id
                                .as_ref()
                                .and_then(|id| tc_id_to_name.get(id).cloned())
                        })
                        .unwrap_or_else(|| "unknown".to_string());
                    let text = msg.content.as_text();
                    let parsed_res = serde_json::from_str::<serde_json::Value>(&text)
                        .unwrap_or_else(|_| serde_json::Value::String(text));

                    raw_contents.push(AntigravityContent {
                        role: "user".to_string(),
                        parts: vec![AntigravityPart {
                            text: None,
                            thought: None,
                            thought_signature: None,
                            function_call: None,
                            function_response: Some(AntigravityFunctionResponse {
                                id: msg.tool_call_id.clone(),
                                name,
                                response: serde_json::json!({ "result": parsed_res }),
                            }),
                        }],
                    });
                }
            }
        }

        let contents = merge_consecutive_contents(raw_contents);

        let (tools, tool_config) = if let Some(ref tools) = request.tools {
            let mut function_declarations = Vec::new();
            for tool in tools {
                if tool.kind == "function" {
                    let mut params = tool.function.parameters.clone();
                    clean_schema_for_gemini(&mut params);
                    function_declarations.push(AntigravityFunctionDeclaration {
                        name: tool.function.name.clone(),
                        description: tool.function.description.clone().unwrap_or_default(),
                        parameters: params,
                    });
                }
            }
            if !function_declarations.is_empty() {
                let mode = match request.tool_choice.as_ref() {
                    Some(serde_json::Value::String(s)) if s == "none" => "NONE",
                    Some(serde_json::Value::String(s)) if s == "required" || s == "any" => "ANY",
                    _ => "AUTO",
                };
                (
                    Some(vec![AntigravityTool {
                        function_declarations: Some(function_declarations),
                    }]),
                    Some(serde_json::json!({
                        "functionCallingConfig": {
                            "mode": mode
                        }
                    })),
                )
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let clean_model = match target_model {
            "antigravity"
            | "gemini-3.8-flash-high"
            | "gemini-3.8-flash-medium"
            | "gemini-3.8-flash-low"
            | "gemini-3.8-flash-tiered"
            | "agy/gemini-3.8-flash-high"
            | "agy/gemini-3.8-flash-medium"
            | "agy/gemini-3.8-flash-low"
            | "agy/gemini-3.8-flash-tiered" => "gemini-3.8-flash-tiered",
            "claude" | "agy/claude-sonnet-4-6" => "claude-sonnet-4-6",
            m => m
                .strip_prefix("antigravity/")
                .or_else(|| m.strip_prefix("agy/"))
                .unwrap_or(m),
        };

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

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let u128_val = Uuid::new_v4().as_u128();
        let session_id = format!("-{}", (u128_val % 9_000_000_000_000_000_000) + 1);
        let hex_suffix = format!("{:08x}", (u128_val & 0xFFFFFFFF) as u32);
        let request_id = format!("agent/{}/{}", now_ms, hex_suffix);

        let envelope = AntigravityRequestEnvelope {
            project: project_id.to_string(),
            request_id,
            model: clean_model.to_string(),
            user_agent: "antigravity".to_string(),
            request_type: "agent".to_string(),
            request: AntigravityInnerRequest {
                session_id: Some(session_id),
                contents,
                generation_config: AntigravityGenerationConfig {
                    temperature: request.temperature.or(Some(0.2)),
                    top_p: request.top_p,
                    max_output_tokens: request.max_tokens.or(Some(8192)),
                },
                safety_settings,
                tools,
                tool_config,
            },
        };

        Ok(envelope)
    }

    pub fn parse_chunk(raw_json: &str, model: &str, chunk_id: &str) -> Result<Option<ChatChunk>> {
        let val: serde_json::Value = serde_json::from_str(raw_json)?;

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
        let mut reasoning_text = String::new();
        let mut tool_call_chunks = Vec::new();

        if let Some(parts_arr) = parts.and_then(|p| p.as_array()) {
            for part in parts_arr {
                let is_thought = part.get("thought").and_then(|t| t.as_bool()).unwrap_or(false);
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    if is_thought {
                        reasoning_text.push_str(text);
                    } else {
                        accumulated_text.push_str(text);
                    }
                }
                if let Some(fc) = part.get("functionCall") {
                    let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                    let args = fc.get("args").cloned().unwrap_or(serde_json::json!({}));
                    let args_str = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                    let tool_id = fc
                        .get("id")
                        .and_then(|i| i.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("call_{}", Uuid::new_v4().simple()));
                    tool_call_chunks.push(ToolCallChunk {
                        index: tool_call_chunks.len() as u32,
                        id: Some(tool_id),
                        kind: Some("function".to_string()),
                        function: FunctionCallChunk {
                            name: Some(name),
                            arguments: args_str,
                        },
                    });
                }
            }
        }

        let finish_reason = if !tool_call_chunks.is_empty() {
            Some("tool_calls".to_string())
        } else {
            candidate
                .and_then(|c| c.get("finishReason"))
                .and_then(|f| f.as_str())
                .map(|f| match f {
                    "STOP" => "stop".to_string(),
                    "MAX_TOKENS" => "length".to_string(),
                    other => other.to_lowercase(),
                })
        };

        if accumulated_text.is_empty()
            && reasoning_text.is_empty()
            && tool_call_chunks.is_empty()
            && finish_reason.is_none()
        {
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
                    tool_calls: if tool_call_chunks.is_empty() {
                        None
                    } else {
                        Some(tool_call_chunks)
                    },
                    reasoning_content: if reasoning_text.is_empty() {
                        None
                    } else {
                        Some(reasoning_text)
                    },
                },
                finish_reason,
            }],
            usage: None,
        }))
    }
}

