#[cfg(test)]
mod tests {
    use crate::memory::{MemoryManager, MemoryCategory};
    use std::env;
    use std::fs;

    #[test]
    fn test_memory_remember_recall() {
        let tmp = env::temp_dir().join("aion_memory_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let manager = MemoryManager::new(&tmp);

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
}
