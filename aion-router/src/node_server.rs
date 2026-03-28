//! 节点 HTTP 控制平面（D1 阶段）
//!
//! 每个分布式节点运行此 HTTP 服务，提供：
//! - `GET /health` — 健康检查
//! - `GET /capabilities` — 该节点的能力列表
//! - `GET /agents` — 该节点的活跃 Agent 列表
//! - `POST /execute` — 直接在该节点执行任务
//!
//! 仅在 `distributed` feature 启用时编译。

#[cfg(feature = "distributed")]
pub mod server {
    use std::sync::Arc;

    use axum::extract::State;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use serde::{Deserialize, Serialize};
    use tracing::info;

    use crate::config;
    use crate::SkillRouter;

    /// 节点服务状态
    pub struct NodeState {
        pub router: Arc<SkillRouter>,
        pub node_id: String,
        pub role: String,
        pub capabilities: Vec<String>,
    }

    /// 启动节点 HTTP 控制平面
    pub async fn start_node_server(state: Arc<NodeState>, port: u16) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/health", get(health))
            .route("/capabilities", get(capabilities))
            .route("/agents", get(agents))
            .route("/execute", post(execute))
            .with_state(state);

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        info!("Node HTTP control plane listening on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    /// `GET /health`
    async fn health(
        State(state): State<Arc<NodeState>>,
    ) -> Json<HealthResponse> {
        Json(HealthResponse {
            status: "ok".to_string(),
            node_id: state.node_id.clone(),
            role: state.role.clone(),
            capabilities_count: state.capabilities.len(),
        })
    }

    #[derive(Serialize)]
    struct HealthResponse {
        status: String,
        node_id: String,
        role: String,
        capabilities_count: usize,
    }

    /// `GET /capabilities`
    async fn capabilities(
        State(state): State<Arc<NodeState>>,
    ) -> Json<serde_json::Value> {
        let reg = state.router.registry();
        let defs: Vec<_> = reg.definitions().cloned().collect();
        Json(serde_json::json!({
            "node_id": state.node_id,
            "role": state.role,
            "capabilities": state.capabilities,
            "registered_definitions": defs.len(),
        }))
    }

    /// `GET /agents`
    async fn agents(
        State(state): State<Arc<NodeState>>,
    ) -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "node_id": state.node_id,
            "role": state.role,
            "nats_url": config::nats_url(),
        }))
    }

    /// `POST /execute`
    async fn execute(
        State(state): State<Arc<NodeState>>,
        Json(req): Json<ExecuteRequest>,
    ) -> Json<serde_json::Value> {
        match state.router.route_with_capability(&req.task, &req.capability, req.context).await {
            Ok(result) => Json(serde_json::json!({
                "status": "ok",
                "node_id": state.node_id,
                "skill": result.skill.metadata.name,
                "result": result.execution.result,
            })),
            Err(e) => Json(serde_json::json!({
                "status": "error",
                "node_id": state.node_id,
                "error": e.to_string(),
            })),
        }
    }

    #[derive(Deserialize)]
    struct ExecuteRequest {
        task: String,
        capability: String,
        #[serde(default)]
        context: Option<serde_json::Value>,
    }
}
