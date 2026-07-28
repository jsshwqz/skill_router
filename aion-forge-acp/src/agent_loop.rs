use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    catalog::CapabilityEntry,
    executor::ToolExecutor,
    planner::{Planner, PlannerAction, PlannerRequest, FORGE_IDENTITY},
    session::HistoryEntry,
};

const MAX_OBSERVATION_BYTES: usize = 32 * 1024;
const TRUNCATION_SUFFIX: &str = "...[truncated]";

/// Immutable inputs for one bounded ACP agent turn.
pub struct TurnRequest {
    /// Persisted ACP model selection.
    pub selected_model: String,
    /// Validated session working directory.
    pub cwd: PathBuf,
    /// Explicit bootstrap instructions accumulated for the session.
    pub instructions: Vec<String>,
    /// Conversation history including the current user turn.
    pub history: Vec<HistoryEntry>,
    /// Exact live capabilities available to the planner.
    pub capabilities: Vec<CapabilityEntry>,
    /// Cooperative cancellation flag for this turn.
    pub cancellation: Arc<AtomicBool>,
}

/// Completed visible response and updated bounded history for one turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    /// Non-empty response emitted to the ACP client.
    pub message: String,
    /// History including tool observations and the assistant response.
    pub history: Vec<HistoryEntry>,
}

/// Event boundary used by ACP transport adapters to stream progress.
#[async_trait]
pub trait SessionEventSink: Send + Sync {
    /// Report that one Forge capability call is starting.
    async fn tool_started(&self, call_id: &str, name: &str, arguments: &Value) -> Result<()>;

    /// Report the terminal result of one Forge capability call.
    async fn tool_finished(&self, call_id: &str, result: &Result<Value>) -> Result<()>;

    /// Emit visible assistant content for the completed turn.
    async fn message_chunk(&self, text: &str) -> Result<()>;
}

/// Stateful, bounded planner/executor loop for one ACP user turn.
pub struct AgentLoop {
    planner: Arc<dyn Planner>,
    executor: Arc<dyn ToolExecutor>,
    max_tool_calls: usize,
}

impl AgentLoop {
    /// Construct a loop with an explicit tool-call limit.
    pub fn new<P, E>(planner: P, executor: E, max_tool_calls: usize) -> Self
    where
        P: Planner + 'static,
        E: ToolExecutor + 'static,
    {
        Self {
            planner: Arc::new(planner),
            executor: Arc::new(executor),
            max_tool_calls,
        }
    }

    /// Run until the planner returns a final answer or a safety bound terminates the turn.
    pub async fn run(&self, request: TurnRequest, sink: &dyn SessionEventSink) -> Result<TurnOutcome> {
        let required_tool = explicit_required_tool(&request.history, &request.capabilities);
        let mut history = request.history;
        let mut repair_error = None;
        let mut repair_used = false;
        let mut tool_contract_repair_used = false;
        let mut required_tool_attempted = false;
        let mut tool_calls = 0usize;
        let mut failed_calls = HashSet::new();

        loop {
            if is_cancelled(&request.cancellation) {
                return finish(sink, history, "请求已取消。".to_string()).await;
            }

            let action = self
                .planner
                .next_action(PlannerRequest {
                    identity: FORGE_IDENTITY.to_string(),
                    selected_model: request.selected_model.clone(),
                    cwd: request.cwd.clone(),
                    instructions: request.instructions.clone(),
                    history: history.clone(),
                    capabilities: request.capabilities.clone(),
                    required_tool: required_tool.clone().filter(|_| !required_tool_attempted),
                    repair_error: repair_error.take(),
                })
                .await;

            let action = match action {
                Ok(action) => action,
                Err(error) if !repair_used => {
                    repair_used = true;
                    repair_error = Some(error.to_string());
                    continue;
                }
                Err(error) => {
                    return finish(sink, history, format!("规划失败：{error}")).await;
                }
            };

            match action {
                PlannerAction::Final { message } => {
                    if let Some(tool) = required_tool.as_ref().filter(|_| !required_tool_attempted) {
                        if !tool_contract_repair_used {
                            tool_contract_repair_used = true;
                            repair_error = Some(format!(
                                "用户明确要求调用工具 {tool}，但上一动作未实际调用该工具。下一动作必须是 call_tool。"
                            ));
                            continue;
                        }
                        return finish(sink, history, format!("规划失败：未按用户要求调用工具 {tool}。")).await;
                    }
                    if is_cancelled(&request.cancellation) {
                        return finish(sink, history, "请求已取消。".to_string()).await;
                    }
                    return finish(sink, history, message).await;
                }
                PlannerAction::CallTool { tool, arguments } => {
                    if tool_calls >= self.max_tool_calls {
                        return finish(
                            sink,
                            history,
                            format!("已达到工具调用上限（{} 次）。", self.max_tool_calls),
                        )
                        .await;
                    }
                    if is_cancelled(&request.cancellation) {
                        return finish(sink, history, "请求已取消。".to_string()).await;
                    }

                    let call_id = Uuid::new_v4().to_string();
                    sink.tool_started(&call_id, &tool, &arguments).await?;
                    if required_tool.as_deref() == Some(tool.as_str()) {
                        required_tool_attempted = true;
                    }
                    tool_calls += 1;

                    if is_cancelled(&request.cancellation) {
                        let cancelled = Err(anyhow!("request cancelled before tool execution"));
                        sink.tool_finished(&call_id, &cancelled).await?;
                        return finish(sink, history, "请求已取消。".to_string()).await;
                    }

                    let result = self.executor.execute(&tool, arguments.clone(), &request.cwd).await;

                    if is_cancelled(&request.cancellation) {
                        let cancelled = Err(anyhow!("request cancelled during tool execution"));
                        sink.tool_finished(&call_id, &cancelled).await?;
                        return finish(sink, history, "请求已取消。".to_string()).await;
                    }

                    sink.tool_finished(&call_id, &result).await?;
                    match result {
                        Ok(value) => {
                            history.push(HistoryEntry::Tool {
                                name: tool,
                                observation: normalize_observation(value.to_string()),
                            });
                        }
                        Err(error) => {
                            let failed_call = json!({
                                "tool": tool,
                                "arguments": arguments,
                            })
                            .to_string();
                            if !failed_calls.insert(failed_call) {
                                return finish(sink, history, format!("检测到重复失败的工具调用：{tool}。")).await;
                            }
                            history.push(HistoryEntry::Tool {
                                name: tool,
                                observation: normalize_observation(format!("error: {error}")),
                            });
                        }
                    }
                }
            }
        }
    }
}

fn explicit_required_tool(history: &[HistoryEntry], capabilities: &[CapabilityEntry]) -> Option<String> {
    let user = history.iter().rev().find_map(|entry| match entry {
        HistoryEntry::User(message) => Some(message.to_ascii_lowercase()),
        _ => None,
    })?;
    let prefixes = [
        "使用 ", "使用", "调用 ", "调用", "执行 ", "执行", "use ", "call ", "run ",
    ];

    capabilities
        .iter()
        .filter(|capability| capability.planner_callable)
        .find(|capability| {
            let name = capability.name.to_ascii_lowercase();
            prefixes.iter().any(|prefix| user.contains(&format!("{prefix}{name}")))
        })
        .map(|capability| capability.name.clone())
}

fn is_cancelled(cancellation: &AtomicBool) -> bool {
    cancellation.load(Ordering::SeqCst)
}

async fn finish(sink: &dyn SessionEventSink, mut history: Vec<HistoryEntry>, message: String) -> Result<TurnOutcome> {
    let message = if message.trim().is_empty() {
        "unknown".to_string()
    } else {
        message
    };
    sink.message_chunk(&message).await?;
    history.push(HistoryEntry::Assistant(message.clone()));
    Ok(TurnOutcome { message, history })
}

fn normalize_observation(mut observation: String) -> String {
    if observation.len() <= MAX_OBSERVATION_BYTES {
        return observation;
    }

    let mut end = MAX_OBSERVATION_BYTES.saturating_sub(TRUNCATION_SUFFIX.len());
    while !observation.is_char_boundary(end) {
        end -= 1;
    }
    observation.truncate(end);
    observation.push_str(TRUNCATION_SUFFIX);
    observation
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
    };

    use anyhow::{anyhow, Result};
    use serde_json::{json, Value};

    use crate::{
        catalog::CapabilityEntry,
        executor::ToolExecutor,
        planner::{Planner, PlannerAction, PlannerRequest},
        session::HistoryEntry,
    };

    use super::{AgentLoop, SessionEventSink, TurnRequest};

    #[derive(Clone)]
    struct ScriptedPlanner {
        actions: Arc<Mutex<VecDeque<Result<PlannerAction>>>>,
        requests: Arc<Mutex<Vec<PlannerRequest>>>,
    }

    impl ScriptedPlanner {
        fn new(actions: Vec<Result<PlannerAction>>) -> Self {
            Self {
                actions: Arc::new(Mutex::new(actions.into())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl Planner for ScriptedPlanner {
        async fn next_action(&self, request: PlannerRequest) -> Result<PlannerAction> {
            self.requests.lock().unwrap().push(request);
            self.actions
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(anyhow!("script exhausted")))
        }
    }

    struct RecordingExecutor {
        results: Mutex<VecDeque<Result<Value>>>,
        calls: Mutex<Vec<(String, Value)>>,
        cancel_during_execute: Option<Arc<AtomicBool>>,
    }

    impl RecordingExecutor {
        fn with_results(results: Vec<Result<Value>>) -> Self {
            Self {
                results: Mutex::new(results.into()),
                calls: Mutex::new(Vec::new()),
                cancel_during_execute: None,
            }
        }
    }

    #[async_trait::async_trait]
    impl ToolExecutor for RecordingExecutor {
        async fn execute(&self, name: &str, arguments: Value, _cwd: &Path) -> Result<Value> {
            self.calls.lock().unwrap().push((name.to_string(), arguments));
            if let Some(cancellation) = &self.cancel_during_execute {
                cancellation.store(true, Ordering::SeqCst);
            }
            self.results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(anyhow!("result script exhausted")))
        }
    }

    #[derive(Default)]
    struct RecordingEventSink {
        events: Mutex<Vec<String>>,
        messages: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl SessionEventSink for RecordingEventSink {
        async fn tool_started(&self, call_id: &str, name: &str, _arguments: &Value) -> Result<()> {
            self.events.lock().unwrap().push(format!("start:{call_id}:{name}"));
            Ok(())
        }

        async fn tool_finished(&self, call_id: &str, result: &Result<Value>) -> Result<()> {
            self.events.lock().unwrap().push(format!(
                "finish:{call_id}:{}",
                if result.is_ok() { "ok" } else { "error" }
            ));
            Ok(())
        }

        async fn message_chunk(&self, text: &str) -> Result<()> {
            self.messages.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    fn call_tool() -> PlannerAction {
        PlannerAction::CallTool {
            tool: "json_parse".to_string(),
            arguments: json!({"text": "{\"ok\":true}"}),
        }
    }

    fn turn_request(cancellation: Arc<AtomicBool>) -> TurnRequest {
        TurnRequest {
            selected_model: "auto".to_string(),
            cwd: PathBuf::from("D:/test/aionui/forge"),
            instructions: Vec::new(),
            history: vec![HistoryEntry::User("parse it".to_string())],
            capabilities: vec![CapabilityEntry {
                name: "json_parse".to_string(),
                description: "Parse JSON".to_string(),
                parameters_schema: json!({"type": "object"}),
                requires_approval: false,
                planner_callable: true,
            }],
            cancellation,
        }
    }

    #[tokio::test]
    async fn final_only_response_is_visible_and_recorded() {
        let planner = ScriptedPlanner::new(vec![Ok(PlannerAction::Final {
            message: "visible".to_string(),
        })]);
        let executor = RecordingExecutor::with_results(Vec::new());
        let sink = RecordingEventSink::default();

        let outcome = AgentLoop::new(planner, executor, 6)
            .run(turn_request(Arc::new(AtomicBool::new(false))), &sink)
            .await
            .unwrap();

        assert_eq!(outcome.message, "visible");
        assert_eq!(sink.messages.lock().unwrap().as_slice(), &["visible"]);
        assert_eq!(
            outcome.history.last(),
            Some(&HistoryEntry::Assistant("visible".to_string()))
        );
    }

    #[tokio::test]
    async fn tool_result_is_consumed_before_final_response() {
        let planner = ScriptedPlanner::new(vec![
            Ok(call_tool()),
            Ok(PlannerAction::Final {
                message: "解析结果是 ok=true。".to_string(),
            }),
        ]);
        let executor = RecordingExecutor::with_results(vec![Ok(json!({"ok": true}))]);
        let sink = RecordingEventSink::default();

        let outcome = AgentLoop::new(planner.clone(), executor, 6)
            .run(turn_request(Arc::new(AtomicBool::new(false))), &sink)
            .await
            .unwrap();

        assert_eq!(outcome.message, "解析结果是 ok=true。");
        assert_eq!(sink.events.lock().unwrap().len(), 2);
        assert!(planner.requests.lock().unwrap()[1]
            .history
            .iter()
            .any(|entry| matches!(entry, HistoryEntry::Tool { observation, .. } if observation.contains("true"))));
    }

    #[tokio::test]
    async fn one_planner_error_is_returned_for_repair() {
        let planner = ScriptedPlanner::new(vec![
            Err(anyhow!("arguments must be a JSON object")),
            Ok(PlannerAction::Final {
                message: "repaired".to_string(),
            }),
        ]);
        let sink = RecordingEventSink::default();

        let outcome = AgentLoop::new(planner.clone(), RecordingExecutor::with_results(Vec::new()), 6)
            .run(turn_request(Arc::new(AtomicBool::new(false))), &sink)
            .await
            .unwrap();

        assert_eq!(outcome.message, "repaired");
        assert!(planner.requests.lock().unwrap()[1]
            .repair_error
            .as_deref()
            .unwrap()
            .contains("JSON object"));
    }

    #[tokio::test]
    async fn explicit_tool_request_cannot_be_satisfied_by_a_false_final_answer() {
        let planner = ScriptedPlanner::new(vec![
            Ok(PlannerAction::Final {
                message: "Aion Forge 已正常工作".to_string(),
            }),
            Ok(PlannerAction::CallTool {
                tool: "echo".to_string(),
                arguments: json!({"text": "Aion Forge 已正常工作"}),
            }),
            Ok(PlannerAction::Final {
                message: "Aion Forge 已正常工作".to_string(),
            }),
        ]);
        let executor = RecordingExecutor::with_results(vec![Ok(json!({
            "echo": "Aion Forge 已正常工作"
        }))]);
        let sink = RecordingEventSink::default();
        let request = TurnRequest {
            selected_model: "auto".to_string(),
            cwd: PathBuf::from("D:/test/aionui/forge"),
            instructions: Vec::new(),
            history: vec![HistoryEntry::User(
                "使用 echo 技能返回：Aion Forge 已正常工作".to_string(),
            )],
            capabilities: vec![CapabilityEntry {
                name: "echo".to_string(),
                description: "Echo text".to_string(),
                parameters_schema: json!({"type": "object"}),
                requires_approval: false,
                planner_callable: true,
            }],
            cancellation: Arc::new(AtomicBool::new(false)),
        };

        let outcome = AgentLoop::new(planner.clone(), executor, 6)
            .run(request, &sink)
            .await
            .unwrap();

        assert_eq!(outcome.message, "Aion Forge 已正常工作");
        assert!(sink.events.lock().unwrap().iter().any(|event| event.contains(":echo")));
        assert!(planner.requests.lock().unwrap()[1]
            .repair_error
            .as_deref()
            .unwrap()
            .contains("echo"));
    }

    #[tokio::test]
    async fn stops_visibly_after_six_tool_calls() {
        let actions = (0..7).map(|_| Ok(call_tool())).collect();
        let results = (0..6).map(|_| Ok(json!({"ok": true}))).collect();
        let sink = RecordingEventSink::default();

        let outcome = AgentLoop::new(
            ScriptedPlanner::new(actions),
            RecordingExecutor::with_results(results),
            6,
        )
        .run(turn_request(Arc::new(AtomicBool::new(false))), &sink)
        .await
        .unwrap();

        assert!(outcome.message.contains("6"));
        assert_eq!(sink.events.lock().unwrap().len(), 12);
    }

    #[tokio::test]
    async fn repeated_identical_failed_call_stops_visibly() {
        let sink = RecordingEventSink::default();
        let outcome = AgentLoop::new(
            ScriptedPlanner::new(vec![Ok(call_tool()), Ok(call_tool())]),
            RecordingExecutor::with_results(vec![Err(anyhow!("bad")), Err(anyhow!("bad"))]),
            6,
        )
        .run(turn_request(Arc::new(AtomicBool::new(false))), &sink)
        .await
        .unwrap();

        assert!(outcome.message.contains("重复"));
        assert_eq!(sink.messages.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn cancellation_is_visible_and_late_tool_result_is_not_recorded() {
        let cancellation = Arc::new(AtomicBool::new(false));
        let executor = RecordingExecutor {
            results: Mutex::new(vec![Ok(json!({"late": true}))].into()),
            calls: Mutex::new(Vec::new()),
            cancel_during_execute: Some(cancellation.clone()),
        };
        let sink = RecordingEventSink::default();

        let outcome = AgentLoop::new(ScriptedPlanner::new(vec![Ok(call_tool())]), executor, 6)
            .run(turn_request(cancellation), &sink)
            .await
            .unwrap();

        assert!(outcome.message.contains("取消"));
        assert!(!outcome
            .history
            .iter()
            .any(|entry| matches!(entry, HistoryEntry::Tool { .. })));
    }
}
