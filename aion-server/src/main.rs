//! aion-server — HTTP REST API for Skill Router
//!
//! 将 Aion 能力路由器以 Web 服务形式对外暴露，
//! 供其他 Rust 服务、aion-forge-cli 或 curl 调试使用。
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

#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderValue, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
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
    // Load .env from multiple locations: exe dir, project root, cwd
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    // project root = exe_dir/../../   (target/debug/ → forge/)
    let project_root = exe_dir
        .as_ref()
        .and_then(|d| d.parent())
        .and_then(|d| d.parent())
        .map(|d| d.to_path_buf());
    let mut env_loaded = false;
    for dir in [std::env::current_dir().ok(), exe_dir, project_root]
        .into_iter()
        .flatten()
    {
        let env_path = dir.join(".env");
        if env_path.exists() && dotenvy::from_path(&env_path).is_ok() {
            env_loaded = true;
            break;
        }
    }
    if env_loaded {
        // Use eprintln since tracing isn't initialized yet
        eprintln!(
            ".env loaded: AI_MODEL={}, AION_PORT={}",
            std::env::var("AI_MODEL").unwrap_or_default(),
            std::env::var("AION_PORT").unwrap_or_default()
        );
    } else {
        eprintln!(".env not found, using existing environment");
    }

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
    aion_router::learner::init_learner(&paths.workspace_root);

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
    // /v1/health 保持公开；其余路由在 AION_API_TOKEN 设置后要求 Bearer token。
    let api_token = auth_token_from_env();
    if api_token.is_some() {
        info!("API authentication enabled (AION_API_TOKEN set)");
    } else {
        info!("API authentication disabled (AION_API_TOKEN not set)");
    }
    let app = build_app(state, api_token)
        // ── Middleware ──
        .layer(TraceLayer::new_for_http())
        .layer(cors);

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

/// 构建应用 Router（含全部 REST 与 WebSocket 路由）。
///
/// - `/v1/health` 保持公开（健康检查，供负载均衡器/探针使用）。
/// - 其余路由置于受保护子 Router 中：当 `api_token` 为 `Some` 时，每个请求
///   必须携带 `Authorization: Bearer <token>`，否则返回 `401 Unauthorized`。
/// - `api_token` 为 `None` 时不附加认证层，行为与未设置 `AION_API_TOKEN` 完全一致。
///
/// 该函数供 `main` 与集成测试复用；不含 TraceLayer / CORS（由调用方决定）。
pub fn build_app(state: Arc<AppState>, api_token: Option<String>) -> Router {
    let protected = Router::new()
        // ── Metrics ──
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
        .with_state(state);

    let protected = match api_token {
        Some(token) => protected.route_layer(middleware::from_fn(move |req, next| {
            auth_middleware(req, next, token.clone())
        })),
        None => protected,
    };

    Router::new()
        .route("/v1/health", get(handlers::health))
        .merge(protected)
}

/// 读取 `AION_API_TOKEN` 环境变量；未设置或为空白时返回 `None`（认证关闭）。
fn auth_token_from_env() -> Option<String> {
    std::env::var("AION_API_TOKEN")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Bearer token 认证中间件：校验 `Authorization: Bearer <token>`，失败返回 401。
///
/// 通过 `route_layer` 仅作用于受保护路由（不含 `/v1/health`）。
async fn auth_middleware(req: Request<axum::body::Body>, next: Next, expected: String) -> Response {
    let authorized = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| secure_eq(token.as_bytes(), expected.as_bytes()));

    if authorized {
        next.run(req).await
    } else {
        // 遵循 RFC 7235：返回 401 并附带认证质询头
        let mut resp = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        resp.headers_mut().insert(
            axum::http::header::WWW_AUTHENTICATE,
            axum::http::HeaderValue::from_static("Bearer"),
        );
        resp
    }
}

/// 恒定时间比较：长度不等直接失败，等长时按位累加 XOR，
/// 避免 token 比较上的时序侧信道。
fn secure_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
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

    base.allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
        ])
}
