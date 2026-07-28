//! 数据格式转换 builtin：text_toon
//!
//! JSON → TOON (Token-Oriented Object Notation) 转换。
//! TOON 是一种紧凑的 LLM 友好数据格式，比 JSON 省约 40% token。
//!
//! 规范见：https://github.com/toon-format/spec

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use super::BuiltinSkill;
use aion_types::types::{ExecutionContext, SkillDefinition};

/// JSON → TOON 转换
pub struct TextToon;

#[async_trait::async_trait]
impl BuiltinSkill for TextToon {
    fn name(&self) -> &'static str {
        "text_toon"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let input = ctx.context["text"]
            .as_str()
            .or_else(|| ctx.context["input"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();

        let parsed: Value = serde_json::from_str(&input).map_err(|e| anyhow!("无效 JSON: {}", e))?;

        let toon = json_to_toon(&parsed, 0);
        let stats = estimate_savings(&toon, &input);

        Ok(json!({
            "format": "toon",
            "output": toon,
            "stats": stats,
        }))
    }
}

pub(super) fn json_to_toon(value: &Value, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            let mut lines: Vec<String> = Vec::new();
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));

            for (key, val) in entries {
                match val {
                    Value::Array(arr) if is_table_array(arr) => {
                        let fields: Vec<&String> = arr[0].as_object().unwrap().keys().collect();
                        let field_str: Vec<String> = fields.iter().map(|f| (*f).clone()).collect();
                        lines.push(format!("{}{}[{}]{{{} }}:", indent, key, arr.len(), field_str.join(",")));
                        for item in arr {
                            if let Value::Object(item_map) = item {
                                let row: Vec<String> = fields
                                    .iter()
                                    .map(|f| item_map.get(*f).map(val_toon).unwrap_or_default())
                                    .collect();
                                lines.push(format!("{}  {}", indent, row.join(",")));
                            }
                        }
                    }
                    Value::Array(arr) if arr.is_empty() => {
                        lines.push(format!("{}{}:", indent, key));
                    }
                    Value::Array(arr) if is_primitive_array(arr) => {
                        let vals: Vec<String> = arr.iter().map(val_toon).collect();
                        lines.push(format!("{}{}[{}]: {}", indent, key, arr.len(), vals.join(",")));
                    }
                    Value::Array(arr) => {
                        // 复杂数组：每行一个 - 前缀
                        lines.push(format!("{}{}:", indent, key));
                        for item in arr {
                            if item.is_object() {
                                // 对象数组：用 TOON 对象行
                                let obj_lines = json_to_toon(item, 0);
                                for (i, line) in obj_lines.lines().enumerate() {
                                    let trimmed = line.trim_start();
                                    if !trimmed.is_empty() {
                                        if i == 0 {
                                            lines.push(format!("{}  - {}", indent, trimmed));
                                        } else {
                                            lines.push(format!("{}    {}", indent, trimmed));
                                        }
                                    }
                                }
                            } else {
                                // 基本类型数组
                                lines.push(format!("{}  - {}", indent, val_toon(item)));
                            }
                        }
                    }
                    Value::Object(_) => {
                        lines.push(format!("{}{}:", indent, key));
                        let child = json_to_toon(val, depth + 1);
                        for child_line in child.lines() {
                            if !child_line.trim().is_empty() {
                                lines.push(child_line.to_string());
                            }
                        }
                    }
                    _ => {
                        lines.push(format!("{}{}: {}", indent, key, val_toon(val)));
                    }
                }
            }
            lines.join("\n")
        }
        Value::Array(arr) => {
            if is_table_array(arr) {
                let fields: Vec<&String> = arr[0].as_object().unwrap().keys().collect();
                let field_str: Vec<String> = fields.iter().map(|f| (*f).clone()).collect();
                let mut lines = vec![format!("{}[{}]{{{} }}:", indent, arr.len(), field_str.join(","))];
                for item in arr {
                    if let Value::Object(item_map) = item {
                        let row: Vec<String> = fields
                            .iter()
                            .map(|f| item_map.get(*f).map(val_toon).unwrap_or_default())
                            .collect();
                        lines.push(format!("{}  {}", indent, row.join(",")));
                    }
                }
                lines.join("\n")
            } else {
                let vals: Vec<String> = arr.iter().map(val_toon).collect();
                format!("{}[{}]: {}", indent, arr.len(), vals.join(","))
            }
        }
        _ => val_toon(value),
    }
}

fn val_toon(val: &Value) -> String {
    match val {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.contains(',') || s.contains(':') || s.contains('"') || s.contains('\n') || s.is_empty() {
                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                s.clone()
            }
        }
        Value::Object(_) | Value::Array(_) => json_to_toon(val, 0),
    }
}

fn is_table_array(arr: &[Value]) -> bool {
    if arr.is_empty() {
        return false;
    }
    let first = match arr[0].as_object() {
        Some(o) => o,
        None => return false,
    };
    let keys: Vec<&String> = first.keys().collect();
    arr.iter().all(|item| {
        item.as_object()
            .is_some_and(|map| map.keys().collect::<Vec<_>>() == keys)
    })
}

fn is_primitive_array(arr: &[Value]) -> bool {
    arr.iter().all(|v| !v.is_object() && !v.is_array())
}

fn estimate_savings(toon: &str, json: &str) -> Value {
    let toon_tokens = (toon.len() / 4).max(1);
    let json_tokens = (json.len() / 4).max(1);
    let savings = if json_tokens > toon_tokens {
        ((json_tokens - toon_tokens) as f64 / json_tokens as f64 * 100.0).round()
    } else if json_tokens < toon_tokens {
        -((toon_tokens - json_tokens) as f64 / toon_tokens as f64 * 100.0).round()
    } else {
        0.0
    };

    json!({
        "json_chars": json.len(),
        "toon_chars": toon.len(),
        "estimated_json_tokens": json_tokens,
        "estimated_toon_tokens": toon_tokens,
        "savings_percent": savings,
    })
}
