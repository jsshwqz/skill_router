#!/usr/bin/env -S cargo run
//! 任务拆解与协调器 (Rust Implementation)
//! 
//! 功能:
//! 1. 分析复杂任务
//! 2. 拆解为子任务
//! 3. 调用其他技能执行
//! 4. 记录技能使用历史 (MemOS inspired)

use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct TaskInput {
    task: String,
}

#[derive(Debug, Serialize)]
struct TaskOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    results: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // 使用默认任务进行测试
        process_task("分析项目并生成报告", start_time)
    } else {
        // 解析 JSON 输入
        match serde_json::from_str::<TaskInput>(&args[1]) {
            Ok(input) => process_task(&input.task, start_time),
            Err(e) => TaskOutput {
                status: "error".to_string(),
                skill: "autonomous_orchestrator".to_string(),
                steps: None,
                results: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    // 记录技能使用历史
    if output.status == "success" {
        record_skill_usage(
            "autonomous_orchestrator",
            &args.get(1).unwrap_or(&"".to_string()),
            &serde_json::to_string_pretty(&output).unwrap_or_default(),
            true,
            output.duration_ms,
            vec!["task_orchestration".to_string(), "autonomous_execution".to_string()],
        );
    } else {
        record_skill_usage(
            "autonomous_orchestrator",
            &args.get(1).unwrap_or(&"".to_string()),
            &serde_json::to_string_pretty(&output).unwrap_or_default(),
            false,
            output.duration_ms,
            vec!["task_orchestration".to_string(), "execution_failed".to_string()],
        );
    }

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_task(task: &str, start_time: std::time::Instant) -> TaskOutput {
    // 拆解任务
    let steps = decompose_task(task);

    // 依次执行子任务（这里使用模拟）
    let results = execute_steps(&steps);

    let duration = start_time.elapsed().as_millis() as u64;
    
    TaskOutput {
        status: "success".to_string(),
        skill: "autonomous_orchestrator".to_string(),
        steps: Some(steps),
        results: Some(results),
        error: None,
        duration_ms: duration,
    }
}

fn decompose_task(task: &str) -> Vec<String> {
    let mut steps = Vec::new();
    
    // 简单的任务拆解逻辑
    if task.contains("分析") || task.contains("报告") {
        steps.push("1. 分析任务需求和目标".to_string());
        steps.push("2. 搜集相关信息和数据".to_string());
        steps.push("3. 整理和分析数据".to_string());
        steps.push("4. 生成分析报告".to_string());
    } else if task.contains("构建") || task.contains("编译") {
        steps.push("1. 检查项目结构".to_string());
        steps.push("2. 安装依赖".to_string());
        steps.push("3. 执行构建".to_string());
        steps.push("4. 验证构建结果".to_string());
    } else {
        steps.push(format!("执行任务: {}", task));
        steps.push("完成任务".to_string());
    }
    
    steps
}

fn execute_steps(steps: &[String]) -> serde_json::Value {
    let mut results = serde_json::Map::new();
    
    for (i, step) in steps.iter().enumerate() {
        results.insert(
            format!("step_{}", i + 1),
            serde_json::json!({
                "task": step,
                "status": "success",
                "result": "完成"
            })
        );
    }
    
    serde_json::Value::Object(results)
}

// ===== 技能记忆记录函数 =====
fn record_skill_usage(
    skill_id: &str,
    input: &str,
    output: &str,
    success: bool,
    execution_time_ms: u64,
    tags: Vec<String>,
) {
    // 构建 memory_manager 命令参数
    let mut args = vec![
        "record-skill-usage".to_string(),
        skill_id.to_string(),
        input.to_string(),
        output.to_string(),
        success.to_string(),
        execution_time_ms.to_string(),
    ];
    
    // 添加标签
    for tag in tags {
        args.push(format!("#{}", tag));
    }

    // 调用 memory_manager 记录技能使用
    let _ = Command::new("./skills/memory_manager/target/release/memory_manager.exe")
        .args(&args)
        .output();
}