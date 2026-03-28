//! 文本处理类 builtin 技能：text_diff, text_embed, markdown_render

use anyhow::Result;
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::{require_text, BuiltinSkill};

// ── text_diff ───────────────────────────────────────────────────────────────

pub struct TextDiff;

#[async_trait::async_trait]
impl BuiltinSkill for TextDiff {
    fn name(&self) -> &'static str { "text_diff" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let a = context.context["a"].as_str().unwrap_or("");
        let b = context.context["b"].as_str().unwrap_or("");
        let al: Vec<&str> = a.lines().collect();
        let bl: Vec<&str> = b.lines().collect();

        // LCS-based unified diff
        let lcs = lcs_table(&al, &bl);
        let mut diff: Vec<Value> = Vec::new();
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut unchanged = 0usize;

        lcs_diff(&lcs, &al, &bl, al.len(), bl.len(), &mut |op, line| {
            match op {
                '-' => { removed += 1; diff.push(json!({"op": "-", "line": line})); }
                '+' => { added += 1; diff.push(json!({"op": "+", "line": line})); }
                _ => { unchanged += 1; diff.push(json!({"op": " ", "line": line})); }
            }
        });

        Ok(json!({"added": added, "removed": removed, "unchanged": unchanged, "diff": diff}))
    }
}

// ── text_embed ──────────────────────────────────────────────────────────────

pub struct TextEmbed;

#[async_trait::async_trait]
impl BuiltinSkill for TextEmbed {
    fn name(&self) -> &'static str { "text_embed" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = require_text(context)?;

        // 停用词（中英文常见）
        const STOPWORDS: &[&str] = &[
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "do", "does", "did", "will", "would", "could",
            "should", "may", "might", "can", "shall", "to", "of", "in", "for",
            "on", "with", "at", "by", "from", "as", "into", "through", "and",
            "but", "or", "not", "no", "if", "then", "than", "that", "this",
            "it", "its", "they", "them", "their", "we", "our", "you", "your",
            "的", "了", "是", "在", "和", "有", "我", "他", "她", "它", "们",
            "这", "那", "就", "也", "都", "被", "把", "让", "用", "不",
        ];

        let mut freq: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut doc_words: std::collections::HashSet<String> = std::collections::HashSet::new();

        for word in text.split(|c: char| c.is_whitespace() || c == ',' || c == '.' || c == '。' || c == '，') {
            let w = word
                .to_ascii_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string();
            if w.len() > 1 && !STOPWORDS.contains(&w.as_str()) {
                *freq.entry(w.clone()).or_insert(0) += 1;
                doc_words.insert(w);
            }
        }

        let total: usize = freq.values().sum();
        let terms = freq.len();
        let doc_count = doc_words.len().max(1) as f64;

        // TF-IDF: TF * log(N/df)，单文档近似 IDF = log(terms/1)
        let vector: serde_json::Map<String, Value> = freq
            .into_iter()
            .map(|(k, v)| {
                let tf = v as f64 / total.max(1) as f64;
                let idf = (doc_count / 1.0).ln().max(1.0); // 单文档 IDF 近似
                (k, json!((tf * idf * 1000.0).round() / 1000.0))
            })
            .collect();

        Ok(json!({"method": "tf_idf", "vector": vector, "terms": terms, "total_words": total}))
    }
}

// ── markdown_render ─────────────────────────────────────────────────────────

pub struct MarkdownRender;

#[async_trait::async_trait]
impl BuiltinSkill for MarkdownRender {
    fn name(&self) -> &'static str { "markdown_render" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = require_text(context)?;
        let mut sections: Vec<Value> = Vec::new();
        let mut heading = String::new();
        let mut body: Vec<String> = Vec::new();
        for line in text.lines() {
            if let Some(stripped) = line.strip_prefix("# ") {
                if !heading.is_empty() {
                    sections.push(json!({"heading": heading, "body": body.join("\n")}));
                    body.clear();
                }
                heading = stripped.to_string();
            } else if let Some(stripped) = line.strip_prefix("## ") {
                body.push(format!("[{}]", stripped));
            } else {
                body.push(line.to_string());
            }
        }
        if !heading.is_empty() {
            sections.push(json!({"heading": heading, "body": body.join("\n")}));
        }
        Ok(json!({"sections": sections, "format": "markdown"}))
    }
}

// ── LCS diff 算法 ──────────────────────────────────────────────────────────

/// 构建 LCS 长度表
fn lcs_table<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<Vec<usize>> {
    let m = a.len();
    let n = b.len();
    let mut table = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            table[i][j] = if a[i - 1] == b[j - 1] {
                table[i - 1][j - 1] + 1
            } else {
                table[i - 1][j].max(table[i][j - 1])
            };
        }
    }
    table
}

/// 回溯 LCS 表生成 diff 操作序列
fn lcs_diff(
    table: &[Vec<usize>],
    a: &[&str],
    b: &[&str],
    i: usize,
    j: usize,
    emit: &mut impl FnMut(char, &str),
) {
    if i > 0 && j > 0 && a[i - 1] == b[j - 1] {
        lcs_diff(table, a, b, i - 1, j - 1, emit);
        emit(' ', a[i - 1]);
    } else if j > 0 && (i == 0 || table[i][j - 1] >= table[i - 1][j]) {
        lcs_diff(table, a, b, i, j - 1, emit);
        emit('+', b[j - 1]);
    } else if i > 0 {
        lcs_diff(table, a, b, i - 1, j, emit);
        emit('-', a[i - 1]);
    }
}
