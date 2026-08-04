//! RAG（检索增强生成）引擎
//!
//! 核心流程：文档摄入 → 分块 → 向量嵌入 → 存储 → 相似度搜索 → 上下文增强生成

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// 文档块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    /// 块 ID
    pub id: String,
    /// 来源（文件路径或 URL）
    pub source: String,
    /// 文本内容
    pub content: String,
    /// 向量嵌入
    pub embedding: Vec<f32>,
    /// 元数据
    #[serde(default)]
    pub metadata: Value,
}

/// Extracts normalized keyword frequencies for lightweight lexical ranking.
#[derive(Debug, Clone)]
pub struct KeywordExtractor {
    stop_words: &'static [&'static str],
}

impl Default for KeywordExtractor {
    fn default() -> Self {
        Self {
            stop_words: &[
                "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "in", "is", "it", "of", "on", "or",
                "that", "the", "this", "to", "was", "were", "with",
            ],
        }
    }
}

impl KeywordExtractor {
    /// Creates an extractor with the built-in English stop-word list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns normalized keywords and their occurrence counts.
    pub fn extract_keywords(&self, text: &str) -> Vec<(String, usize)> {
        let mut frequencies = std::collections::HashMap::new();
        for token in text.split_whitespace() {
            let keyword = token
                .trim_matches(|character: char| !character.is_alphanumeric())
                .to_lowercase();
            if keyword.len() > 1 && !self.stop_words.contains(&keyword.as_str()) {
                *frequencies.entry(keyword).or_insert(0) += 1;
            }
        }

        let mut keywords: Vec<_> = frequencies.into_iter().collect();
        keywords.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        keywords
    }

    /// Computes a bounded-document-length BM25-like relevance score.
    pub fn bm25_score(&self, document_keywords: &[(String, usize)], query_keywords: &[(String, usize)]) -> f64 {
        if query_keywords.is_empty() {
            return 0.0;
        }

        let document_len = document_keywords.iter().map(|(_, count)| *count).sum::<usize>() as f64;
        let normalized_len = document_len.max(1.0);
        let score = query_keywords.iter().fold(0.0, |score, (query, query_count)| {
            let frequency = document_keywords
                .iter()
                .find(|(keyword, _)| keyword == query)
                .map_or(0.0, |(_, count)| *count as f64);
            if frequency == 0.0 {
                return score;
            }

            let k1 = 1.5;
            let b = 0.75;
            let numerator = *query_count as f64 * frequency * (k1 + 1.0);
            let denominator = frequency + k1 * (1.0 - b + b * normalized_len / 100.0);
            score + numerator / denominator
        });
        score / query_keywords.len() as f64
    }
}

/// RAG 知识库状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagStatus {
    /// 总文档数
    pub document_count: usize,
    /// 总块数
    pub chunk_count: usize,
    /// 来源列表
    pub sources: Vec<String>,
    /// 存储路径
    pub store_path: String,
}

/// RAG 检索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// 文档块
    pub chunk: DocumentChunk,
    /// 相似度分数（0.0 - 1.0）
    pub score: f64,
}

/// Reorders retrieval results using an additional query-aware scoring strategy.
pub trait Reranker: Send + Sync {
    /// Returns the stable strategy name.
    fn name(&self) -> &str;

    /// Returns at most `top_k` results in descending relevance order.
    fn rerank(&self, query: &str, results: &[RetrievalResult], top_k: usize) -> Vec<RetrievalResult>;
}

/// Combines semantic similarity and keyword relevance with a linear weight.
#[derive(Debug, Clone, Copy)]
pub struct LinearReranker {
    /// Weight assigned to the existing semantic score, clamped to `0.0..=1.0`.
    pub alpha: f64,
}

impl Default for LinearReranker {
    fn default() -> Self {
        Self { alpha: 0.7 }
    }
}

impl Reranker for LinearReranker {
    fn name(&self) -> &str {
        "linear_combination"
    }

    fn rerank(&self, query: &str, results: &[RetrievalResult], top_k: usize) -> Vec<RetrievalResult> {
        let extractor = KeywordExtractor::new();
        let query_keywords = extractor.extract_keywords(query);
        let alpha = self.alpha.clamp(0.0, 1.0);
        let mut ranked: Vec<_> = results
            .iter()
            .enumerate()
            .map(|(position, result)| {
                let document_keywords = extractor.extract_keywords(&result.chunk.content);
                let keyword_score = extractor.bm25_score(&document_keywords, &query_keywords);
                (alpha * result.score + (1.0 - alpha) * keyword_score, position)
            })
            .collect();
        ranked.sort_by(|left, right| {
            right
                .0
                .partial_cmp(&left.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.1.cmp(&right.1))
        });
        ranked
            .into_iter()
            .take(top_k)
            .map(|(_, position)| results[position].clone())
            .collect()
    }
}

/// RAG 引擎
pub struct RagEngine {
    /// 所有文档块
    chunks: Vec<DocumentChunk>,
    /// 存储路径
    store_path: PathBuf,
    /// usearch HNSW 索引（余弦相似度检索加速层）。
    /// `None` 表示索引不可用，检索降级为手写全量余弦扫描。
    index: Option<Index>,
}

/// usearch 索引文件名（存于 rag/ 目录下）
const INDEX_FILE: &str = "usearch.index";

impl RagEngine {
    /// 创建或加载 RAG 引擎
    pub fn load_or_create(state_dir: &Path) -> Result<Self> {
        let store_path = state_dir.join("rag");
        std::fs::create_dir_all(&store_path)?;

        let chunks_file = store_path.join("chunks.json");
        let chunks = if chunks_file.exists() {
            let content = std::fs::read_to_string(&chunks_file)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut engine = Self {
            chunks,
            store_path,
            index: None,
        };
        engine.restore_index();
        Ok(engine)
    }

    /// 摄入文档
    pub async fn ingest(&mut self, source: &str, content: &str) -> Result<usize> {
        let text_chunks = Self::split_into_chunks(content, 500);
        let mut count = 0;

        for (i, chunk_text) in text_chunks.iter().enumerate() {
            if chunk_text.trim().is_empty() {
                continue;
            }

            let embedding = self.get_embedding(chunk_text).await?;
            let chunk_id = format!("{}_{}", Self::simple_hash(source), i);

            let chunk = DocumentChunk {
                id: chunk_id,
                source: source.to_string(),
                content: chunk_text.clone(),
                embedding,
                metadata: json!({ "chunk_index": i, "total_chunks": text_chunks.len() }),
            };

            // 去重：如果同 source + index 已存在，替换
            self.chunks.retain(|c| c.id != chunk.id);
            self.chunks.push(chunk);
            count += 1;
        }

        self.rebuild_index();
        self.save()?;
        tracing::info!(
            source = %source,
            chunks = count,
            total = self.chunks.len(),
            "RAG: document ingested"
        );

        Ok(count)
    }

    /// 检索最相关的文档块
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<RetrievalResult> {
        if top_k == 0 {
            return Vec::new();
        }

        // 优先使用 usearch HNSW 索引加速（仅当查询维度与索引维度一致时）
        if let Some(index) = &self.index {
            if index.dimensions() == query_embedding.len() {
                if let Ok(matches) = index.search(query_embedding, top_k) {
                    let mut results: Vec<RetrievalResult> = matches
                        .keys
                        .iter()
                        .zip(matches.distances.iter())
                        .filter_map(|(&key, _dist)| {
                            let pos = key as usize;
                            self.chunks.get(pos).map(|chunk| RetrievalResult {
                                chunk: chunk.clone(),
                                // 分数用手写余弦计算，保证与降级路径分数语义完全一致
                                score: Self::cosine_similarity(query_embedding, &chunk.embedding),
                            })
                        })
                        .collect();
                    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                    results.truncate(top_k);
                    return results;
                }
            }
        }

        // 降级：usearch 不可用或维度不匹配时，使用手写全量余弦扫描
        self.bruteforce_search(query_embedding, top_k)
    }

    /// 检索并用 AI 生成增强回答
    pub async fn query(&self, question: &str, top_k: usize) -> Result<Value> {
        // 1. 获取问题的嵌入向量
        let query_embedding = self.get_embedding(question).await?;

        // 2. 检索相关文档
        let results = self.search(&query_embedding, top_k);

        if results.is_empty() {
            return Ok(json!({
                "answer": "知识库中没有找到相关内容。",
                "sources": [],
                "chunks_searched": self.chunks.len()
            }));
        }

        // 3. 组装上下文
        let context_parts: Vec<String> = results
            .iter()
            .map(|r| format!("[来源: {}] {}", r.chunk.source, r.chunk.content))
            .collect();
        let context = context_parts.join("\n\n");

        // 4. 调用 AI 生成回答，失败时 fallback 到原始 chunks
        let (answer, ai_generated) = match self.generate_answer(question, &context).await {
            Ok(a) if a != "无法生成回答" => (a, true),
            Ok(_) | Err(_) => {
                // AI 不可用或返回空回答，fallback 返回原始检索内容
                let fallback = format!("（AI 暂不可用，以下为检索到的原始内容）\n\n{}", context);
                (fallback, false)
            }
        };

        // 5. 返回结果
        let sources: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "source": r.chunk.source,
                    "score": r.score,
                    "preview": r.chunk.content.chars().take(100).collect::<String>(),
                })
            })
            .collect();

        Ok(json!({
            "answer": answer,
            "ai_generated": ai_generated,
            "sources": sources,
            "chunks_searched": self.chunks.len(),
            "chunks_matched": results.len()
        }))
    }

    /// 获取知识库状态
    pub fn status(&self) -> RagStatus {
        let mut sources: Vec<String> = self
            .chunks
            .iter()
            .map(|c| c.source.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        sources.sort();

        RagStatus {
            document_count: sources.len(),
            chunk_count: self.chunks.len(),
            sources,
            store_path: self.store_path.display().to_string(),
        }
    }

    /// 保存到磁盘
    fn save(&self) -> Result<()> {
        let chunks_file = self.store_path.join("chunks.json");
        let content = serde_json::to_string(&self.chunks)?;
        std::fs::write(chunks_file, content)?;
        if let Some(index) = &self.index {
            self.save_index(index)?;
        }
        Ok(())
    }

    /// 保存 usearch 索引到磁盘
    fn save_index(&self, index: &Index) -> Result<()> {
        let index_file = self.store_path.join(INDEX_FILE);
        let path = index_file
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("RAG: index path is not valid UTF-8"))?;
        index.save(path)?;
        Ok(())
    }

    /// 尝试从磁盘恢复 usearch 索引；文件缺失/损坏/与 chunks 不一致时重建；
    /// usearch 不可用时保持 `None`（降级手写全量余弦扫描）。
    fn restore_index(&mut self) {
        let index_file = self.store_path.join(INDEX_FILE);
        if index_file.exists() {
            if let Ok(index) = Self::open_index(&index_file) {
                let dim = index.dimensions();
                let expected = self
                    .chunks
                    .iter()
                    .filter(|c| !c.embedding.is_empty() && c.embedding.len() == dim)
                    .count();
                if index.size() == expected {
                    self.index = Some(index);
                    return;
                }
                tracing::warn!(
                    expected,
                    actual = index.size(),
                    "RAG: usearch index size mismatch, rebuilding"
                );
            }
        }
        self.rebuild_index();
    }

    /// 从磁盘加载 usearch 索引（一步完成 metadata 读取 + 构造 + 加载）
    fn open_index(index_file: &Path) -> Result<Index> {
        let path = index_file
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("RAG: index path is not valid UTF-8"))?;
        Ok(Index::restore(path)?)
    }

    /// 从当前 chunks 全量重建 usearch 索引；失败时置 `None`（不传播，降级手写）。
    fn rebuild_index(&mut self) {
        self.index = Self::build_index(&self.chunks);
    }

    /// 用 usearch 构建 HNSW 索引（Cosine 度量）。
    /// key 为 chunk 在 `chunks` 中的位置；跳过空 embedding 或维度不一致的块。
    /// 返回 `None` 表示索引不可用。
    fn build_index(chunks: &[DocumentChunk]) -> Option<Index> {
        let dim = chunks
            .iter()
            .find(|c| !c.embedding.is_empty())
            .map(|c| c.embedding.len())?;

        let options = IndexOptions {
            dimensions: dim,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        };

        let index = match Index::new(&options) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "RAG: usearch index creation failed, falling back to brute-force search"
                );
                return None;
            }
        };
        let _ = index.reserve(chunks.len());

        for (pos, chunk) in chunks.iter().enumerate() {
            if chunk.embedding.is_empty() || chunk.embedding.len() != dim {
                continue;
            }
            if let Err(e) = index.add(pos as u64, &chunk.embedding) {
                tracing::warn!(
                    error = %e,
                    "RAG: usearch index add failed, falling back to brute-force search"
                );
                return None;
            }
        }
        Some(index)
    }

    /// 手写全量余弦相似度检索（降级路径，保留原有检索语义）
    fn bruteforce_search(&self, query_embedding: &[f32], top_k: usize) -> Vec<RetrievalResult> {
        let mut scored: Vec<RetrievalResult> = self
            .chunks
            .iter()
            .map(|chunk| {
                let score = Self::cosine_similarity(query_embedding, &chunk.embedding);
                RetrievalResult {
                    chunk: chunk.clone(),
                    score,
                }
            })
            .collect();

        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// 文本分块（按段落，每块约 max_chars 字）
    fn split_into_chunks(text: &str, max_chars: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();

        for paragraph in text.split("\n\n") {
            let paragraph = paragraph.trim();
            if paragraph.is_empty() {
                continue;
            }

            if current.len() + paragraph.len() > max_chars && !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }

            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(paragraph);
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        // 对超长块进行二次分割
        let mut final_chunks = Vec::new();
        for chunk in chunks {
            if chunk.len() <= max_chars {
                final_chunks.push(chunk);
            } else {
                let chars: Vec<char> = chunk.chars().collect();
                for sub in chars.chunks(max_chars) {
                    final_chunks.push(sub.iter().collect());
                }
            }
        }

        // 如果完全为空，强制分割原文
        if final_chunks.is_empty() && !text.is_empty() {
            let chars: Vec<char> = text.chars().collect();
            for chunk in chars.chunks(max_chars) {
                final_chunks.push(chunk.iter().collect());
            }
        }

        final_chunks
    }

    /// 获取文本的向量嵌入
    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let base_url = std::env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key = std::env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string());
        let model = std::env::var("AI_EMBEDDING_MODEL")
            .or_else(|_| std::env::var("AI_MODEL"))
            .unwrap_or_else(|_| "nomic-embed-text".to_string());

        let body = json!({
            "model": model,
            "input": text
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let resp: Value = match client
            .post(format!("{}/embeddings", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(_) => return Ok(Self::fallback_embedding(text)),
            },
            Err(_) => return Ok(Self::fallback_embedding(text)),
        };

        // OpenAI 格式
        if let Some(data) = resp["data"][0]["embedding"].as_array() {
            let embedding: Vec<f32> = data.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
            if !embedding.is_empty() {
                return Ok(embedding);
            }
        }

        // Ollama 格式
        if let Some(emb) = resp["embedding"].as_array() {
            let embedding: Vec<f32> = emb.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
            if !embedding.is_empty() {
                return Ok(embedding);
            }
        }

        // 降级：使用简单的词袋向量
        Ok(Self::fallback_embedding(text))
    }

    /// 降级嵌入：简单的词频向量（在没有 embedding API 时使用）
    fn fallback_embedding(text: &str) -> Vec<f32> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut vec = vec![0.0f32; 128];
        for word in &words {
            let hash = word
                .bytes()
                .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
            let idx = (hash as usize) % 128;
            vec[idx] += 1.0;
        }
        // 归一化
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }
        vec
    }

    /// 余弦相似度
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            (dot / (norm_a * norm_b)) as f64
        }
    }

    /// AI 生成回答
    async fn generate_answer(&self, question: &str, context: &str) -> Result<String> {
        let base_url = std::env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key = std::env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string());
        let model = std::env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string());

        let system_prompt = format!(
            "你是一个智能助手。根据以下知识库内容回答用户的问题。\n\
             只使用提供的内容回答，如果内容中没有答案，请说明。\n\n\
             知识库内容：\n{}",
            context
        );

        let body = json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": question}
            ],
            "temperature": 0.3,
            "max_tokens": 512
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let resp: Value = client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp["choices"][0]["message"]["content"]
            .as_str()
            .or_else(|| resp["result"].as_str())
            .unwrap_or("无法生成回答")
            .to_string())
    }

    /// 简易字符串哈希
    fn simple_hash(s: &str) -> String {
        let hash = s
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        format!("{:012x}", hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_chunks() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph that is longer.";
        let chunks = RagEngine::split_into_chunks(text, 30);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_split_no_paragraphs() {
        // 没有段落分隔符时，应按字符数强制分割
        let text = "ABCDEFGHIJ".repeat(5); // 50 chars, no \n\n
        let chunks = RagEngine::split_into_chunks(&text, 20);
        assert!(chunks.len() >= 2, "expected >= 2 chunks, got {}", chunks.len());
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 1.0];
        let b = vec![1.0, 0.0, 1.0];
        let sim = RagEngine::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = RagEngine::cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_different_length() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 1.0];
        let sim = RagEngine::cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0); // 不同长度返回 0
    }

    #[test]
    fn test_fallback_embedding() {
        let emb = RagEngine::fallback_embedding("hello world test");
        assert_eq!(emb.len(), 128);
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01); // 归一化后应接近 1
    }

    #[test]
    fn test_fallback_embedding_similarity() {
        let emb1 = RagEngine::fallback_embedding("rust programming language");
        let emb2 = RagEngine::fallback_embedding("rust programming tutorial");
        let emb3 = RagEngine::fallback_embedding("cooking recipes dessert");
        let sim_related = RagEngine::cosine_similarity(&emb1, &emb2);
        let sim_unrelated = RagEngine::cosine_similarity(&emb1, &emb3);
        // 相关文本应该有更高的相似度
        assert!(sim_related > sim_unrelated);
    }

    #[test]
    fn test_rag_status_empty() {
        let tmp = std::env::temp_dir().join("aion-rag-test-status");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let engine = RagEngine::load_or_create(&tmp).unwrap();
        let status = engine.status();
        assert_eq!(status.chunk_count, 0);
        assert_eq!(status.document_count, 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_simple_hash() {
        let h1 = RagEngine::simple_hash("test.md");
        let h2 = RagEngine::simple_hash("test.md");
        let h3 = RagEngine::simple_hash("other.md");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_usearch_index_roundtrip() {
        // usearch 不可用时自动跳过（降级路径由其余测试覆盖）
        if Index::new(&IndexOptions {
            dimensions: 8,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        })
        .is_err()
        {
            return;
        }

        let chunks: Vec<DocumentChunk> = vec![
            DocumentChunk {
                id: "a_0".into(),
                source: "a.md".into(),
                content: "rust programming".into(),
                embedding: RagEngine::fallback_embedding("rust programming language"),
                metadata: json!({}),
            },
            DocumentChunk {
                id: "b_0".into(),
                source: "b.md".into(),
                content: "cooking recipes".into(),
                embedding: RagEngine::fallback_embedding("cooking recipes dessert"),
                metadata: json!({}),
            },
        ];

        let index = RagEngine::build_index(&chunks).expect("usearch index should build");
        let query = RagEngine::fallback_embedding("rust programming tutorial");
        let matches = index.search(&query, 2).unwrap();
        assert!(!matches.keys.is_empty());
        // 相关文本块应被命中
        assert_eq!(matches.keys[0], 0);
    }

    #[test]
    fn keyword_extractor_counts_terms_and_filters_stop_words() {
        let extractor = KeywordExtractor::new();
        let keywords = extractor.extract_keywords("Rust rust and memory");

        assert_eq!(keywords, vec![("rust".to_string(), 2), ("memory".to_string(), 1)]);
    }

    #[test]
    fn bm25_score_prefers_documents_with_query_terms() {
        let extractor = KeywordExtractor::new();
        let query = extractor.extract_keywords("rust memory");
        let matching = extractor.extract_keywords("rust rust memory storage");
        let unrelated = extractor.extract_keywords("cooking dessert recipe");

        assert!(extractor.bm25_score(&matching, &query) > extractor.bm25_score(&unrelated, &query));
    }

    #[test]
    fn linear_reranker_orders_by_combined_score_and_truncates() {
        let results = vec![
            RetrievalResult {
                chunk: DocumentChunk {
                    id: "semantic".into(),
                    source: "semantic.md".into(),
                    content: "unrelated cooking".into(),
                    embedding: vec![],
                    metadata: json!({}),
                },
                score: 0.9,
            },
            RetrievalResult {
                chunk: DocumentChunk {
                    id: "keyword".into(),
                    source: "keyword.md".into(),
                    content: "rust memory rust memory".into(),
                    embedding: vec![],
                    metadata: json!({}),
                },
                score: 0.4,
            },
        ];
        let reranker = LinearReranker { alpha: 0.2 };

        let reranked = reranker.rerank("rust memory", &results, 1);

        assert_eq!(reranked.len(), 1);
        assert_eq!(reranked[0].chunk.id, "keyword");
    }
}
