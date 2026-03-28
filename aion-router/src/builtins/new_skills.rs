//! 新增 builtin 技能：json_query, regex_match, skill_report

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::{extract_text, BuiltinSkill};

// ── json_query ──────────────────────────────────────────────────────────────

/// JSONPath 查询：从 JSON 数据中提取匹配路径的值
///
/// context 字段：
/// - `data`: JSON 对象或数组（必填）
/// - `path`: JSONPath 表达式，如 `$.store.book[0].title`（必填）
pub struct JsonQuery;

#[async_trait::async_trait]
impl BuiltinSkill for JsonQuery {
    fn name(&self) -> &'static str { "json_query" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let data_str = context.context["data"]
            .as_str()
            .ok_or_else(|| anyhow!("json_query requires 'data' (JSON string) in context"))?;
        let path = context.context["path"]
            .as_str()
            .ok_or_else(|| anyhow!("json_query requires 'path' (JSONPath expression) in context"))?;

        let data: Value = serde_json::from_str(data_str)
            .map_err(|e| anyhow!("invalid JSON in 'data': {}", e))?;

        // 简易 JSONPath 实现：支持 $.key.subkey[N] 格式
        let results = simple_jsonpath(&data, path);

        Ok(json!({
            "path": path,
            "matches": results,
            "count": results.len(),
        }))
    }
}

/// 简易 JSONPath 查询（支持 $.a.b[0].c 风格路径）
fn simple_jsonpath(data: &Value, path: &str) -> Vec<Value> {
    let path = path.trim_start_matches('$').trim_start_matches('.');
    if path.is_empty() {
        return vec![data.clone()];
    }

    let mut current = vec![data.clone()];
    for segment in split_path(path) {
        let mut next = Vec::new();
        for val in &current {
            if let Some(idx_str) = segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                // 数组索引
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(item) = val.as_array().and_then(|arr| arr.get(idx)) {
                        next.push(item.clone());
                    }
                } else if idx_str == "*" {
                    if let Some(arr) = val.as_array() {
                        next.extend(arr.iter().cloned());
                    }
                }
            } else {
                // 对象 key
                if let Some(v) = val.get(&segment) {
                    next.push(v.clone());
                }
            }
        }
        current = next;
    }
    current
}

/// 将 JSONPath 路径分割为段
fn split_path(path: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_bracket = false;

    for ch in path.chars() {
        match ch {
            '[' => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                in_bracket = true;
                current.push('[');
            }
            ']' => {
                current.push(']');
                segments.push(std::mem::take(&mut current));
                in_bracket = false;
            }
            '.' if !in_bracket => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

// ── regex_match ─────────────────────────────────────────────────────────────

/// 正则表达式匹配
///
/// context 字段：
/// - `text`/`input`: 待匹配文本（必填）
/// - `pattern`: 正则表达式（必填）
/// - `mode`: "find_all"（默认）/ "is_match" / "captures"
pub struct RegexMatch;

#[async_trait::async_trait]
impl BuiltinSkill for RegexMatch {
    fn name(&self) -> &'static str { "regex_match" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = extract_text(context);
        let pattern = context.context["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("regex_match requires 'pattern' in context"))?;
        let mode = context.context["mode"].as_str().unwrap_or("find_all");

        let re = regex::Regex::new(pattern)
            .map_err(|e| anyhow!("invalid regex '{}': {}", pattern, e))?;

        match mode {
            "is_match" => {
                let matched = re.is_match(&text);
                Ok(json!({
                    "pattern": pattern,
                    "is_match": matched,
                }))
            }
            "captures" => {
                let caps: Vec<Value> = re
                    .captures_iter(&text)
                    .map(|cap| {
                        let groups: Vec<Value> = cap
                            .iter()
                            .map(|m| match m {
                                Some(m) => json!(m.as_str()),
                                None => Value::Null,
                            })
                            .collect();
                        json!(groups)
                    })
                    .collect();
                Ok(json!({
                    "pattern": pattern,
                    "captures": caps,
                    "count": caps.len(),
                }))
            }
            _ => {
                // find_all
                let matches: Vec<&str> = re.find_iter(&text).map(|m| m.as_str()).collect();
                Ok(json!({
                    "pattern": pattern,
                    "matches": matches,
                    "count": matches.len(),
                }))
            }
        }
    }
}

// ── code_lint ─────────────────────────────────────────────────────────────

/// 代码静态检查：纯 Rust 规则检测（不调 AI）
///
/// 检测：TODO/FIXME、println!/console.log、长行、空 catch、硬编码密码模式等
pub struct CodeLint;

#[async_trait::async_trait]
impl BuiltinSkill for CodeLint {
    fn name(&self) -> &'static str { "code_lint" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let code = extract_text(context);
        let mut issues: Vec<Value> = Vec::new();

        for (i, line) in code.lines().enumerate() {
            let lineno = i + 1;
            let trimmed = line.trim();

            // TODO/FIXME 未处理
            if trimmed.contains("TODO") || trimmed.contains("FIXME") || trimmed.contains("HACK") {
                issues.push(json!({"line": lineno, "severity": "info", "rule": "todo-comment", "message": "未处理的 TODO/FIXME/HACK 注释"}));
            }

            // 调试输出残留
            for pattern in &["println!", "print!", "dbg!", "console.log", "console.error", "System.out.print"] {
                if trimmed.contains(pattern) && !trimmed.starts_with("//") && !trimmed.starts_with("#") {
                    issues.push(json!({"line": lineno, "severity": "warning", "rule": "debug-output", "message": format!("可能的调试输出残留: {}", pattern)}));
                }
            }

            // 长行（超过 120 字符）
            if line.len() > 120 && !trimmed.starts_with("//") && !trimmed.starts_with("#") {
                issues.push(json!({"line": lineno, "severity": "info", "rule": "long-line", "message": format!("行长 {} 超过 120 字符", line.len())}));
            }

            // 硬编码密码/密钥模式
            let lower = trimmed.to_lowercase();
            for keyword in &["password", "secret", "api_key", "apikey", "token", "private_key"] {
                if lower.contains(keyword) && (trimmed.contains("=\"") || trimmed.contains("= \"") || trimmed.contains("='")) {
                    issues.push(json!({"line": lineno, "severity": "error", "rule": "hardcoded-secret", "message": format!("疑似硬编码 {} ", keyword)}));
                }
            }

            // 空 catch/except 块
            if trimmed == "catch {}" || trimmed == "catch (e) {}" || trimmed == "except:" || trimmed == "except Exception:" {
                issues.push(json!({"line": lineno, "severity": "warning", "rule": "empty-catch", "message": "空异常处理块，可能吞没错误"}));
            }

            // unwrap() 使用（Rust 特定）
            if trimmed.contains(".unwrap()") && !trimmed.starts_with("//") {
                issues.push(json!({"line": lineno, "severity": "warning", "rule": "unwrap-usage", "message": "使用 unwrap() 可能 panic，考虑用 ? 或 unwrap_or"}));
            }
        }

        let errors = issues.iter().filter(|i| i["severity"] == "error").count();
        let warnings = issues.iter().filter(|i| i["severity"] == "warning").count();
        let infos = issues.iter().filter(|i| i["severity"] == "info").count();
        let score = if issues.is_empty() { 10 } else { (10 - errors * 3 - warnings).max(0).min(10) };

        Ok(json!({
            "issues": issues,
            "summary": { "errors": errors, "warnings": warnings, "infos": infos, "total": issues.len() },
            "score": format!("{}/10", score),
            "lines_analyzed": code.lines().count(),
        }))
    }
}

// ── code_test ─────────────────────────────────────────────────────────────

/// 测试脚手架生成：分析代码结构，生成测试骨架
pub struct CodeTest;

#[async_trait::async_trait]
impl BuiltinSkill for CodeTest {
    fn name(&self) -> &'static str { "code_test" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let code = extract_text(context);

        // 检测语言和提取函数签名
        let mut functions: Vec<Value> = Vec::new();
        let mut language = "unknown";

        for (i, line) in code.lines().enumerate() {
            let trimmed = line.trim();

            // Rust: fn name(...)
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") || trimmed.starts_with("pub async fn ") || trimmed.starts_with("async fn ") {
                language = "rust";
                if let Some(name) = extract_fn_name(trimmed) {
                    functions.push(json!({"name": name, "line": i + 1, "language": "rust"}));
                }
            }
            // Python: def name(...)
            else if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                language = "python";
                if let Some(name) = extract_fn_name(trimmed) {
                    functions.push(json!({"name": name, "line": i + 1, "language": "python"}));
                }
            }
            // JS/TS: function name(...) or const name = (...) =>
            else if trimmed.starts_with("function ") || trimmed.starts_with("export function ") {
                language = "javascript";
                if let Some(name) = extract_fn_name(trimmed) {
                    functions.push(json!({"name": name, "line": i + 1, "language": "javascript"}));
                }
            }
        }

        // 生成测试骨架
        let scaffold = match language {
            "rust" => generate_rust_tests(&functions),
            "python" => generate_python_tests(&functions),
            "javascript" => generate_js_tests(&functions),
            _ => "// 未识别的语言，无法生成测试骨架".to_string(),
        };

        Ok(json!({
            "language": language,
            "functions_found": functions.len(),
            "functions": functions,
            "test_scaffold": scaffold,
        }))
    }
}

fn extract_fn_name(line: &str) -> Option<String> {
    let line = line.trim()
        .trim_start_matches("pub ")
        .trim_start_matches("async ")
        .trim_start_matches("export ")
        .trim_start_matches("fn ")
        .trim_start_matches("def ")
        .trim_start_matches("function ");
    let name: String = line.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if name.is_empty() || name == "main" { None } else { Some(name) }
}

fn generate_rust_tests(functions: &[Value]) -> String {
    let mut out = String::from("#[cfg(test)]\nmod tests {\n    use super::*;\n\n");
    for f in functions {
        let name = f["name"].as_str().unwrap_or("unknown");
        out.push_str(&format!("    #[test]\n    fn test_{}() {{\n        // TODO: 实现测试\n        todo!(\"test {}\");\n    }}\n\n", name, name));
    }
    out.push_str("}\n");
    out
}

fn generate_python_tests(functions: &[Value]) -> String {
    let mut out = String::from("import pytest\n\n");
    for f in functions {
        let name = f["name"].as_str().unwrap_or("unknown");
        out.push_str(&format!("def test_{}():\n    # TODO: 实现测试\n    pass\n\n", name));
    }
    out
}

fn generate_js_tests(functions: &[Value]) -> String {
    let mut out = String::from("describe('module', () => {\n");
    for f in functions {
        let name = f["name"].as_str().unwrap_or("unknown");
        out.push_str(&format!("  test('{}', () => {{\n    // TODO: 实现测试\n    expect(true).toBe(true);\n  }});\n\n", name));
    }
    out.push_str("});\n");
    out
}

// ── skill_report ──────────────────────────────────────────────────────────

/// 技能学习报告：展示所有能力的执行统计、成功率、质量评分
pub struct SkillReport;

#[async_trait::async_trait]
impl BuiltinSkill for SkillReport {
    fn name(&self) -> &'static str { "skill_report" }

    async fn execute(&self, _skill: &SkillDefinition, _context: &ExecutionContext) -> Result<Value> {
        match crate::learner::learner() {
            Some(learner) => Ok(learner.report()),
            None => Ok(json!({"error": "学习引擎未初始化"})),
        }
    }
}
