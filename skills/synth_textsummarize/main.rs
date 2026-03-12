#!/usr/bin/env -S cargo run
//! 文本摘要技能 (Rust Implementation)
//! 
//! 功能:
//! 1. 接收长文本输入
//! 2. 生成文本摘要
//! 3. 返回摘要结果

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Deserialize)]
struct SummaryInput {
    text: String,
    #[serde(default)]
    max_length: usize,
}

#[derive(Debug, Serialize)]
struct SummaryOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // 使用默认文本进行测试
        let default_text = "这是很长的文本内容。它包含多个句子和段落。我们需要从中提取关键信息。生成简洁的摘要。";
        process_summary(default_text, 50, start_time)
    } else {
        // 解析 JSON 输入
        match serde_json::from_str::<SummaryInput>(&args[1]) {
            Ok(input) => process_summary(&input.text, input.max_length, start_time),
            Err(e) => SummaryOutput {
                status: "error".to_string(),
                skill: "synth_textsummarize".to_string(),
                summary: None,
                original_length: None,
                summary_length: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_summary(text: &str, max_length: usize, start_time: std::time::Instant) -> SummaryOutput {
    let original_length = text.len();
    
    // 简单的摘要逻辑（真实场景应使用 LLM 或 NLP 库）
    let summary = if text.len() <= max_length {
        Some(text.to_string())
    } else {
        // 截断文本并添加省略号
        let truncated = &text[..max_length.saturating_sub(3)];
        Some(format!("{}...", truncated))
    };

    let summary_length = summary.as_ref().map(|s| s.len());

    let duration = start_time.elapsed().as_millis() as u64;
    
    SummaryOutput {
        status: "success".to_string(),
        skill: "synth_textsummarize".to_string(),
        summary,
        original_length: Some(original_length),
        summary_length,
        error: None,
        duration_ms: duration,
    }
}
