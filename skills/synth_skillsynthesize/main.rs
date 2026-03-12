#!/usr/bin/env -S cargo run
//! 技能合成技能 (Rust Implementation)
//! 
//! 功能:
//! 1. 根据描述生成新技能
//! 2. 生成技能代码文件
//! 3. 返回技能信息

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

#[derive(Debug, Deserialize)]
struct SkillInput {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SkillOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skill_info: Option<SkillInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

#[derive(Debug, Serialize)]
struct SkillInfo {
    name: String,
    description: String,
    capabilities: Vec<String>,
    path: String,
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // 使用默认输入进行测试
        let default_input = SkillInput {
            name: "test_skill".to_string(),
            description: "测试技能".to_string(),
            capabilities: vec!["test".to_string()],
        };
        process_skill(default_input, start_time)
    } else {
        // 解析 JSON 输入
        match serde_json::from_str::<SkillInput>(&args[1]) {
            Ok(input) => process_skill(input, start_time),
            Err(e) => SkillOutput {
                status: "error".to_string(),
                skill: "synth_skillsynthesize".to_string(),
                skill_info: None,
                code: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_skill(input: SkillInput, start_time: std::time::Instant) -> SkillOutput {
    let skill_name = input.name;
    let skill_path = format!("skills/{}", skill_name);

    // 生成技能代码
    let code = format!(r#"#!/usr/bin/env -S cargo run
//! {} (Rust Implementation)
//!
//! 功能:
//! {}

use serde {{Deserialize, Serialize}};
use std::env;

#[derive(Debug, Deserialize)]
struct SkillInput {{
    #[serde(default)]
    input: String,
}}

#[derive(Debug, Serialize)]
struct SkillOutput {{
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}}

fn main() {{
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {{
        SkillOutput {{
            status: "success".to_string(),
            skill: "{}".to_string(),
            data: Some(serde_json::json!(["result": "default"])),
            error: None,
            duration_ms: 0,
        }}
    }} else {{
        match serde_json::from_str::<SkillInput>(&args[1]) {{
            Ok(_input) => SkillOutput {{
                status: "success".to_string(),
                skill: "{}".to_string(),
                data: Some(serde_json::json!({{"received": true}})),
                error: None,
                duration_ms: 0,
            }},
            Err(e) => SkillOutput {{
                status: "error".to_string(),
                skill: "{}".to_string(),
                data: None,
                error: Some(format!("JSON parse error: {{}}", e)),
                duration_ms: 0,
            }},
        }}
    }};

    println!("{{}}", serde_json::to_string_pretty(&output).unwrap());
}}
"#, input.description, input.description.replace("\n", "\n//! "), skill_name, skill_name, skill_name);

    // 保存技能文件
    let _ = fs::create_dir_all(&skill_path);
    let _ = fs::write(format!("{}/main.rs", skill_path), &code);
    let capabilities_json = input.capabilities.iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(",");
    let _ = fs::write(format!("{}/skill.json", skill_path), &format!(
        r#"{{"name":"{}","version":"0.1.0","description":"{}","capabilities":[{}],"entrypoint":"main.rs"}}"#,
        skill_name, input.description, capabilities_json
    ));

    let duration = start_time.elapsed().as_millis() as u64;
    
    SkillOutput {
        status: "success".to_string(),
        skill: "synth_skillsynthesize".to_string(),
        skill_info: Some(SkillInfo {
            name: skill_name,
            description: input.description,
            capabilities: input.capabilities,
            path: skill_path,
        }),
        code: Some(code),
        error: None,
        duration_ms: duration,
    }
}
