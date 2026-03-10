use crate::models::{SkillMetadata, Registry};
use lru::LruCache;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::num::NonZeroUsize;

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub skill: SkillMetadata,
    pub capabilities: Vec<String>,
    pub cached_at: Instant,
    pub hit_count: u64,
}

pub struct SkillCache {
    cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    ttl: Duration,
    hits: Arc<RwLock<u64>>,
    misses: Arc<RwLock<u64>>,
}

impl SkillCache {
    pub fn new(ttl_seconds: u64, max_entries: usize) -> Self {
        let size = NonZeroUsize::new(max_entries).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(size))),
            ttl: Duration::from_secs(ttl_seconds),
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
        }
    }
    
    pub async fn get(&self, capability: &str) -> Option<SkillMetadata> {
        let mut cache = self.cache.write().await;
        
        if let Some(entry) = cache.get_mut(capability) {
            if entry.cached_at.elapsed() < self.ttl {
                entry.hit_count += 1;
                let mut hits = self.hits.write().await;
                *hits += 1;
                return Some(entry.skill.clone());
            } else {
                cache.pop(capability);
            }
        }
        
        let mut misses = self.misses.write().await;
        *misses += 1;
        None
    }
    
    pub async fn put(&self, capability: String, skill: SkillMetadata) {
        let mut cache = self.cache.write().await;
        let entry = CacheEntry {
            skill,
            capabilities: vec![capability.clone()],
            cached_at: Instant::now(),
            hit_count: 0,
        };
        cache.put(capability, entry);
    }
    
    pub async fn warmup(&self, registry: &Registry) {
        let mut cache = self.cache.write().await;
        
        for (name, skill) in &registry.skills {
            for cap in &skill.capabilities {
                let entry = CacheEntry {
                    skill: skill.clone(),
                    capabilities: skill.capabilities.clone(),
                    cached_at: Instant::now(),
                    hit_count: 0,
                };
                cache.put(cap.clone(), entry);
            }
        }
    }
    
    pub async fn invalidate(&self, capability: &str) {
        let mut cache = self.cache.write().await;
        cache.pop(capability);
    }
    
    pub async fn invalidate_skill(&self, skill_name: &str) {
        let mut cache = self.cache.write().await;
        let caps_to_remove: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.skill.name == skill_name)
            .map(|(cap, _)| cap.clone())
            .collect();
        
        for cap in caps_to_remove {
            cache.pop(&cap);
        }
    }
    
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
    
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let hits = *self.hits.read().await;
        let misses = *self.misses.read().await;
        
        CacheStats {
            entries: cache.len(),
            hits,
            misses,
            hit_rate: if hits + misses > 0 {
                hits as f64 / (hits + misses) as f64
            } else {
                0.0
            },
        }
    }
    
    pub async fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let expired: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.cached_at.elapsed() >= self.ttl)
            .map(|(cap, _)| cap.clone())
            .collect();
        
        let count = expired.len();
        for cap in expired {
            cache.pop(&cap);
        }
        count
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}