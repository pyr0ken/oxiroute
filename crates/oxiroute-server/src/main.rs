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
use oxiroute_core::ChatRequest;
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

    let storage = StorageManager::new(data_dir);
    storage.init()?;

    let auth_mgr = Arc::new(AntigravityAuthManager::load_from_omniroute_sqlite(
        &storage.db_path,
        &storage.env_path,
    )?);

    auth_mgr.clone().spawn_background_refresher(args.proxy.clone());

    let engine = Arc::new(Engine::new(args.proxy.as_deref()));

    let tui_state = if args.tui {
        let state = Arc::new(TuiState::new(args.port, args.proxy.clone()));
        let acc_displays = auth_mgr
            .list_accounts()
            .iter()
            .map(|a| AccountDisplay {
                id: a.email.clone(),
                provider: "antigravity".to_string(),
                name: a.email.clone(),
                email: a.email.clone(),
                priority: a.priority,
                is_active: a.is_active,
                status_text: "Active ●".to_string(),
            })
            .collect();
        state.set_accounts(acc_displays);

        // Pre-populate default smart combos
        state.set_combos(vec![
            tui::ComboDisplay {
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
            tui::ComboDisplay {
                id: "c-flash-fallback".to_string(),
                name: "flash-speed-router".to_string(),
                strategy: "round-robin".to_string(),
                models: vec![
                    "antigravity/gemini-3.7-flash-low".to_string(),
                    "antigravity/gemini-2.5-flash".to_string(),
                ],
                proxy_name: None,
            },
        ]);

        if let Some(ref _p) = args.proxy {
            state.set_proxies(vec![tui::ProxyDisplay {
                id: "p-default".to_string(),
                name: "Default SOCKS5 Egress".to_string(),
                protocol: "socks5h".to_string(),
                host: "127.0.0.1".to_string(),
                port: 5454,
                status: "Active ● (Connected)".to_string(),
            }]);
        }
        Some(state)
    } else {
        None
    };

    let app_state = AppState {
        engine,
        antigravity_auth: auth_mgr,
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
            tui::run_tui(state)?;
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
