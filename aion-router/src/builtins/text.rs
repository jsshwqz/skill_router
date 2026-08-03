//! 文本处理类 builtin 技能：text_diff, text_embed, markdown_render

use anyhow::Result;
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::{extract_text, require_text, BuiltinSkill};

macro_rules! ai_text_builtin {
    ($type_name:ident, $tool_name:literal, $instruction:literal) => {
        pub struct $type_name;

        #[async_trait::async_trait]
        impl BuiltinSkill for $type_name {
            fn name(&self) -> &'static str {
                $tool_name
            }

            async fn execute(&self, skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
                let mut delegated = skill.clone();
                let mut instruction = $instruction.to_string();
                
                // P3-A: Add mode/length parameters for TextSummarize
                if std::any::TypeId::of::<$type_name>() == std::any::TypeId::of::<TextSummarize>() {
                    if let Some(mode) = context.context["mode"].as_str() {
                        instruction.push_str(&format!("
Mode: {}", mode));
                    }
                    if let Some(len) = context.context["max_length"].as_u64() {
                        instruction.push_str(&format!("
Max length: {} characters", len));
                    }
                }
                // P3-A: Add labels constraint for TextClassify
                if std::any::TypeId::of::<$type_name>() == std::any::TypeId::of::<TextClassify>() {
                    if let Some(labels) = context.context["labels"].as_array() {
                        let label_list: Vec<String> = labels.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        instruction.push_str(&format!("
Allowed labels: {}", label_list.join(", ")));
                    }
                }
                // P3-A: Add schema for TextExtract
                if std::any::TypeId::of::<$type_name>() == std::any::TypeId::of::<TextExtract>() {
                    if let Some(schema) = context.context["schema"].as_str() {
                        instruction.push_str(&format!("
Extraction schema: {}", schema));
                    }
                }
                // P3-E: Add glossary for TextTranslate
                if std::any::TypeId::of::<$type_name>() == std::any::TypeId::of::<TextTranslate>() {
                    if let Some(glossary) = context.context["glossary"].as_object() {
                        let glossary_str: Vec<String> = glossary.iter()
                            .map(|(k, v)| format!("{}: {}", k, v.as_str().unwrap_or("")))
                            .collect();
                        instruction.push_str(&format!("
Glossary (preserve these terms):
{}", glossary_str.join("
")));
                    }
                }
                
                delegated.metadata.instruction = Some(instruction);
                super::ai::AiTask.execute(&delegated, context).await
            }
        }
    };
}

ai_text_builtin!(
    TextSummarize,
    "text_summarize",
    "Summarize the input accurately and concisely while preserving key facts."
);
ai_text_builtin!(
    TextClassify,
    "text_classify",
    "Classify the input using the requested labels or the smallest useful label set."
);
ai_text_builtin!(
    TextExtract,
    "text_extract",
    "Extract the requested entities and facts from the input without inventing information."
);
ai_text_builtin!(
    TextTranslate,
    "text_translate",
    "Translate the input into the requested target language while preserving meaning and tone."
);

// ── text_diff ───────────────────────────────────────────────────────────────

pub struct TextDiff;

#[async_trait::async_trait]
impl BuiltinSkill for TextDiff {
    fn name(&self) -> &'static str {
        "text_diff"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let a = context.context["a"].as_str().unwrap_or("");
        let b = context.context["b"].as_str().unwrap_or("");

        // similar 库的行级 diff
        let text_diff = similar::TextDiff::from_lines(a, b);
        let mut diff: Vec<Value> = Vec::new();
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut unchanged = 0usize;
        for change in text_diff.iter_all_changes() {
            let line = change.value().trim_end_matches('\n');
            match change.tag() {
                similar::ChangeTag::Delete => {
                    removed += 1;
                    diff.push(json!({"op": "-", "line": line}));
                }
                similar::ChangeTag::Insert => {
                    added += 1;
                    diff.push(json!({"op": "+", "line": line}));
                }
                similar::ChangeTag::Equal => {
                    unchanged += 1;
                    diff.push(json!({"op": " ", "line": line}));
                }
            }
        }

        Ok(json!({"added": added, "removed": removed, "unchanged": unchanged, "diff": diff}))
    }
}

// ── text_embed ──────────────────────────────────────────────────────────────

/// Get semantic embedding from external API (OpenAI/Ollama compatible)
async fn get_semantic_embedding(text: &str) -> Result<Vec<f32>> {
    use std::env;
    
    let base_url = env::var("AI_EMBEDDING_BASE_URL")
        .unwrap_or_else(|_| env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string()));
    let api_key = env::var("AI_API_KEY").unwrap_or_default();
    let model = env::var("AI_EMBEDDING_MODEL")
        .unwrap_or_else(|_| env::var("AI_MODEL").unwrap_or("nomic-embed-text".to_string()));

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "input": text,
    });

    let resp = match client
        .post(format!("{}/embeddings", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
    {
        Ok(r) => match r.json::<Value>().await {
            Ok(v) => v,
            Err(_) => return Err(anyhow::anyhow!("Failed to parse embedding response")),
        },
        Err(_) => return Err(anyhow::anyhow!("Embedding API request failed")),
    };

    // Try OpenAI format first
    if let Some(data) = resp.get("data").and_then(|d| d.get(0)) {
        if let Some(embedding) = data.get("embedding").and_then(|e| e.as_array()) {
            let vec: Vec<f32> = embedding.iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            if !vec.is_empty() {
                return Ok(vec);
            }
        }
    }

    // Try Ollama format
    if let Some(embedding) = resp.get("embedding").and_then(|e| e.as_array()) {
        let vec: Vec<f32> = embedding.iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        if !vec.is_empty() {
            return Ok(vec);
        }
    }

    Err(anyhow::anyhow!("Embedding response has no valid data"))
}

pub struct TextEmbed;

#[async_trait::async_trait]
impl BuiltinSkill for TextEmbed {
    fn name(&self) -> &'static str {
        "text_embed"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = require_text(context)?;
        
        // Check if user requested semantic embedding
        let use_semantic = context.context.get("semantic").and_then(|v| v.as_bool()).unwrap_or(false);
        
        if use_semantic {
            // Try semantic embedding from API
            match get_semantic_embedding(&text).await {
                Ok(vec) => {
                    return Ok(json!({
                        "method": "semantic_embedding",
                        "vector": vec,
                        "dimensions": vec.len(),
                        "note": "Semantic embedding from external API"
                    }));
                }
                Err(e) => {
                    tracing::warn!("Semantic embedding failed: {}, falling back to TF-IDF", e);
                }
            }
        }

        // Fallback to TF-IDF
        // 停用词（中英文常见）
        const STOPWORDS: &[&str] = &[
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do", "does",
            "did", "will", "would", "could", "should", "may", "might", "can", "shall", "to", "of", "in", "for", "on",
            "with", "at", "by", "from", "as", "into", "through", "and", "but", "or", "not", "no", "if", "then", "than",
            "that", "this", "it", "its", "they", "them", "their", "we", "our", "you", "your", "的", "了", "是", "在",
            "和", "有", "我", "他", "她", "它", "们", "这", "那", "就", "也", "都", "被", "把", "让", "用", "不",
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
    fn name(&self) -> &'static str {
        "markdown_render"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let text = require_text(context)?;
        let parser = pulldown_cmark::Parser::new(&text);
        let mut sections: Vec<Value> = Vec::new();
        let mut heading = String::new();
        let mut body: Vec<String> = Vec::new();
        let mut heading_buf = String::new();
        let mut heading_level: Option<u8> = None;
        let mut line_buf = String::new();

        macro_rules! flush_line {
            () => {
                if !line_buf.is_empty() {
                    body.push(std::mem::take(&mut line_buf));
                } else {
                    line_buf.clear();
                }
            };
        }

        for event in parser {
            match event {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { level, .. }) => {
                    heading_level = Some(level as u8);
                    if level as u8 == 1 {
                        flush_line!();
                        if !heading.is_empty() {
                            sections.push(json!({"heading": heading, "body": body.join("\n")}));
                            body.clear();
                        }
                        heading.clear();
                    }
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                    if let Some(level) = heading_level.take() {
                        let title = std::mem::take(&mut heading_buf).trim().to_string();
                        if level == 1 {
                            heading = title;
                        } else if !title.is_empty() {
                            flush_line!();
                            body.push(format!("[{}]", title));
                        }
                    }
                }
                pulldown_cmark::Event::Text(t) => {
                    if heading_level.is_some() {
                        if !heading_buf.is_empty() {
                            heading_buf.push(' ');
                        }
                        heading_buf.push_str(&t);
                    } else {
                        line_buf.push_str(&t);
                    }
                }
                pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                    flush_line!();
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph) => {
                    flush_line!();
                }
                _ => {}
            }
        }
        flush_line!();
        if !heading.is_empty() {
            sections.push(json!({"heading": heading, "body": body.join("\n")}));
        }
        Ok(json!({"sections": sections, "format": "markdown"}))
    }
}

// ── text_wordcount ──────────────────────────────────────────────────────────

pub struct TextWordcount;

#[async_trait::async_trait]
impl BuiltinSkill for TextWordcount {
    fn name(&self) -> &'static str {
        "text_wordcount"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        // 对 text_wordcount 做宽松输入兼容：
        // 当 context.text 缺失时，自动回退到 input/task，避免自然语言调用直接失败。
        let text = extract_text(context);

        let word_count = text.split_whitespace().count();
        let char_count = text.chars().count();
        let line_count = if text.is_empty() { 0 } else { text.lines().count() };

        Ok(json!({
            "word_count": word_count,
            "char_count": char_count,
            "line_count": line_count
        }))
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use aion_types::types::SkillMetadata;
    use aion_types::types::{PermissionSet, SkillSource};
    use std::path::PathBuf;

    fn dummy_skill() -> SkillDefinition {
        SkillDefinition {
            metadata: SkillMetadata {
                name: "text_wordcount_placeholder".to_string(),
                version: "0.0.0".to_string(),
                capabilities: vec!["text_wordcount".to_string()],
                entrypoint: "builtin:text_wordcount".to_string(),
                permissions: PermissionSet::default(),
                instruction: None,
                engine_capable: false,
            },
            root_dir: PathBuf::new(),
            source: SkillSource::Local,
        }
    }

    #[tokio::test]
    async fn text_wordcount_fallback_to_task() {
        let ctx = ExecutionContext::new("hello world", "text_wordcount");
        let out = TextWordcount.execute(&dummy_skill(), &ctx).await.unwrap();
        assert_eq!(out["word_count"].as_u64(), Some(2));
        assert_eq!(out["line_count"].as_u64(), Some(1));
    }
}

