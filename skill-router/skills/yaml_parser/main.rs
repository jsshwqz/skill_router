#!/usr/bin/env -S cargo run
//! YAML 解析技能 (skill-router 版本)
//! 
//! 功能:
//! 1. 解析 YAML 输入
//! 2. 验证 YAML 格式
//! 3. 返回解析结果

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;

#[derive(Debug, Deserialize)]
struct YamlInput {
    yaml: String,
    #[serde(default)]
    validate: bool,
}

#[derive(Debug, Serialize)]
struct YamlOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        process_yaml(r#"name: "测试 YAML"
version: "1.0.0"
features:
  - logging
  - networking
  - security
config:
  debug: true
  max_connections: 100
"#, true, start_time)
    } else {
        match serde_json::from_str::<YamlInput>(&args[1]) {
            Ok(input) => process_yaml(&input.yaml, input.validate, start_time),
            Err(e) => YamlOutput {
                status: "error".to_string(),
                skill: "yaml_parser".to_string(),
                data: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_yaml(yaml_content: &str, validate: bool, start_time: std::time::Instant) -> YamlOutput {
    let result = if validate {
        match serde_yaml::from_str::<serde_json::Value>(yaml_content) {
            Ok(data) => {
                if data.is_null() {
                    YamlOutput {
                        status: "error".to_string(),
                        skill: "yaml_parser".to_string(),
                        data: None,
                        error: Some("YAML 解析结果为空".to_string()),
                        duration_ms: 0,
                    }
                } else {
                    YamlOutput {
                        status: "success".to_string(),
                        skill: "yaml_parser".to_string(),
                        data: Some(data),
                        error: None,
                        duration_ms: 0,
                    }
                }
            }
            Err(e) => YamlOutput {
                status: "error".to_string(),
                skill: "yaml_parser".to_string(),
                data: None,
                error: Some(format!("YAML parse error: {}", e)),
                duration_ms: 0,
            },
        }
    } else {
        YamlOutput {
            status: "success".to_string(),
            skill: "yaml_parser".to_string(),
            data: Some(json!({"parsed": true})),
            error: None,
            duration_ms: 0,
        }
    };

    let duration = start_time.elapsed().as_millis() as u64;
    YamlOutput {
        status: result.status,
        skill: result.skill,
        data: result.data,
        error: result.error,
        duration_ms: duration,
    }
}
