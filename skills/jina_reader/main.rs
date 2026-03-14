#!/usr/bin/env -S cargo run
//! Jina Reader 技能 (Rust Implementation)
//! 
//! 功能:
//! 1. 使用 Jina Reader API 提取网页内容
//! 2. 返回干净的 Markdown 格式内容
//! 3. 支持超时控制和错误处理

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct ReaderInput {
    url: String,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Debug, Serialize)]
struct ReaderOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // 测试模式：使用默认URL
        process_reader("https://example.com", 30, start_time).await
    } else {
        // 解析输入参数
        match serde_json::from_str::<ReaderInput>(&args[1]) {
            Ok(input) => {
                // 验证URL格式
                if url::Url::parse(&input.url).is_err() {
                    ReaderOutput {
                        status: "error".to_string(),
                        skill: "jina_reader".to_string(),
                        content: None,
                        error: Some(format!("Invalid URL format: {}", input.url)),
                        duration_ms: 0,
                    }
                } else {
                    process_reader(&input.url, input.timeout_seconds, start_time).await
                }
            }
            Err(e) => ReaderOutput {
                status: "error".to_string(),
                skill: "jina_reader".to_string(),
                content: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

async fn process_reader(url: &str, timeout_seconds: u64, start_time: std::time::Instant) -> ReaderOutput {
    let jina_url = format!("https://r.jina.ai/{}", url);
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .user_agent("Skill-Router/0.1.0")
        .build()
        .unwrap();

    let response = match client.get(&jina_url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            return ReaderOutput {
                status: "error".to_string(),
                skill: "jina_reader".to_string(),
                content: None,
                error: Some(format!("Network error: {}", e)),
                duration_ms: start_time.elapsed().as_millis() as u64,
            };
        }
    };

    let duration = start_time.elapsed().as_millis() as u64;

    if !response.status().is_success() {
        return ReaderOutput {
            status: "error".to_string(),
            skill: "jina_reader".to_string(),
            content: None,
            error: Some(format!(
                "Jina Reader returned status: {}",
                response.status()
            )),
            duration_ms: duration,
        };
    }

    match response.text().await {
        Ok(content) => ReaderOutput {
            status: "success".to_string(),
            skill: "jina_reader".to_string(),
            content: Some(content),
            error: None,
            duration_ms: duration,
        },
        Err(e) => ReaderOutput {
            status: "error".to_string(),
            skill: "jina_reader".to_string(),
            content: None,
            error: Some(format!("Content decode error: {}", e)),
            duration_ms: duration,
        },
    }
}