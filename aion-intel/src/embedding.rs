//! Embedding provider trait for semantic search

use anyhow::Result;
use serde_json::{Value, json};

/// Trait for embedding providers (OpenAI, Ollama, local, etc.)
pub trait EmbeddingProvider: Send + Sync {
    /// Get embedding vector for text (sync wrapper)
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Get model name
    fn model_name(&self) -> &str;

    /// Get dimensions
    fn dimensions(&self) -> usize;
}

/// OpenAI-compatible embedding provider
#[derive(Debug, Clone)]
pub struct OpenAIEmbeddingProvider {
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAIEmbeddingProvider {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    pub async fn embed_async(&self, text: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();
        let body = json!({
            "model": &self.model,
            "input": text,
        });

        let resp: Value = client
            .post(&format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        if let Some(data) = resp.get("data").and_then(|d| d.get(0)) {
            if let Some(embedding) = data.get("embedding").and_then(|e| e.as_array()) {
                return Ok(embedding.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect());
            }
        }

        anyhow::bail!("Invalid embedding response: {:?}", resp)
    }
}

impl EmbeddingProvider for OpenAIEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Use blocking runtime for async call
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.embed_async(text))
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        1536
    }
}

/// Ollama embedding provider
#[derive(Debug, Clone)]
pub struct OllamaEmbeddingProvider {
    base_url: String,
    model: String,
}

impl OllamaEmbeddingProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    pub async fn embed_async(&self, text: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();
        let body = json!({
            "model": &self.model,
            "prompt": text,
        });

        let resp: Value = client
            .post(&format!("{}/api/embeddings", self.base_url))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        if let Some(embedding) = resp.get("embedding").and_then(|e| e.as_array()) {
            return Ok(embedding.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect());
        }

        anyhow::bail!("Invalid Ollama embedding response: {:?}", resp)
    }
}

impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.embed_async(text))
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        768
    }
}

/// TF-IDF fallback provider (no external API needed)
#[derive(Debug, Clone)]
pub struct TfIdfProvider;

impl TfIdfProvider {
    pub fn new() -> Self {
        Self
    }
}

impl EmbeddingProvider for TfIdfProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        const DIMENSIONS: usize = 128;
        let mut vec = vec![0.0f32; DIMENSIONS];

        for word in text.split_whitespace() {
            let hash = murmurhash32(word.as_bytes()) % DIMENSIONS as u32;
            vec[hash as usize] += 1.0;
        }

        let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vec.iter_mut() {
                *v /= norm;
            }
        }

        Ok(vec)
    }

    fn model_name(&self) -> &str {
        "tfidf-fallback"
    }

    fn dimensions(&self) -> usize {
        128
    }
}

/// Create embedding provider from environment/config
pub fn create_embedding_provider() -> Box<dyn EmbeddingProvider> {
    if let (Ok(base_url), Ok(api_key)) = (std::env::var("AI_EMBEDDING_BASE_URL"), std::env::var("AI_API_KEY")) {
        let model = std::env::var("AI_EMBEDDING_MODEL").unwrap_or_else(|_| "text-embedding-3-small".to_string());
        return Box::new(OpenAIEmbeddingProvider::new(&base_url, &api_key, &model));
    }

    if let Ok(base_url) = std::env::var("OLLAMA_BASE_URL") {
        let model = std::env::var("OLLAMA_EMBEDDING_MODEL").unwrap_or_else(|_| "nomic-embed-text".to_string());
        return Box::new(OllamaEmbeddingProvider::new(&base_url, &model));
    }

    Box::new(TfIdfProvider::new())
}

fn murmurhash32(data: &[u8]) -> u32 {
    let mut h = 0xdeadbeef_u32;
    for &b in data {
        h = h.wrapping_mul(0x01000193);
        h ^= b as u32;
    }
    h = h.wrapping_add(data.len() as u32);
    h ^ (h >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfidf_provider() {
        let provider = TfIdfProvider::new();
        let vec = provider.embed("hello world test").unwrap();
        assert_eq!(vec.len(), 128);
    }
}
