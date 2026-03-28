//! WebSocket 实时事件推送
//!
//! `GET /v1/stream/{session_id}` 端点升级为 WebSocket 连接，
//! 按 session_id 过滤并推送 `ServerEvent` 事件。

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use tracing::{info, warn};

use crate::AppState;

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!("WebSocket connection requested for session: {}", session_id);
    ws.on_upgrade(move |socket| handle_ws(socket, session_id, state))
}

/// 处理 WebSocket 连接：订阅事件总线，过滤并转发匹配的事件
async fn handle_ws(mut socket: WebSocket, session_id: String, state: Arc<AppState>) {
    info!("WebSocket connected for session: {}", session_id);
    let mut rx = state.event_bus.subscribe();

    loop {
        tokio::select! {
            // 从事件总线接收事件
            event = rx.recv() => {
                match event {
                    Ok(evt) if evt.session_id() == session_id => {
                        match serde_json::to_string(&evt) {
                            Ok(json) => {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    break; // 客户端断开
                                }
                            }
                            Err(e) => {
                                warn!("Failed to serialize event: {}", e);
                            }
                        }
                    }
                    Ok(_) => {} // 其他 session 的事件，忽略
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WebSocket lagged {} events for session {}", n, session_id);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            // 监听客户端消息（主要处理关闭）
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // 忽略其他客户端消息
                }
            }
        }
    }

    info!("WebSocket disconnected for session: {}", session_id);
}

use tokio::sync::broadcast;
