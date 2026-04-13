//! aion-server — HTTP REST API for Skill Router
//!
//! 将 Aion 能力路由器以 Web 服务形式对外暴露，
//! 供其他 Rust 服务、aion-cli 远程模式或 curl 调试使用。
//! 所有代码遵循项目规定：仅使用 Rust。
//!
//! # Endpoints
//!
//! | Method | Path                  | Description                          |
//! |--------|-----------------------|--------------------------------------|
//! | GET    | /v1/health            | Health check                         |
//! | GET    | /v1/capabilities      | List all registered capabilities     |
//! | POST   | /v1/route             | Route task (natural language)         |
//! | POST   | /v1/route/native      | Route task (structured AiNativePayload) |
//! | GET    | /v1/memory/recall     | Recall memories by query             |
//! | POST   | /v1/memory/remember   | Store a new memory entry             |
//! | GET    | /v1/memory/stats      | Memory store statistics              |
//! | GET    | /v1/agents            | Agent node information               |
//! | POST   | /v1/agents/delegate   | Delegate task to specific agent      |
//! | GET    | /v1/metrics           | Prometheus metrics (placeholder)     |

mod error;
mod events;
mod handlers;
mod telemetry;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use axum::http::{HeaderValue, Method};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

use aion_memory::memory::MemoryManager;
use aion_router::SkillRouter;
use aion_types::types::RouterPaths;

/// Shared application state injected into all handlers
pub struct AppState {
    pub router: Arc<SkillRouter>,
    pub memory: Arc<MemoryManager>,
    pub paths: RouterPaths,
    pub prometheus: metrics_exporter_prometheus::PrometheusHandle,
    pub event_bus: Arc<events::EventBus>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv().ok();

    // Structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("aion_server=info,aion_router=info,tower_http=info")),
        )
        .init();

    // Workspace paths (default: current directory)
    let workdir = std::env::var("AION_WORKSPACE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let paths = RouterPaths::for_workspace(&workdir);
    info!("Workspace: {:?}", paths.workspace_root);

    // Initialize metrics
    let prometheus_handle = telemetry::init_prometheus();

    // Initialize core services
    let skill_router = Arc::new(SkillRouter::new(paths.clone())?);
    let memory_manager = Arc::new(MemoryManager::new(&paths.workspace_root));

    let event_bus = Arc::new(events::EventBus::new(256));

    let state = Arc::new(AppState {
        router: skill_router,
        memory: memory_manager,
        paths,
        prometheus: prometheus_handle,
        event_bus,
    });

    // CORS policy: read allowed origins from CORS_ALLOWED_ORIGINS env var.
    // Use "*" for fully permissive (development only).
    // Default: localhost dev servers.
    let cors = build_cors_layer();

    // Build router
    let app = Router::new()
        // ── Health & Info ──
        .route("/v1/health", get(handlers::health))
        .route("/v1/metrics", get(handlers::metrics))
        // ── Capabilities ──
        .route("/v1/capabilities", get(handlers::list_capabilities))
        // ── Routing ──
        .route("/v1/route", post(handlers::route_task))
        .route("/v1/route/native", post(handlers::route_native))
        // ── Memory ──
        .route("/v1/memory/recall", get(handlers::memory_recall))
        .route("/v1/memory/remember", post(handlers::memory_remember))
        .route("/v1/memory/stats", get(handlers::memory_stats))
        // ── Agent Management ──
        .route("/v1/agents", get(handlers::agents_info))
        .route("/v1/agents/delegate", post(handlers::agent_delegate))
        // ── WebSocket ──
        .route("/v1/stream/{session_id}", get(ws::ws_handler))
        // ── Middleware ──
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    // Bind address
    let host = std::env::var("AION_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("AION_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::new(host.parse()?, port);
    info!("aion-server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Build CORS layer from `CORS_ALLOWED_ORIGINS` environment variable.
///
/// - If set to `"*"`: fully permissive (development mode).
/// - If set to comma-separated origins (e.g. `"https://app.example.com,https://admin.example.com"`):
///   only those origins are allowed.
/// - If unset: defaults to `http://localhost:3000,http://localhost:8080`.
fn build_cors_layer() -> CorsLayer {
    let raw = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:3000,http://localhost:8080".to_string());

    let base = if raw.trim() == "*" {
        info!("CORS: permissive mode (all origins allowed)");
        CorsLayer::new().allow_origin(Any)
    } else {
        let origins: Vec<HeaderValue> = raw
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    return None;
                }
                match s.parse::<HeaderValue>() {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!("Ignoring invalid CORS origin '{}': {}", s, e);
                        None
                    }
                }
            })
            .collect();
        info!("CORS: allowing {} origin(s)", origins.len());
        CorsLayer::new().allow_origin(origins)
    };

    base.allow_methods([
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
    ])
    .allow_headers([
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
        axum::http::header::ACCEPT,
    ])
}
