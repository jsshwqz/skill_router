#[cfg(test)]
mod tests {
    use crate::memory::{MemoryManager, MemoryCategory};
    use std::env;
    use std::fs;

    /// 创建一个预填充了多条记忆的 MemoryManager（复用测试基础设施）
    fn setup_manager(dir_suffix: &str) -> (MemoryManager, std::path::PathBuf) {
        let tmp = env::temp_dir().join(format!("aion_memory_{}", dir_suffix));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let manager = MemoryManager::new(&tmp);
        (manager, tmp)
    }

    #[test]
    fn test_memory_remember_recall() {
        let (manager, tmp) = setup_manager("test_basic");

        let id = manager.remember(
            MemoryCategory::Decision,
            "Use Rust for all new modules",
            "test_session",
            8,
        ).unwrap();
        assert!(!id.is_empty());

        let results = manager.recall("Rust modules", 5).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.content.contains("Rust")));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_memory_recall_by_category() {
        let (manager, tmp) = setup_manager("test_category");

        manager.remember(MemoryCategory::Decision, "Decision A", "s1", 5).unwrap();
        manager.remember(MemoryCategory::Lesson, "Lesson B", "s1", 7).unwrap();
        manager.remember(MemoryCategory::Decision, "Decision C", "s1", 9).unwrap();
        manager.remember(MemoryCategory::Error, "Error D", "s1", 3).unwrap();

        let decisions = manager.recall_by_category(&MemoryCategory::Decision, 10).unwrap();
        assert_eq!(decisions.len(), 2);
        // 按 importance 降序排列
        assert!(decisions[0].importance >= decisions[1].importance);
        assert!(decisions.iter().all(|e| e.category == MemoryCategory::Decision));

        let lessons = manager.recall_by_category(&MemoryCategory::Lesson, 10).unwrap();
        assert_eq!(lessons.len(), 1);
        assert_eq!(lessons[0].content, "Lesson B");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_memory_recall_scoring() {
        let (manager, tmp) = setup_manager("test_scoring");

        // 低重要性但关键词匹配 → score = 1*10 + 2 = 12
        manager.remember(MemoryCategory::Decision, "Use Python scripts", "s1", 2).unwrap();
        // 高重要性且关键词匹配 → score = 1*10 + 9 = 19
        manager.remember(MemoryCategory::Decision, "Use Rust for scripts", "s1", 9).unwrap();
        // 不含关键词但 importance > 0 → score = 0*10 + 10 = 10（也通过 > 0 过滤）
        manager.remember(MemoryCategory::Lesson, "Database is PostgreSQL", "s1", 10).unwrap();

        let results = manager.recall("scripts", 5).unwrap();
        // 评分逻辑: keyword_hits * 10 + importance，所有 importance > 0 的条目都会返回
        assert_eq!(results.len(), 3);
        // 关键词匹配 + 高 importance 的 "Use Rust for scripts" 应排第一（score=19）
        assert!(results[0].content.contains("Rust"),
            "Keyword match + high importance should rank first, got: '{}'",
            results[0].content);
        // "Use Python scripts"（score=12）排第二
        assert!(results[1].content.contains("Python"),
            "Keyword match + low importance should rank second, got: '{}'",
            results[1].content);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_memory_stats() {
        let (manager, tmp) = setup_manager("test_stats");

        manager.remember(MemoryCategory::Decision, "D1", "s1", 5).unwrap();
        manager.remember(MemoryCategory::Decision, "D2", "s1", 5).unwrap();
        manager.remember(MemoryCategory::Lesson, "L1", "s1", 5).unwrap();
        manager.remember(MemoryCategory::Error, "E1", "s1", 5).unwrap();

        let stats = manager.stats().unwrap();
        assert_eq!(stats["total_memories"], 4);
        assert_eq!(stats["decisions"], 2);
        assert_eq!(stats["lessons"], 1);
        assert_eq!(stats["errors"], 1);
        assert_eq!(stats["preferences"], 0);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_memory_generate_context_md() {
        let (manager, tmp) = setup_manager("test_context_md");

        manager.remember(MemoryCategory::Architecture, "Microservice arch", "s1", 8).unwrap();
        manager.remember(MemoryCategory::Decision, "Use axum for HTTP", "s1", 7).unwrap();

        let md = manager.generate_context_md().unwrap();
        assert!(md.contains("# Project Context"));
        assert!(md.contains("## Architecture Decisions"));
        assert!(md.contains("Microservice arch"));
        assert!(md.contains("## Key Decisions"));
        assert!(md.contains("Use axum for HTTP"));

        // 验证文件也被写入
        assert!(tmp.join("CONTEXT.md").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_memory_cache_consistency() {
        let (manager, tmp) = setup_manager("test_cache");

        manager.remember(MemoryCategory::Decision, "Cache test entry", "s1", 5).unwrap();

        // 第一次 load（从磁盘）
        let store1 = manager.load().unwrap();
        // 第二次 load（应走缓存）
        let store2 = manager.load().unwrap();

        assert_eq!(store1.entries.len(), store2.entries.len());
        assert_eq!(store1.entries[0].content, store2.entries[0].content);
        assert_eq!(store1.version, store2.version);

        let _ = fs::remove_dir_all(&tmp);
    }
}
