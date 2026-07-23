//! Stateful ACP server for the Aion Forge agent.

use std::sync::{atomic::Ordering, Arc};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, Implementation, InitializeRequest, InitializeResponse, NewSessionRequest,
    NewSessionResponse, PromptRequest, PromptResponse, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
    StopReason,
};
use agent_client_protocol::{Agent, Client, ConnectionTo, Dispatch, Stdio, UntypedMessage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{
    agent_loop::{AgentLoop, SessionEventSink, TurnRequest},
    catalog::CapabilityCatalog,
    executor::ForgeToolExecutor,
    model_catalog::ModelCatalog,
    planner::{AiTaskPlanner, BuiltinAiExecutor},
    session::{HistoryEntry, PromptDisposition, SessionStore},
};

const MODEL_CONFIG_ID: &str = "model";
const MAX_TOOL_CALLS: usize = 6;

/// Stateful Forge runtime shared by all requests on one ACP process.
struct ForgeAcpAgent {
    sessions: SessionStore,
    models: ModelCatalog,
    capabilities: CapabilityCatalog,
    agent_loop: AgentLoop,
}

enum RequestFailure {
    Invalid(anyhow::Error),
    Internal(anyhow::Error),
    MethodNotFound(String),
}

impl RequestFailure {
    fn invalid(error: anyhow::Error) -> Self {
        Self::Invalid(error)
    }

    fn internal(error: anyhow::Error) -> Self {
        Self::Internal(error)
    }

    fn into_protocol_error(self) -> agent_client_protocol::Error {
        match self {
            Self::Invalid(error) => agent_client_protocol::Error::new(-32602, error.to_string()),
            Self::Internal(error) => agent_client_protocol::Error::new(-32603, error.to_string()),
            Self::MethodNotFound(method) => {
                agent_client_protocol::Error::new(-32601, format!("method not found: {method}"))
            }
        }
    }
}

impl ForgeAcpAgent {
    fn from_environment() -> Self {
        let models = ModelCatalog::from_environment();
        let capabilities = CapabilityCatalog::live();
        let planner = AiTaskPlanner::new(models.clone(), Arc::new(BuiltinAiExecutor));
        Self {
            sessions: SessionStore::default(),
            models,
            capabilities,
            agent_loop: AgentLoop::new(planner, ForgeToolExecutor::default(), MAX_TOOL_CALLS),
        }
    }

    async fn handle_request(
        &self,
        method: &str,
        params: Value,
        connection: &ConnectionTo<Client>,
    ) -> std::result::Result<Value, RequestFailure> {
        match method {
            "session/new" => self.new_session(params).await,
            "session/set_config_option" => self.set_config_option(params).await,
            "session/prompt" => self.prompt(params, connection).await,
            "session/set_model" | "session/select_model" => self.set_legacy_model(params).await,
            "shutdown" | "exit" => Ok(Value::Null),
            other => Err(RequestFailure::MethodNotFound(other.to_string())),
        }
    }

    async fn handle_notification(&self, method: &str, params: Value) -> Result<()> {
        match method {
            "session/cancel" => {
                let notification: CancelNotification =
                    serde_json::from_value(params).context("invalid session/cancel parameters")?;
                self.sessions.cancel(&notification.session_id.to_string()).await
            }
            "notifications/initialized" | "initialized" | "exit" => Ok(()),
            other => {
                tracing::debug!(method = other, "ignoring unsupported ACP notification");
                Ok(())
            }
        }
    }

    async fn new_session(&self, params: Value) -> std::result::Result<Value, RequestFailure> {
        let requested_model = requested_model(&params).map(str::to_string);
        let request: NewSessionRequest = serde_json::from_value(params)
            .context("invalid session/new parameters")
            .map_err(RequestFailure::invalid)?;
        let selected_model = match requested_model {
            Some(model) => {
                self.models.resolve(&model).map_err(RequestFailure::invalid)?;
                model
            }
            None => self.models.default_model().to_string(),
        };
        let session_id = self
            .sessions
            .create(request.cwd, selected_model.clone())
            .await
            .map_err(RequestFailure::invalid)?;
        let response = NewSessionResponse::new(session_id)
            .config_options(vec![self.models.session_config_option_for(&selected_model)]);
        serde_json::to_value(response)
            .context("serialize session/new response")
            .map_err(RequestFailure::internal)
    }

    async fn set_config_option(&self, params: Value) -> std::result::Result<Value, RequestFailure> {
        let request: SetSessionConfigOptionRequest = serde_json::from_value(params)
            .context("invalid session/set_config_option parameters")
            .map_err(RequestFailure::invalid)?;
        if request.config_id.to_string() != MODEL_CONFIG_ID {
            return Err(RequestFailure::invalid(anyhow::anyhow!(
                "unsupported session config option '{}'; expected 'model'",
                request.config_id
            )));
        }
        let selected_model = request
            .value
            .as_value_id()
            .map(ToString::to_string)
            .ok_or_else(|| anyhow::anyhow!("model config option requires a string value"))
            .map_err(RequestFailure::invalid)?;
        self.models.resolve(&selected_model).map_err(RequestFailure::invalid)?;
        self.sessions
            .set_model(&request.session_id.to_string(), selected_model.clone())
            .await
            .map_err(RequestFailure::invalid)?;
        serde_json::to_value(SetSessionConfigOptionResponse::new(vec![self
            .models
            .session_config_option_for(&selected_model)]))
        .context("serialize session/set_config_option response")
        .map_err(RequestFailure::internal)
    }

    async fn set_legacy_model(&self, params: Value) -> std::result::Result<Value, RequestFailure> {
        let session_id = session_id(&params).map_err(RequestFailure::invalid)?;
        let selected_model = requested_model(&params)
            .ok_or_else(|| anyhow::anyhow!("legacy model selection requires model or modelId"))
            .map_err(RequestFailure::invalid)?;
        self.models.resolve(selected_model).map_err(RequestFailure::invalid)?;
        self.sessions
            .set_model(session_id, selected_model.to_string())
            .await
            .map_err(RequestFailure::invalid)?;
        Ok(json!({
            "configOptions": [self.models.session_config_option_for(selected_model)]
        }))
    }

    async fn prompt(
        &self,
        params: Value,
        connection: &ConnectionTo<Client>,
    ) -> std::result::Result<Value, RequestFailure> {
        let requested = requested_model(&params).map(str::to_string);
        let request: PromptRequest = serde_json::from_value(params.clone())
            .context("invalid session/prompt parameters")
            .map_err(RequestFailure::invalid)?;
        let session_id = request.session_id.to_string();
        self.sessions
            .snapshot(&session_id)
            .await
            .map_err(RequestFailure::invalid)?;
        let sink = AcpEventSink {
            connection,
            session_id: &session_id,
        };

        if let Some(model) = requested {
            if let Err(error) = self.models.resolve(&model) {
                let message = error.to_string();
                sink.message_chunk(&message).await.map_err(RequestFailure::internal)?;
                self.sessions
                    .append_history(&session_id, HistoryEntry::Assistant(message))
                    .await
                    .map_err(RequestFailure::internal)?;
                return prompt_response(StopReason::EndTurn).map_err(RequestFailure::internal);
            }
            self.sessions
                .set_model(&session_id, model)
                .await
                .map_err(RequestFailure::internal)?;
        }

        let text = prompt_text(&params);
        if text.trim().is_empty() {
            let message = "请求内容为空。".to_string();
            sink.message_chunk(&message).await.map_err(RequestFailure::internal)?;
            self.sessions
                .append_history(&session_id, HistoryEntry::Assistant(message))
                .await
                .map_err(RequestFailure::internal)?;
            return prompt_response(StopReason::EndTurn).map_err(RequestFailure::internal);
        }

        if self
            .sessions
            .ingest_prompt(&session_id, &text)
            .await
            .map_err(RequestFailure::internal)?
            == PromptDisposition::BootstrapStored
        {
            return prompt_response(StopReason::EndTurn).map_err(RequestFailure::internal);
        }

        let cancellation = self
            .sessions
            .start_prompt(&session_id)
            .await
            .map_err(RequestFailure::internal)?;
        let snapshot = self
            .sessions
            .snapshot(&session_id)
            .await
            .map_err(RequestFailure::internal)?;
        let original_history_len = snapshot.history.len();
        let outcome = self
            .agent_loop
            .run(
                TurnRequest {
                    selected_model: snapshot.selected_model,
                    cwd: snapshot.cwd,
                    instructions: snapshot.instructions,
                    history: snapshot.history,
                    capabilities: self.capabilities.entries().to_vec(),
                    cancellation: Arc::clone(&cancellation),
                },
                &sink,
            )
            .await;

        match outcome {
            Ok(outcome) => {
                for entry in outcome.history.into_iter().skip(original_history_len) {
                    self.sessions
                        .append_history(&session_id, entry)
                        .await
                        .map_err(RequestFailure::internal)?;
                }
            }
            Err(error) => {
                let message = format!("Aion Forge 执行失败：{error}");
                sink.message_chunk(&message).await.map_err(RequestFailure::internal)?;
                self.sessions
                    .append_history(&session_id, HistoryEntry::Assistant(message))
                    .await
                    .map_err(RequestFailure::internal)?;
            }
        }

        prompt_response(if cancellation.load(Ordering::SeqCst) {
            StopReason::Cancelled
        } else {
            StopReason::EndTurn
        })
        .map_err(RequestFailure::internal)
    }
}

struct AcpEventSink<'a> {
    connection: &'a ConnectionTo<Client>,
    session_id: &'a str,
}

#[async_trait]
impl SessionEventSink for AcpEventSink<'_> {
    async fn tool_started(&self, call_id: &str, name: &str, arguments: &Value) -> Result<()> {
        self.send_update(json!({
            "sessionUpdate": "tool_call",
            "toolCallId": call_id,
            "title": name,
            "status": "in_progress",
            "rawInput": arguments,
        }))
    }

    async fn tool_finished(&self, call_id: &str, result: &Result<Value>) -> Result<()> {
        let update = match result {
            Ok(value) => json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": call_id,
                "status": "completed",
                "rawOutput": value,
            }),
            Err(error) => json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": call_id,
                "status": "failed",
                "rawOutput": {"error": error.to_string()},
            }),
        };
        self.send_update(update)
    }

    async fn message_chunk(&self, text: &str) -> Result<()> {
        self.send_update(json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": text},
        }))
    }
}

impl AcpEventSink<'_> {
    fn send_update(&self, update: Value) -> Result<()> {
        let params = json!({"sessionId": self.session_id, "update": update});
        self.connection
            .send_notification(UntypedMessage::new("session/update", params)?)
            .context("send ACP session/update notification")?;
        Ok(())
    }
}

/// Run the standard ACP JSON-RPC server over stdin and stdout.
pub async fn run_acp_server() -> Result<()> {
    let forge = Arc::new(ForgeAcpAgent::from_environment());
    let dispatch_forge = Arc::clone(&forge);

    Agent
        .builder()
        .name("aion-forge")
        .on_receive_request(
            async move |initialize: InitializeRequest, responder, _connection| {
                responder.respond(
                    InitializeResponse::new(initialize.protocol_version)
                        .agent_capabilities(AgentCapabilities::new())
                        .agent_info(Implementation::new("aion-forge", env!("CARGO_PKG_VERSION")).title("Aion Forge")),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_dispatch(
            async move |message: Dispatch, connection: ConnectionTo<Client>| {
                dispatch_message(Arc::clone(&dispatch_forge), message, connection).await
            },
            agent_client_protocol::on_receive_dispatch!(),
        )
        .connect_to(Stdio::new().with_debug(|line, direction| {
            tracing::trace!(?direction, bytes = line.len(), "ACP transport line");
        }))
        .await
        .map_err(anyhow::Error::new)
}

async fn dispatch_message(
    forge: Arc<ForgeAcpAgent>,
    message: Dispatch,
    connection: ConnectionTo<Client>,
) -> agent_client_protocol::Result<()> {
    match message {
        Dispatch::Request(message, responder) => {
            match forge.handle_request(&message.method, message.params, &connection).await {
                Ok(result) => responder.respond(result),
                Err(error) => responder.respond_with_error(error.into_protocol_error()),
            }
        }
        Dispatch::Notification(message) => {
            if let Err(error) = forge.handle_notification(&message.method, message.params).await {
                tracing::warn!(%error, method = %message.method, "ACP notification failed");
            }
            Ok(())
        }
        Dispatch::Response(_, _) => Ok(()),
    }
}

fn prompt_text(params: &Value) -> String {
    if let Some(message) = params.get("message").and_then(Value::as_str) {
        return message.to_string();
    }

    params
        .get("prompt")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn requested_model(params: &Value) -> Option<&str> {
    params
        .get("model")
        .or_else(|| params.get("modelId"))
        .or_else(|| params.pointer("/_meta/model"))
        .or_else(|| params.pointer("/_meta/modelId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
}

fn session_id(params: &Value) -> Result<&str> {
    params
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("request requires sessionId"))
}

fn prompt_response(stop_reason: StopReason) -> Result<Value> {
    serde_json::to_value(PromptResponse::new(stop_reason)).context("serialize session/prompt response")
}

#[cfg(test)]
mod tests {
    use super::{prompt_text, requested_model};
    use serde_json::json;

    #[test]
    fn reads_standard_acp_prompt_content_blocks() {
        let params = json!({
            "prompt": [
                {"type": "text", "text": "第一段"},
                {"type": "image", "data": "ignored"},
                {"type": "text", "text": "第二段"}
            ]
        });

        assert_eq!(prompt_text(&params), "第一段\n第二段");
    }

    #[test]
    fn reads_model_compatibility_fields() {
        assert_eq!(requested_model(&json!({"modelId": "exact"})), Some("exact"));
        assert_eq!(
            requested_model(&json!({"_meta": {"model": "meta-model"}})),
            Some("meta-model")
        );
    }
}
