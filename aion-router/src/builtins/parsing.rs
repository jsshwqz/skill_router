//! 解析类 builtin 技能：yaml_parse, json_parse, toml_parse, csv_parse, pdf_parse

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::{extract_text, yaml_scalar, BuiltinSkill};

// ── yaml_parse ──────────────────────────────────────────────────────────────

pub struct YamlParse;

#[async_trait::async_trait]
impl BuiltinSkill for YamlParse {
    fn name(&self) -> &'static str { "yaml_parse" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        match parse_yaml_naive(&text) {
            Ok(p) => Ok(json!({"parsed": p, "format": "yaml"})),
            Err(e) => Ok(json!({"error": e.to_string(), "raw": text, "format": "yaml"})),
        }
    }
}

fn parse_yaml_naive(text: &str) -> Result<Value> {
    let mut root = serde_json::Map::new();
    let mut current_key: Option<String> = None;
    let mut list_buf: Vec<Value> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(stripped) = t.strip_prefix("- ") {
            list_buf.push(yaml_scalar(stripped));
            continue;
        }
        if !list_buf.is_empty() {
            if let Some(k) = current_key.take() {
                root.insert(k, Value::Array(std::mem::take(&mut list_buf)));
            }
        }
        if let Some(pos) = t.find(": ") {
            let key = t[..pos].trim().to_string();
            let val = t[pos + 2..].trim();
            if val.is_empty() {
                current_key = Some(key);
            } else {
                root.insert(key, yaml_scalar(val));
            }
        } else if t.ends_with(':') {
            current_key = Some(t.trim_end_matches(':').to_string());
        }
    }
    if !list_buf.is_empty() {
        if let Some(k) = current_key {
            root.insert(k, Value::Array(list_buf));
        }
    }
    if root.is_empty() {
        anyhow::bail!("no key-value pairs found in YAML");
    }
    Ok(Value::Object(root))
}

// ── json_parse ──────────────────────────────────────────────────────────────

pub struct JsonParse;

#[async_trait::async_trait]
impl BuiltinSkill for JsonParse {
    fn name(&self) -> &'static str { "json_parse" }

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
    fn name(&self) -> &'static str { "toml_parse" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        let mut root = serde_json::Map::new();
        let mut section = String::new();
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if t.starts_with('[') && t.ends_with(']') {
                section = t[1..t.len() - 1].to_string();
                root.entry(section.clone())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
                continue;
            }
            if let Some(eq) = t.find(" = ") {
                let key = t[..eq].trim().to_string();
                let val = yaml_scalar(t[eq + 3..].trim());
                if section.is_empty() {
                    root.insert(key, val);
                } else if let Some(Value::Object(sec)) = root.get_mut(&section) {
                    sec.insert(key, val);
                }
            }
        }
        Ok(json!({"parsed": Value::Object(root), "format": "toml"}))
    }
}

// ── csv_parse ───────────────────────────────────────────────────────────────

pub struct CsvParse;

/// RFC 4180 兼容的 CSV 字段分割：支持引号转义、引号内逗号、双引号转义
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes => {
                // 双引号转义 "" → "
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' if !in_quotes && current.is_empty() => {
                in_quotes = true;
            }
            ',' if !in_quotes => {
                fields.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

#[async_trait::async_trait]
impl BuiltinSkill for CsvParse {
    fn name(&self) -> &'static str { "csv_parse" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        let mut lines = text.lines();
        let headers: Vec<String> = split_csv_line(lines.next().unwrap_or(""))
            .into_iter()
            .map(|h| h.trim().to_string())
            .collect();
        let rows: Vec<Value> = lines
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let cells = split_csv_line(line);
                let obj: serde_json::Map<String, Value> = headers
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        (
                            h.clone(),
                            yaml_scalar(cells.get(i).map(|s| s.trim()).unwrap_or("")),
                        )
                    })
                    .collect();
                Value::Object(obj)
            })
            .collect();
        let count = rows.len();
        Ok(json!({"headers": headers, "rows": rows, "count": count, "format": "csv"}))
    }
}

// ── pdf_parse ──────────────────────────────────────────────────────────────

/// 纯 Rust PDF 文本提取器
///
/// 支持两种模式：
/// - 文件路径：context.text = "/path/to/file.pdf"
/// - 直接解析：从已知路径读取 PDF 二进制并提取文本流
///
/// 实现方式：解析 PDF stream 对象中的文本操作符（Tj, TJ, ', "）
pub struct PdfParse;

#[async_trait::async_trait]
impl BuiltinSkill for PdfParse {
    fn name(&self) -> &'static str { "pdf_parse" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let input = extract_text(context);
        let path = input.trim();

        // 读取 PDF 文件
        let data = if std::path::Path::new(path).exists() {
            std::fs::read(path)
                .map_err(|e| anyhow!("无法读取文件 '{}': {}", path, e))?
        } else {
            return Ok(json!({
                "error": format!("文件不存在: {}", path),
                "hint": "请提供有效的 PDF 文件路径",
                "format": "pdf"
            }));
        };

        // 解析 PDF
        let text = extract_pdf_text(&data)?;
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

/// 从 PDF 二进制数据中提取文本
///
/// 轻量级实现：扫描 PDF stream 对象，解压 FlateDecode，提取文本操作符
fn extract_pdf_text(data: &[u8]) -> Result<String> {
    let content = String::from_utf8_lossy(data);
    let mut all_text = String::new();

    // 方法 1：提取 stream...endstream 中的文本操作符
    let mut pos = 0;
    while let Some(start) = content[pos..].find("stream\r\n").or_else(|| content[pos..].find("stream\n")) {
        let stream_start = pos + start + if content[pos + start..].starts_with("stream\r\n") { 8 } else { 7 };
        if let Some(end) = content[stream_start..].find("endstream") {
            let stream_end = stream_start + end;
            let stream_bytes = &data[stream_start..stream_end.min(data.len())];

            // 尝试 FlateDecode 解压
            let decoded = try_flate_decode(stream_bytes)
                .unwrap_or_else(|| stream_bytes.to_vec());

            let stream_text = String::from_utf8_lossy(&decoded);
            extract_text_operators(&stream_text, &mut all_text);

            pos = stream_end + 9;
        } else {
            break;
        }
    }

    // 方法 2：如果 stream 方式没提取到内容，尝试直接找文本字符串
    if all_text.trim().is_empty() {
        // 查找括号内的文本 (text)
        let mut in_parens = false;
        let mut depth = 0;
        let mut current = String::new();
        for ch in content.chars() {
            match ch {
                '(' if !in_parens => { in_parens = true; depth = 1; current.clear(); }
                '(' if in_parens => { depth += 1; current.push(ch); }
                ')' if in_parens => {
                    depth -= 1;
                    if depth == 0 {
                        in_parens = false;
                        let cleaned = unescape_pdf_string(&current);
                        if cleaned.len() > 1 && cleaned.chars().any(|c| c.is_alphanumeric()) {
                            if !all_text.is_empty() { all_text.push(' '); }
                            all_text.push_str(&cleaned);
                        }
                    } else {
                        current.push(ch);
                    }
                }
                _ if in_parens => current.push(ch),
                _ => {}
            }
        }
    }

    if all_text.trim().is_empty() {
        anyhow::bail!("未能从 PDF 中提取到文本。可能是扫描件/图片 PDF，需要 OCR 支持。");
    }

    Ok(all_text.trim().to_string())
}

/// 尝试 zlib/FlateDecode 解压
fn try_flate_decode(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).ok()?;
    if output.is_empty() { None } else { Some(output) }
}

/// 从 PDF content stream 中提取文本操作符（Tj, TJ, ', "）的内容
fn extract_text_operators(stream: &str, out: &mut String) {
    for line in stream.lines() {
        let t = line.trim();
        if t.ends_with("Tj") || t.ends_with("'") || t.ends_with("\"") || t.ends_with("TJ") {
            extract_parens_text(t, out);
            extract_hex_string_text(t, out);
        }
    }
}

/// 提取十六进制字符串 <4865...> 中的文本
fn extract_hex_string_text(line: &str, out: &mut String) {
    let mut pos = 0;
    let bytes = line.as_bytes();
    while pos < bytes.len() {
        if bytes[pos] == b'<' && pos + 1 < bytes.len() && bytes[pos + 1] != b'<' {
            if let Some(end) = line[pos + 1..].find('>') {
                let hex = &line[pos + 1..pos + 1 + end];
                let hex_clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                // 尝试 UTF-16BE 解码（CID 字体常用）
                if hex_clean.len() >= 4 && hex_clean.len().is_multiple_of(4) {
                    let mut decoded = String::new();
                    for chunk in hex_clean.as_bytes().chunks(4) {
                        if let Ok(s) = std::str::from_utf8(chunk) {
                            if let Ok(v) = u16::from_str_radix(s, 16) {
                                if let Some(c) = char::from_u32(v as u32) {
                                    if c.is_alphanumeric() || c.is_ascii_punctuation() || c == ' ' {
                                        decoded.push(c);
                                    }
                                }
                            }
                        }
                    }
                    if !decoded.is_empty() {
                        if !out.is_empty() && !out.ends_with(' ') { out.push(' '); }
                        out.push_str(&decoded);
                    }
                } else {
                    // 单字节十六进制
                    let mut decoded = String::new();
                    for pair in hex_clean.as_bytes().chunks(2) {
                        if let Ok(s) = std::str::from_utf8(pair) {
                            if let Ok(b) = u8::from_str_radix(s, 16) {
                                if b.is_ascii_graphic() || b == b' ' {
                                    decoded.push(b as char);
                                }
                            }
                        }
                    }
                    if !decoded.is_empty() {
                        if !out.is_empty() && !out.ends_with(' ') { out.push(' '); }
                        out.push_str(&decoded);
                    }
                }
                pos = pos + 1 + end + 1;
                continue;
            }
        }
        pos += 1;
    }
}

/// 提取一行中所有 (...) 内的文本
fn extract_parens_text(line: &str, out: &mut String) {
    let mut in_parens = false;
    let mut current = String::new();
    let mut escape = false;

    for ch in line.chars() {
        if escape {
            match ch {
                'n' => current.push('\n'),
                'r' => current.push('\r'),
                't' => current.push('\t'),
                '\\' => current.push('\\'),
                '(' => current.push('('),
                ')' => current.push(')'),
                _ => { current.push('\\'); current.push(ch); }
            }
            escape = false;
            continue;
        }
        match ch {
            '(' if !in_parens => { in_parens = true; current.clear(); }
            ')' if in_parens => {
                in_parens = false;
                if !current.is_empty() {
                    if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('\n') {
                        out.push(' ');
                    }
                    out.push_str(&current);
                }
            }
            '\\' if in_parens => escape = true,
            _ if in_parens => current.push(ch),
            _ => {}
        }
    }
}

/// PDF 字符串反转义
fn unescape_pdf_string(s: &str) -> String {
    let mut out = String::new();
    let mut escape = false;
    for ch in s.chars() {
        if escape {
            match ch {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                _ => out.push(ch),
            }
            escape = false;
        } else if ch == '\\' {
            escape = true;
        } else {
            out.push(ch);
        }
    }
    out
}
