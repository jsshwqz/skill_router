//! 解析类 builtin 技能：yaml_parse, json_parse, toml_parse, csv_parse, pdf_parse

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use yaml_rust2::{Yaml, YamlLoader};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::{extract_text, yaml_scalar, BuiltinSkill};

// ── yaml_parse ──────────────────────────────────────────────────────────────

pub struct YamlParse;

#[async_trait::async_trait]
impl BuiltinSkill for YamlParse {
    fn name(&self) -> &'static str {
        "yaml_parse"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        match parse_yaml_naive(&text) {
            Ok(p) => Ok(json!({"parsed": p, "format": "yaml"})),
            Err(e) => Ok(json!({"error": e.to_string(), "raw": text, "format": "yaml"})),
        }
    }
}

fn parse_yaml_naive(text: &str) -> Result<Value> {
    let docs = YamlLoader::load_from_str(text).map_err(|e| anyhow!("YAML 解析失败: {}", e))?;
    let doc = docs.first().cloned().unwrap_or(Yaml::BadValue);
    let value = yaml_to_value(&doc);
    match &value {
        Value::Object(m) if m.is_empty() => anyhow::bail!("no key-value pairs found in YAML"),
        Value::Null => anyhow::bail!("no key-value pairs found in YAML"),
        _ => Ok(value),
    }
}

/// 将 yaml-rust2 的 Yaml 值转换为 serde_json::Value
fn yaml_to_value(y: &Yaml) -> Value {
    match y {
        Yaml::Real(r) => r
            .parse::<f64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(r.clone())),
        Yaml::Integer(i) => Value::from(*i),
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Array(a) => Value::Array(a.iter().map(yaml_to_value).collect()),
        Yaml::Hash(h) => {
            let mut map = serde_json::Map::new();
            for (k, v) in h.iter() {
                if let Some(key) = k.as_str() {
                    map.insert(key.to_string(), yaml_to_value(v));
                }
            }
            Value::Object(map)
        }
        Yaml::Null | Yaml::BadValue => Value::Null,
        // Alias 在 YamlLoader 阶段已解析为实际节点，此处兜底
        Yaml::Alias(_) => Value::Null,
    }
}

// ── json_parse ──────────────────────────────────────────────────────────────

pub struct JsonParse;

#[async_trait::async_trait]
impl BuiltinSkill for JsonParse {
    fn name(&self) -> &'static str {
        "json_parse"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        match serde_json::from_str::<Value>(&text) {
            Ok(p) => Ok(json!({"parsed": p, "format": "json"})),
            Err(e) => Ok(json!({"error": e.to_string(), "raw": text, "format": "json"})),
        }
    }
}

// ── toml_parse ──────────────────────────────────────────────────────────────

pub struct TomlParse;

#[async_trait::async_trait]
impl BuiltinSkill for TomlParse {
    fn name(&self) -> &'static str {
        "toml_parse"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        match toml::from_str::<toml::Value>(&text) {
            Ok(v) => {
                let parsed = serde_json::to_value(v)?;
                Ok(json!({"parsed": parsed, "format": "toml"}))
            }
            Err(e) => Ok(json!({"error": e.to_string(), "raw": text, "format": "toml"})),
        }
    }
}

// ── csv_parse ───────────────────────────────────────────────────────────────

pub struct CsvParse;

#[async_trait::async_trait]
impl BuiltinSkill for CsvParse {
    fn name(&self) -> &'static str {
        "csv_parse"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(text.as_bytes());
        let headers: Vec<String> = rdr
            .headers()
            .map_err(|e| anyhow!("CSV header 解析失败: {}", e))?
            .iter()
            .map(|h| h.trim().to_string())
            .collect();
        let mut rows: Vec<Value> = Vec::new();
        for record in rdr.records() {
            let record = record.map_err(|e| anyhow!("CSV 解析失败: {}", e))?;
            if record.iter().all(|c| c.trim().is_empty()) {
                continue;
            }
            let obj: serde_json::Map<String, Value> = headers
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let cell = record.get(i).map(str::trim).unwrap_or("");
                    (h.clone(), yaml_scalar(cell))
                })
                .collect();
            rows.push(Value::Object(obj));
        }
        let count = rows.len();
        Ok(json!({"headers": headers, "rows": rows, "count": count, "format": "csv"}))
    }
}

// ── pdf_parse ──────────────────────────────────────────────────────────────

/// PDF 文本提取器（基于 pdf-extract 库）
///
/// 支持两种模式：
/// - 文件路径：context.text = "/path/to/file.pdf"
/// - 直接解析：从已知路径读取 PDF 二进制并提取文本流
pub struct PdfParse;

#[async_trait::async_trait]
impl BuiltinSkill for PdfParse {
    fn name(&self) -> &'static str {
        "pdf_parse"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let input = extract_text(context);
        let path = input.trim();

        // 读取 PDF 文件
        let data = if std::path::Path::new(path).exists() {
            std::fs::read(path).map_err(|e| anyhow!("无法读取文件 '{}': {}", path, e))?
        } else {
            return Ok(json!({
                "error": format!("文件不存在: {}", path),
                "hint": "请提供有效的 PDF 文件路径",
                "format": "pdf"
            }));
        };

        // 解析 PDF
        let text = pdf_extract::extract_text_from_mem(&data).map_err(|e| anyhow!("PDF 文本提取失败: {}", e))?;
        let pages = text.matches('\x0C').count().max(1); // form feed = page break
        let char_count = text.len();

        Ok(json!({
            "text": text,
            "pages_estimated": pages,
            "characters": char_count,
            "source": path,
            "format": "pdf"
        }))
    }
}
