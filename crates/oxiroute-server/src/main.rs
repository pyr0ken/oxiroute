pub mod tui;

use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use futures_util::StreamExt;
use oxiroute_auth::{default_oxiroute_dir, AntigravityAuthManager, StorageManager};
use oxiroute_core::{ChatRequest, ChatResponse, Choice, Message, Usage};
use oxiroute_engine::{Engine, PromptCompactor};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tui::{AccountDisplay, TuiState};

#[derive(Parser, Debug)]
#[command(name = "oxiroute", about = "Fast, lightweight AI gateway & LLM router in pure Rust")]
struct Args {
    #[arg(short, long, default_value_t = 20200)]
    port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, env = "OXI_PROXY")]
    proxy: Option<String>,

    #[arg(long, env = "OXI_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Run with interactive terminal dashboard
    #[arg(short, long)]
    tui: bool,
}

#[derive(Clone)]
struct AppState {
    engine: Arc<Engine>,
    antigravity_auth: Arc<AntigravityAuthManager>,
    tui_state: Option<Arc<TuiState>>,
}

async fn health_check() -> &'static str {
    "Oxiroute is running healthy! 🦀"
}

async fn list_models() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "object": "list",
        "data": [
            {
                "id": "antigravity/claude-sonnet-4-6",
                "object": "model",
                "created": 1700000000,
                "owned_by": "antigravity"
            },
            {
                "id": "antigravity/claude-opus-4-6-thinking",
                "object": "model",
                "created": 1700000000,
                "owned_by": "antigravity"
            },
            {
                "id": "antigravity/gemini-3.7-flash-low",
                "object": "model",
                "created": 1700000000,
                "owned_by": "antigravity"
            },
            {
                "id": "antigravity/gemini-2.5-flash",
                "object": "model",
                "created": 1700000000,
                "owned_by": "antigravity"
            },
            {
                "id": "claude-sonnet-4-6",
                "object": "model",
                "created": 1700000000,
                "owned_by": "antigravity"
            },
            {
                "id": "gemini-2.5-flash",
                "object": "model",
                "created": 1700000000,
                "owned_by": "antigravity"
            }
        ]
    }))
}

async fn chat_completions(
    State(state): State<AppState>,
    Json(raw_request): Json<ChatRequest>,
) -> Response {
    let start_time = Instant::now();
    let request = PromptCompactor::compact_request(raw_request);
    let model_name = request.model.clone();

    if let Some(ref tui) = state.tui_state {
        tui.record_request_start();
        tui.add_log("INFO", format!("Inbound request for model: {}", model_name));
    } else {
        tracing::info!("Handling chat request for model: {}", model_name);
    }

    let executor = match state.engine.antigravity() {
        Some(e) => e,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": {
                        "message": "Provider executor not initialized",
                        "type": "internal_error"
                    }
                })),
            )
                .into_response();
        }
    };

    let candidate_accounts = state.antigravity_auth.get_accounts_ordered();
    if candidate_accounts.is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "message": "No active provider connection available",
                    "type": "auth_error"
                }
            })),
        )
            .into_response();
    }

    let tui_ref = state.tui_state.clone();
    let model_for_done = model_name.clone();

    match executor
        .execute_stream_with_failover(&request, &model_name, &candidate_accounts)
        .await
    {
        Ok((stream, active_account)) => {
            let acc_for_done = active_account.clone();
            if let Some(ref tui) = tui_ref {
                tui.add_log("INFO", format!("Routed '{}' -> Account: {}", model_name, active_account));
            } else {
                tracing::info!(
                    "Successfully dispatched model '{}' via account: {}",
                    model_name,
                    active_account
                );
            }

            if !request.stream {
                let mut full_content = String::new();
                let mut finish_reason = Some("stop".to_string());
                let mut chunk_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
                let mut created = chrono::Utc::now().timestamp();
                let mut accumulated_tool_calls: Vec<oxiroute_core::ToolCall> = Vec::new();
                let mut pinned_stream = Box::pin(stream);

                while let Some(res) = pinned_stream.next().await {
                    match res {
                        Ok(chunk) => {
                            chunk_id = chunk.id;
                            created = chunk.created;
                            if let Some(first_choice) = chunk.choices.first() {
                                if let Some(ref text) = first_choice.delta.content {
                                    full_content.push_str(text);
                                }
                                if let Some(ref tcs) = first_choice.delta.tool_calls {
                                    for tc in tcs {
                                        accumulated_tool_calls.push(oxiroute_core::ToolCall {
                                            id: tc.id.clone().unwrap_or_else(|| {
                                                format!("call_{}", uuid::Uuid::new_v4().simple())
                                            }),
                                            kind: tc.kind.clone().unwrap_or_else(|| "function".to_string()),
                                            function: oxiroute_core::FunctionCall {
                                                name: tc.function.name.clone().unwrap_or_default(),
                                                arguments: tc.function.arguments.clone(),
                                            },
                                        });
                                    }
                                }
                                if let Some(ref reason) = first_choice.finish_reason {
                                    finish_reason = Some(reason.clone());
                                }
                            }
                        }
                        Err(e) => {
                            let latency_ms = start_time.elapsed().as_millis() as u64;
                            if let Some(ref tui) = tui_ref {
                                tui.record_request_end(&model_name, "failed", 0, latency_ms, 502);
                                tui.add_log("ERROR", format!("Execution failed: {}", e));
                            } else {
                                tracing::error!("Stream error during aggregation: {}", e);
                            }
                            return (
                                StatusCode::BAD_GATEWAY,
                                Json(serde_json::json!({
                                    "error": {
                                        "message": e.to_string(),
                                        "type": "provider_error"
                                    }
                                })),
                            )
                                .into_response();
                        }
                    }
                }

                let latency_ms = start_time.elapsed().as_millis() as u64;
                if let Some(ref tui) = tui_ref {
                    tui.record_request_end(&model_for_done, &acc_for_done, 50, latency_ms, 200);
                    tui.add_log("INFO", format!("Request completed in {}ms", latency_ms));
                }

                if !accumulated_tool_calls.is_empty() {
                    finish_reason = Some("tool_calls".to_string());
                }

                let response = ChatResponse {
                    id: chunk_id,
                    object: "chat.completion".to_string(),
                    created,
                    model: model_for_done,
                    choices: vec![Choice {
                        index: 0,
                        message: Message {
                            role: oxiroute_core::Role::Assistant,
                            content: oxiroute_core::MessageContent::Text(full_content),
                            name: None,
                            tool_calls: if accumulated_tool_calls.is_empty() {
                                None
                            } else {
                                Some(accumulated_tool_calls)
                            },
                            tool_call_id: None,
                        },
                        finish_reason,
                    }],
                    usage: Some(Usage {
                        prompt_tokens: 10,
                        completion_tokens: 10,
                        total_tokens: 20,
                        prompt_tokens_details: None,
                        completion_tokens_details: None,
                    }),
                };

                return (StatusCode::OK, Json(response)).into_response();
            }

            let sse_stream = stream.map(move |res| match res {
                Ok(chunk) => {
                    let json_str = serde_json::to_string(&chunk).unwrap_or_default();
                    Ok::<Event, Infallible>(Event::default().data(json_str))
                }
                Err(e) => {
                    let err_json = serde_json::json!({
                        "error": {
                            "message": e.to_string(),
                            "type": "provider_error"
                        }
                    });
                    Ok::<Event, Infallible>(Event::default().data(err_json.to_string()))
                }
            });

            let done_stream = futures_util::stream::once(async move {
                let latency_ms = start_time.elapsed().as_millis() as u64;
                if let Some(ref tui) = tui_ref {
                    tui.record_request_end(&model_for_done, &acc_for_done, 50, latency_ms, 200);
                    tui.add_log("INFO", format!("Stream completed in {}ms", latency_ms));
                }
                Ok::<Event, Infallible>(Event::default().data("[DONE]"))
            });

            let combined_stream = sse_stream.chain(done_stream);

            Sse::new(combined_stream)
                .keep_alive(KeepAlive::default())
                .into_response()
        }
        Err(e) => {
            let latency_ms = start_time.elapsed().as_millis() as u64;
            if let Some(ref tui) = tui_ref {
                tui.record_request_end(&model_name, "failed", 0, latency_ms, 502);
                tui.add_log("ERROR", format!("Execution failed: {}", e));
            } else {
                tracing::error!("Execution error after all failovers: {}", e);
            }
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": {
                        "message": e.to_string(),
                        "type": "provider_error"
                    }
                })),
            )
                .into_response()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !args.tui {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    let data_dir = args.data_dir.unwrap_or_else(default_oxiroute_dir);

    let storage = Arc::new(StorageManager::new(data_dir));
    storage.init()?;

    let auth_mgr = Arc::new(AntigravityAuthManager::load_from_omniroute_sqlite(
        &storage.db_path,
        &storage.env_path,
    )?);

    auth_mgr.clone().spawn_background_refresher(args.proxy.clone());

    let engine = Arc::new(Engine::new(args.proxy.as_deref()));

    let tui_state = if args.tui {
        let state = Arc::new(TuiState::new(args.port, args.proxy.clone(), Some(storage.clone())));

        // 1. Load historical burnt tokens and historical timeline from database
        if let Ok((in_tok, out_tok, reason_tok)) = storage.get_historical_usage_totals() {
            let total_hist = in_tok + out_tok + reason_tok;
            state.set_historical_burned_tokens(total_hist);
        }

        if let Ok(hist_timeline) = storage.load_historical_timeline(30) {
            for (id, ts, model, account, tokens, latency, status) in hist_timeline.into_iter().rev() {
                state.record_historical_timeline_item(id, ts, model, account, tokens, latency, status);
            }
        }

        // 2. Load accounts / providers from SQLite
        let mut acc_displays = Vec::new();
        if let Ok(providers) = storage.load_provider_connections() {
            for p in providers {
                let display_name = p
                    .name
                    .clone()
                    .unwrap_or_else(|| p.email.clone().unwrap_or_else(|| p.provider.clone()));
                let email = p.email.clone().unwrap_or_else(|| display_name.clone());
                let status_text = if p.is_active {
                    "Active ●".to_string()
                } else {
                    "Disabled ○".to_string()
                };
                acc_displays.push(AccountDisplay {
                    id: p.id,
                    provider: p.provider,
                    name: display_name,
                    email,
                    priority: p.priority,
                    is_active: p.is_active,
                    status_text,
                });
            }
        }
        if acc_displays.is_empty() {
            acc_displays = auth_mgr
                .list_accounts()
                .iter()
                .map(|a| AccountDisplay {
                    id: a.id.clone(),
                    provider: "antigravity".to_string(),
                    name: a.email.clone(),
                    email: a.email.clone(),
                    priority: a.priority,
                    is_active: a.is_active,
                    status_text: "Active ●".to_string(),
                })
                .collect();
        }
        state.set_accounts(acc_displays);

        // 2. Load Combos from SQLite
        let mut db_combos = storage.load_combos().unwrap_or_default();
        if db_combos.is_empty() {
            let default_combos = vec![
                oxiroute_auth::ComboRecord {
                    id: "c-claude-agent".to_string(),
                    name: "claude-smart-coding".to_string(),
                    strategy: "priority".to_string(),
                    models: vec![
                        "antigravity/claude-sonnet-4-6".to_string(),
                        "antigravity/claude-opus-4-6-thinking".to_string(),
                        "antigravity/gemini-3.7-flash-low".to_string(),
                    ],
                    proxy_name: args.proxy.clone(),
                },
                oxiroute_auth::ComboRecord {
                    id: "c-flash-fallback".to_string(),
                    name: "flash-speed-router".to_string(),
                    strategy: "round-robin".to_string(),
                    models: vec![
                        "antigravity/gemini-3.7-flash-low".to_string(),
                        "antigravity/gemini-2.5-flash".to_string(),
                    ],
                    proxy_name: None,
                },
            ];
            for c in &default_combos {
                let _ = storage.save_combo(c);
            }
            db_combos = default_combos;
        }
        state.set_combos(
            db_combos
                .into_iter()
                .map(|c| tui::ComboDisplay {
                    id: c.id,
                    name: c.name,
                    strategy: c.strategy,
                    models: c.models,
                    proxy_name: c.proxy_name,
                    status_text: None,
                })
                .collect(),
        );

        // 3. Load Proxies from SQLite
        let mut db_proxies = storage.load_proxies().unwrap_or_default();
        if db_proxies.is_empty() {
            if let Some(ref _p) = args.proxy {
                let def_proxy = oxiroute_auth::ProxyRecord {
                    id: "p-default".to_string(),
                    name: Some("Default SOCKS5 Egress".to_string()),
                    protocol: "socks5h".to_string(),
                    host: "127.0.0.1".to_string(),
                    port: 5454,
                    username: None,
                    password: None,
                    status: "active".to_string(),
                };
                let _ = storage.save_proxy(&def_proxy);
                db_proxies.push(def_proxy);
            }
        }
        state.set_proxies(
            db_proxies
                .into_iter()
                .map(|p| tui::ProxyDisplay {
                    id: p.id,
                    name: p.name.unwrap_or_else(|| format!("{}:{}", p.host, p.port)),
                    protocol: p.protocol,
                    host: p.host,
                    port: p.port,
                    status: if p.status == "active" {
                        "Active ●".to_string()
                    } else {
                        "Inactive ○".to_string()
                    },
                })
                .collect(),
        );

        Some(state)
    } else {
        None
    };

    let app_state = AppState {
        engine,
        antigravity_auth: auth_mgr.clone(),
        tui_state: tui_state.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .layer(cors)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    if args.tui {
        // Run Axum server in background task
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // Run interactive TUI in foreground thread
        if let Some(state) = tui_state {
            tui::run_tui(state, auth_mgr.clone())?;
        }
    } else {
        tracing::info!("🦀 Oxiroute listening on http://{}", addr);
        if let Some(ref p) = args.proxy {
            tracing::info!("Using configured egress proxy: {}", p);
        }
        axum::serve(listener, app).await?;
    }

    Ok(())
}
