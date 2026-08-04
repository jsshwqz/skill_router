//! Integration tests for aion-server REST API
//!
//! Uses `tower::ServiceExt` to drive the axum `Router` as an in-process service
//! without binding to a TCP port.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt; // for `oneshot`

use aion_memory::memory::MemoryManager;
use aion_router::SkillRouter;
use aion_types::types::RouterPaths;

use crate::AppState;
use crate::build_app;
use crate::events::EventBus;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build an `AppState` backed by a temporary directory.
fn test_state(tmp: &Path) -> Arc<AppState> {
    let paths = RouterPaths::for_workspace(tmp);
    let router = SkillRouter::new(paths.clone()).expect("SkillRouter::new");
    let memory = MemoryManager::new(tmp);
    // Use `build()` instead of `install_recorder()` to avoid global side-effects.
    let prometheus = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();

    Arc::new(AppState {
        router: Arc::new(router),
        memory: Arc::new(memory),
        paths,
        prometheus,
        event_bus: Arc::new(EventBus::new(64)),
    })
}

/// Build the axum Router with all routes and the given state (authentication disabled).
fn test_app(state: Arc<AppState>) -> Router {
    build_app(state, None)
}

/// Send a request and return (StatusCode, parsed JSON body).
async fn json_response(app: Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/health").body(Body::empty()).unwrap();

    let (status, body) = json_response(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "aion-server");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn capabilities_returns_array() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/capabilities").body(Body::empty()).unwrap();

    let (status, body) = json_response(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array(), "capabilities should return a JSON array");
}

#[tokio::test]
async fn metrics_returns_text() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/metrics").body(Body::empty()).unwrap();

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    // Metrics returns plain text, not JSON
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    // A fresh recorder returns an empty or minimal string — just check it doesn't error
    // Metrics endpoint responds successfully (content may be empty for fresh recorder)
    let _ = text;
}

#[tokio::test]
async fn route_task_empty_task_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::post("/v1/route")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"task": ""}"#))
        .unwrap();

    let (status, body) = json_response(app, req).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("empty"));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn route_task_whitespace_only_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::post("/v1/route")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"task": "   "}"#))
        .unwrap();

    let (status, _body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn memory_recall_empty_query_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/memory/recall?query=").body(Body::empty()).unwrap();

    let (status, body) = json_response(app, req).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("query"));
}

#[tokio::test]
async fn memory_recall_no_query_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/memory/recall").body(Body::empty()).unwrap();

    let (status, body) = json_response(app, req).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("query"));
}

#[tokio::test]
async fn memory_remember_then_recall() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());

    // Remember
    {
        let app = test_app(state.clone());
        let req = Request::post("/v1/memory/remember")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"category": "Decision", "content": "chose axum over actix", "importance": 8}"#,
            ))
            .unwrap();

        let (status, body) = json_response(app, req).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert!(body["entry_id"].is_string());
    }

    // Recall
    {
        let app = test_app(state.clone());
        let req = Request::get("/v1/memory/recall?query=axum&limit=5")
            .body(Body::empty())
            .unwrap();

        let (status, body) = json_response(app, req).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert!(body["count"].as_u64().unwrap() >= 1);
        let entries = body["entries"].as_array().unwrap();
        assert!(!entries.is_empty());
        assert!(entries[0]["content"].as_str().unwrap().contains("axum"));
    }
}

#[tokio::test]
async fn memory_remember_empty_content_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::post("/v1/memory/remember")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"category": "Lesson", "content": ""}"#))
        .unwrap();

    let (status, body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("content"));
}

#[tokio::test]
async fn memory_stats_returns_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/memory/stats").body(Body::empty()).unwrap();

    let (status, body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object());
}

#[tokio::test]
async fn agents_info_returns_node_info() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/agents").body(Body::empty()).unwrap();

    let (status, body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["node_role"].is_string());
    assert!(body["node_capabilities"].is_array());
}

#[tokio::test]
async fn agent_delegate_missing_target_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    // AiNativePayload without target_agent_id
    let payload = serde_json::json!({
        "intent": "test_intent",
        "parameters": {},
        "priority": "normal",
        "metadata": {
            "agent_id": "test-agent",
            "session_id": "test-session",
            "timestamp": 0,
            "backend": "ollama",
            "model": ""
        },
        "delegation_chain": []
    });

    let req = Request::post("/v1/agents/delegate")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let (status, body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("target_agent_id"));
}

#[tokio::test]
async fn event_bus_publish_subscribe() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    bus.publish(crate::events::ServerEvent::TaskStarted {
        session_id: "s1".to_string(),
        task: "hello".to_string(),
        capability: None,
        timestamp: 100,
    });

    let evt = rx.recv().await.unwrap();
    assert_eq!(evt.session_id(), "s1");
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::get("/v1/nonexistent").body(Body::empty()).unwrap();

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn route_task_missing_body_returns_4xx() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    let req = Request::post("/v1/route")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let resp = app.oneshot(req).await.expect("oneshot");
    // Missing `task` field → axum deserialization error (422) or our validation (400)
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn memory_remember_default_importance() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state.clone());

    // Omit importance — should default to 5
    let req = Request::post("/v1/memory/remember")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"category": "Lesson", "content": "always test defaults"}"#,
        ))
        .unwrap();

    let (status, body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    // Verify the entry was stored with importance 5
    let app2 = test_app(state);
    let req2 = Request::get("/v1/memory/recall?query=defaults")
        .body(Body::empty())
        .unwrap();
    let (_status, body2) = json_response(app2, req2).await;
    let entries = body2["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert_eq!(entries[0]["importance"], 5);
}

// ── Authentication (Bearer token) ───────────────────────────────────────────

#[tokio::test]
async fn auth_requires_token_on_protected_routes() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = build_app(state, Some("secret-token".to_string()));

    // 无 Authorization 头 → 401
    let req = Request::get("/v1/capabilities").body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 错误 token → 401
    let req = Request::get("/v1/capabilities")
        .header("authorization", "Bearer wrong-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 正确 token → 200
    let req = Request::get("/v1/capabilities")
        .header("authorization", "Bearer secret-token")
        .body(Body::empty())
        .unwrap();
    let (status, _body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn auth_health_stays_public() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = build_app(state, Some("secret-token".to_string()));

    // 即使配置了 token，/v1/health 仍可匿名访问
    let req = Request::get("/v1/health").body(Body::empty()).unwrap();
    let (status, body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn no_auth_when_token_unset() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(tmp.path());
    let app = test_app(state);

    // AION_API_TOKEN 未设置（None）时，路由无需认证即可访问
    let req = Request::get("/v1/capabilities").body(Body::empty()).unwrap();
    let (status, _body) = json_response(app, req).await;
    assert_eq!(status, StatusCode::OK);
}
