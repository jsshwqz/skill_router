use std::{
    collections::VecDeque,
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use aion_forge_acp::{
    agent_loop::{AgentLoop, SessionEventSink, TurnOutcome, TurnRequest},
    catalog::CapabilityCatalog,
    executor::ToolExecutor,
    planner::{Planner, PlannerAction, PlannerRequest},
    session::{HistoryEntry, PromptDisposition, SessionStore},
};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

#[derive(Clone)]
struct ScriptedPlanner {
    requests: Arc<Mutex<Vec<PlannerRequest>>>,
    actions: Arc<Mutex<VecDeque<PlannerAction>>>,
}

impl ScriptedPlanner {
    fn new(actions: Vec<PlannerAction>) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            actions: Arc::new(Mutex::new(actions.into())),
        }
    }

    fn requests(&self) -> Vec<PlannerRequest> {
        self.requests.lock().expect("requests lock should be available").clone()
    }
}

#[async_trait]
impl Planner for ScriptedPlanner {
    async fn next_action(&self, request: PlannerRequest) -> Result<PlannerAction> {
        self.requests
            .lock()
            .expect("requests lock should be available")
            .push(request);
        self.actions
            .lock()
            .expect("actions lock should be available")
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("scripted planner ran out of actions"))
    }
}

#[derive(Clone, Default)]
struct RecordingExecutor {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
}

#[async_trait]
impl ToolExecutor for RecordingExecutor {
    async fn execute(&self, name: &str, arguments: Value, _cwd: &Path) -> Result<Value> {
        self.calls
            .lock()
            .expect("calls lock should be available")
            .push((name.to_string(), arguments));
        Ok(json!({"parsed": {"answer": 42}}))
    }
}

#[derive(Default)]
struct RecordingSink {
    messages: Mutex<Vec<String>>,
}

#[async_trait]
impl SessionEventSink for RecordingSink {
    async fn tool_started(&self, _call_id: &str, _name: &str, _arguments: &Value) -> Result<()> {
        Ok(())
    }

    async fn tool_finished(&self, _call_id: &str, _result: &Result<Value>) -> Result<()> {
        Ok(())
    }

    async fn message_chunk(&self, text: &str) -> Result<()> {
        self.messages
            .lock()
            .expect("messages lock should be available")
            .push(text.to_string());
        Ok(())
    }
}

#[tokio::test]
async fn preserves_forge_identity_capabilities_history_tools_and_exact_model() {
    let store = SessionStore::default();
    let session_id = store
        .create(
            std::env::current_dir().expect("test cwd should exist"),
            "test-exact-model".to_string(),
        )
        .await
        .expect("session should be created");
    assert_eq!(
        store
            .ingest_prompt(&session_id, "[Skill: aion-forge]\nUse live Forge capabilities.")
            .await
            .expect("bootstrap should be stored"),
        PromptDisposition::BootstrapStored
    );

    let planner = ScriptedPlanner::new(vec![
        PlannerAction::Final {
            message: "我是 Aion Forge。".to_string(),
        },
        PlannerAction::CallTool {
            tool: "json_parse".to_string(),
            arguments: json!({"text": "{\"answer\":42}"}),
        },
        PlannerAction::Final {
            message: "解析结果 answer=42。".to_string(),
        },
    ]);
    let executor = RecordingExecutor::default();
    let agent_loop = AgentLoop::new(planner.clone(), executor.clone(), 6);
    let catalog = CapabilityCatalog::live();
    let sink = RecordingSink::default();

    run_turn(&store, &agent_loop, &catalog, &sink, &session_id, "你是谁？").await;
    run_turn(
        &store,
        &agent_loop,
        &catalog,
        &sink,
        &session_id,
        "解析 JSON：{\"answer\":42}",
    )
    .await;

    let requests = planner.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|request| request.identity.contains("Aion Forge")));
    assert!(requests
        .iter()
        .all(|request| request.selected_model == "test-exact-model"));
    assert!(requests
        .iter()
        .all(|request| { request.instructions == vec!["[Skill: aion-forge]\nUse live Forge capabilities."] }));
    assert!(requests
        .iter()
        .all(|request| request.capabilities.iter().any(|entry| entry.name == "json_parse")));
    assert!(!requests[0]
        .history
        .iter()
        .any(|entry| { matches!(entry, HistoryEntry::User(text) if text.starts_with("[Skill:")) }));
    assert!(requests[1]
        .history
        .contains(&HistoryEntry::Assistant("我是 Aion Forge。".to_string())));
    assert!(requests[2].history.iter().any(|entry| {
        matches!(entry, HistoryEntry::Tool { name, observation }
            if name == "json_parse" && observation.contains("42"))
    }));

    assert_eq!(
        executor
            .calls
            .lock()
            .expect("calls lock should be available")
            .as_slice(),
        &[("json_parse".to_string(), json!({"text": "{\"answer\":42}"}))]
    );
    assert_eq!(
        sink.messages
            .lock()
            .expect("messages lock should be available")
            .as_slice(),
        &["我是 Aion Forge。".to_string(), "解析结果 answer=42。".to_string()]
    );
}

async fn run_turn(
    store: &SessionStore,
    agent_loop: &AgentLoop,
    catalog: &CapabilityCatalog,
    sink: &RecordingSink,
    session_id: &str,
    prompt: &str,
) -> TurnOutcome {
    assert_eq!(
        store
            .ingest_prompt(session_id, prompt)
            .await
            .expect("user prompt should be stored"),
        PromptDisposition::UserTurn
    );
    let cancellation: Arc<AtomicBool> = store.start_prompt(session_id).await.expect("prompt should start");
    let snapshot = store.snapshot(session_id).await.expect("session should exist");
    let previous_history_len = snapshot.history.len();
    let outcome = agent_loop
        .run(
            TurnRequest {
                selected_model: snapshot.selected_model,
                cwd: snapshot.cwd,
                instructions: snapshot.instructions,
                history: snapshot.history,
                capabilities: catalog.entries().to_vec(),
                cancellation,
            },
            sink,
        )
        .await
        .expect("turn should complete");
    for entry in outcome.history.iter().skip(previous_history_len).cloned() {
        store
            .append_history(session_id, entry)
            .await
            .expect("new history should persist");
    }
    outcome
}
