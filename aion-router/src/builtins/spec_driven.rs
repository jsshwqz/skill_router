//! Spec-Driven Development — 规格驱动开发
//!
//! 大型代码改造的五阶段流水线 builtin。
//! 通过 `action` 参数分派：analyze / decompose / plan / execute / status

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::spec_types::*;
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

// ── 全局项目存储 ──────────────────────────────────────────────

static SPEC_STORE: OnceLock<Mutex<HashMap<String, SpecProject>>> = OnceLock::new();

fn spec_store() -> &'static Mutex<HashMap<String, SpecProject>> {
    SPEC_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 确保磁盘上已有的项目被加载
fn ensure_loaded(workspace: &Path) {
    let specs_dir = workspace.join(".skill-router").join("specs");
    if !specs_dir.exists() {
        return;
    }
    let mut store = spec_store().lock().unwrap();
    if let Ok(entries) = fs::read_dir(&specs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(proj) = serde_json::from_str::<SpecProject>(&data) {
                        store.entry(proj.project_id.clone()).or_insert(proj);
                    }
                }
            }
        }
    }
}

// ── 持久化 ────────────────────────────────────────────────────

fn specs_dir(workspace: &str) -> PathBuf {
    Path::new(workspace).join(".skill-router").join("specs")
}

fn persist_project(project: &SpecProject) -> Result<()> {
    let dir = specs_dir(&project.workspace_path);
    fs::create_dir_all(&dir)?;

    // JSON
    let json_path = dir.join(format!("{}.json", project.project_id));
    fs::write(&json_path, serde_json::to_string_pretty(project)?)?;

    // MASTER.md
    let md_path = dir.join(format!("MASTER_{}.md", project.project_id));
    fs::write(&md_path, render_master_md(project))?;

    Ok(())
}

// ── MASTER.md 渲染 ────────────────────────────────────────────

fn render_master_md(p: &SpecProject) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Spec-Driven: {}\n\n", p.goal));
    md.push_str(&format!("> Project ID: `{}`\n", p.project_id));
    md.push_str(&format!("> Created: {}\n", fmt_ts(p.created_at)));
    md.push_str(&format!("> Updated: {}\n\n", fmt_ts(p.updated_at)));

    // Phase Progress
    md.push_str("## Phase Progress\n\n");
    md.push_str("| # | Phase | Status | Started | Completed |\n");
    md.push_str("|---|-------|--------|---------|-----------|\n");
    for (i, phase) in p.phases.iter().enumerate() {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            i + 1,
            phase.kind,
            phase.status,
            phase.started_at.map(fmt_ts).unwrap_or_else(|| "—".into()),
            phase.completed_at.map(fmt_ts).unwrap_or_else(|| "—".into()),
        ));
    }

    // Tasks
    if !p.tasks.is_empty() {
        md.push_str("\n## Sub-Tasks\n\n");
        md.push_str("| ID | Title | Depends On | Status | Test Strategy |\n");
        md.push_str("|----|-------|------------|--------|---------------|\n");
        for t in &p.tasks {
            let deps = if t.depends_on.is_empty() {
                "—".to_string()
            } else {
                t.depends_on.join(", ")
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                t.id, t.title, deps, t.status, t.test_strategy,
            ));
        }
    }

    // Risks
    if !p.risks.is_empty() {
        md.push_str("\n## Risk Assessment\n\n");
        md.push_str("| Area | Severity | Description | Mitigation |\n");
        md.push_str("|------|----------|-------------|------------|\n");
        for r in &p.risks {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                r.area, r.severity, r.description, r.mitigation,
            ));
        }
    }

    // Lessons
    if !p.lessons.is_empty() {
        md.push_str("\n## Lessons Learned\n\n");
        for l in &p.lessons {
            let icon = if l.success { "+" } else { "-" };
            md.push_str(&format!("- [{}][{}] {}\n", icon, l.phase, l.content));
        }
    }

    md
}

fn fmt_ts(epoch: u64) -> String {
    // 简单的 UTC 格式
    let secs = epoch;
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let h = time_secs / 3600;
    let m = (time_secs % 3600) / 60;
    // 从 1970-01-01 计算日期
    let (y, mo, d) = epoch_to_ymd(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, m)
}

fn epoch_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut y = 1970;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let months: [u64; 12] = if is_leap(y) {
        [31,29,31,30,31,30,31,31,30,31,30,31]
    } else {
        [31,28,31,30,31,30,31,31,30,31,30,31]
    };
    let mut mo = 1u64;
    for &ml in &months {
        if days < ml { break; }
        days -= ml;
        mo += 1;
    }
    (y, mo, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn new_project_id() -> String {
    format!("spec_{:x}", now_epoch())
}

// ── Passthrough 检测 ──────────────────────────────────────────

fn is_passthrough() -> bool {
    std::env::var("AI_PASSTHROUGH")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

// ── BuiltinSkill 实现 ─────────────────────────────────────────

pub struct SpecDriven;

#[async_trait::async_trait]
impl BuiltinSkill for SpecDriven {
    fn name(&self) -> &'static str {
        "spec_driven"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let action = ctx.context.get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        match action {
            "analyze" => handle_analyze(ctx),
            "decompose" => handle_decompose(ctx),
            "plan" => handle_plan(ctx),
            "execute" => handle_execute(ctx),
            "status" => handle_status(ctx),
            other => Err(anyhow!("spec_driven: unknown action '{}'", other)),
        }
    }
}

// ── action: analyze ───────────────────────────────────────────

fn handle_analyze(ctx: &ExecutionContext) -> Result<Value> {
    let goal = ctx.context.get("goal")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("analyze requires 'goal' parameter"))?;

    let workspace = ctx.context.get("workspace")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());

    let project_id = ctx.context.get("project_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(new_project_id);

    // 创建项目
    let mut project = SpecProject::new(project_id.clone(), goal.to_string(), workspace);
    if let Some(phase) = project.phase_mut(PhaseKind::Analyze) {
        phase.status = PhaseStatus::InProgress;
        phase.started_at = Some(now_epoch());
    }

    // 如果提供了分析结果（从 passthrough 回填），直接完成
    if let Some(result) = ctx.context.get("analysis_result") {
        if let Some(phase) = project.phase_mut(PhaseKind::Analyze) {
            phase.status = PhaseStatus::Completed;
            phase.completed_at = Some(now_epoch());
            phase.output = Some(result.clone());
        }
        // 提取 risks
        if let Some(risks) = result.get("risk_areas").and_then(|v| v.as_array()) {
            for r in risks {
                project.risks.push(SpecRisk {
                    area: r.get("area").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    severity: r.get("severity").and_then(|v| v.as_str()).unwrap_or("medium").to_string(),
                    description: r.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    mitigation: r.get("mitigation").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                });
            }
        }
    }

    project.touch();
    persist_project(&project)?;
    spec_store().lock().unwrap().insert(project_id.clone(), project);

    // 记录到学习引擎
    if let Some(learner) = crate::learner::learner() {
        learner.record("spec_driven", true, std::time::Duration::from_millis(10));
    }

    if is_passthrough() && ctx.context.get("analysis_result").is_none() {
        Ok(json!({
            "type": "passthrough",
            "workflow": "spec_driven",
            "action": "analyze",
            "project_id": project_id,
            "instruction": "请扫描工作区代码库，针对以下目标进行深度分析。输出 JSON 包含 files_scanned(数量), dependency_graph(模块依赖), risk_areas(数组,含area/severity/description/mitigation), complexity_estimate(low/medium/high)。分析完成后调用 spec_driven 工具：action=analyze, project_id=此ID, analysis_result=你的分析JSON。",
            "input": ctx.context.get("goal").unwrap_or(&json!("")),
        }))
    } else {
        Ok(json!({
            "project_id": project_id,
            "action": "analyze",
            "status": "in_progress",
            "next_step": "完成分析后，调用 spec_driven action=decompose project_id=此ID"
        }))
    }
}

// ── action: decompose ─────────────────────────────────────────

fn handle_decompose(ctx: &ExecutionContext) -> Result<Value> {
    let project_id = ctx.context.get("project_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("decompose requires 'project_id'"))?;

    let mut store = spec_store().lock().unwrap();
    let project = store.get_mut(project_id)
        .ok_or_else(|| anyhow!("project '{}' not found", project_id))?;

    // 如果提供了任务列表（从 passthrough 回填）
    if let Some(tasks_val) = ctx.context.get("tasks") {
        if let Ok(tasks) = serde_json::from_value::<Vec<SpecTask>>(tasks_val.clone()) {
            project.tasks = tasks;
        }
        if let Some(phase) = project.phase_mut(PhaseKind::Decompose) {
            phase.status = PhaseStatus::Completed;
            phase.completed_at = Some(now_epoch());
            phase.output = Some(tasks_val.clone());
        }
        project.touch();
        persist_project(project)?;
        return Ok(json!({
            "project_id": project_id,
            "action": "decompose",
            "status": "completed",
            "task_count": project.tasks.len(),
            "next_step": "调用 spec_driven action=plan project_id=此ID"
        }));
    }

    // 标记阶段开始
    if let Some(phase) = project.phase_mut(PhaseKind::Decompose) {
        phase.status = PhaseStatus::InProgress;
        phase.started_at = Some(now_epoch());
    }
    project.touch();
    persist_project(project)?;

    let analysis_output = project.phase(PhaseKind::Analyze)
        .and_then(|p| p.output.as_ref())
        .cloned()
        .unwrap_or(json!({"goal": project.goal}));
    let goal = project.goal.clone();
    let pid = project_id.to_string();

    drop(store);

    if is_passthrough() {
        Ok(json!({
            "type": "passthrough",
            "workflow": "spec_driven",
            "action": "decompose",
            "project_id": pid,
            "instruction": "基于以下分析结果，将项目目标分解为有序子任务列表。每个任务是一个 JSON 对象，包含：id(T1/T2/...), title, description, depends_on(前置任务ID数组), test_strategy, rollback_plan。注意依赖顺序。完成后调用 spec_driven：action=decompose, project_id=此ID, tasks=[你的任务列表JSON数组]。",
            "input": { "goal": goal, "analysis": analysis_output },
        }))
    } else {
        Ok(json!({
            "project_id": pid,
            "action": "decompose",
            "status": "in_progress",
            "analysis": analysis_output,
        }))
    }
}

// ── action: plan ──────────────────────────────────────────────

fn handle_plan(ctx: &ExecutionContext) -> Result<Value> {
    let project_id = ctx.context.get("project_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("plan requires 'project_id'"))?;

    let mut store = spec_store().lock().unwrap();
    let project = store.get_mut(project_id)
        .ok_or_else(|| anyhow!("project '{}' not found", project_id))?;

    // 如果提供了执行计划（从 passthrough 回填）
    if let Some(plan_result) = ctx.context.get("plan_result") {
        if let Some(phase) = project.phase_mut(PhaseKind::Plan) {
            phase.status = PhaseStatus::Completed;
            phase.completed_at = Some(now_epoch());
            phase.output = Some(plan_result.clone());
        }
        // 更新任务排序（如果提供了 execution_order）
        if let Some(order) = plan_result.get("execution_order").and_then(|v| v.as_array()) {
            let order_ids: Vec<String> = order.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let mut sorted = Vec::new();
            for id in &order_ids {
                if let Some(pos) = project.tasks.iter().position(|t| t.id == *id) {
                    sorted.push(project.tasks.remove(pos));
                }
            }
            // 剩余未在 order 中的任务追加到末尾
            sorted.append(&mut project.tasks);
            project.tasks = sorted;
        }
        project.touch();
        persist_project(project)?;
        return Ok(json!({
            "project_id": project_id,
            "action": "plan",
            "status": "completed",
            "next_step": "调用 spec_driven action=execute project_id=此ID 开始执行"
        }));
    }

    if let Some(phase) = project.phase_mut(PhaseKind::Plan) {
        phase.status = PhaseStatus::InProgress;
        phase.started_at = Some(now_epoch());
    }
    project.touch();
    persist_project(project)?;

    let tasks_json = serde_json::to_value(&project.tasks)?;
    let goal = project.goal.clone();
    let pid = project_id.to_string();
    drop(store);

    if is_passthrough() {
        Ok(json!({
            "type": "passthrough",
            "workflow": "spec_driven",
            "action": "plan",
            "project_id": pid,
            "instruction": "为以下子任务列表生成执行计划。1) 确定执行顺序（拓扑排序）。2) 为每个任务定义 gate_check（进入条件）。3) 设置 rollback_strategy。输出 JSON：{execution_order: [\"T1\",\"T2\",...], gate_checks: {T1: \"...\", T2: \"...\"}, rollback_strategy: \"...\"}。完成后调用 spec_driven：action=plan, project_id=此ID, plan_result=你的计划JSON。",
            "input": { "goal": goal, "tasks": tasks_json },
        }))
    } else {
        // 非 passthrough：简单拓扑排序
        Ok(json!({
            "project_id": pid,
            "action": "plan",
            "status": "in_progress",
            "tasks": tasks_json,
        }))
    }
}

// ── action: execute ───────────────────────────────────────────

fn handle_execute(ctx: &ExecutionContext) -> Result<Value> {
    let project_id = ctx.context.get("project_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("execute requires 'project_id'"))?;

    let mut store = spec_store().lock().unwrap();
    let project = store.get_mut(project_id)
        .ok_or_else(|| anyhow!("project '{}' not found", project_id))?;

    // 标记 execute 阶段为进行中
    if let Some(phase) = project.phase_mut(PhaseKind::Execute) {
        if phase.status == PhaseStatus::Pending {
            phase.status = PhaseStatus::InProgress;
            phase.started_at = Some(now_epoch());
        }
    }

    // 提交任务结果
    if let Some(task_id) = ctx.context.get("task_id").and_then(|v| v.as_str()) {
        if let Some(task) = project.tasks.iter_mut().find(|t| t.id == task_id) {
            if let Some(result) = ctx.context.get("task_result") {
                task.status = PhaseStatus::Completed;
                task.output = Some(result.clone());
                project.lessons.push(SpecLesson {
                    phase: PhaseKind::Execute,
                    content: format!("Task {} ({}) completed", task_id, task.title),
                    success: true,
                    timestamp: now_epoch(),
                });
            } else if let Some(error) = ctx.context.get("task_error").and_then(|v| v.as_str()) {
                task.status = PhaseStatus::Failed;
                task.error = Some(error.to_string());
                project.lessons.push(SpecLesson {
                    phase: PhaseKind::Execute,
                    content: format!("Task {} ({}) failed: {}", task_id, task.title, error),
                    success: false,
                    timestamp: now_epoch(),
                });
            }
        }
    }

    // 检查是否全部完成
    if project.all_tasks_done() {
        if let Some(phase) = project.phase_mut(PhaseKind::Execute) {
            phase.status = PhaseStatus::Completed;
            phase.completed_at = Some(now_epoch());
        }
        // 自动触发 learn 阶段
        let total = project.tasks.len();
        let ok = project.tasks.iter().filter(|t| t.status == PhaseStatus::Completed).count();
        let fail = total - ok;
        let lessons_count = project.lessons.len();
        let learn_output = json!({
            "total_tasks": total,
            "succeeded": ok,
            "failed": fail,
            "lessons_count": lessons_count,
        });
        if let Some(phase) = project.phase_mut(PhaseKind::Learn) {
            phase.status = PhaseStatus::Completed;
            phase.started_at = Some(now_epoch());
            phase.completed_at = Some(now_epoch());
            phase.output = Some(learn_output);
        }
        project.touch();
        persist_project(project)?;

        if let Some(learner) = crate::learner::learner() {
            learner.record("spec_driven", fail == 0, std::time::Duration::from_secs(1));
        }

        return Ok(json!({
            "project_id": project_id,
            "action": "execute",
            "status": "all_done",
            "summary": { "total": total, "succeeded": ok, "failed": fail },
            "lessons": project.lessons,
        }));
    }

    // 找下一个可执行任务
    let next = project.next_executable_task().cloned();
    project.touch();
    persist_project(project)?;

    let pid = project_id.to_string();
    drop(store);

    match next {
        Some(task) => {
            if is_passthrough() {
                Ok(json!({
                    "type": "passthrough",
                    "workflow": "spec_driven",
                    "action": "execute",
                    "project_id": pid,
                    "instruction": format!(
                        "执行以下子任务。完成后调用 spec_driven：action=execute, project_id={}, task_id={}, task_result=你的执行结果。如果失败传 task_error。",
                        pid, task.id
                    ),
                    "task": {
                        "id": task.id,
                        "title": task.title,
                        "description": task.description,
                        "test_strategy": task.test_strategy,
                        "rollback_plan": task.rollback_plan,
                    },
                }))
            } else {
                Ok(json!({
                    "project_id": pid,
                    "action": "execute",
                    "next_task": task,
                }))
            }
        }
        None => Ok(json!({
            "project_id": pid,
            "action": "execute",
            "status": "waiting",
            "message": "没有可执行的任务（可能有依赖未完成）"
        })),
    }
}

// ── action: status ────────────────────────────────────────────

fn handle_status(ctx: &ExecutionContext) -> Result<Value> {
    let workspace = ctx.context.get("workspace")
        .and_then(|v| v.as_str())
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    ensure_loaded(&workspace);

    let store = spec_store().lock().unwrap();

    if let Some(pid) = ctx.context.get("project_id").and_then(|v| v.as_str()) {
        // 查看特定项目
        match store.get(pid) {
            Some(p) => Ok(json!({
                "project_id": p.project_id,
                "goal": p.goal,
                "phases": p.phases,
                "tasks": p.tasks,
                "risks": p.risks,
                "lessons": p.lessons,
                "master_md": format!("{}/.skill-router/specs/MASTER_{}.md", p.workspace_path, p.project_id),
            })),
            None => Err(anyhow!("project '{}' not found", pid)),
        }
    } else {
        // 列出所有项目
        let projects: Vec<Value> = store.values().map(|p| {
            let current_phase = p.phases.iter()
                .find(|ph| ph.status == PhaseStatus::InProgress)
                .map(|ph| ph.kind.to_string())
                .unwrap_or_else(|| "idle".into());
            json!({
                "project_id": p.project_id,
                "goal": p.goal,
                "current_phase": current_phase,
                "task_progress": format!("{}/{}",
                    p.tasks.iter().filter(|t| t.status == PhaseStatus::Completed).count(),
                    p.tasks.len()
                ),
            })
        }).collect();

        Ok(json!({
            "projects": projects,
            "total": projects.len(),
        }))
    }
}
