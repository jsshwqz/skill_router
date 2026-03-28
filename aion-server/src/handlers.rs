//! REST API route handlers
//!
//! All handlers call async `SkillRouter` methods directly.
//! No `spawn_blocking` needed since the routing pipeline is fully async.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use aion_types::ai_native::AiNativePayload;

use crate::error::{ApiError, AppError};
use crate::AppState;

// ── Health & Info ────────────────────────────────────────────────────────────

/// `GET /v1/health`
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        service: "aion-server".to_string(),
    })
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub service: String,
}

// ── Capabilities ─────────────────────────────────────────────────────────────

/// `GET /v1/capabilities`
pub async fn list_capabilities(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let reg = state.router.registry();
    let defs: Vec<_> = reg.definitions().cloned().collect();
    let result = serde_json::to_value(&defs)
        .map_err(|e| anyhow::anyhow!("serialization error: {}", e))?;
    Ok(Json(result))
}

// ── Route (natural language) ─────────────────────────────────────────────────

/// `POST /v1/route`
///
/// Request body:
/// ```json
/// { "task": "summarize this text", "context": { ... } }
/// ```
pub async fn route_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RouteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    if req.task.trim().is_empty() {
        return Err(ApiError::bad_request("task field is required and cannot be empty"));
    }

    info!("API route request: task='{}'", req.task);

    let session_id = uuid::Uuid::new_v4().to_string();
    let start = std::time::Instant::now();

    // 发射 TaskStarted 事件
    state.event_bus.publish(crate::events::ServerEvent::TaskStarted {
        session_id: session_id.clone(),
        task: req.task.clone(),
        capability: None,
        timestamp: chrono::Utc::now().timestamp(),
    });

    match state.router.route_with_context(&req.task, req.context).await {
        Ok(route_result) => {
            // 发射 TaskCompleted 事件
            state.event_bus.publish(crate::events::ServerEvent::TaskCompleted {
                session_id: session_id.clone(),
                skill: route_result.skill.metadata.name.clone(),
                status: route_result.execution.status.clone(),
                duration_ms: start.elapsed().as_millis() as u64,
                timestamp: chrono::Utc::now().timestamp(),
            });

            let response = serde_json::json!({
                "status": "ok",
                "session_id": session_id,
                "capability": route_result.capability,
                "skill": route_result.skill.metadata.name,
                "execution": {
                    "status": route_result.execution.status,
                    "result": route_result.execution.result,
                    "error": route_result.execution.error,
                },
                "lifecycle": format!("{:?}", route_result.lifecycle),
            });
            Ok(Json(response))
        }
        Err(e) => {
            // 发射 TaskFailed 事件
            state.event_bus.publish(crate::events::ServerEvent::TaskFailed {
                session_id,
                error: e.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            });

            warn!("Route failed: {}", e);
            Err(ApiError::internal(format!("routing failed: {}", e)))
        }
    }
}

#[derive(Deserialize)]
pub struct RouteRequest {
    pub task: String,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

// ── Route Native (structured AI-to-AI) ───────────────────────────────────────

/// `POST /v1/route/native`
///
/// Accepts `AiNativePayload` directly — the primary Agent-to-Agent interface.
pub async fn route_native(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AiNativePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    info!("API native route: intent='{}', capability={:?}", payload.intent, payload.capability);

    // Check delegation depth
    let max_depth = aion_router::config::max_delegation_depth();
    if payload.delegation_depth_exceeded(max_depth) {
        return Err(ApiError::bad_request(format!(
            "delegation chain exceeds maximum depth of {}", max_depth
        )));
    }

    match state.router.route_native(payload).await {
        Ok(route_result) => {
            let response = serde_json::json!({
                "status": "ok",
                "capability": route_result.capability,
                "skill": route_result.skill.metadata.name,
                "execution": {
                    "status": route_result.execution.status,
                    "result": route_result.execution.result,
                    "error": route_result.execution.error,
                },
                "lifecycle": format!("{:?}", route_result.lifecycle),
            });
            Ok(Json(response))
        }
        Err(e) => {
            warn!("Native route failed: {}", e);
            Err(ApiError::internal(format!("native routing failed: {}", e)))
        }
    }
}

// ── Memory ───────────────────────────────────────────────────────────────────

/// `GET /v1/memory/recall?query=...&limit=10`
pub async fn memory_recall(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RecallParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let query = params.query.unwrap_or_default();
    let limit = params.limit.unwrap_or(10);

    if query.is_empty() {
        return Err(ApiError::bad_request("query parameter is required"));
    }

    info!("API memory recall: query='{}', limit={}", query, limit);

    // Memory operations are synchronous but fast (local file I/O)
    let memory = state.memory.clone();
    match memory.recall(&query, limit) {
        Ok(entries) => {
            let response = serde_json::json!({
                "status": "ok",
                "count": entries.len(),
                "entries": entries.iter().map(|e| serde_json::json!({
                    "id": e.id,
                    "category": format!("{:?}", e.category),
                    "content": e.content,
                    "importance": e.importance,
                    "timestamp": e.timestamp,
                })).collect::<Vec<_>>(),
            });
            Ok(Json(response))
        }
        Err(e) => Err(ApiError::internal(format!("recall failed: {}", e))),
    }
}

#[derive(Deserialize)]
pub struct RecallParams {
    pub query: Option<String>,
    pub limit: Option<usize>,
}

/// `POST /v1/memory/remember`
///
/// ```json
/// { "category": "Decision", "content": "We chose axum over actix", "session_id": "abc", "importance": 7 }
/// ```
pub async fn memory_remember(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RememberRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    if req.content.trim().is_empty() {
        return Err(ApiError::bad_request("content field is required"));
    }

    info!("API memory remember: category={}, importance={}", req.category, req.importance);

    let memory = state.memory.clone();
    let category = parse_memory_category(&req.category);
    let session_id = req.session_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    match memory.remember(category, &req.content, &session_id, req.importance) {
        Ok(entry_id) => {
            let response = serde_json::json!({
                "status": "ok",
                "entry_id": entry_id,
            });
            Ok(Json(response))
        }
        Err(e) => Err(ApiError::internal(format!("remember failed: {}", e))),
    }
}

#[derive(Deserialize)]
pub struct RememberRequest {
    pub category: String,
    pub content: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "default_importance")]
    pub importance: u8,
}

fn default_importance() -> u8 { 5 }

/// `GET /v1/memory/stats`
pub async fn memory_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let memory = state.memory.clone();
    let stats = memory.stats()?;
    Ok(Json(stats))
}

// ── Agent Info ───────────────────────────────────────────────────────────────

/// `GET /v1/agents`
pub async fn agents_info() -> Json<serde_json::Value> {
    let role = aion_router::config::node_role();
    let caps = aion_router::config::node_capabilities();
    let nats = aion_router::config::nats_url();

    Json(serde_json::json!({
        "node_role": role,
        "node_capabilities": caps,
        "nats_connected": nats.is_some(),
        "nats_url": nats,
        "message": "Full multi-agent orchestration available via CLI (aion-cli agent run) or NATS bus"
    }))
}

/// `POST /v1/agents/delegate`
pub async fn agent_delegate(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<AiNativePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let target = match &payload.target_agent_id {
        Some(id) => id.clone(),
        None => return Err(ApiError::bad_request("target_agent_id is required for delegation")),
    };

    info!("API delegate: intent='{}' -> agent '{}'", payload.intent, target);

    let from = payload.metadata.agent_id.clone();
    payload = payload.with_delegation_hop(&from, &target, "HTTP API delegation");

    let max_depth = aion_router::config::max_delegation_depth();
    if payload.delegation_depth_exceeded(max_depth) {
        return Err(ApiError::bad_request(format!(
            "delegation chain exceeds maximum depth of {}", max_depth
        )));
    }

    match state.router.route_native(payload).await {
        Ok(route_result) => {
            let response = serde_json::json!({
                "status": "ok",
                "delegated_to": target,
                "execution_mode": "local_mvp",
                "capability": route_result.capability,
                "skill": route_result.skill.metadata.name,
                "execution": {
                    "status": route_result.execution.status,
                    "result": route_result.execution.result,
                    "error": route_result.execution.error,
                },
            });
            Ok(Json(response))
        }
        Err(e) => {
            warn!("Delegation failed: {}", e);
            Err(ApiError::internal(format!("delegation failed: {}", e)))
        }
    }
}

// ── Metrics (placeholder) ────────────────────────────────────────────────────

/// `GET /v1/metrics`
///
/// 渲染真实 Prometheus 指标（skill_executions_total, skill_execution_duration_seconds 等）
pub async fn metrics(State(state): State<Arc<AppState>>) -> String {
    state.prometheus.render()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn parse_memory_category(s: &str) -> aion_memory::memory::MemoryCategory {
    use aion_memory::memory::MemoryCategory;
    match s.to_lowercase().as_str() {
        "decision"      => MemoryCategory::Decision,
        "lesson"        => MemoryCategory::Lesson,
        "error"         => MemoryCategory::Error,
        "preference"    => MemoryCategory::Preference,
        "architecture"  => MemoryCategory::Architecture,
        "taskprogress" | "task_progress" => MemoryCategory::TaskProgress,
        _ => MemoryCategory::Lesson,
    }
}
