# Stateful Aion Forge ACP Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Replace the one-shot ACP adapter with a stateful Aion Forge agent that retains session context, advertises only enabled models, enforces exact model selection, and executes live Forge builtins through a bounded tool loop.

**Architecture:** Keep `aion-forge acp` as the canonical process entry point, use the official Rust ACP SDK for typed protocol messages, and split the implementation into model catalog, session state, capability catalog, planner, executor, agent loop, and transport modules. The planner emits provider-neutral typed actions, while the executor invokes the existing `BuiltinRegistry`; production dependencies are hidden behind traits so tests never require a live model endpoint.

**Tech Stack:** Rust 2021, Tokio, `agent-client-protocol` 1.3, Serde, `async-trait`, existing `aion-router` and `aion-types`, Cargo test/clippy/fmt.

---

## Task 1: Add the official ACP dependency and preserve the process contract

**Files:**

- Modify: `Cargo.toml`
- Modify: `aion-forge-acp/Cargo.toml`
- Modify: `aion-forge-cli/tests/acp_contract.rs`
- Modify: `aion-forge-acp/src/acp.rs`

- [x] **Step 1: Update the process contract test to send a valid ACP initialize request**

Replace the empty initialize parameters with a standard ACP request and close stdin after the request. Assert that stdout contains JSON only, the response echoes the ID, and the negotiated protocol version is present.

```rust
let input = concat!(
    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"aion-forge-test","title":"Aion Forge Test","version":"0.1.0"}}}"#,
    "\n"
);

let response = &responses[0];
assert_eq!(response["id"], 1);
assert_eq!(response["result"]["protocolVersion"], 1);
assert!(response["result"]["agentInfo"]["name"].is_string());
```

- [x] **Step 2: Run the focused test and confirm it fails against the current hand-written response**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-cli --test acp_contract -- --nocapture`

Observed: the legacy adapter accepted malformed initialize parameters, so the focused failure asserted that missing required ACP fields must return JSON-RPC `-32602`.

- [x] **Step 3: Add shared dependencies**

Add to root `[workspace.dependencies]`:

```rust
// Cargo.toml entries to add as TOML, shown here as the exact values.
// agent-client-protocol = "1.3.0"
```

Add to `aion-forge-acp/Cargo.toml`:

```rust
// agent-client-protocol = { workspace = true }
// serde = { workspace = true }
// async-trait = { workspace = true }
// uuid = { workspace = true }
// tracing = { workspace = true }
```

- [x] **Step 4: Replace only the initialize/shutdown transport shell with the official SDK**

Build the ACP `Agent`, register an `InitializeRequest` handler, and connect through `Stdio::new().with_debug(debug_callback)` so the wire format remains newline-delimited JSON for AionUI. The debug callback must log only direction and byte length through `tracing::trace!`; it must not copy request bodies or credentials into logs.

Retain a narrow raw-dispatch compatibility handler for the legacy `shutdown` request until the existing standalone tests are migrated. Do not retain MCP `tools/list` or `tools/call` behavior in the ACP server; MCP remains under `aion-forge mcp-server`.

- [x] **Step 5: Run the contract test and commit**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-cli --test acp_contract -- --nocapture`

Expected: one successful initialization response, JSON-only stdout, and a clean exit after stdin closes.

Commit: `git commit -m "refactor(acp): adopt official Rust transport"`

## Task 2: Build the enabled model catalog and strict resolver

**Files:**

- Create: `aion-forge-acp/src/model_catalog.rs`
- Modify: `aion-forge-acp/src/lib.rs`

- [x] **Step 1: Write failing catalog tests**

Cover deduplication, disabled/empty entries, default selection, exact resolution, and the explicit `auto` exception.

```rust
#[test]
fn rejects_unknown_model_without_fallback() {
    let catalog = ModelCatalog::from_endpoints(vec![endpoint("deepseek-chat")], None);

    let error = catalog.resolve("missing-model").unwrap_err();

    assert!(error.to_string().contains("missing-model"));
    assert!(error.to_string().contains("deepseek-chat"));
}

#[test]
fn auto_is_the_only_fallback_selection() {
    let catalog = ModelCatalog::from_endpoints(vec![endpoint("deepseek-chat")], None);

    assert!(matches!(catalog.resolve("auto"), Ok(ModelResolution::Auto)));
    assert!(matches!(
        catalog.resolve("deepseek-chat"),
        Ok(ModelResolution::Exact(endpoint)) if endpoint.model == "deepseek-chat"
    ));
}
```

- [x] **Step 2: Run the module test and confirm the module is missing**

Observed RED: compilation failed because `ModelCatalog` and `ModelResolution` were not defined.

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp model_catalog -- --nocapture`

Expected: compile failure because `ModelCatalog` does not exist.

- [x] **Step 3: Implement the pure model catalog**

Use these public types and invariants:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResolution {
    Auto,
    Exact(AiEndpoint),
}

#[derive(Debug, Clone)]
pub struct ModelCatalog {
    endpoints: Vec<AiEndpoint>,
    default_model: String,
}

impl ModelCatalog {
    pub fn from_endpoints(endpoints: Vec<AiEndpoint>, requested_default: Option<&str>) -> Self;
    pub fn from_environment() -> Self;
    pub fn default_model(&self) -> &str;
    pub fn model_ids(&self) -> Vec<&str>;
    pub fn resolve(&self, selected: &str) -> anyhow::Result<ModelResolution>;
    pub fn session_config_option(&self) -> agent_client_protocol::SessionConfigOption;
}
```

Filter endpoints whose model, base URL, or key is empty; deduplicate by model ID while preserving priority. Use `AI_MODEL` only if that exact model remains enabled. Otherwise select the first enabled model; if there are no enabled endpoints, default to `auto` and return a visible configuration error when execution is attempted.

The ACP select option ID must be `model`. Its values must be `auto` followed by the enabled model IDs, with no hard-coded DeepSeek, Claude, Qwen, or GLM entries.

- [x] **Step 4: Run tests and commit**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp model_catalog -- --nocapture`

Expected: all model catalog tests pass.

Commit: `git commit -m "feat(acp): add strict dynamic model catalog"`

## Task 3: Add stateful sessions, bootstrap ingestion, and cancellation

**Files:**

- Create: `aion-forge-acp/src/session.rs`
- Modify: `aion-forge-acp/src/lib.rs`

- [x] **Step 1: Write failing session lifecycle tests**

Test creation, unknown IDs, model persistence, bounded history, bootstrap classification, and cancellation replacement.

```rust
#[tokio::test]
async fn bootstrap_prompt_becomes_instruction_not_user_history() {
    let store = SessionStore::default();
    let session_id = store
        .create(PathBuf::from("D:/test/aionui/forge"), "auto".to_string())
        .await
        .unwrap();

    let disposition = store
        .ingest_prompt(&session_id, "[Skill: aion-forge]\nUse Forge tools first.")
        .await
        .unwrap();

    assert_eq!(disposition, PromptDisposition::BootstrapStored);
    let snapshot = store.snapshot(&session_id).await.unwrap();
    assert_eq!(snapshot.instructions.len(), 1);
    assert!(snapshot.history.is_empty());
}

#[tokio::test]
async fn ordinary_skill_question_remains_a_user_turn() {
    let store = session_store();
    let session_id = create_session(&store).await;

    let disposition = store
        .ingest_prompt(&session_id, "你有哪些技能？")
        .await
        .unwrap();

    assert_eq!(disposition, PromptDisposition::UserTurn);
}
```

- [x] **Step 2: Run and confirm failure**

Observed RED: compilation failed because `SessionStore`, `HistoryEntry`, and `PromptDisposition` were absent.

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp session -- --nocapture`

Expected: compile failure because the session store is absent.

- [x] **Step 3: Implement the session store**

Use an `Arc<RwLock<HashMap<String, SessionState>>>`. Keep public APIs documented and return snapshots instead of exposing locks.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryEntry {
    User(String),
    Assistant(String),
    Tool { name: String, observation: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptDisposition {
    BootstrapStored,
    UserTurn,
}

#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub id: String,
    pub cwd: PathBuf,
    pub selected_model: String,
    pub instructions: Vec<String>,
    pub history: Vec<HistoryEntry>,
    pub cancellation: Arc<AtomicBool>,
}
```

Recognize bootstrap content only when the trimmed prompt begins with `[Assistant Rules]` or `[Skill: `. Validate that `cwd` exists and is a directory. Keep the latest 48 history entries, preserving instruction entries separately. Starting a prompt replaces the cancellation flag; `session/cancel` sets the active flag to true.

- [x] **Step 4: Run tests and commit**

The full crate suite also exposed an obsolete empty `initialize` request in `cli_isolation`; it now sends the required standard ACP fields.

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp session -- --nocapture`

Expected: session tests pass with deterministic in-memory state.

Commit: `git commit -m "feat(acp): persist session state and bootstrap rules"`

## Task 4: Generate the live capability catalog and execute registered tools

**Files:**

- Create: `aion-forge-acp/src/catalog.rs`
- Create: `aion-forge-acp/src/executor.rs`
- Modify: `aion-forge-acp/src/lib.rs`

- [x] **Step 1: Write failing catalog and executor tests**

Assert that every exposed capability exists in `BuiltinRegistry`, descriptions come from `CapabilityRegistry`, recursive planner entries are not callable, unknown names fail, and valid parameters reach a fake builtin executor.

```rust
#[test]
fn live_catalog_does_not_advertise_missing_builtins() {
    let registry = BuiltinRegistry::default_registry();
    let catalog = CapabilityCatalog::from_registries(
        &registry,
        &CapabilityRegistry::builtin(),
    );

    assert!(catalog.entries().iter().all(|entry| registry.get(&entry.name).is_some()));
    assert!(catalog.entries().iter().any(|entry| entry.name == "text_summarize"));
}

#[tokio::test]
async fn executor_rejects_recursive_planner_entry() {
    let executor = ForgeToolExecutor::default();

    let error = executor
        .execute("ai_task", serde_json::json!({"task": "loop"}), Path::new("."))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("not callable from ACP planner"));
}
```

- [x] **Step 2: Run and confirm failure**

Observed RED: compilation failed because `CapabilityCatalog`, `ForgeToolExecutor`, and `ToolExecutor` were absent.

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp catalog -- --nocapture`

Expected: compile failure because the catalog and executor types are absent.

- [x] **Step 3: Implement the capability catalog**

Intersect `BuiltinRegistry::list_skills()` with `CapabilityRegistry::builtin()`. Store name, description, parameter schema, approval metadata, and a `planner_callable` flag. Expose the exact live list for identity answers; set `planner_callable` to false for `ai_task` and `autonomous_agent` so the ACP planner cannot recursively create another planner loop. The intersection intentionally excludes metadata-only `text_summarize`, which has no registered builtin executor.

```rust
#[derive(Debug, Clone)]
pub struct CapabilityEntry {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
    pub requires_approval: bool,
    pub planner_callable: bool,
}
```

- [x] **Step 4: Implement the executor trait and production adapter**

```rust
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(
        &self,
        name: &str,
        arguments: serde_json::Value,
        cwd: &Path,
    ) -> anyhow::Result<serde_json::Value>;
}
```

`ForgeToolExecutor` owns a `BuiltinRegistry`, validates an object argument, checks the capability is planner-callable, builds the existing `SkillDefinition` and `ExecutionContext`, and calls the builtin. Return normalized JSON; never log parameters that may contain secrets.

- [x] **Step 5: Run tests and commit**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp catalog executor -- --nocapture`

Expected: catalog and executor tests pass.

Commit: `git commit -m "feat(acp): expose and execute live Forge capabilities"`

## Task 5: Add typed planner actions and exact-model AI requests

**Files:**

- Create: `aion-forge-acp/src/planner.rs`
- Modify: `aion-forge-acp/src/lib.rs`

- [x] **Step 1: Write failing action parser tests**

Cover final responses, valid tool calls, fenced JSON, malformed envelopes, non-object arguments, and one repair request.

```rust
#[test]
fn parses_provider_neutral_tool_action() {
    let action = PlannerAction::parse(
        r#"{"action":"call_tool","tool":"text_summarize","arguments":{"text":"abc"}}"#,
    )
    .unwrap();

    assert_eq!(
        action,
        PlannerAction::CallTool {
            tool: "text_summarize".to_string(),
            arguments: serde_json::json!({"text": "abc"}),
        }
    );
}
```

- [x] **Step 2: Run and confirm failure**

Observed RED: compilation failed because the planner action, planner trait, AI executor boundary, and production planner were absent.

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp planner -- --nocapture`

Expected: compile failure because planner actions are absent.

- [x] **Step 3: Implement planner types and trait**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PlannerAction {
    Final { message: String },
    CallTool { tool: String, arguments: serde_json::Value },
}

#[derive(Debug, Clone)]
pub struct PlannerRequest {
    pub selected_model: String,
    pub cwd: PathBuf,
    pub instructions: Vec<String>,
    pub history: Vec<HistoryEntry>,
    pub capabilities: Vec<CapabilityEntry>,
    pub repair_error: Option<String>,
}

#[async_trait]
pub trait Planner: Send + Sync {
    async fn next_action(&self, request: PlannerRequest) -> anyhow::Result<PlannerAction>;
}
```

Accept one outer JSON code fence, then require a single object with either `action=final` plus a non-empty `message`, or `action=call_tool` plus a registered `tool` and object `arguments`.

- [x] **Step 4: Implement the production `AiTaskPlanner`**

Resolve the session model through `ModelCatalog` before invoking `ai_task`. For an exact model, pass that exact ID. For `auto`, omit the model constraint and allow the router priority policy. Reject unavailable exact models before any AI call.

Build the planner prompt with the project eight-step framework: role, task context, detailed positive rules, two XML-wrapped examples, XML-wrapped task data, exact JSON output schema, stepwise analysis instruction, and an `unknown` escape path. Include the live capability catalog and bounded session history. Do not hard-code a capability count.

Before committing the final prompt template, run the Forge audit:

Run (PowerShell): `rtk aion-forge --tool prompt_audit --params '{"prompt":"<the exact planner template>"}' --quiet`

Observed: `prompt_audit` reported all 8 framework items present with no critical issues.

- [x] **Step 5: Test exact model forwarding with a fake AI executor and commit**

The fake must record the `model` field. Assert `deepseek-chat` remains `deepseek-chat`, an unknown model returns an error without a request, and only `auto` omits the exact model.

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp planner -- --nocapture`

Expected: parser and forwarding tests pass without network access.

Commit: `git commit -m "feat(acp): add typed exact-model planner"`

## Task 6: Implement the bounded tool loop

**Files:**

- Create: `aion-forge-acp/src/agent_loop.rs`
- Modify: `aion-forge-acp/src/lib.rs`

- [x] **Step 1: Write failing loop integration tests with fakes**

Test final-only responses, tool result consumption, multi-turn history, one malformed repair, six-call limit, repeated identical failure, and cancellation.

```rust
#[tokio::test]
async fn tool_result_is_consumed_before_final_response() {
    let planner = ScriptedPlanner::new(vec![
        PlannerAction::CallTool {
            tool: "json_parse".to_string(),
            arguments: serde_json::json!({"text": "{\"ok\":true}"}),
        },
        PlannerAction::Final {
            message: "解析结果是 ok=true。".to_string(),
        },
    ]);
    let executor = RecordingExecutor::succeeding(serde_json::json!({"ok": true}));
    let sink = RecordingEventSink::default();

    let outcome = AgentLoop::new(planner, executor, 6)
        .run(turn_request(), &sink)
        .await
        .unwrap();

    assert_eq!(outcome.message, "解析结果是 ok=true。");
    assert_eq!(sink.tool_events().len(), 2);
    assert!(outcome.history.iter().any(|entry| matches!(
        entry,
        HistoryEntry::Tool { name, observation }
            if name == "json_parse" && observation.contains("true")
    )));
}
```

- [x] **Step 2: Run and confirm failure**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp agent_loop -- --nocapture`

Expected: compile failure because the loop and event sink are absent.

Observed: the focused test target failed to compile because `AgentLoop`, `SessionEventSink`, and `TurnRequest` were not yet defined.

- [x] **Step 3: Implement the event sink and loop**

```rust
#[async_trait]
pub trait SessionEventSink: Send + Sync {
    async fn tool_started(&self, call_id: &str, name: &str, arguments: &serde_json::Value)
        -> anyhow::Result<()>;
    async fn tool_finished(&self, call_id: &str, result: &anyhow::Result<serde_json::Value>)
        -> anyhow::Result<()>;
    async fn message_chunk(&self, text: &str) -> anyhow::Result<()>;
}

pub struct TurnOutcome {
    pub message: String,
    pub history: Vec<HistoryEntry>,
}
```

Check cancellation before planning, before executing each tool, and before recording each result. Generate a UUID call ID. Stop after six tool calls. Permit one parser repair by sending the parse error back through `repair_error`. Track the canonical JSON of each failed `{tool, arguments}` pair and stop visibly when the same failed call repeats.

Every terminal path must emit a non-empty `message_chunk`, except the recognized bootstrap path handled before the loop. Normalize tool observations to at most 32 KiB before adding them to history.

- [x] **Step 4: Run tests and commit**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp agent_loop -- --nocapture`

Expected: all loop boundary tests pass with no provider calls.

Commit: `git commit -m "feat(acp): run bounded Forge tool loop"`

## Task 7: Wire typed ACP session handlers and AionUI compatibility

**Files:**

- Rewrite: `aion-forge-acp/src/acp.rs`
- Modify: `aion-forge-acp/src/lib.rs`
- Create: `aion-forge-acp/tests/acp_agent_contract.rs`
- Modify: `aion-forge-cli/tests/acp_contract.rs`

- [x] **Step 1: Write failing protocol tests**

Launch the canonical CLI and exercise:

- `initialize`
- `session/new` returning a session ID and `model` configuration option
- `session/set_config_option` persisting an enabled model
- A bootstrap `session/prompt` returning `end_turn` without an AI request
- unknown session returning a JSON-RPC error
- JSON-only stdout
- provider credentials absent from both stdout and stderr

Use an environment with one deterministic enabled model ID, set its API key to `acp-secret-sentinel`, and do not send an ordinary prompt in the process test.

```rust
assert_eq!(new_session["result"]["configOptions"][0]["id"], "model");
assert_eq!(new_session["result"]["configOptions"][0]["currentValue"], "test-model");
assert_eq!(bootstrap_response["result"]["stopReason"], "end_turn");
assert!(unknown_session["error"]["message"].as_str().unwrap().contains("unknown session"));
assert!(!stdout.contains("acp-secret-sentinel"));
assert!(!stderr.contains("acp-secret-sentinel"));
```

- [x] **Step 2: Run and confirm failure**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp --test acp_agent_contract -- --nocapture`

Expected: failure because the current adapter does not persist sessions or expose stable config options.

Observed RED: `session/new` returned no `configOptions`, proving that the legacy adapter did not expose a selectable session model.

- [x] **Step 3: Implement `ForgeAcpAgent` and handlers**

`ForgeAcpAgent` owns shared `SessionStore`, `ModelCatalog`, `CapabilityCatalog`, and `AgentLoop`. Register typed handlers for `InitializeRequest`, `NewSessionRequest`, `SetSessionConfigOptionRequest`, and `PromptRequest`, plus a cancellation notification handler.

For `session/new`, validate `cwd`, choose the requested enabled model or catalog default, create the session, and return the model config option. For `session/set_config_option`, accept only option ID `model`, validate through `ModelCatalog`, persist it, and emit a config-option update if required by the SDK.

For `session/prompt`, concatenate text blocks in order, reject empty content visibly, ingest bootstrap envelopes without invoking the planner, and route ordinary turns through `AgentLoop`.

- [x] **Step 4: Map loop events to standard ACP updates**

Map final text to `SessionUpdate::AgentMessageChunk`. Map tool start to `SessionUpdate::ToolCall` with a stable call ID and arguments. Map success/failure to `SessionUpdate::ToolCallUpdate`, preserving the same call ID and using terminal status values. Return `PromptResponse::new(StopReason::EndTurn)` after the visible chunk is sent.

All tracing stays on stderr. Remove `eprintln!` and any stdout diagnostic text from the library.

- [x] **Step 5: Add the narrow compatibility layer**

Accept a model supplied in AionUI session creation or prompt extension fields and update the same session model field after validation. Accept the previously observed legacy model-selection request only as a raw dispatch fallback. Unknown legacy model IDs must produce the same visible list of valid options and must not call `ai_task`.

- [x] **Step 6: Run protocol tests and commit**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp --test acp_agent_contract -- --nocapture`

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-cli --test acp_contract -- --nocapture`

Expected: typed and canonical CLI protocol tests pass; stdout parses line-by-line as JSON.

Commit: `git commit -m "feat(acp): wire stateful ACP session protocol"`

## Task 8: Prove identity, tools, history, and model persistence end to end

**Files:**

- Create: `aion-forge-acp/tests/agent_engine.rs`
- Modify: `aion-forge-acp/src/planner.rs`
- Modify: `aion-forge-acp/src/agent_loop.rs`
- Modify: `aion-forge-acp/src/session.rs`

- [ ] **Step 1: Write an end-to-end fake-engine test**

Create a fake planner that records each `PlannerRequest` and returns scripted actions. Create a fake executor that records calls. Run three turns in the same session:

1. Store `[Skill: aion-forge]` bootstrap instructions.
2. Ask for Forge identity and return a final response.
3. Ask for JSON parsing, execute `json_parse`, consume its observation, and return the final response.

Assert the second and third planner requests contain Forge identity, the live registry capability names, stored bootstrap instructions, earlier user/assistant history, and the exact selected model.

- [ ] **Step 2: Run and confirm failure before final integration adjustments**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp --test agent_engine -- --nocapture`

Expected: at least one assertion fails until the final history/catalog plumbing is complete.

- [ ] **Step 3: Complete only the missing integration plumbing**

Ensure successful user turns append user and assistant entries atomically. Ensure tool observations are appended before the next planner request. Ensure bootstrap prompts never appear as user history. Ensure the planner request receives a fresh capability catalog generated from the live registries.

- [ ] **Step 4: Run the complete ACP test suite and commit**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp --all-targets -- --nocapture`

Expected: unit, protocol, and engine tests pass without network access.

Commit: `git commit -m "test(acp): cover stateful Forge agent behavior"`

## Task 9: Verify, build, deploy, and refresh AionUI discovery

**Files:**

- Modify only if required by observed discovery state: AionUI Agent registration/configuration through the `aionui-config` skill
- Update: `docs/superpowers/plans/2026-07-23-stateful-forge-acp-agent.md` checkbox state during execution

- [ ] **Step 1: Run repository-quality checks**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo fmt --all -- --check`

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo clippy -p aion-forge-acp -p aion-forge-cli --all-targets -- -D warnings`

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo test -p aion-forge-acp -p aion-forge-cli --all-targets`

Run (PowerShell): `rtk git diff --check`

Expected: all commands exit zero.

- [ ] **Step 2: Request a Forge code review**

Run the available Aion Forge review tool against the changed files. Prefer `haoojiang_review`; if it is unavailable, use `code_lint`. Resolve only findings tied to ACP correctness, security, protocol compliance, or regressions.

Run (PowerShell): `rtk aion-forge --tool haoojiang_review --params '{"task":"Review the stateful ACP agent implementation for protocol correctness, exact model selection, cancellation, tool-loop safety, stdout purity, and secret leakage."}' --quiet`

Expected: no unresolved high-severity finding. Record a Forge tool failure rather than replacing it with an unapproved external reviewer.

- [ ] **Step 3: Build release binaries**

Run (PowerShell): `$env:CARGO_HOME = Join-Path $env:TEMP 'aion-forge-cargo-direct'; rtk cargo build --release -p aion-forge-cli -p aion-forge-acp`

Expected: `target/release/aion-forge.exe` and `target/release/aion-forge-acp.exe` exist.

- [ ] **Step 4: Replace the deployed binaries safely**

Stop only the AionUI-owned Forge ACP process if it is running. Resolve the source and destination absolute paths, verify both destinations stay inside `D:/test/aionui/forge`, then copy the release binaries to the canonical root executables used by AionUI. Preserve the previous binaries with timestamped `.bak` names until AionUI verification succeeds.

- [ ] **Step 5: Refresh AionUI Agent metadata through the AionUI configuration skill**

Read and follow `aionui-config`. Confirm the Agent command is the canonical `D:/test/aionui/forge/aion-forge.exe` with argument `acp`. Refresh or recreate only the stale Aion Forge development Agent entry; do not change unrelated Agents or global provider settings.

- [ ] **Step 6: Perform real AionUI acceptance tests**

Open a new Aion Forge conversation and verify:

1. `你是谁？` identifies Aion Forge.
2. `你有哪些技能？` returns names derived from the live registry rather than zero skills or a hard-coded count.
3. A deterministic builtin request emits visible tool lifecycle updates and a visible final answer.
4. A follow-up question uses prior-turn context.
5. The model selector lists only `auto` and currently enabled model IDs.
6. Switching between two actually enabled models changes the selected session model and the outbound request model.
7. Selecting an invalid model returns a visible error and does not fall back to DeepSeek.
8. A new ordinary prompt never ends with “这次请求没有产生任何可见回复。”

Capture stderr logs with secrets redacted and confirm no late tool result is written after cancellation.

- [ ] **Step 7: Remove backups after acceptance and record the session**

After all acceptance checks pass, remove only the timestamped backups created in Step 4. Call `record_change` once per changed file, call `record_decision` for the exact-model/explicit-auto policy and the official ACP transport choice, generate `session_report`, and store that report through `memory_remember` with category `Decision`.

Commit any final test-only adjustment as: `git commit -m "fix(acp): complete AionUI agent integration"`

## Completion Gate

The implementation is complete only when all of the following are true:

- `aion-forge acp` uses the official typed ACP transport and emits JSON only on stdout.
- Sessions retain instructions, model selection, conversation history, tool observations, and cancellation state.
- Identity and capabilities are generated from the live Forge registries.
- Ordinary prompts can run real builtins through the bounded tool loop.
- Exact models never silently cross-fallback; only `auto` permits router fallback.
- Unit, protocol, integration, formatting, lint, and release build checks pass.
- A fresh AionUI conversation passes the eight acceptance checks above.
