use anyhow::Result;
use serde_json::{json, Value};

use super::memory::{MemoryCategory, MemoryManager, MemoryStore};

/// Memory Distiller: compresses and prioritizes long-term memory.
///
/// Over time, the memory store grows. The distiller applies decay, dedup,
/// and summarization to keep the store lean while preserving critical knowledge.
pub struct MemoryDistiller;

impl MemoryDistiller {
    /// Run a full distillation pass on the memory store.
    pub fn distill(manager: &MemoryManager, max_entries: usize) -> Result<DistillReport> {
        let mut store = manager.load()?;
        let original_count = store.entries.len();

        // Phase 1: Remove duplicates (same content)
        let before_dedup = store.entries.len();
        Self::dedup(&mut store);
        let removed_dupes = before_dedup - store.entries.len();

        // Phase 2: Decay — reduce importance of stale, unaccessed entries
        Self::apply_decay(&mut store);

        // Phase 3: Evict low-value entries if over capacity
        let before_evict = store.entries.len();
        Self::evict(&mut store, max_entries);
        let evicted = before_evict - store.entries.len();

        // Phase 4: Merge related lessons into consolidated entries
        let merged = Self::merge_related_lessons(&mut store);

        store.last_updated = now_epoch();
        manager.save(&store)?;

        Ok(DistillReport {
            original_count,
            final_count: store.entries.len(),
            duplicates_removed: removed_dupes,
            low_value_evicted: evicted,
            lessons_merged: merged,
        })
    }

    /// Remove entries with identical content.
    fn dedup(store: &mut MemoryStore) {
        let mut seen = std::collections::HashSet::new();
        store.entries.retain(|entry| {
            let key = entry.content.trim().to_ascii_lowercase();
            seen.insert(key)
        });
    }

    /// Decay importance of entries that haven't been accessed recently.
    fn apply_decay(store: &mut MemoryStore) {
        let now = now_epoch();
        let one_week = 7 * 24 * 3600;
        for entry in &mut store.entries {
            let last_touch = entry.last_accessed.max(entry.timestamp);
            let age = now.saturating_sub(last_touch);
            if age > one_week && entry.access_count == 0 && entry.importance > 1 {
                entry.importance = entry.importance.saturating_sub(1);
            }
        }
    }

    /// Evict the lowest-value entries when over capacity.
    fn evict(store: &mut MemoryStore, max_entries: usize) {
        if store.entries.len() <= max_entries {
            return;
        }
        // Sort by composite score: importance * 10 + access_count (ascending for eviction)
        store.entries.sort_by(|a, b| {
            let score_a = (a.importance as u64) * 10 + a.access_count;
            let score_b = (b.importance as u64) * 10 + b.access_count;
            score_a.cmp(&score_b)
        });
        let to_remove = store.entries.len() - max_entries;
        store.entries.drain(0..to_remove);
        // Re-sort by timestamp (newest first)
        store.entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    /// Merge similar lessons into consolidated entries.
    /// Returns the number of merges performed.
    fn merge_related_lessons(store: &mut MemoryStore) -> usize {
        let lesson_indices: Vec<usize> = store
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.category == MemoryCategory::Lesson)
            .map(|(i, _)| i)
            .collect();

        if lesson_indices.len() < 2 {
            return 0;
        }

        let mut merge_count = 0;
        let mut to_remove: Vec<usize> = Vec::new();

        for i in 0..lesson_indices.len() {
            if to_remove.contains(&lesson_indices[i]) {
                continue;
            }
            for j in (i + 1)..lesson_indices.len() {
                if to_remove.contains(&lesson_indices[j]) {
                    continue;
                }
                let a = &store.entries[lesson_indices[i]];
                let b = &store.entries[lesson_indices[j]];
                if Self::content_similarity(&a.content, &b.content) > 0.6 {
                    // Merge b into a
                    let merged_content = format!(
                        "{} [merged: {}]",
                        a.content, b.content
                    );
                    let merged_importance = a.importance.max(b.importance);
                    let merged_access = a.access_count + b.access_count;

                    // Apply to entry a
                    let idx_a = lesson_indices[i];
                    store.entries[idx_a].content = merged_content;
                    store.entries[idx_a].importance = merged_importance;
                    store.entries[idx_a].access_count = merged_access;

                    to_remove.push(lesson_indices[j]);
                    merge_count += 1;
                }
            }
        }

        // Remove merged entries (in reverse order to preserve indices)
        to_remove.sort_unstable();
        to_remove.reverse();
        for idx in to_remove {
            store.entries.remove(idx);
        }

        merge_count
    }

    /// Simple word-overlap similarity metric (Jaccard-like).
    fn content_similarity(a: &str, b: &str) -> f64 {
        let a_lower = a.to_ascii_lowercase();
        let words_a: std::collections::HashSet<&str> =
            a_lower.split_whitespace().collect();
        let b_lower = b.to_ascii_lowercase();
        let words_b: std::collections::HashSet<&str> =
            b_lower.split_whitespace().collect();
        if words_a.is_empty() && words_b.is_empty() {
            return 1.0;
        }
        let intersection = words_a.intersection(&words_b).count() as f64;
        let union = words_a.union(&words_b).count() as f64;
        if union == 0.0 {
            0.0
        } else {
            intersection / union
        }
    }
}

// ── Report ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DistillReport {
    pub original_count: usize,
    pub final_count: usize,
    pub duplicates_removed: usize,
    pub low_value_evicted: usize,
    pub lessons_merged: usize,
}

impl DistillReport {
    pub fn to_json(&self) -> Value {
        json!({
            "original_count": self.original_count,
            "final_count": self.final_count,
            "duplicates_removed": self.duplicates_removed,
            "low_value_evicted": self.low_value_evicted,
            "lessons_merged": self.lessons_merged,
        })
    }
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
