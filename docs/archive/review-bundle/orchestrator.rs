//! 多模型编排器 — 用 Rust 重写 ai-orchestrator
//!
//! 通过 CLI subprocess 调用 Claude/Codex/Gemini，支持 9 种协作模式。
//! Passthrough 模式下，合并/评估步骤由宿主 LLM 直接处理。

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};
use tokio::process::Command;
use tracing::{info, warn};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

/// 安全截断 UTF-8 字符串（不会在多字节字符中间断开）
fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

// ── 异步任务存储 ────────────────────────────────────────────────────────────

use std::sync::{Mutex, OnceLock};

/// 异步任务的结果
#[derive(Clone, serde::Serialize)]
struct AsyncTaskResult {
    task_id: String,
    workflow: String,
    status: String,       // "running" | "done" | "error"
    started_at: u64,
    finished_at: Option<u64>,
    result: Option<Value>,
}

/// 全局异步任务存储
static ASYNC_TASKS: OnceLock<Mutex<HashMap<String, AsyncTaskResult>>> = OnceLock::new();

fn task_store() -> &'static Mutex<HashMap<String, AsyncTaskResult>> {
    ASYNC_TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 生成短 task_id（含随机因子避免碰撞）
fn short_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("orch_{:08x}_{:04x}", now_secs() as u32 ^ std::process::id(), seq)
}

/// 查询异步任务结果的 builtin
pub struct AsyncTaskQuery;

#[async_trait::async_trait]
impl BuiltinSkill for AsyncTaskQuery {
    fn name(&self) -> &'static str { "async_task_query" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let task_id = ctx.context["task_id"].as_str().unwrap_or("");

        if task_id.is_empty() {
            // 列出所有任务
            let store = task_store().lock().unwrap_or_else(|e| e.into_inner());
            let tasks: Vec<Value> = store.values()
                .map(|t| json!({
                    "task_id": t.task_id,
                    "workflow": t.workflow,
                    "status": t.status,
                    "started_at": t.started_at,
                    "finished_at": t.finished_at,
                }))
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

// ── 引擎抽象 ─────────────────────────────────────────────────────────────────

/// AI 引擎标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

/// 从环境变量读取的编排器配置
struct OrchestratorConfig {
    claude_cli: String,
    codex_cli: String,
    gemini_cli: String,
    openai_model: String,
    timeout: Duration,
    passthrough: bool,
}

impl OrchestratorConfig {
    fn from_env() -> Self {
        Self {
            claude_cli: std::env::var("CLAUDE_CLI").unwrap_or_else(|_| "claude".into()),
            codex_cli: std::env::var("CODEX_CLI").unwrap_or_else(|_| "codex".into()),
            gemini_cli: std::env::var("GEMINI_CLI").unwrap_or_else(|_| "gemini".into()),
            openai_model: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4".into()),
            timeout: Duration::from_secs(
                std::env::var("REQUEST_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(180),
            ),
            passthrough: std::env::var("AI_PASSTHROUGH")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
    }
}

/// 调用单个引擎 CLI，返回 stdout 或错误描述
/// 用临时文件传递 prompt，避免 Windows cmd 编码和长度限制
async fn call_engine(engine: Engine, prompt: &str, cfg: &OrchestratorConfig) -> String {
    // 安全策略：prompt 始终通过 stdin 管道传递，绝不拼入命令行参数或 shell 字符串，
    // 从根本上消除命令注入风险（&|^% 等 shell 元字符不会被解释）。

    let cli_path = |base: &str| -> String {
        let p = base.replace('/', "\\");
        // Windows 需要 .cmd 后缀才能通过 cmd /c 调用 npm 全局命令
        if cfg!(windows) && !p.ends_with(".cmd") && !p.ends_with(".exe") && !p.contains('\\') {
            format!("{}.cmd", p)
        } else {
            p
        }
    };

    let result = match engine {
        Engine::Claude => {
            run_cli_with_stdin(
                &cli_path(&cfg.claude_cli),
                &["-p", "-", "--output-format", "text"],
                prompt,
                cfg.timeout,
            ).await
        }
        Engine::OpenAi => {
            run_cli_with_stdin(
                &cli_path(&cfg.codex_cli),
                &["exec", "-", "--skip-git-repo-check"],
                prompt,
                cfg.timeout,
            ).await
        }
        Engine::Gemini => {
            run_cli_with_stdin(
                &cli_path(&cfg.gemini_cli),
                &["-p", "-"],
                prompt,
                cfg.timeout,
            ).await
        }
    };

    match result {
        Ok(output) => output,
        Err(e) => format!("[{} Error] {}", engine.label(), e),
    }
}

/// 安全执行 CLI 命令，prompt 通过 stdin 管道传入（无 shell 注入面）
///
/// 对于 .cmd 文件使用 `cmd /c cli_path args...`，对于 .exe 直接执行。
/// prompt 不出现在命令行参数中，仅通过 stdin 写入。
async fn run_cli_with_stdin(
    cli: &str,
    args: &[&str],
    stdin_data: &str,
    timeout: Duration,
) -> Result<String> {
    use tokio::io::AsyncWriteExt;

    // Windows .cmd 需要通过 cmd /c 启动
    let mut cmd = if cfg!(windows) && cli.ends_with(".cmd") {
        let mut c = Command::new("cmd");
        c.arg("/c").arg(cli);
        for a in args { c.arg(a); }
        c
    } else {
        let mut c = Command::new(cli);
        for a in args { c.arg(a); }
        c
    };

    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("CLI 启动失败 '{}': {}", cli, e))?;

    // 先取 child id 用于超时时 kill
    let child_id = child.id();

    // 通过 stdin 管道写入 prompt（安全，不经过 shell 解析）
    if let Some(mut stdin) = child.stdin.take() {
        let data = stdin_data.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(data.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });
    }

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow::anyhow!("{}", stderr.trim()))
            }
        }
        Ok(Err(e)) => Err(anyhow::anyhow!("执行失败: {}", e)),
        Err(_) => {
            // 超时：通过 taskkill 强制终止子进程树，避免孤儿进程
            if let Some(pid) = child_id {
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .output().await;
            }
            warn!("CLI 命令超时 ({}s)，已终止子进程: {}", timeout.as_secs(), cli);
            Err(anyhow::anyhow!("请求超时 ({}s)", timeout.as_secs()))
        }
    }
}

/// 并行调用多个引擎（使用 tokio JoinSet，无需额外依赖）
async fn call_engines_parallel(
    tasks: &[(Engine, String)],
    _cfg: &OrchestratorConfig,
) -> HashMap<String, String> {
    let mut set = tokio::task::JoinSet::new();
    for (engine, prompt) in tasks {
        let engine = *engine;
        let prompt = prompt.clone();
        set.spawn(async move {
            let cfg = OrchestratorConfig::from_env();
            let result = call_engine(engine, &prompt, &cfg).await;
            (engine.label().to_string(), result)
        });
    }

    let mut results = HashMap::new();
    while let Some(Ok((label, result))) = set.join_next().await {
        results.insert(label, result);
    }
    results
}

/// 解析引擎列表参数
fn parse_engines(ctx: &ExecutionContext) -> Vec<Engine> {
    let defaults = vec![Engine::Claude, Engine::OpenAi, Engine::Gemini];
    let arr = match ctx.context.get("engines").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return defaults,
    };
    arr.iter()
        .filter_map(|v| match v.as_str()? {
            "claude" => Some(Engine::Claude),
            "openai" => Some(Engine::OpenAi),
            "gemini" => Some(Engine::Gemini),
            _ => None,
        })
        .collect()
}

/// 格式化多引擎结果为结构化文本
fn format_multi_results(results: &HashMap<String, String>) -> String {
    let mut out = String::new();
    for (engine, result) in results {
        out.push_str(&format!("## {} 的回答\n\n{}\n\n---\n\n", engine, result));
    }
    out
}

/// 动态等待时长：根据 workflow 类型和环境变量决定
fn default_wait_secs(workflow: &str) -> u64 {
    // 环境变量覆盖：AION_ORCH_WAIT_SECS=20
    if let Ok(val) = std::env::var("AION_ORCH_WAIT_SECS") {
        if let Ok(secs) = val.parse::<u64>() {
            return secs;
        }
    }
    match workflow {
        // 单引擎任务
        "code_generate" | "long_context" => 50,
        // 双引擎
        "cross_review" | "serial_optimize" => 55,
        // 三引擎并行（必须在 MCP 60s 超时内返回）
        "parallel_solve" | "triple_vote" | "triangle_review"
        | "smart_collaborate" | "research" => 55,
        _ => 50,
    }
}

/// 通用编排任务启动器（带动态等待）
///
/// 启动后台任务，等待指定时间。
/// - 在等待窗口内完成 → 直接返回完整结果
/// - 超时 → 返回 task_id，客户端可用 async_task_query 轮询
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

    // 注册任务
    {
        let mut store = task_store().lock().unwrap_or_else(|e| e.into_inner());
        store.insert(task_id.clone(), AsyncTaskResult {
            task_id: task_id.clone(),
            workflow: workflow_name.clone(),
            status: "running".to_string(),
            started_at: now_secs(),
            finished_at: None,
            result: None,
        });
    }

    // 启动后台任务
    let handle = tokio::spawn(async move {
        let result = task_fn(input).await;
        let tid_inner = tid.clone();
        let wf = workflow_name.clone();
        let mut store = task_store().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = store.get_mut(&tid_inner) {
            task.status = "done".to_string();
            task.finished_at = Some(now_secs());
            task.result = Some(result.clone());
        }
        info!("async orchestration [{}] {} completed", tid, wf);
        result
    });

    // 动态等待：在窗口内完成则直接返回结果
    match tokio::time::timeout(Duration::from_secs(wait), handle).await {
        Ok(Ok(result)) => {
            info!("orchestration [{}] {} returned within {}s", task_id, workflow, wait);
            json!({
                "type": "completed",
                "task_id": task_id,
                "workflow": workflow,
                "status": "done",
                "result": result
            })
        }
        _ => {
            // 超时或出错，任务仍在后台跑
            info!("orchestration [{}] {} still running after {}s, returning task_id", task_id, workflow, wait);
            json!({
                "type": "async",
                "task_id": task_id,
                "workflow": workflow,
                "status": "running",
                "waited_secs": wait,
                "hint": "使用 async_task_query 工具查询结果，传入 task_id",
            })
        }
    }
}

/// 旧接口兼容：立即返回 task_id（不等待）
fn spawn_async_orchestration(
    workflow: &str,
    input: Value,
    task_fn: impl FnOnce(Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Value> + Send>> + Send + 'static,
) -> Value {
    let task_id = short_id();
    let workflow_name = workflow.to_string();
    let tid = task_id.clone();

    {
        let mut store = task_store().lock().unwrap_or_else(|e| e.into_inner());
        store.insert(task_id.clone(), AsyncTaskResult {
            task_id: task_id.clone(),
            workflow: workflow_name.clone(),
            status: "running".to_string(),
            started_at: now_secs(),
            finished_at: None,
            result: None,
        });
    }

    tokio::spawn(async move {
        let result = task_fn(input).await;
        let mut store = task_store().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = store.get_mut(&tid) {
            task.status = "done".to_string();
            task.finished_at = Some(now_secs());
            task.result = Some(result);
        }
        info!("async orchestration [{}] {} completed", tid, workflow_name);
    });

    json!({
        "type": "async",
        "task_id": task_id,
        "workflow": workflow,
        "status": "running",
        "hint": "使用 async_task_query 工具查询结果，传入 task_id",
    })
}

// ── 9 个 BuiltinSkill 实现 ──────────────────────────────────────────────────

// 1. 并行求解
pub struct AiParallelSolve;

#[async_trait::async_trait]
impl BuiltinSkill for AiParallelSolve {
    fn name(&self) -> &'static str { "ai_parallel_solve" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let problem = ctx.context["problem"].as_str().unwrap_or(&ctx.task).to_string();
        info!("ai_parallel_solve: '{}'", safe_truncate(&problem, 50));

        if cfg.passthrough {
            return Ok(json!({
                "type": "passthrough",
                "instruction": "请详细解决以下问题，给出完整方案，从多个角度分析优劣。",
                "input": problem,
                "workflow": "parallel_solve"
            }));
        }

        // 异步后台执行
        let engines: Vec<String> = parse_engines(ctx).iter().map(|e| e.label().to_string()).collect();
        let input = json!({"problem": problem, "engines": engines});
        Ok(spawn_orchestration_with_wait("parallel_solve", input, None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let problem = input["problem"].as_str().unwrap_or("");
            let prompt = format!("请详细解决以下问题，给出完整方案：\n\n{}", problem);
            let engines: Vec<Engine> = input["engines"].as_array()
                .map(|a| a.iter().filter_map(|v| match v.as_str()? {
                    "claude" => Some(Engine::Claude), "openai" => Some(Engine::OpenAi), "gemini" => Some(Engine::Gemini), _ => None
                }).collect()).unwrap_or_else(|| vec![Engine::Claude, Engine::OpenAi, Engine::Gemini]);
            let tasks: Vec<_> = engines.iter().map(|e| (*e, prompt.clone())).collect();
            let solutions = call_engines_parallel(&tasks, &cfg).await;
            json!({"problem": problem, "solutions": solutions, "engines_used": engines.iter().map(|e| e.label()).collect::<Vec<_>>()})
        })).await)
    }
}

// 2. 三方投票
pub struct AiTripleVote;

#[async_trait::async_trait]
impl BuiltinSkill for AiTripleVote {
    fn name(&self) -> &'static str { "ai_triple_vote" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let problem = ctx.context["problem"].as_str().unwrap_or(&ctx.task);
        info!("ai_triple_vote: '{}'", safe_truncate(&problem, 50));

        if cfg.passthrough {
            return Ok(json!({
                "type": "passthrough",
                "instruction": "请从多个角度分析以下问题，给出评分和推荐方案。",
                "input": problem,
                "options": ctx.context.get("options"),
                "workflow": "triple_vote"
            }));
        }

        Ok(spawn_orchestration_with_wait("triple_vote", json!({"problem": problem, "options": ctx.context.get("options")}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let problem = input["problem"].as_str().unwrap_or("");
            let prompt = format!("问题：{}\n请给出方案并评分。", problem);
            let tasks = vec![(Engine::Claude, prompt.clone()), (Engine::OpenAi, prompt.clone()), (Engine::Gemini, prompt.clone())];
            let votes = call_engines_parallel(&tasks, &cfg).await;
            json!({"problem": problem, "votes": votes})
        })).await)
    }
}

// 3. 三角审查
pub struct AiTriangleReview;

#[async_trait::async_trait]
impl BuiltinSkill for AiTriangleReview {
    fn name(&self) -> &'static str { "ai_triangle_review" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let code = ctx.context["code"].as_str().unwrap_or(&ctx.task).to_string();
        let context_info = ctx.context.get("context").and_then(|v| v.as_str()).unwrap_or("").to_string();
        info!("ai_triangle_review: {} bytes of code", code.len());

        if cfg.passthrough {
            return Ok(json!({"type": "passthrough", "instruction": "从5个角度审查代码：正确性、性能、安全、风格、可维护性。", "input": code, "context": context_info, "workflow": "triangle_review"}));
        }

        Ok(spawn_orchestration_with_wait("triangle_review", json!({"code": code, "context": context_info}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let code = input["code"].as_str().unwrap_or("");
            let prompt = format!("审查以下代码（正确性、性能、安全、风格、可维护性）：\n```\n{}\n```", code);
            let tasks = vec![(Engine::Claude, prompt.clone()), (Engine::OpenAi, prompt.clone()), (Engine::Gemini, prompt.clone())];
            let reviews = call_engines_parallel(&tasks, &cfg).await;
            json!({"reviews": reviews})
        })).await)
    }
}

// 4. 代码生成
pub struct AiCodeGenerate;

#[async_trait::async_trait]
impl BuiltinSkill for AiCodeGenerate {
    fn name(&self) -> &'static str { "ai_code_generate" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let task = ctx.context["task"].as_str().unwrap_or(&ctx.task).to_string();
        let language = ctx.context.get("language").and_then(|v| v.as_str()).unwrap_or("python").to_string();
        info!("ai_code_generate: {}", safe_truncate(&task, 50));

        if cfg.passthrough {
            return Ok(json!({"type": "passthrough", "instruction": format!("请用 {} 实现以下功能。", language), "input": task, "language": language, "workflow": "code_generate"}));
        }

        Ok(spawn_orchestration_with_wait("code_generate", json!({"task": task, "language": language}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let task = input["task"].as_str().unwrap_or("");
            let lang = input["language"].as_str().unwrap_or("python");
            let code = call_engine(Engine::Claude, &format!("用 {} 实现：\n{}", lang, task), &cfg).await;
            let review = call_engine(Engine::OpenAi, &format!("审查代码：\n```\n{}\n```", code), &cfg).await;
            json!({"code": code, "review": review})
        })).await)
    }
}

// 5. 智能协作
pub struct AiSmartCollaborate;

#[async_trait::async_trait]
impl BuiltinSkill for AiSmartCollaborate {
    fn name(&self) -> &'static str { "ai_smart_collaborate" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let task = ctx.context["task"].as_str().unwrap_or(&ctx.task).to_string();
        info!("ai_smart_collaborate: '{}'", safe_truncate(&task, 50));

        if cfg.passthrough {
            return Ok(json!({"type": "passthrough", "instruction": "请提出完整解决方案，多角度分析。", "input": task, "workflow": "smart_collaborate"}));
        }

        Ok(spawn_orchestration_with_wait("smart_collaborate", json!({"task": task}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let task = input["task"].as_str().unwrap_or("");
            let prompt = format!("请提出完整解决方案：\n\n{}", task);
            let tasks = vec![(Engine::Claude, prompt.clone()), (Engine::OpenAi, prompt.clone()), (Engine::Gemini, prompt.clone())];
            let proposals = call_engines_parallel(&tasks, &cfg).await;
            json!({"proposals": proposals})
        })).await)
    }
}

// 6. 研究分析
pub struct AiResearch;

#[async_trait::async_trait]
impl BuiltinSkill for AiResearch {
    fn name(&self) -> &'static str { "ai_research" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let topic = ctx.context["topic"].as_str().unwrap_or(&ctx.task).to_string();
        let depth = ctx.context.get("depth").and_then(|v| v.as_str()).unwrap_or("comprehensive").to_string();
        info!("ai_research: '{}'", safe_truncate(&topic, 50));

        if cfg.passthrough {
            return Ok(json!({"type": "passthrough", "instruction": "从理论、实践、趋势三维度研究。", "input": topic, "depth": depth, "workflow": "research"}));
        }

        Ok(spawn_orchestration_with_wait("research", json!({"topic": topic, "depth": depth}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let topic = input["topic"].as_str().unwrap_or("");
            let tasks = vec![
                (Engine::Claude, format!("理论分析：{}", topic)),
                (Engine::OpenAi, format!("实践视角：{}", topic)),
                (Engine::Gemini, format!("趋势分析：{}", topic)),
            ];
            let research = call_engines_parallel(&tasks, &cfg).await;
            json!({"research": research})
        })).await)
    }
}

// 7. 串行优化
pub struct AiSerialOptimize;

#[async_trait::async_trait]
impl BuiltinSkill for AiSerialOptimize {
    fn name(&self) -> &'static str { "ai_serial_optimize" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let code = ctx.context["code"].as_str().unwrap_or(&ctx.task).to_string();
        info!("ai_serial_optimize: {} bytes", code.len());

        if cfg.passthrough {
            return Ok(json!({"type": "passthrough", "instruction": "分析代码问题，优化并验证。", "input": code, "workflow": "serial_optimize"}));
        }

        Ok(spawn_orchestration_with_wait("serial_optimize", json!({"code": code}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let code = input["code"].as_str().unwrap_or("");
            let analysis = call_engine(Engine::Claude, &format!("分析代码：\n```\n{}\n```", code), &cfg).await;
            let optimized = call_engine(Engine::Gemini, &format!("优化：\n{}\n```\n{}\n```", analysis, code), &cfg).await;
            let verify = call_engine(Engine::OpenAi, &format!("验证：\n{}", optimized), &cfg).await;
            json!({"analysis": analysis, "optimized": optimized, "verification": verify})
        })).await)
    }
}

// 8. 长上下文处理
pub struct AiLongContext;

#[async_trait::async_trait]
impl BuiltinSkill for AiLongContext {
    fn name(&self) -> &'static str { "ai_long_context" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let content = ctx.context["content"].as_str().unwrap_or(&ctx.task).to_string();
        let task = ctx.context.get("task").and_then(|v| v.as_str()).unwrap_or("分析并总结").to_string();
        info!("ai_long_context: {} chars", content.len());

        if cfg.passthrough {
            return Ok(json!({"type": "passthrough", "instruction": format!("任务：{}", task), "input": content, "workflow": "long_context"}));
        }

        Ok(spawn_orchestration_with_wait("long_context", json!({"content": content, "task": task}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let content = input["content"].as_str().unwrap_or("");
            let task = input["task"].as_str().unwrap_or("");
            let result = call_engine(Engine::Gemini, &format!("{}：\n{}", task, content), &cfg).await;
            json!({"analysis": result})
        })).await)
    }
}

// 9. 交叉审查
pub struct AiCrossReview;

#[async_trait::async_trait]
impl BuiltinSkill for AiCrossReview {
    fn name(&self) -> &'static str { "ai_cross_review" }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let cfg = OrchestratorConfig::from_env();
        let code = ctx.context["code"].as_str().unwrap_or(&ctx.task).to_string();
        let context_info = ctx.context.get("context").and_then(|v| v.as_str()).unwrap_or("").to_string();
        info!("ai_cross_review: {} bytes", code.len());

        if cfg.passthrough {
            return Ok(json!({"type": "passthrough", "instruction": "审查代码，指出问题和建议。", "input": code, "context": context_info, "workflow": "cross_review"}));
        }

        Ok(spawn_orchestration_with_wait("cross_review", json!({"code": code, "context": context_info}), None, |input| Box::pin(async move {
            let cfg = OrchestratorConfig::from_env();
            let code = input["code"].as_str().unwrap_or("");
            let prompt = format!("审查代码：\n```\n{}\n```", code);
            let tasks: Vec<_> = vec![Engine::Claude, Engine::OpenAi].iter().map(|e| (*e, prompt.clone())).collect();
            let reviews = call_engines_parallel(&tasks, &cfg).await;
            json!({"reviews": reviews})
        })).await)
    }
}
