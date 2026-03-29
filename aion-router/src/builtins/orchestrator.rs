//! 多模型编排器 — 用 Rust 重写 ai-orchestrator
//!
//! 通过 CLI subprocess 调用 Claude/Codex/Gemini，支持 9 种协作模式。
//! 重点能力：
//! - MCP / 直连双等待窗口
//! - 三模型方案讨论 / 共识 / 争议复审 / 结果仲裁
//! - CLI 健康状态持久化与降级
//! - 统一错误分类与审计轨迹

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{info, warn};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn short_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("orch_{:08x}_{:04x}", now_secs() as u32 ^ std::process::id(), seq)
}

fn workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn state_dir() -> PathBuf {
    workspace_root().join(".skill-router")
}

fn ensure_state_dir() -> PathBuf {
    let dir = state_dir();
    let _ = fs::create_dir_all(&dir);
    dir
}

fn cli_health_path() -> PathBuf {
    ensure_state_dir().join("cli_health.json")
}

fn orchestration_trace_path() -> PathBuf {
    ensure_state_dir().join("orchestration_trace.jsonl")
}

fn engine_cache_path() -> PathBuf {
    ensure_state_dir().join("engine_result_cache.json")
}

fn file_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn ansi_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").expect("valid ansi regex"))
}

fn strip_ansi(s: &str) -> String {
    ansi_regex().replace_all(s, "").into_owned()
}

fn is_noise_line(engine: Engine, line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "Loaded cached credentials." {
        return true;
    }
    if trimmed.starts_with("warning: Falling back from WebSockets") {
        return true;
    }
    if trimmed.starts_with("Skill conflict detected") {
        return true;
    }
    matches!(engine, Engine::OpenAi) && trimmed.eq_ignore_ascii_case("codex")
}

fn normalize_output(engine: Engine, raw: &str) -> String {
    strip_ansi(raw)
        .lines()
        .filter(|line| !is_noise_line(engine, line))
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn excerpt(value: &str, max: usize) -> String {
    safe_truncate(value.trim(), max).to_string()
}

fn normalize_token(s: &str) -> String {
    s.to_lowercase().replace('：', ":").replace(
        [
            ' ', '\t', '\r', '\n', '_', '-', '|', '>', ',', '，', '。', '；', ';', ':', '/', '\\',
        ],
        "",
    )
}

fn split_compact_list(value: &str, separators: &[char]) -> Vec<String> {
    value
        .split(|c| separators.contains(&c))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[derive(Clone, serde::Serialize)]
struct AsyncTaskResult {
    task_id: String,
    workflow: String,
    status: String,
    started_at: u64,
    finished_at: Option<u64>,
    result: Option<Value>,
}

static ASYNC_TASKS: OnceLock<Mutex<HashMap<String, AsyncTaskResult>>> = OnceLock::new();

fn task_store() -> &'static Mutex<HashMap<String, AsyncTaskResult>> {
    ASYNC_TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct AsyncTaskQuery;

#[async_trait::async_trait]
impl BuiltinSkill for AsyncTaskQuery {
    fn name(&self) -> &'static str {
        "async_task_query"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let task_id = ctx.context["task_id"].as_str().unwrap_or("");

        if task_id.is_empty() {
            let store = task_store().lock().unwrap_or_else(|e| e.into_inner());
            let tasks: Vec<Value> = store
                .values()
                .map(|t| {
                    json!({
                        "task_id": t.task_id,
                        "workflow": t.workflow,
                        "status": t.status,
                        "started_at": t.started_at,
                        "finished_at": t.finished_at,
                    })
                })
                .collect();
            return Ok(json!({"tasks": tasks, "count": tasks.len()}));
        }

        let store = task_store().lock().unwrap_or_else(|e| e.into_inner());
        match store.get(task_id) {
            Some(task) => Ok(json!({
                "task_id": task.task_id,
                "workflow": task.workflow,
                "status": task.status,
                "started_at": task.started_at,
                "finished_at": task.finished_at,
                "result": task.result,
            })),
            None => Ok(json!({"error": format!("任务 '{}' 不存在", task_id)})),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Claude,
    OpenAi,
    Gemini,
}

impl Engine {
    fn label(&self) -> &'static str {
        match self {
            Engine::Claude => "claude",
            Engine::OpenAi => "openai",
            Engine::Gemini => "gemini",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "claude" => Some(Self::Claude),
            "openai" => Some(Self::OpenAi),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HealthStatus {
    Healthy,
    Degraded,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EngineHealthState {
    status: HealthStatus,
    successes: u64,
    failures: u64,
    consecutive_failures: u32,
    avg_latency_ms: f64,
    last_error_kind: Option<String>,
    last_updated_at: u64,
    cooldown_until: Option<u64>,
}

impl Default for EngineHealthState {
    fn default() -> Self {
        Self {
            status: HealthStatus::Healthy,
            successes: 0,
            failures: 0,
            consecutive_failures: 0,
            avg_latency_ms: 0.0,
            last_error_kind: None,
            last_updated_at: now_secs(),
            cooldown_until: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CliHealthStore {
    engines: BTreeMap<String, EngineHealthState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EngineResultCacheStore {
    entries: BTreeMap<String, CachedEngineResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedEngineResult {
    engine: String,
    phase: String,
    cache_key: String,
    output: String,
    duration_ms: u64,
    stored_at: u64,
}

fn load_cli_health() -> CliHealthStore {
    let _guard = file_lock().lock().unwrap_or_else(|e| e.into_inner());
    let path = cli_health_path();
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => CliHealthStore::default(),
    }
}

fn save_cli_health(store: &CliHealthStore) {
    let _guard = file_lock().lock().unwrap_or_else(|e| e.into_inner());
    let path = cli_health_path();
    if let Ok(bytes) = serde_json::to_vec_pretty(store) {
        let _ = fs::write(path, bytes);
    }
}

fn load_engine_cache() -> EngineResultCacheStore {
    let _guard = file_lock().lock().unwrap_or_else(|e| e.into_inner());
    let path = engine_cache_path();
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => EngineResultCacheStore::default(),
    }
}

fn save_engine_cache(store: &EngineResultCacheStore) {
    let _guard = file_lock().lock().unwrap_or_else(|e| e.into_inner());
    let path = engine_cache_path();
    if let Ok(bytes) = serde_json::to_vec_pretty(store) {
        let _ = fs::write(path, bytes);
    }
}

fn cache_ttl_secs() -> u64 {
    std::env::var("AION_ENGINE_CACHE_TTL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(6 * 60 * 60)
}

fn phase_grace_window_secs() -> u64 {
    std::env::var("AION_PHASE_GRACE_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3)
}

fn cache_key(engine: Engine, phase: &str, prompt: &str, cfg: &OrchestratorConfig) -> String {
    let mut hasher = DefaultHasher::new();
    engine.hash(&mut hasher);
    phase.hash(&mut hasher);
    prompt.hash(&mut hasher);
    cfg.claude_model.hash(&mut hasher);
    cfg.openai_model.hash(&mut hasher);
    format!("{}:{}:{:016x}", engine.label(), phase, hasher.finish())
}

fn cached_success_report(
    engine: Engine,
    phase: &str,
    prompt: &str,
    cfg: &OrchestratorConfig,
    status_before: &str,
) -> Option<EngineCallReport> {
    let key = cache_key(engine, phase, prompt, cfg);
    let now = now_secs();
    let mut store = load_engine_cache();
    store
        .entries
        .retain(|_, entry| now.saturating_sub(entry.stored_at) <= cache_ttl_secs());
    save_engine_cache(&store);
    let entry = store.entries.get(&key)?.clone();
    Some(EngineCallReport {
        engine: entry.engine,
        phase: entry.phase,
        success: true,
        status_before: status_before.to_string(),
        status_after: "healthy".to_string(),
        duration_ms: 0,
        output: Some(entry.output.clone()),
        output_excerpt: Some(excerpt(&entry.output, 320)),
        error_kind: None,
        error_message: None,
        exit_code: None,
        stdout_excerpt: Some("cache hit".to_string()),
        stderr_excerpt: None,
        skipped: false,
        cache_hit: true,
    })
}

fn persist_success_cache(
    engine: Engine,
    phase: &str,
    prompt: &str,
    cfg: &OrchestratorConfig,
    output: &str,
    duration_ms: u64,
) {
    let key = cache_key(engine, phase, prompt, cfg);
    let now = now_secs();
    let mut store = load_engine_cache();
    store
        .entries
        .retain(|_, entry| now.saturating_sub(entry.stored_at) <= cache_ttl_secs());
    store.entries.insert(
        key.clone(),
        CachedEngineResult {
            engine: engine.label().to_string(),
            phase: phase.to_string(),
            cache_key: key,
            output: output.to_string(),
            duration_ms,
            stored_at: now,
        },
    );
    save_engine_cache(&store);
}

fn engine_state(engine: Engine) -> EngineHealthState {
    load_cli_health()
        .engines
        .get(engine.label())
        .cloned()
        .unwrap_or_default()
}

fn engine_lock(engine: Engine) -> &'static AsyncMutex<()> {
    static CLAUDE_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    static OPENAI_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    static GEMINI_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    match engine {
        Engine::Claude => CLAUDE_LOCK.get_or_init(|| AsyncMutex::new(())),
        Engine::OpenAi => OPENAI_LOCK.get_or_init(|| AsyncMutex::new(())),
        Engine::Gemini => GEMINI_LOCK.get_or_init(|| AsyncMutex::new(())),
    }
}

#[derive(Debug, Clone)]
struct OrchestratorConfig {
    claude_cli: String,
    codex_cli: String,
    gemini_cli: String,
    claude_model: String,
    openai_model: String,
    timeout: Duration,
    phase_window: Duration,
    passthrough: bool,
}

impl OrchestratorConfig {
    fn from_env() -> Self {
        Self {
            claude_cli: std::env::var("CLAUDE_CLI").unwrap_or_else(|_| "claude".into()),
            codex_cli: std::env::var("CODEX_CLI").unwrap_or_else(|_| "codex".into()),
            gemini_cli: std::env::var("GEMINI_CLI").unwrap_or_else(|_| "gemini".into()),
            claude_model: std::env::var("CLAUDE_MODEL").unwrap_or_else(|_| "sonnet".into()),
            openai_model: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4".into()),
            timeout: Duration::from_secs(
                std::env::var("REQUEST_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(180),
            ),
            phase_window: Duration::from_secs(
                std::env::var("AION_PHASE_WINDOW_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(40),
            ),
            passthrough: std::env::var("AI_PASSTHROUGH")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FailureKind {
    AuthError,
    Timeout,
    EmptyOutput,
    ModelNotFound,
    ProcessError,
    SafetyBlock,
}

impl FailureKind {
    fn as_str(&self) -> &'static str {
        match self {
            FailureKind::AuthError => "auth_error",
            FailureKind::Timeout => "timeout",
            FailureKind::EmptyOutput => "empty_output",
            FailureKind::ModelNotFound => "model_not_found",
            FailureKind::ProcessError => "process_error",
            FailureKind::SafetyBlock => "safety_block",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct EngineCallReport {
    engine: String,
    phase: String,
    success: bool,
    status_before: String,
    status_after: String,
    duration_ms: u64,
    #[serde(skip_serializing)]
    output: Option<String>,
    output_excerpt: Option<String>,
    error_kind: Option<String>,
    error_message: Option<String>,
    exit_code: Option<i32>,
    stdout_excerpt: Option<String>,
    stderr_excerpt: Option<String>,
    skipped: bool,
    #[serde(default)]
    cache_hit: bool,
}

#[derive(Debug)]
enum CliRunOutcome {
    Completed {
        status_success: bool,
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
        duration_ms: u64,
    },
    Timeout {
        duration_ms: u64,
    },
}

fn classify_failure(stderr: &str, stdout: &str) -> FailureKind {
    let message = format!("{}\n{}", stderr, stdout).to_lowercase();
    if message.contains("401")
        || message.contains("unauthorized")
        || message.contains("authentication")
        || message.contains("sign in")
        || message.contains("login")
        || message.contains("invalid bearer token")
    {
        FailureKind::AuthError
    } else if message.contains("modelnotfound")
        || message.contains("model not found")
        || message.contains("unknown model")
    {
        FailureKind::ModelNotFound
    } else if message.contains("policy") || message.contains("safety") || message.contains("blocked") {
        FailureKind::SafetyBlock
    } else if stderr.trim().is_empty() && stdout.trim().is_empty() {
        FailureKind::EmptyOutput
    } else {
        FailureKind::ProcessError
    }
}

fn update_engine_health(report: &EngineCallReport) {
    let mut store = load_cli_health();
    let entry = store.engines.entry(report.engine.clone()).or_default();
    let total_before = entry.successes + entry.failures;
    let next_total = total_before + 1;
    entry.avg_latency_ms = if total_before == 0 {
        report.duration_ms as f64
    } else {
        ((entry.avg_latency_ms * total_before as f64) + report.duration_ms as f64) / next_total as f64
    };
    entry.last_updated_at = now_secs();

    if report.success {
        entry.successes += 1;
        entry.consecutive_failures = 0;
        entry.last_error_kind = None;
        entry.cooldown_until = None;
        entry.status = if entry.failures > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
    } else {
        entry.failures += 1;
        entry.consecutive_failures += 1;
        entry.last_error_kind = report.error_kind.clone();
        let backoff = match entry.consecutive_failures {
            0 | 1 => 30,
            2 => 60,
            3 => 120,
            _ => 300,
        };
        entry.cooldown_until = Some(now_secs() + backoff);
        entry.status = if entry.consecutive_failures >= 3
            || matches!(report.error_kind.as_deref(), Some("auth_error" | "model_not_found"))
        {
            HealthStatus::Down
        } else {
            HealthStatus::Degraded
        };
    }
    save_cli_health(&store);
}

fn engine_in_cooldown(engine: Engine) -> Option<String> {
    let state = engine_state(engine);
    match state.cooldown_until {
        Some(until) if until > now_secs() => Some(format!("{} in cooldown until {}", engine.label(), until)),
        _ => None,
    }
}

async fn run_cli_with_stdin(cli: &str, args: &[&str], stdin_data: &str, timeout: Duration) -> Result<CliRunOutcome> {
    use tokio::io::AsyncWriteExt;

    let started = Instant::now();
    let mut cmd = if cfg!(windows) && cli.ends_with(".cmd") {
        let mut c = Command::new("cmd");
        c.arg("/c").arg(cli);
        for arg in args {
            c.arg(arg);
        }
        c
    } else {
        let mut c = Command::new(cli);
        for arg in args {
            c.arg(arg);
        }
        c
    };

    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("CLI 启动失败 '{}': {}", cli, e))?;

    let child_id = child.id();
    if let Some(mut stdin) = child.stdin.take() {
        let data = stdin_data.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(data.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });
    }

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(CliRunOutcome::Completed {
            status_success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            duration_ms: started.elapsed().as_millis() as u64,
        }),
        Ok(Err(e)) => Err(anyhow::anyhow!("执行失败: {}", e)),
        Err(_) => {
            if let Some(pid) = child_id {
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .output()
                    .await;
            }
            warn!("CLI 命令超时 ({}s)，已终止子进程: {}", timeout.as_secs(), cli);
            Ok(CliRunOutcome::Timeout {
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
    }
}

async fn call_engine_detailed(engine: Engine, prompt: &str, cfg: &OrchestratorConfig, phase: &str) -> EngineCallReport {
    let status_before = match engine_state(engine).status {
        HealthStatus::Healthy => "healthy",
        HealthStatus::Degraded => "degraded",
        HealthStatus::Down => "down",
    }
    .to_string();

    if let Some(report) = cached_success_report(engine, phase, prompt, cfg, &status_before) {
        return report;
    }

    if let Some(reason) = engine_in_cooldown(engine) {
        let report = EngineCallReport {
            engine: engine.label().to_string(),
            phase: phase.to_string(),
            success: false,
            status_before: status_before.clone(),
            status_after: "down".to_string(),
            duration_ms: 0,
            output: None,
            output_excerpt: None,
            error_kind: Some(FailureKind::ProcessError.as_str().to_string()),
            error_message: Some(reason),
            exit_code: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            skipped: true,
            cache_hit: false,
        };
        update_engine_health(&report);
        return report;
    }

    let cli_path = |base: &str| -> String {
        let p = base.replace('/', "\\");
        if cfg!(windows) && !p.ends_with(".cmd") && !p.ends_with(".exe") && !p.contains('\\') {
            format!("{}.cmd", p)
        } else {
            p
        }
    };

    let (cli, args): (String, Vec<String>) = match engine {
        Engine::Claude => (
            cli_path(&cfg.claude_cli),
            vec![
                "-p".to_string(),
                "-".to_string(),
                "--output-format".to_string(),
                "text".to_string(),
                "--model".to_string(),
                cfg.claude_model.clone(),
                "--no-session-persistence".to_string(),
                "--disable-slash-commands".to_string(),
            ],
        ),
        Engine::OpenAi => (
            cli_path(&cfg.codex_cli),
            vec![
                "exec".to_string(),
                "-".to_string(),
                "--skip-git-repo-check".to_string(),
                "-m".to_string(),
                cfg.openai_model.clone(),
            ],
        ),
        Engine::Gemini => (cli_path(&cfg.gemini_cli), vec!["-p".to_string(), "-".to_string()]),
    };

    let _guard = engine_lock(engine).lock().await;
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let report = match run_cli_with_stdin(&cli, &args_ref, prompt, cfg.timeout).await {
        Ok(CliRunOutcome::Completed {
            status_success,
            stdout,
            stderr,
            exit_code,
            duration_ms,
        }) => {
            let normalized_stdout = normalize_output(engine, &stdout);
            if status_success && !normalized_stdout.is_empty() {
                persist_success_cache(engine, phase, prompt, cfg, &normalized_stdout, duration_ms);
                EngineCallReport {
                    engine: engine.label().to_string(),
                    phase: phase.to_string(),
                    success: true,
                    status_before: status_before.clone(),
                    status_after: "healthy".to_string(),
                    duration_ms,
                    output: Some(normalized_stdout.clone()),
                    output_excerpt: Some(excerpt(&normalized_stdout, 320)),
                    error_kind: None,
                    error_message: None,
                    exit_code,
                    stdout_excerpt: Some(excerpt(&stdout, 240)),
                    stderr_excerpt: if stderr.trim().is_empty() {
                        None
                    } else {
                        Some(excerpt(&stderr, 240))
                    },
                    skipped: false,
                    cache_hit: false,
                }
            } else {
                let failure_kind = if status_success {
                    FailureKind::EmptyOutput
                } else {
                    classify_failure(&stderr, &stdout)
                };
                let stderr_trimmed = stderr.trim();
                let stdout_trimmed = stdout.trim();
                let error_message = if !stderr_trimmed.is_empty() {
                    stderr_trimmed.to_string()
                } else if !stdout_trimmed.is_empty() {
                    format!(
                        "stderr empty; exit_code={:?}; stdout_excerpt={}",
                        exit_code,
                        excerpt(stdout_trimmed, 200)
                    )
                } else {
                    format!("stderr empty; stdout empty; exit_code={:?}", exit_code)
                };
                EngineCallReport {
                    engine: engine.label().to_string(),
                    phase: phase.to_string(),
                    success: false,
                    status_before: status_before.clone(),
                    status_after: "degraded".to_string(),
                    duration_ms,
                    output: None,
                    output_excerpt: None,
                    error_kind: Some(failure_kind.as_str().to_string()),
                    error_message: Some(error_message),
                    exit_code,
                    stdout_excerpt: if stdout_trimmed.is_empty() {
                        None
                    } else {
                        Some(excerpt(stdout_trimmed, 240))
                    },
                    stderr_excerpt: if stderr_trimmed.is_empty() {
                        None
                    } else {
                        Some(excerpt(stderr_trimmed, 240))
                    },
                    skipped: false,
                    cache_hit: false,
                }
            }
        }
        Ok(CliRunOutcome::Timeout { duration_ms }) => EngineCallReport {
            engine: engine.label().to_string(),
            phase: phase.to_string(),
            success: false,
            status_before: status_before.clone(),
            status_after: "degraded".to_string(),
            duration_ms,
            output: None,
            output_excerpt: None,
            error_kind: Some(FailureKind::Timeout.as_str().to_string()),
            error_message: Some(format!("请求超时 ({}s)", cfg.timeout.as_secs())),
            exit_code: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            skipped: false,
            cache_hit: false,
        },
        Err(e) => EngineCallReport {
            engine: engine.label().to_string(),
            phase: phase.to_string(),
            success: false,
            status_before: status_before.clone(),
            status_after: "degraded".to_string(),
            duration_ms: 0,
            output: None,
            output_excerpt: None,
            error_kind: Some(FailureKind::ProcessError.as_str().to_string()),
            error_message: Some(e.to_string()),
            exit_code: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            skipped: false,
            cache_hit: false,
        },
    };

    update_engine_health(&report);
    report
}

async fn call_engines_parallel(
    tasks: &[(Engine, String)],
    cfg: &OrchestratorConfig,
    phase: &str,
) -> Vec<EngineCallReport> {
    let mut set = tokio::task::JoinSet::new();
    for (engine, prompt) in tasks {
        let engine = *engine;
        let prompt = prompt.clone();
        let cfg = cfg.clone();
        let phase = phase.to_string();
        set.spawn(async move { call_engine_detailed(engine, &prompt, &cfg, &phase).await });
    }

    let mut results = Vec::new();
    while let Some(Ok(report)) = set.join_next().await {
        results.push(report);
    }
    results
}

#[derive(Debug, Default)]
struct PhaseBatch {
    reports: Vec<EngineCallReport>,
    pending_engines: Vec<String>,
}

fn phase_quorum_target(phase: &str, total: usize) -> usize {
    match total {
        0 => 0,
        1 => 1,
        2 => {
            if matches!(phase, "proposal" | "dispute_review" | "review" | "arbiter") {
                1
            } else {
                2
            }
        }
        _ => 2,
    }
}

fn phase_supports_background_completion(phase: &str) -> bool {
    matches!(phase, "proposal" | "dispute_review" | "review" | "arbiter")
}

fn sorted_strings<I>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut values: Vec<String> = items.into_iter().collect();
    values.sort();
    values.dedup();
    values
}

fn pending_snapshot(pending_engines: &HashSet<String>) -> Vec<String> {
    sorted_strings(pending_engines.iter().cloned())
}

async fn call_engines_windowed(tasks: &[(Engine, String)], cfg: &OrchestratorConfig, phase: &str) -> PhaseBatch {
    if tasks.is_empty() {
        return PhaseBatch::default();
    }

    if !phase_supports_background_completion(phase) {
        return PhaseBatch {
            reports: call_engines_parallel(tasks, cfg, phase).await,
            pending_engines: Vec::new(),
        };
    }

    let quorum = phase_quorum_target(phase, tasks.len());
    let deadline = tokio::time::Instant::now() + cfg.phase_window;
    let grace_window = Duration::from_secs(phase_grace_window_secs());
    let (tx, mut rx) = mpsc::unbounded_channel();

    for (engine, prompt) in tasks {
        let engine = *engine;
        let prompt = prompt.clone();
        let cfg = cfg.clone();
        let phase = phase.to_string();
        let tx = tx.clone();
        tokio::spawn(async move {
            let report = call_engine_detailed(engine, &prompt, &cfg, &phase).await;
            if tx.send(report.clone()).is_err() {
                append_trace(&json!({
                    "timestamp": now_millis(),
                    "phase": phase,
                    "engine": report.engine,
                    "late_completion": true,
                    "success": report.success,
                    "cache_hit": report.cache_hit,
                    "duration_ms": report.duration_ms,
                    "error_kind": report.error_kind,
                }));
            }
        });
    }
    drop(tx);

    let mut reports: Vec<EngineCallReport> = Vec::new();
    let mut quorum_started_at = None;
    while reports.len() < tasks.len() {
        let successful = reports.iter().filter(|report| report.success).count();
        if successful >= quorum && quorum_started_at.is_none() {
            quorum_started_at = Some(tokio::time::Instant::now());
        }

        let next_deadline = quorum_started_at
            .map(|started| std::cmp::min(deadline, started + grace_window))
            .unwrap_or(deadline);
        if tokio::time::Instant::now() >= next_deadline {
            break;
        }

        match tokio::time::timeout_at(next_deadline, rx.recv()).await {
            Ok(Some(report)) => reports.push(report),
            Ok(None) | Err(_) => break,
        }
    }

    let seen: HashSet<String> = reports.iter().map(|report| report.engine.clone()).collect();
    let pending_engines = sorted_strings(
        tasks
            .iter()
            .map(|(engine, _)| engine.label().to_string())
            .filter(|engine| !seen.contains(engine)),
    );

    PhaseBatch {
        reports,
        pending_engines,
    }
}

fn parse_engines(ctx: &ExecutionContext) -> Vec<Engine> {
    let defaults = vec![Engine::Claude, Engine::OpenAi, Engine::Gemini];
    let arr = match ctx.context.get("engines").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return defaults,
    };
    let engines: Vec<Engine> = arr.iter().filter_map(|v| Engine::from_label(v.as_str()?)).collect();
    if engines.is_empty() {
        defaults
    } else {
        engines
    }
}

fn is_high_risk(task: &str, ctx: &ExecutionContext) -> bool {
    match ctx.context.get("risk_level").and_then(|v| v.as_str()) {
        Some("high") => return true,
        Some("low" | "medium") => return false,
        _ => {}
    }
    let lowered = task.to_lowercase();
    [
        "重构",
        "迁移",
        "删除",
        "生产",
        "合规",
        "合同",
        "架构",
        "security",
        "audit",
        "refactor",
        "migrate",
        "delete",
        "compliance",
        "critical",
        "contract",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn force_triple_execute(ctx: &ExecutionContext) -> bool {
    ctx.context
        .get("force_triple_execute")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn infer_target_path(task: &str, workflow: &str) -> &'static str {
    let lowered = task.to_lowercase();
    if workflow.contains("review") || lowered.contains("审查") || lowered.contains("review") {
        "code_review"
    } else if lowered.contains("合同")
        || lowered.contains("文档")
        || lowered.contains("long")
        || lowered.contains("context")
        || lowered.contains("report")
    {
        "long_context_analysis"
    } else if lowered.contains("数据")
        || lowered.contains("csv")
        || lowered.contains("sql")
        || lowered.contains("algorithm")
        || lowered.contains("math")
        || lowered.contains("leetcode")
    {
        "data_analysis"
    } else if lowered.contains("代码")
        || lowered.contains("重构")
        || lowered.contains("bug")
        || lowered.contains("fix")
        || lowered.contains("refactor")
        || lowered.contains("debug")
    {
        "code_refactor"
    } else if workflow == "research" {
        "research_synthesis"
    } else {
        "generic_problem_solving"
    }
}

fn preferred_engines(task: &str, workflow: &str) -> Vec<Engine> {
    match infer_target_path(task, workflow) {
        "data_analysis" => vec![Engine::OpenAi, Engine::Claude, Engine::Gemini],
        "long_context_analysis" | "research_synthesis" => vec![Engine::Gemini, Engine::Claude, Engine::OpenAi],
        _ => vec![Engine::Claude, Engine::OpenAi, Engine::Gemini],
    }
}

fn select_primary_engine(task: &str, workflow: &str, candidates: &[Engine]) -> Engine {
    preferred_engines(task, workflow)
        .into_iter()
        .find(|engine| candidates.contains(engine))
        .unwrap_or_else(|| candidates.first().copied().unwrap_or(Engine::Claude))
}

#[derive(Debug, Clone, Serialize)]
struct Proposal {
    engine: String,
    plan_id: String,
    target_path: String,
    primary_engine: String,
    review_engines: Vec<String>,
    execution_mode: String,
    key_risks: Vec<String>,
    execution_order: Vec<String>,
    verify_method: String,
    summary: String,
    raw: String,
}

fn parse_keyed_lines(raw: &str) -> HashMap<String, String> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_uppercase(), value.trim().to_string()))
        })
        .collect()
}

fn parse_proposal_output(engine: Engine, raw: &str, task: &str, workflow: &str) -> Proposal {
    let fields = parse_keyed_lines(raw);
    let inferred_primary = select_primary_engine(task, workflow, &[Engine::Claude, Engine::OpenAi, Engine::Gemini])
        .label()
        .to_string();
    Proposal {
        engine: engine.label().to_string(),
        plan_id: engine.label().to_string(),
        target_path: fields
            .get("TARGET_PATH")
            .cloned()
            .unwrap_or_else(|| infer_target_path(task, workflow).to_string()),
        primary_engine: fields.get("PRIMARY_ENGINE").cloned().unwrap_or(inferred_primary),
        review_engines: fields
            .get("REVIEW_ENGINES")
            .map(|v| split_compact_list(v, &['|', ',']))
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec!["claude".into(), "openai".into(), "gemini".into()]),
        execution_mode: fields
            .get("EXECUTION_MODE")
            .cloned()
            .unwrap_or_else(|| "primary_plus_review".to_string()),
        key_risks: fields
            .get("KEY_RISKS")
            .map(|v| split_compact_list(v, &['|', ',']))
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec!["未明确风险".to_string()]),
        execution_order: fields
            .get("EXECUTION_ORDER")
            .map(|v| split_compact_list(v, &['>', '|']))
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec!["discuss".to_string(), "execute".to_string(), "review".to_string()]),
        verify_method: fields
            .get("VERIFY")
            .cloned()
            .unwrap_or_else(|| "通过结果质量和可验证输出检查".to_string()),
        summary: fields.get("SUMMARY").cloned().unwrap_or_else(|| excerpt(raw, 180)),
        raw: raw.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
struct ReviewDecision {
    engine: String,
    stance: String,
    preferred_plan_id: String,
    rationale: String,
    verify_method: String,
}

fn parse_review_output(engine: Engine, raw: &str, fallback_plan: &str) -> ReviewDecision {
    let fields = parse_keyed_lines(raw);
    ReviewDecision {
        engine: engine.label().to_string(),
        stance: fields
            .get("STANCE")
            .cloned()
            .unwrap_or_else(|| "conditional".to_string()),
        preferred_plan_id: fields
            .get("PREFERRED_PLAN_ID")
            .cloned()
            .unwrap_or_else(|| fallback_plan.to_string()),
        rationale: fields.get("RATIONALE").cloned().unwrap_or_else(|| excerpt(raw, 180)),
        verify_method: fields
            .get("VERIFY")
            .cloned()
            .unwrap_or_else(|| "通过输出质量和可验证产物复核".to_string()),
    }
}

fn proposals_agree(proposals: &[Proposal]) -> bool {
    if proposals.len() < 2 {
        return true;
    }
    let target = normalize_token(&proposals[0].target_path);
    let primary = normalize_token(&proposals[0].primary_engine);
    let order = proposals[0]
        .execution_order
        .iter()
        .map(|s| normalize_token(s))
        .collect::<Vec<_>>()
        .join(">");
    let base_risks = proposals[0]
        .key_risks
        .iter()
        .map(|risk| normalize_token(risk))
        .collect::<Vec<_>>();

    proposals.iter().skip(1).all(|proposal| {
        let same_target = normalize_token(&proposal.target_path) == target;
        let same_primary = normalize_token(&proposal.primary_engine) == primary;
        let same_order = proposal
            .execution_order
            .iter()
            .map(|s| normalize_token(s))
            .collect::<Vec<_>>()
            .join(">")
            == order;
        let proposal_risks = proposal
            .key_risks
            .iter()
            .map(|risk| normalize_token(risk))
            .collect::<Vec<_>>();
        let shared_risk = base_risks.iter().any(|risk| proposal_risks.contains(risk));
        same_target && same_primary && same_order && shared_risk
    })
}

fn disputed_points(proposals: &[Proposal]) -> Vec<String> {
    let mut disputes = Vec::new();
    if proposals.len() < 2 {
        return disputes;
    }
    let first = &proposals[0];
    if proposals
        .iter()
        .any(|p| normalize_token(&p.target_path) != normalize_token(&first.target_path))
    {
        disputes.push("目标路径不一致".to_string());
    }
    if proposals
        .iter()
        .any(|p| normalize_token(&p.primary_engine) != normalize_token(&first.primary_engine))
    {
        disputes.push("主执行模型选择不一致".to_string());
    }
    let first_order = first
        .execution_order
        .iter()
        .map(|s| normalize_token(s))
        .collect::<Vec<_>>();
    if proposals
        .iter()
        .any(|p| p.execution_order.iter().map(|s| normalize_token(s)).collect::<Vec<_>>() != first_order)
    {
        disputes.push("执行顺序不一致".to_string());
    }
    if proposals.iter().any(|p| {
        !p.key_risks.iter().any(|risk| {
            first
                .key_risks
                .iter()
                .any(|left| normalize_token(left) == normalize_token(risk))
        })
    }) {
        disputes.push("关键风险判断不一致".to_string());
    }
    disputes
}

fn choose_plan_from_reviews(reviews: &[ReviewDecision], fallback: &str) -> (String, String) {
    let mut votes: BTreeMap<String, usize> = BTreeMap::new();
    for review in reviews {
        *votes.entry(review.preferred_plan_id.clone()).or_default() += 1;
    }
    let selected = votes
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(plan_id, _)| plan_id.clone())
        .unwrap_or_else(|| fallback.to_string());
    let reason = if votes.is_empty() {
        format!("无有效复审投票，回退到 {}", fallback)
    } else {
        let counts = votes
            .iter()
            .map(|(plan, count)| format!("{}={}票", plan, count))
            .collect::<Vec<_>>()
            .join(", ");
        format!("复审投票结果：{}", counts)
    };
    (selected, reason)
}

fn proposal_prompt(task: &str, workflow: &str, risk_level: &str) -> String {
    format!(
        "你正在参与三模型协作的方案讨论阶段。\n\
请只输出以下字段，每行一项，不要添加任何额外说明：\n\
TARGET_PATH: 从 [code_refactor, data_analysis, long_context_analysis, research_synthesis, code_review, generic_problem_solving] 里选一个\n\
PRIMARY_ENGINE: 从 [claude, openai, gemini] 里选一个\n\
REVIEW_ENGINES: 用 | 分隔列出另外两个引擎\n\
EXECUTION_MODE: 从 [single_primary, primary_plus_review, triple_execute] 里选一个\n\
KEY_RISKS: 用 | 分隔列出 1-3 个关键风险短语\n\
EXECUTION_ORDER: 用 > 分隔列出 2-4 个步骤\n\
VERIFY: 一句话说明如何验证结果\n\
SUMMARY: 一句话概括方案\n\
\n\
WORKFLOW: {}\n\
RISK_LEVEL: {}\n\
TASK: {}\n",
        workflow,
        risk_level,
        task
    )
}

fn dispute_review_prompt(task: &str, proposals: &[Proposal]) -> String {
    let mut body = String::from("你正在参与第二轮争议复审。请先阅读候选方案，再只输出固定字段。\n候选方案：\n");
    for proposal in proposals {
        body.push_str(&format!(
            "- PLAN_ID={} TARGET_PATH={} PRIMARY_ENGINE={} RISKS={} ORDER={} SUMMARY={}\n",
            proposal.plan_id,
            proposal.target_path,
            proposal.primary_engine,
            proposal.key_risks.join(" | "),
            proposal.execution_order.join(" > "),
            proposal.summary
        ));
    }
    body.push_str(
        "\n请只输出以下字段，每行一项，不要添加额外说明：\n\
STANCE: support | oppose | conditional\n\
PREFERRED_PLAN_ID: 从候选 plan_id 中选一个\n\
RATIONALE: 一句话说明原因\n\
VERIFY: 一句话说明最终怎么验证\n",
    );
    body.push_str(&format!("\nTASK: {}\n", task));
    body
}

fn execution_prompt(task: &str, proposal: &Proposal, workflow: &str) -> String {
    format!(
        "你现在进入执行阶段，不再继续讨论。\n\
WORKFLOW: {}\n\
SELECTED_PLAN_ID: {}\n\
TARGET_PATH: {}\n\
PRIMARY_ENGINE: {}\n\
KEY_RISKS: {}\n\
EXECUTION_ORDER: {}\n\
VERIFY: {}\n\
TASK: {}\n\
\n请直接给出最终执行结果。如果是代码任务，给出完整可落地实现；如果是分析任务，给出清晰结论与依据。",
        workflow,
        proposal.plan_id,
        proposal.target_path,
        proposal.primary_engine,
        proposal.key_risks.join(" | "),
        proposal.execution_order.join(" > "),
        proposal.verify_method,
        task
    )
}

fn review_execution_prompt(task: &str, proposal: &Proposal, primary_output: &str) -> String {
    format!(
        "你是执行结果审核者。请只输出以下字段，每行一项：\n\
VERDICT: approve | revise | reject\n\
RISK_LEVEL: low | medium | high\n\
ISSUES: 用 | 分隔问题点，没有问题写 none\n\
SUGGESTED_FIX: 一句话给出修正建议\n\
\nTASK: {}\n\
PLAN: {}\n\
PRIMARY_OUTPUT:\n{}\n",
        task, proposal.summary, primary_output
    )
}

fn arbitration_prompt(task: &str, execution_outputs: &BTreeMap<String, String>) -> String {
    let mut prompt = String::from(
        "你正在做结果仲裁。请阅读各引擎执行结果，并只输出以下字段：\n\
WINNER_ENGINE: claude | openai | gemini\n\
RATIONALE: 一句话说明为什么它更好\n\
VERIFY: 一句话说明如何验证胜出结果\n\n",
    );
    prompt.push_str(&format!("TASK: {}\n", task));
    for (engine, output) in execution_outputs {
        prompt.push_str(&format!("ENGINE={} OUTPUT=\n{}\n\n", engine, output));
    }
    prompt
}

fn parse_winner(raw: &str, fallback: &str) -> (String, String, String) {
    let fields = parse_keyed_lines(raw);
    (
        fields
            .get("WINNER_ENGINE")
            .cloned()
            .unwrap_or_else(|| fallback.to_string()),
        fields.get("RATIONALE").cloned().unwrap_or_else(|| excerpt(raw, 180)),
        fields
            .get("VERIFY")
            .cloned()
            .unwrap_or_else(|| "通过输出质量和验证结果判断".to_string()),
    )
}

#[derive(Debug, Clone, Serialize)]
struct WorkflowOutcome {
    workflow: String,
    task: String,
    participants: Vec<String>,
    pending_engines: Vec<String>,
    phase_trace: Vec<Value>,
    proposal_summary: Vec<Proposal>,
    consensus_status: String,
    disputed_points: Vec<String>,
    selected_plan_id: String,
    execution_mode: String,
    arbiter_reason: Option<String>,
    participants_status: Vec<EngineCallReport>,
    degraded: bool,
    failures: Vec<Value>,
    final_solution: Option<String>,
    solutions: BTreeMap<String, String>,
    review_feedback: BTreeMap<String, String>,
    engines_used: Vec<String>,
}

fn append_trace(record: &Value) {
    let _guard = file_lock().lock().unwrap_or_else(|e| e.into_inner());
    let path = orchestration_trace_path();
    let mut file = match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => file,
        Err(_) => return,
    };
    let _ = writeln!(file, "{}", record);
}

fn reports_to_failures(reports: &[EngineCallReport]) -> Vec<Value> {
    reports
        .iter()
        .filter(|report| !report.success)
        .map(|report| {
            json!({
                "engine": report.engine,
                "phase": report.phase,
                "kind": report.error_kind,
                "message": report.error_message,
                "exit_code": report.exit_code,
                "stderr_excerpt": report.stderr_excerpt,
                "stdout_excerpt": report.stdout_excerpt,
                "skipped": report.skipped,
            })
        })
        .collect()
}

async fn run_collaboration_workflow(
    workflow: &str,
    task: &str,
    selected_engines: Vec<Engine>,
    risk_level: &str,
    force_triple: bool,
) -> Value {
    let cfg = OrchestratorConfig::from_env();
    let mut phase_trace = Vec::new();
    let mut all_reports = Vec::new();
    let mut pending_engines = HashSet::new();

    let proposal_tasks: Vec<_> = selected_engines
        .iter()
        .map(|engine| (*engine, proposal_prompt(task, workflow, risk_level)))
        .collect();
    let proposal_batch = call_engines_windowed(&proposal_tasks, &cfg, "proposal").await;
    let proposal_reports = proposal_batch.reports;
    let proposal_pending = proposal_batch.pending_engines;
    pending_engines.extend(proposal_pending.iter().cloned());
    let proposal_failures = reports_to_failures(&proposal_reports);
    let mut proposals = Vec::new();
    for report in &proposal_reports {
        if report.success {
            if let Some(output) = &report.output {
                if let Some(engine) = Engine::from_label(&report.engine) {
                    proposals.push(parse_proposal_output(engine, output, task, workflow));
                }
            }
        }
    }
    phase_trace.push(json!({
        "phase": "proposal",
        "completed_at": now_millis(),
        "success_count": proposals.len(),
        "failure_count": proposal_failures.len(),
        "pending_engines": proposal_pending,
    }));
    all_reports.extend(proposal_reports.clone());

    let proposal_quorum = phase_quorum_target("proposal", selected_engines.len());
    let mut consensus_status = if proposals.len() < proposal_quorum {
        "unresolved".to_string()
    } else if proposals_agree(&proposals) {
        "agreed".to_string()
    } else {
        "disputed".to_string()
    };
    let mut disputed = if consensus_status == "agreed" {
        Vec::new()
    } else {
        disputed_points(&proposals)
    };
    let mut selected_plan_id = proposals
        .first()
        .map(|proposal| proposal.plan_id.clone())
        .unwrap_or_else(|| {
            select_primary_engine(task, workflow, &selected_engines)
                .label()
                .to_string()
        });
    let mut arbiter_reason = None;

    if consensus_status == "disputed" && proposals.len() >= 2 {
        let review_tasks: Vec<_> = selected_engines
            .iter()
            .map(|engine| (*engine, dispute_review_prompt(task, &proposals)))
            .collect();
        let review_batch = call_engines_windowed(&review_tasks, &cfg, "dispute_review").await;
        let review_reports = review_batch.reports;
        let review_pending = review_batch.pending_engines;
        pending_engines.extend(review_pending.iter().cloned());
        let successful_reviews: Vec<ReviewDecision> = review_reports
            .iter()
            .filter(|report| report.success)
            .filter_map(|report| {
                let output = report.output.as_ref()?;
                let engine = Engine::from_label(&report.engine)?;
                Some(parse_review_output(engine, output, &selected_plan_id))
            })
            .collect();
        let (review_selected, reason) = choose_plan_from_reviews(&successful_reviews, &selected_plan_id);
        selected_plan_id = review_selected;
        arbiter_reason = Some(reason);
        consensus_status = if successful_reviews.is_empty() {
            "unresolved".to_string()
        } else if successful_reviews
            .iter()
            .all(|review| review.preferred_plan_id == selected_plan_id)
        {
            "agreed".to_string()
        } else if successful_reviews
            .iter()
            .any(|review| review.preferred_plan_id == selected_plan_id)
        {
            "disputed".to_string()
        } else {
            "unresolved".to_string()
        };
        phase_trace.push(json!({
            "phase": "dispute_review",
            "completed_at": now_millis(),
            "review_count": successful_reviews.len(),
            "selected_plan_id": selected_plan_id,
            "pending_engines": review_pending,
        }));
        all_reports.extend(review_reports);
    }

    let selected_proposal = proposals
        .iter()
        .find(|proposal| proposal.plan_id == selected_plan_id)
        .cloned()
        .unwrap_or_else(|| Proposal {
            engine: selected_plan_id.clone(),
            plan_id: selected_plan_id.clone(),
            target_path: infer_target_path(task, workflow).to_string(),
            primary_engine: select_primary_engine(task, workflow, &selected_engines)
                .label()
                .to_string(),
            review_engines: selected_engines
                .iter()
                .map(|engine| engine.label().to_string())
                .filter(|engine| engine != &selected_plan_id)
                .collect(),
            execution_mode: "primary_plus_review".to_string(),
            key_risks: vec!["回退到任务默认方案".to_string()],
            execution_order: vec!["execute".to_string(), "review".to_string()],
            verify_method: "通过输出质量与结果验证".to_string(),
            summary: "使用任务默认调度方案".to_string(),
            raw: String::new(),
        });

    let mut execution_mode = if selected_engines.len() <= 1 {
        "single_primary".to_string()
    } else if force_triple || risk_level == "high" || consensus_status == "unresolved" {
        "triple_execute".to_string()
    } else {
        "primary_plus_review".to_string()
    };
    if selected_proposal.execution_mode == "triple_execute"
        && !execution_mode.eq("triple_execute")
        && risk_level != "low"
    {
        execution_mode = "triple_execute".to_string();
    }

    let mut solutions = BTreeMap::new();
    let mut review_feedback = BTreeMap::new();
    let mut final_solution = None;

    if execution_mode == "triple_execute" {
        let exec_tasks: Vec<_> = selected_engines
            .iter()
            .map(|engine| (*engine, execution_prompt(task, &selected_proposal, workflow)))
            .collect();
        let exec_reports = call_engines_parallel(&exec_tasks, &cfg, "execute").await;
        for report in &exec_reports {
            if report.success {
                if let Some(output) = &report.output {
                    solutions.insert(report.engine.clone(), output.clone());
                }
            }
        }
        let success_engines: Vec<Engine> = exec_reports
            .iter()
            .filter_map(|report| {
                if report.success {
                    Engine::from_label(&report.engine)
                } else {
                    None
                }
            })
            .collect();
        let winner_engine = if solutions.len() <= 1 {
            solutions
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| selected_proposal.primary_engine.clone())
        } else {
            let arbitration_tasks: Vec<_> = success_engines
                .iter()
                .map(|engine| (*engine, arbitration_prompt(task, &solutions)))
                .collect();
            let arbitration_batch = call_engines_windowed(&arbitration_tasks, &cfg, "arbiter").await;
            let arbitration_reports = arbitration_batch.reports;
            let arbitration_pending = arbitration_batch.pending_engines;
            pending_engines.extend(arbitration_pending.iter().cloned());
            let mut votes: BTreeMap<String, usize> = BTreeMap::new();
            let mut reasons = Vec::new();
            for report in &arbitration_reports {
                if report.success {
                    if let Some(output) = &report.output {
                        let (winner, rationale, verify) = parse_winner(output, &selected_proposal.primary_engine);
                        *votes.entry(winner.clone()).or_default() += 1;
                        reasons.push(format!("{}: {} / {}", report.engine, rationale, verify));
                    }
                }
            }
            all_reports.extend(arbitration_reports);
            let winner = votes
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(engine, _)| engine.clone())
                .unwrap_or_else(|| selected_proposal.primary_engine.clone());
            arbiter_reason = Some(if reasons.is_empty() {
                format!("三套结果已执行，按默认主模型 {} 选中", selected_proposal.primary_engine)
            } else {
                format!("结果仲裁：{}", reasons.join(" | "))
            });
            winner
        };
        final_solution = solutions.get(&winner_engine).cloned();
        all_reports.extend(exec_reports);
        phase_trace.push(json!({
            "phase": "execution_or_arbiter",
            "completed_at": now_millis(),
            "mode": execution_mode,
            "winner_engine": winner_engine,
            "pending_engines": pending_snapshot(&pending_engines),
        }));
    } else {
        let primary_engine = Engine::from_label(&selected_proposal.primary_engine)
            .unwrap_or_else(|| select_primary_engine(task, workflow, &selected_engines));
        let primary_prompt = execution_prompt(task, &selected_proposal, workflow);
        let primary_report = call_engine_detailed(primary_engine, &primary_prompt, &cfg, "execute").await;
        if primary_report.success {
            if let Some(output) = &primary_report.output {
                solutions.insert(primary_report.engine.clone(), output.clone());
                final_solution = Some(output.clone());
            }
        } else {
            let fallback_engine = selected_engines
                .iter()
                .copied()
                .find(|engine| engine.label() != primary_engine.label())
                .unwrap_or(primary_engine);
            let fallback_report = call_engine_detailed(fallback_engine, &primary_prompt, &cfg, "execute").await;
            if fallback_report.success {
                if let Some(output) = &fallback_report.output {
                    solutions.insert(fallback_report.engine.clone(), output.clone());
                    final_solution = Some(output.clone());
                    arbiter_reason = Some(format!(
                        "主执行 {} 失败，已回退到 {}",
                        primary_engine.label(),
                        fallback_engine.label()
                    ));
                }
            }
            all_reports.push(primary_report.clone());
            all_reports.push(fallback_report.clone());
        }
        if !all_reports.iter().any(|report| report.phase == "execute") {
            all_reports.push(primary_report.clone());
        }

        let review_source = final_solution.clone().unwrap_or_default();
        let review_tasks: Vec<_> = selected_engines
            .iter()
            .filter(|engine| engine.label() != selected_proposal.primary_engine)
            .map(|engine| {
                (
                    *engine,
                    review_execution_prompt(task, &selected_proposal, &review_source),
                )
            })
            .collect();
        let review_batch = call_engines_windowed(&review_tasks, &cfg, "review").await;
        let review_reports = review_batch.reports;
        let review_pending = review_batch.pending_engines;
        pending_engines.extend(review_pending.iter().cloned());
        for report in &review_reports {
            if report.success {
                if let Some(output) = &report.output {
                    review_feedback.insert(report.engine.clone(), output.clone());
                }
            }
        }
        all_reports.extend(review_reports);
        phase_trace.push(json!({
            "phase": "execution_or_arbiter",
            "completed_at": now_millis(),
            "mode": execution_mode,
            "primary_engine": selected_proposal.primary_engine,
            "pending_engines": review_pending,
        }));
    }

    if disputed.is_empty() && consensus_status != "agreed" {
        disputed = vec!["方案仍未完全收敛".to_string()];
    }

    let degraded = !pending_engines.is_empty() || all_reports.iter().any(|report| !report.success || report.skipped);
    let outcome = WorkflowOutcome {
        workflow: workflow.to_string(),
        task: task.to_string(),
        participants: selected_engines
            .iter()
            .map(|engine| engine.label().to_string())
            .collect(),
        pending_engines: pending_snapshot(&pending_engines),
        phase_trace,
        proposal_summary: proposals.clone(),
        consensus_status,
        disputed_points: disputed,
        selected_plan_id: selected_plan_id.clone(),
        execution_mode: execution_mode.clone(),
        arbiter_reason,
        participants_status: all_reports.clone(),
        degraded,
        failures: reports_to_failures(&all_reports),
        final_solution: final_solution.clone(),
        solutions: solutions.clone(),
        review_feedback: review_feedback.clone(),
        engines_used: selected_engines
            .iter()
            .map(|engine| engine.label().to_string())
            .collect(),
    };

    append_trace(&json!({
        "task_id": short_id(),
        "timestamp": now_millis(),
        "workflow": workflow,
        "selected_plan_id": selected_plan_id,
        "execution_mode": execution_mode,
        "consensus_status": outcome.consensus_status,
        "degraded": degraded,
        "participants": outcome.participants,
        "pending_engines": outcome.pending_engines,
    }));

    serde_json::to_value(outcome).unwrap_or_else(|_| json!({"error": "failed to serialize orchestration outcome"}))
}

const MCP_WAIT_CAP: u64 = 55;

fn default_wait_secs(workflow: &str) -> u64 {
    let is_mcp = std::env::var("AION_MCP_MODE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let configured = std::env::var("AION_ORCH_WAIT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    let raw = configured.unwrap_or_else(|| match workflow {
        "code_generate" | "long_context" => 50,
        "cross_review" | "serial_optimize" => 55,
        "parallel_solve" | "triple_vote" | "triangle_review" | "smart_collaborate" | "research" => 55,
        _ => 50,
    });

    if is_mcp && raw > MCP_WAIT_CAP {
        info!("MCP mode: capping wait from {}s to {}s", raw, MCP_WAIT_CAP);
        MCP_WAIT_CAP
    } else {
        raw
    }
}

async fn spawn_orchestration_with_wait(
    workflow: &str,
    input: Value,
    wait_secs: Option<u64>,
    task_fn: impl FnOnce(Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Value> + Send>> + Send + 'static,
) -> Value {
    let task_id = short_id();
    let workflow_name = workflow.to_string();
    let tid = task_id.clone();
    let wait = wait_secs.unwrap_or_else(|| default_wait_secs(workflow));

    {
        let mut store = task_store().lock().unwrap_or_else(|e| e.into_inner());
        store.insert(
            task_id.clone(),
            AsyncTaskResult {
                task_id: task_id.clone(),
                workflow: workflow_name.clone(),
                status: "running".to_string(),
                started_at: now_secs(),
                finished_at: None,
                result: None,
            },
        );
    }

    let handle = tokio::spawn(async move {
        let result = task_fn(input).await;
        let mut store = task_store().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = store.get_mut(&tid) {
            task.status = "done".to_string();
            task.finished_at = Some(now_secs());
            task.result = Some(result.clone());
        }
        info!("async orchestration [{}] {} completed", tid, workflow_name);
        result
    });

    match tokio::time::timeout(Duration::from_secs(wait), handle).await {
        Ok(Ok(result)) => json!({
            "type": "completed",
            "task_id": task_id,
            "workflow": workflow,
            "status": "done",
            "waited_secs": wait,
            "result": result
        }),
        _ => json!({
            "type": "async",
            "task_id": task_id,
            "workflow": workflow,
            "status": "running",
            "waited_secs": wait,
            "hint": "使用 async_task_query 工具查询结果，传入 task_id",
        }),
    }
}

pub struct AiParallelSolve;

#[async_trait::async_trait]
impl BuiltinSkill for AiParallelSolve {
    fn name(&self) -> &'static str {
        "ai_parallel_solve"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let task = ctx.context["problem"]
            .as_str()
            .or_else(|| ctx.context["task"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();
        info!("ai_parallel_solve: '{}'", safe_truncate(&task, 50));

        if cfg.passthrough {
            return Ok(json!({
                "type": "passthrough",
                "instruction": "三模型先讨论再执行。若争议未解，升级为多方案执行并按可验证结果仲裁。",
                "input": task,
                "workflow": "parallel_solve"
            }));
        }

        let engines: Vec<String> = parse_engines(ctx)
            .iter()
            .map(|engine| engine.label().to_string())
            .collect();
        let risk_level = ctx
            .context
            .get("risk_level")
            .and_then(|v| v.as_str())
            .unwrap_or(if is_high_risk(&task, ctx) { "high" } else { "medium" })
            .to_string();
        let force = force_triple_execute(ctx);
        let input = json!({"task": task, "engines": engines, "risk_level": risk_level, "force_triple_execute": force});
        Ok(spawn_orchestration_with_wait("parallel_solve", input, None, |input| {
            Box::pin(async move {
                let task = input["task"].as_str().unwrap_or("").to_string();
                let engines = input["engines"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| Engine::from_label(v.as_str()?))
                            .collect::<Vec<_>>()
                    })
                    .filter(|engines| !engines.is_empty())
                    .unwrap_or_else(|| vec![Engine::Claude, Engine::OpenAi, Engine::Gemini]);
                let risk = input["risk_level"].as_str().unwrap_or("medium");
                let force = input["force_triple_execute"].as_bool().unwrap_or(false);
                run_collaboration_workflow("parallel_solve", &task, engines, risk, force).await
            })
        })
        .await)
    }
}

pub struct AiTripleVote;

#[async_trait::async_trait]
impl BuiltinSkill for AiTripleVote {
    fn name(&self) -> &'static str {
        "ai_triple_vote"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let problem = ctx.context["problem"]
            .as_str()
            .or_else(|| ctx.context["task"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();
        info!("ai_triple_vote: '{}'", safe_truncate(&problem, 50));

        if cfg.passthrough {
            return Ok(json!({
                "type": "passthrough",
                "instruction": "三模型先讨论选项与风险，再形成一致投票；若仍分歧，则执行并按结果仲裁。",
                "input": problem,
                "options": ctx.context.get("options"),
                "workflow": "triple_vote"
            }));
        }

        let mut task = format!("问题：{}", problem);
        if let Some(options) = ctx.context.get("options").and_then(|v| v.as_array()) {
            let options = options.iter().filter_map(|item| item.as_str()).collect::<Vec<_>>();
            if !options.is_empty() {
                task.push_str(&format!("\n候选项：{}", options.join(" | ")));
            }
        }

        let risk_level = ctx
            .context
            .get("risk_level")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .to_string();
        let force = force_triple_execute(ctx);
        let input = json!({"task": task, "engines": ["claude","openai","gemini"], "risk_level": risk_level, "force_triple_execute": force});
        Ok(spawn_orchestration_with_wait("triple_vote", input, None, |input| {
            Box::pin(async move {
                let task = input["task"].as_str().unwrap_or("").to_string();
                let risk = input["risk_level"].as_str().unwrap_or("medium");
                let force = input["force_triple_execute"].as_bool().unwrap_or(false);
                run_collaboration_workflow(
                    "triple_vote",
                    &task,
                    vec![Engine::Claude, Engine::OpenAi, Engine::Gemini],
                    risk,
                    force,
                )
                .await
            })
        })
        .await)
    }
}

pub struct AiTriangleReview;

#[async_trait::async_trait]
impl BuiltinSkill for AiTriangleReview {
    fn name(&self) -> &'static str {
        "ai_triangle_review"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let code = ctx.context["code"].as_str().unwrap_or(&ctx.task).to_string();
        let context_info = ctx
            .context
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        info!("ai_triangle_review: {} bytes of code", code.len());

        if cfg.passthrough {
            return Ok(
                json!({"type": "passthrough", "instruction": "从正确性、性能、安全、风格、可维护性五个角度审查代码。", "input": code, "context": context_info, "workflow": "triangle_review"}),
            );
        }

        Ok(spawn_orchestration_with_wait("triangle_review", json!({"code": code, "context": context_info}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let code = input["code"].as_str().unwrap_or("");
            let prompt = format!("审查以下代码（正确性、性能、安全、风格、可维护性）：\n```\n{}\n```", code);
            let tasks = vec![
                (Engine::Claude, prompt.clone()),
                (Engine::OpenAi, prompt.clone()),
                (Engine::Gemini, prompt.clone()),
            ];
            let reviews = call_engines_parallel(&tasks, &cfg, "review").await;
            json!({
                "reviews": reviews.iter().filter_map(|report| report.output.as_ref().map(|output| (report.engine.clone(), output.clone()))).collect::<BTreeMap<_, _>>(),
                "participants_status": reviews,
                "degraded": reviews.iter().any(|report| !report.success),
                "failures": reports_to_failures(&reviews),
            })
        })).await)
    }
}

pub struct AiCodeGenerate;

#[async_trait::async_trait]
impl BuiltinSkill for AiCodeGenerate {
    fn name(&self) -> &'static str {
        "ai_code_generate"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let task = ctx.context["task"].as_str().unwrap_or(&ctx.task).to_string();
        let language = ctx
            .context
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("python")
            .to_string();
        let primary = ctx
            .context
            .get("primary")
            .and_then(|v| v.as_str())
            .and_then(Engine::from_label)
            .unwrap_or(Engine::Claude);
        let reviewer = ctx
            .context
            .get("reviewer")
            .and_then(|v| v.as_str())
            .and_then(Engine::from_label)
            .unwrap_or(Engine::OpenAi);
        info!("ai_code_generate: {}", safe_truncate(&task, 50));

        if cfg.passthrough {
            return Ok(
                json!({"type": "passthrough", "instruction": format!("请用 {} 实现以下功能。", language), "input": task, "language": language, "workflow": "code_generate"}),
            );
        }

        Ok(spawn_orchestration_with_wait(
            "code_generate",
            json!({"task": task, "language": language, "primary": primary.label(), "reviewer": reviewer.label()}),
            None,
            |input| {
                Box::pin(async move {
                    let cfg = OrchestratorConfig::from_env();
                    let task = input["task"].as_str().unwrap_or("");
                    let language = input["language"].as_str().unwrap_or("python");
                    let primary = input["primary"]
                        .as_str()
                        .and_then(Engine::from_label)
                        .unwrap_or(Engine::Claude);
                    let reviewer = input["reviewer"]
                        .as_str()
                        .and_then(Engine::from_label)
                        .unwrap_or(Engine::OpenAi);

                    let code_report = call_engine_detailed(
                        primary,
                        &format!("请用 {} 实现以下功能：\n{}", language, task),
                        &cfg,
                        "execute",
                    )
                    .await;
                    let generated_code = code_report.output.clone().unwrap_or_default();
                    let review_report = if generated_code.is_empty() {
                        None
                    } else {
                        Some(
                            call_engine_detailed(
                                reviewer,
                                &review_execution_prompt(
                                    task,
                                    &Proposal {
                                        engine: primary.label().to_string(),
                                        plan_id: primary.label().to_string(),
                                        target_path: "code_refactor".to_string(),
                                        primary_engine: primary.label().to_string(),
                                        review_engines: vec![reviewer.label().to_string()],
                                        execution_mode: "primary_plus_review".to_string(),
                                        key_risks: vec!["实现正确性".to_string()],
                                        execution_order: vec!["execute".to_string(), "review".to_string()],
                                        verify_method: "通过代码审查与测试验证".to_string(),
                                        summary: "代码生成后复审".to_string(),
                                        raw: String::new(),
                                    },
                                    &generated_code,
                                ),
                                &cfg,
                                "review",
                            )
                            .await,
                        )
                    };

                    let mut participants = vec![code_report.clone()];
                    if let Some(review_report) = &review_report {
                        participants.push(review_report.clone());
                    }
                    json!({
                        "primary": primary.label(),
                        "reviewer": reviewer.label(),
                        "code": generated_code,
                        "review": review_report.and_then(|report| report.output),
                        "participants_status": participants,
                        "degraded": participants.iter().any(|report| !report.success),
                        "failures": reports_to_failures(&participants),
                    })
                })
            },
        )
        .await)
    }
}

pub struct AiSmartCollaborate;

#[async_trait::async_trait]
impl BuiltinSkill for AiSmartCollaborate {
    fn name(&self) -> &'static str {
        "ai_smart_collaborate"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let task = ctx.context["task"].as_str().unwrap_or(&ctx.task).to_string();
        info!("ai_smart_collaborate: '{}'", safe_truncate(&task, 50));

        if cfg.passthrough {
            return Ok(
                json!({"type": "passthrough", "instruction": "三模型先讨论、再收敛、再执行；若争议未解，则按可验证结果仲裁。", "input": task, "workflow": "smart_collaborate"}),
            );
        }

        let risk_level = ctx
            .context
            .get("risk_level")
            .and_then(|v| v.as_str())
            .unwrap_or(if is_high_risk(&task, ctx) { "high" } else { "medium" })
            .to_string();
        let force = force_triple_execute(ctx);
        let input = json!({"task": task, "engines": ["claude","openai","gemini"], "risk_level": risk_level, "force_triple_execute": force});
        Ok(
            spawn_orchestration_with_wait("smart_collaborate", input, None, |input| {
                Box::pin(async move {
                    let task = input["task"].as_str().unwrap_or("").to_string();
                    let risk = input["risk_level"].as_str().unwrap_or("medium");
                    let force = input["force_triple_execute"].as_bool().unwrap_or(false);
                    run_collaboration_workflow(
                        "smart_collaborate",
                        &task,
                        vec![Engine::Claude, Engine::OpenAi, Engine::Gemini],
                        risk,
                        force,
                    )
                    .await
                })
            })
            .await,
        )
    }
}

pub struct AiResearch;

#[async_trait::async_trait]
impl BuiltinSkill for AiResearch {
    fn name(&self) -> &'static str {
        "ai_research"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let topic = ctx.context["topic"].as_str().unwrap_or(&ctx.task).to_string();
        let depth = ctx
            .context
            .get("depth")
            .and_then(|v| v.as_str())
            .unwrap_or("comprehensive")
            .to_string();
        info!("ai_research: '{}'", safe_truncate(&topic, 50));

        if cfg.passthrough {
            return Ok(
                json!({"type": "passthrough", "instruction": "从理论、实践、趋势三维度研究。", "input": topic, "depth": depth, "workflow": "research"}),
            );
        }

        let risk = if depth == "deep" { "high" } else { "medium" };
        Ok(spawn_orchestration_with_wait("research", json!({"task": format!("研究主题：{}\n深度：{}", topic, depth), "engines": ["claude","openai","gemini"], "risk_level": risk, "force_triple_execute": depth == "deep"}), None, |input| Box::pin(async move {
            let task = input["task"].as_str().unwrap_or("").to_string();
            let risk = input["risk_level"].as_str().unwrap_or("medium");
            let force = input["force_triple_execute"].as_bool().unwrap_or(false);
            run_collaboration_workflow("research", &task, vec![Engine::Claude, Engine::OpenAi, Engine::Gemini], risk, force).await
        })).await)
    }
}

pub struct AiSerialOptimize;

#[async_trait::async_trait]
impl BuiltinSkill for AiSerialOptimize {
    fn name(&self) -> &'static str {
        "ai_serial_optimize"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let code = ctx.context["code"].as_str().unwrap_or(&ctx.task).to_string();
        info!("ai_serial_optimize: {} bytes", code.len());

        if cfg.passthrough {
            return Ok(
                json!({"type": "passthrough", "instruction": "分析代码问题，优化并验证。", "input": code, "workflow": "serial_optimize"}),
            );
        }

        Ok(
            spawn_orchestration_with_wait("serial_optimize", json!({"code": code}), None, |input| {
                Box::pin(async move {
                    let cfg = OrchestratorConfig::from_env();
                    let code = input["code"].as_str().unwrap_or("");
                    let analysis = call_engine_detailed(
                        Engine::Claude,
                        &format!("分析代码：\n```\n{}\n```", code),
                        &cfg,
                        "analyze",
                    )
                    .await;
                    let optimized = if let Some(output) = &analysis.output {
                        call_engine_detailed(
                            Engine::Gemini,
                            &format!("根据分析优化代码：\n{}\n原代码：\n```{}\n```", output, code),
                            &cfg,
                            "optimize",
                        )
                        .await
                    } else {
                        call_engine_detailed(
                            Engine::Gemini,
                            &format!("优化代码：\n```{}\n```", code),
                            &cfg,
                            "optimize",
                        )
                        .await
                    };
                    let verify_prompt = optimized.output.clone().unwrap_or_default();
                    let verify = call_engine_detailed(
                        Engine::OpenAi,
                        &format!("验证以下优化结果：\n{}", verify_prompt),
                        &cfg,
                        "verify",
                    )
                    .await;
                    let participants = vec![analysis.clone(), optimized.clone(), verify.clone()];
                    json!({
                        "analysis": analysis.output,
                        "optimized": optimized.output,
                        "verification": verify.output,
                        "participants_status": participants,
                        "degraded": participants.iter().any(|report| !report.success),
                        "failures": reports_to_failures(&participants),
                    })
                })
            })
            .await,
        )
    }
}

pub struct AiLongContext;

#[async_trait::async_trait]
impl BuiltinSkill for AiLongContext {
    fn name(&self) -> &'static str {
        "ai_long_context"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let content = ctx.context["content"].as_str().unwrap_or(&ctx.task).to_string();
        let task = ctx
            .context
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("分析并总结")
            .to_string();
        info!("ai_long_context: {} chars", content.len());

        if cfg.passthrough {
            return Ok(
                json!({"type": "passthrough", "instruction": format!("任务：{}", task), "input": content, "workflow": "long_context"}),
            );
        }

        Ok(spawn_orchestration_with_wait(
            "long_context",
            json!({"content": content, "task": task}),
            None,
            |input| {
                Box::pin(async move {
                    let cfg = OrchestratorConfig::from_env();
                    let content = input["content"].as_str().unwrap_or("");
                    let task = input["task"].as_str().unwrap_or("");
                    let report =
                        call_engine_detailed(Engine::Gemini, &format!("{}：\n{}", task, content), &cfg, "execute")
                            .await;
                    let participants = vec![report.clone()];
                    json!({
                        "analysis": report.output,
                        "participants_status": participants,
                        "degraded": !report.success,
                        "failures": reports_to_failures(&participants),
                    })
                })
            },
        )
        .await)
    }
}

pub struct AiCrossReview;

#[async_trait::async_trait]
impl BuiltinSkill for AiCrossReview {
    fn name(&self) -> &'static str {
        "ai_cross_review"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let code = ctx.context["code"].as_str().unwrap_or(&ctx.task).to_string();
        info!("ai_cross_review: {} bytes", code.len());

        if cfg.passthrough {
            return Ok(
                json!({"type": "passthrough", "instruction": "双引擎交叉审查代码，指出问题和建议。", "input": code, "workflow": "cross_review"}),
            );
        }

        let engines = parse_engines(ctx);
        let review_engines = if engines.len() >= 2 {
            engines.into_iter().take(2).collect::<Vec<_>>()
        } else {
            vec![Engine::Claude, Engine::OpenAi]
        };
        Ok(spawn_orchestration_with_wait("cross_review", json!({"code": code, "engines": review_engines.iter().map(|e| e.label()).collect::<Vec<_>>() }), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let code = input["code"].as_str().unwrap_or("");
            let engines = input["engines"].as_array()
                .map(|arr| arr.iter().filter_map(|v| Engine::from_label(v.as_str()?)).collect::<Vec<_>>())
                .filter(|engines| !engines.is_empty())
                .unwrap_or_else(|| vec![Engine::Claude, Engine::OpenAi]);
            let prompt = format!("审查代码：\n```\n{}\n```", code);
            let tasks: Vec<_> = engines.iter().map(|engine| (*engine, prompt.clone())).collect();
            let reviews = call_engines_parallel(&tasks, &cfg, "review").await;
            json!({
                "reviews": reviews.iter().filter_map(|report| report.output.as_ref().map(|output| (report.engine.clone(), output.clone()))).collect::<BTreeMap<_, _>>(),
                "participants_status": reviews,
                "degraded": reviews.iter().any(|report| !report.success),
                "failures": reports_to_failures(&reviews),
            })
        })).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_wait_time_in_mcp_mode() {
        std::env::set_var("AION_MCP_MODE", "1");
        std::env::set_var("AION_ORCH_WAIT_SECS", "130");
        assert_eq!(default_wait_secs("parallel_solve"), 55);
        std::env::remove_var("AION_MCP_MODE");
        std::env::remove_var("AION_ORCH_WAIT_SECS");
    }

    #[test]
    fn parses_proposal_output() {
        let raw = "\
TARGET_PATH: code_refactor\n\
PRIMARY_ENGINE: claude\n\
REVIEW_ENGINES: openai | gemini\n\
EXECUTION_MODE: primary_plus_review\n\
KEY_RISKS: 回归风险 | 边界条件\n\
EXECUTION_ORDER: analyze > execute > review\n\
VERIFY: 运行测试并对比输出\n\
SUMMARY: 先改再审\n";
        let proposal = parse_proposal_output(Engine::Claude, raw, "重构代码", "parallel_solve");
        assert_eq!(proposal.target_path, "code_refactor");
        assert_eq!(proposal.primary_engine, "claude");
        assert_eq!(proposal.execution_order, vec!["analyze", "execute", "review"]);
    }

    #[test]
    fn detects_consensus() {
        let left = Proposal {
            engine: "claude".into(),
            plan_id: "claude".into(),
            target_path: "code_refactor".into(),
            primary_engine: "claude".into(),
            review_engines: vec!["openai".into(), "gemini".into()],
            execution_mode: "primary_plus_review".into(),
            key_risks: vec!["回归风险".into()],
            execution_order: vec!["analyze".into(), "execute".into(), "review".into()],
            verify_method: "测试".into(),
            summary: "A".into(),
            raw: String::new(),
        };
        let mut right = left.clone();
        right.engine = "openai".into();
        right.plan_id = "openai".into();
        assert!(proposals_agree(&[left, right]));
    }

    #[test]
    fn classifies_model_not_found() {
        let kind = classify_failure("ModelNotFoundError: gemini-999", "");
        assert_eq!(kind.as_str(), "model_not_found");
    }
}
