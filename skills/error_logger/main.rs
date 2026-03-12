#!/usr/bin/env -S cargo run
//! 错误日志自动记录技能 (Error Logger Skill)

use serde::{Deserialize, Serialize};
use std::fs;
use std::time::Instant;

const ERROR_LOG_FILE: &str = "ERROR_LOG.md";
const CHECKLIST_FILE: &str = "TROUBLESHOOTING_CHECKLIST.md";

#[derive(Debug, Deserialize)]
struct ErrorInfo {
    title: String,
    #[serde(default)]
    #[serde(alias = "type")]
    error_type: String,
    #[serde(default)]
    affected: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    symptom: String,
    #[serde(default)]
    root_cause: String,
    #[serde(default)]
    solution: String,
    #[serde(default)]
    verification: String,
    #[serde(default)]
    checklist: Vec<String>,
    #[serde(default = "default_notify")]
    notify: bool,
}

fn default_notify() -> bool {
    true
}

#[derive(Debug, Serialize)]
struct SkillResponse {
    status: String,
    skill: String,
    error_id: String,
    duration_ms: u64,
}

fn read_file(filepath: &str) -> String {
    fs::read_to_string(filepath).unwrap_or_default()
}

fn write_file(filepath: &str, content: &str) -> Result<(), String> {
    fs::write(filepath, content).map_err(|e| e.to_string())
}

fn get_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs() as i64;
    if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        "unknown".to_string()
    }
}

fn get_date() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs() as i64;
    if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0) {
        dt.format("%Y-%m-%d").to_string()
    } else {
        "unknown".to_string()
    }
}

fn generate_error_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    format!("ERR-{}", timestamp)
}

fn add_new_error(error_info: &ErrorInfo) -> Result<String, String> {
    let timestamp = get_timestamp();
    let error_id = generate_error_id();
    let date = get_date();

    let error_section = format!(
        r#"
## 错误 {}: {}

> **记录日期**: {}
> **错误类型**: {}
> **影响范围**: {}
> **严重程度**: {}
> **错误ID**: {}

### 现象
{}

### 根本原因
{}

### 解决方案
{}

### 验证结果
{}
"#,
        error_id, error_info.title, timestamp, error_info.error_type,
        error_info.affected, error_info.severity, error_id, error_info.symptom,
        error_info.root_cause, error_info.solution, error_info.verification
    );

    let mut content = read_file(ERROR_LOG_FILE);

    if content.contains("最后更新") {
        content = content.replace(
            "最后更新**: 2026-03-12",
            &format!("最后更新**: {}", date),
        );
    }

    content.push_str(&error_section);

    write_file(ERROR_LOG_FILE, &content)?;

    Ok(error_id)
}

fn update_checklist(checklist: &[String]) -> Result<(), String> {
    let content = read_file(CHECKLIST_FILE);
    let date = get_date();
    let new_items: String = checklist
        .iter()
        .map(|item| format!("- [ ] {}\n", item))
        .collect();

    let new_section = format!("\n## 最近更新 ({})\n\n{}\n", date, new_items);

    let mut updated_content = content;
    updated_content.push_str(&new_section);

    write_file(CHECKLIST_FILE, &updated_content)?;

    Ok(())
}

fn notify_error(error_info: &ErrorInfo, error_id: &str) {
    let date = get_timestamp();

    println!("\n============================================================");
    println!("[ERROR NOTIFICATION] 新错误已记录: {}", error_id);
    println!("============================================================");
    println!("标题: {}", error_info.title);
    println!("类型: {}", error_info.error_type);
    println!("日期: {}", date);
    println!("文件: {}", ERROR_LOG_FILE);
    println!("ID: {}", error_id);
    println!("============================================================\n");
}

fn main() -> Result<(), String> {
    let start_time = Instant::now();
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        let default_error = ErrorInfo {
            title: "PowerShell 命令连接符错误".to_string(),
            error_type: "语法错误".to_string(),
            affected: "所有 Windows 用户".to_string(),
            severity: "中等".to_string(),
            symptom: "用户反馈'又没反应了吗'，命令无响应".to_string(),
            root_cause: "在 PowerShell 中使用了 && 连接符（Bash 语法）".to_string(),
            solution: "改用分号 ; 分隔命令".to_string(),
            verification: "cargo build --release 验证通过".to_string(),
            checklist: vec![
                "确认在项目根目录".to_string(),
                "确认 Rust 已安装".to_string(),
                "确认命令格式正确".to_string(),
            ],
            notify: true,
        };

        process_error(&default_error, start_time)?;
        return Ok(());
    }

    let json_input = &args[1];

    match serde_json::from_str::<ErrorInfo>(json_input) {
        Ok(error_info) => {
            process_error(&error_info, start_time)?;
        }
        Err(e) => {
            eprintln!("[ERROR] 无法解析 JSON 输入: {}", e);
            return Err(format!("JSON parse error: {}", e));
        }
    }

    Ok(())
}

fn process_error(error_info: &ErrorInfo, start_time: Instant) -> Result<(), String> {
    println!("\n[INFO] 正在记录错误...");

    let error_id = add_new_error(error_info)?;
    println!("  [OK] 错误 {} 已记录", error_id);

    if !error_info.checklist.is_empty() {
        match update_checklist(&error_info.checklist) {
            Ok(()) => println!("  [OK] 检查清单已更新 ({}) 项", error_info.checklist.len()),
            Err(e) => eprintln!("  [WARN] 检查清单更新失败: {}", e),
        }
    }

    if error_info.notify {
        notify_error(error_info, &error_id);
    }

    println!("\n[OK] 错误记录完成\n");

    let duration = start_time.elapsed().as_millis() as u64;

    let response = SkillResponse {
        status: "success".to_string(),
        skill: "error_logger".to_string(),
        error_id,
        duration_ms: duration,
    };

    let json_output = serde_json::to_string_pretty(&response).expect("Failed to serialize response");
    println!("{}", json_output);

    Ok(())
}
