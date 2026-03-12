#!/usr/bin/env -S cargo run
//! JSON 解析合成技能 (Rust Implementation)
//! 
//! 功能:
//! 1. 分析输入 JSON
//! 2. 提取关键信息
//! 3. 返回结构化数据

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Deserialize)]
struct JsonInput {
    json: String,
    #[serde(default)]
    extract_fields: Vec<String>,
}

#[derive(Debug, Serialize)]
struct JsonOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parsed: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extracted: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // 使用默认 JSON 进行测试
        let default_json = r#"{"name": "测试", "version": "1.0.0", "features": ["a", "b"]}"#;
        process_json(default_json, vec![], start_time)
    } else {
        // 解析 JSON 输入
        match serde_json::from_str::<JsonInput>(&args[1]) {
            Ok(input) => process_json(&input.json, input.extract_fields, start_time),
            Err(e) => JsonOutput {
                status: "error".to_string(),
                skill: "synth_jsonparse".to_string(),
                parsed: None,
                extracted: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_json(json_content: &str, extract_fields: Vec<String>, start_time: std::time::Instant) -> JsonOutput {
    // 解析 JSON
    let parsed = match serde_json::from_str::<serde_json::Value>(json_content) {
        Ok(v) => v,
        Err(e) => {
            return JsonOutput {
                status: "error".to_string(),
                skill: "synth_jsonparse".to_string(),
                parsed: None,
                extracted: None,
                error: Some(format!("Invalid JSON: {}", e)),
                duration_ms: 0,
            };
        }
    };

    // 提取指定字段
    let extracted = if extract_fields.is_empty() {
        None
    } else {
        let mut result = serde_json::Map::new();
        if let serde_json::Value::Object(obj) = &parsed {
            for field in extract_fields {
                if let Some(value) = obj.get(&field) {
                    result.insert(field, value.clone());
                }
            }
        }
        if result.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(result))
        }
    };

    let duration = start_time.elapsed().as_millis() as u64;
    
    JsonOutput {
        status: "success".to_string(),
        skill: "synth_jsonparse".to_string(),
        parsed: Some(parsed),
        extracted,
        error: None,
        duration_ms: duration,
    }
}
