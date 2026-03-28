//! RAG（检索增强生成）Builtin 技能
//!
//! 提供 rag_ingest、rag_query、rag_status 三个内置能力。

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_intel::rag::RagEngine;
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

/// RAG 文档摄入
pub struct RagIngest;

#[async_trait::async_trait]
impl BuiltinSkill for RagIngest {
    fn name(&self) -> &'static str {
        "rag_ingest"
    }

    async fn execute(
        &self,
        _skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let source = context.context["source"]
            .as_str()
            .or_else(|| context.context["file"].as_str())
            .ok_or_else(|| anyhow!("rag_ingest requires 'source' (file path) in context"))?;

        let content = if let Some(text) = context.context["content"].as_str() {
            text.to_string()
        } else {
            // 尝试从文件读取
            std::fs::read_to_string(source)
                .map_err(|e| anyhow!("failed to read file '{}': {}", source, e))?
        };

        let state_dir = std::env::current_dir().unwrap_or_default().join(".skill-router");
        let mut engine = RagEngine::load_or_create(&state_dir)?;
        let chunk_count = engine.ingest(source, &content).await?;

        Ok(json!({
            "status": "ok",
            "source": source,
            "chunks_added": chunk_count,
            "total_chunks": engine.status().chunk_count
        }))
    }
}

/// RAG 知识检索与回答
pub struct RagQuery;

#[async_trait::async_trait]
impl BuiltinSkill for RagQuery {
    fn name(&self) -> &'static str {
        "rag_query"
    }

    async fn execute(
        &self,
        _skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let question = context.context["query"]
            .as_str()
            .or_else(|| context.context["question"].as_str())
            .unwrap_or(&context.task);

        let top_k = context.context["top_k"]
            .as_u64()
            .unwrap_or(3) as usize;

        let state_dir = std::env::current_dir().unwrap_or_default().join(".skill-router");
        let engine = RagEngine::load_or_create(&state_dir)?;

        let result = engine.query(question, top_k).await?;
        Ok(result)
    }
}

/// RAG 知识库状态
pub struct RagStatus;

#[async_trait::async_trait]
impl BuiltinSkill for RagStatus {
    fn name(&self) -> &'static str {
        "rag_status"
    }

    async fn execute(
        &self,
        _skill: &SkillDefinition,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        let state_dir = std::env::current_dir().unwrap_or_default().join(".skill-router");
        let engine = RagEngine::load_or_create(&state_dir)?;
        let status = engine.status();

        Ok(json!({
            "document_count": status.document_count,
            "chunk_count": status.chunk_count,
            "sources": status.sources,
            "store_path": status.store_path
        }))
    }
}
