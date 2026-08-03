//! RAG（检索增强生成）引擎
//!
//! 核心流程：文档摄�?�?分块 �?向量嵌入 �?存储 �?相似度搜�?�?上下文增强生�?

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// 文档�?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    /// �?ID
    pub id: String,
    /// 来源（文件路径或 URL�?
    pub source: String,
    /// 文本内容
    pub content: String,
    /// 向量嵌入
    pub embedding: Vec<f32>,
    /// 元数�?
    #[serde(default)]
    pub metadata: Value,
}
/// Simple keyword extractor for BM25-like scoring (lightweight alternative to tantivy)
#[derive(Debug, Clone)]
pub struct KeywordExtractor {
    stop_words: Vec<&'static str>,
}

impl Default for KeywordExtractor {
    fn default() -> Self {
        Self {
            stop_words: vec![
                "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
                "have", "has", "had", "do", "does", "did", "will", "would", "could",
                "should", "may", "might", "can", "shall", "to", "of", "in", "for",
                "on", "with", "at", "by", "from", "as", "into", "through", "and",
                "but", "or", "not", "no", "if", "then", "than", "that", "this",
                "it", "its", "they", "them", "their", "we", "our", "you", "your",
                "�?, "�?, "�?, "�?, "�?, "�?, "�?, "�?, "�?, "�?, "�?,
                "一", "一�?, "�?, "�?, "�?, "�?, "�?, "�?, "�?, "�?, "�?,
                "着", "没有", "�?, "�?, "自己", "�?
            ],
        }
    }
}

impl KeywordExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Extract keywords from text with frequency counting
    pub fn extract_keywords(&self, text: &str) -> Vec<(String, usize)> {
        let mut freq: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        
        for word in text.split(|c: char| c.is_whitespace() || c == ',' || c == '.' || c == '�? || c == '�?) {
            let w = word.to_ascii_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string();
            
            if w.len() > 1 && !self.stop_words.contains(&w.as_str()) {
                *freq.entry(w).or_insert(0) += 1;
            }
        }
        
        let mut keywords: Vec<(String, usize)> = freq.into_iter().collect();
        keywords.sort_by(|a, b| b.1.cmp(&a.1));
        keywords
    }
    
    /// Calculate BM25-like score for a document against query keywords
    pub fn bm25_score(&self, doc_keywords: &[(String, usize)], query_keywords: &[(String, usize)]) -> f64 {
        if query_keywords.is_empty() {
            return 0.0;
        }
        
        let k1 = 1.5;  // BM25 parameter
        let b = 0.75;  // BM25 parameter
        
        let doc_len = doc_keywords.iter().map(|(_, count)| count).sum::<usize>() as f64;
        let avg_len = doc_len.max(1.0);
        
        let mut score = 0.0;
        for (query_term, q_freq) in query_keywords {
            let doc_freq = doc_keywords.iter()
                .find(|(term, _)| term == query_term)
                .map(|(_, count)| *count as f64)
                .unwrap_or(0.0);
            
            if doc_freq > 0.0 {
                let numerator = q_freq as f64 * doc_freq * (k1 + 1.0);
                let denominator = doc_freq + k1 * (1.0 - b + b * avg_len / 100.0);
                score += numerator / denominator;
            }
        }
        
        score / query_keywords.len() as f64
    }
}



/// RAG 知识库状�?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagStatus {
    /// 总文档数
    pub document_count: usize,
    /// 总块�?
    pub chunk_count: usize,
    /// 来源列表
    pub sources: Vec<String>,
    /// 存储路径
    pub store_path: String,
}

/// RAG 检索结�?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// 文档�?
    pub chunk: DocumentChunk,
    /// 相似度分数（0.0 - 1.0�?
    pub score: f64,
}

/// RAG 引擎
pub struct RagEngine {
    /// 所有文档块
    chunks: Vec<DocumentChunk>,
    /// 存储路径
    store_path: PathBuf,
    /// usearch HNSW 索引（余弦相似度检索加速层）�?
    /// `None` 表示索引不可用，检索降级为手写全量余弦扫描�?
    index: Option<Index>,
}

/// usearch 索引文件名（存于 rag/ 目录下）
const INDEX_FILE: &str = "usearch.index";


/// Reranker trait for combining multiple scoring signals
pub trait Reranker: Send + Sync {
    fn name(&self) -> &str;
    fn rerank(&self, query: &str, results: &[RetrievalResult], top_k: usize) -> Vec<RetrievalResult>;
}

/// Simple linear combination reranker: alpha * semantic + (1-alpha) * keyword
pub struct LinearReranker {
    pub alpha: f64,  // weight for semantic score
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
        let keyword_ext = KeywordExtractor::new();
        let query_keywords = keyword_ext.extract_keywords(query);
        
        let mut scored: Vec<(f64, usize)> = results.iter().enumerate().map(|(i, r)| {
            let chunk_keywords = keyword_ext.extract_keywords(&r.chunk.content);
            let keyword_score = keyword_ext.bm25_score(&chunk_keywords, &query_keywords);
            let combined = self.alpha * r.score + (1.0 - self.alpha) * keyword_score;
            (combined, i)
        }).collect();
        
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        
        scored.iter().take(top_k)
            .map(|(_, idx)| results[*idx].clone())
            .collect()
    }
}


impl RagEngine {
    /// 创建或加�?RAG 引擎
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
            keyword_extractor: KeywordExtractor::new(),
            reranker: Box::new(LinearReranker::default()),
        };
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

        // 优先使用 usearch HNSW 索引加速（仅当查询维度与索引维度一致时�?
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
                                // 分数用手写余弦计算，保证与降级路径分数语义完全一�?
                                score: Self::cosine_similarity(query_embedding, &chunk.embedding),
                            })
                        })
                        .collect();
                    results.sort_by(|a, b| {
                        b.score
                            .partial_cmp(&a.score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    results.truncate(top_k);
                    return results;
                }
            }
        }

        // 降级：usearch 不可用或维度不匹配时，使用手写全量余弦扫�?
        self.bruteforce_search(query_embedding, top_k)
    }

    /// 检索并�?AI 生成增强回答
    pub async fn query(&self, question: &str, top_k: usize) -> Result<Value> {
        // 1. 获取问题的嵌入向�?
        let query_embedding = self.get_embedding(question).await?;

        // 2. 检索相关文�?
        let results = self.search(&query_embedding, top_k);

        if results.is_empty() {
            return Ok(json!({
                "answer": "知识库中没有找到相关内容�?,
                "sources": [],
                "chunks_searched": self.chunks.len()
            }));
        }

        // 3. 组装上下�?
        let context_parts: Vec<String> = results
            .iter()
            .map(|r| format!("[来源: {}] {}", r.chunk.source, r.chunk.content))
            .collect();
        let context = context_parts.join("\n\n");

        // 4. 调用 AI 生成回答，失败时 fallback 到原�?chunks
        let (answer, ai_generated) = match self.generate_answer(question, &context).await {
            Ok(a) if a != "无法生成回答" => (a, true),
            Ok(_) | Err(_) => {
                // AI 不可用或返回空回答，fallback 返回原始检索内�?
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

    /// 获取知识库状�?
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

    /// 保存到磁�?
    fn save(&self) -> Result<()> {
        let chunks_file = self.store_path.join("chunks.json");
        let content = serde_json::to_string(&self.chunks)?;
        std::fs::write(chunks_file, content)?;
        if let Some(index) = &self.index {
            self.save_index(index)?;
        }
        Ok(())
    }

    /// 保存 usearch 索引到磁�?
    fn save_index(&self, index: &Index) -> Result<()> {
        let index_file = self.store_path.join(INDEX_FILE);
        let path = index_file
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("RAG: index path is not valid UTF-8"))?;
        index.save(path)?;
        Ok(())
    }

    /// 尝试从磁盘恢�?usearch 索引；文件缺�?损坏/�?chunks 不一致时重建�?
    /// usearch 不可用时保持 `None`（降级手写全量余弦扫描）�?
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

    /// 从磁盘加�?usearch 索引（一步完�?metadata 读取 + 构�?+ 加载�?
    fn open_index(index_file: &Path) -> Result<Index> {
        let path = index_file
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("RAG: index path is not valid UTF-8"))?;
        Ok(Index::restore(path)?)
    }

    /// 从当�?chunks 全量重建 usearch 索引；失败时�?`None`（不传播，降级手写）�?
    fn rebuild_index(&mut self) {
        self.index = Self::build_index(&self.chunks);
    }

    /// �?usearch 构建 HNSW 索引（Cosine 度量）�?
    /// key �?chunk �?`chunks` 中的位置；跳过空 embedding 或维度不一致的块�?
    /// 返回 `None` 表示索引不可用�?
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

        // 如果完全为空，强制分割原�?
        if final_chunks.is_empty() && !text.is_empty() {
            let chars: Vec<char> = text.chars().collect();
            for chunk in chars.chunks(max_chars) {
                final_chunks.push(chunk.iter().collect());
            }
        }

        final_chunks
    }

    /// 获取文本的向量嵌�?
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
        // 归一�?
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }
        vec
    }

    /// 余弦相似�?
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
        assert!((norm - 1.0).abs() < 0.01); // 归一化后应接�?1
    }

    #[test]
    fn test_fallback_embedding_similarity() {
        let emb1 = RagEngine::fallback_embedding("rust programming language");
        let emb2 = RagEngine::fallback_embedding("rust programming tutorial");
        let emb3 = RagEngine::fallback_embedding("cooking recipes dessert");
        let sim_related = RagEngine::cosine_similarity(&emb1, &emb2);
        let sim_unrelated = RagEngine::cosine_similarity(&emb1, &emb3);
        // 相关文本应该有更高的相似�?
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
        // usearch 不可用时自动跳过（降级路径由其余测试覆盖�?
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
        // 相关文本块应被命�?
        assert_eq!(matches.keys[0], 0);
    }
}
