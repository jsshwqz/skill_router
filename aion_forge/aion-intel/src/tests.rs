#[cfg(test)]
mod tests {
    use crate::discovery_radar::{DiscoveryRadar, SearchHit, SearchSource};
    use crate::immunity::ImmunitySystem;
    use crate::synth::Synthesizer;
    use aion_types::types::RouterPaths;
    use std::fs;
    use std::env;

    #[test]
    fn test_discovery_radar_cascade() {
        let hits = vec![
            SearchHit {
                title: "Google Result".into(),
                url: "https://example.com/a".into(),
                snippet: "Found via Google API".into(),
                source: SearchSource::GoogleApi,
                relevance_score: 1.0,
            },
            SearchHit {
                title: "🦆 Result".into(),
                url: "https://example.com/b".into(),
                snippet: "Found via HTTP fallback".into(),
                source: SearchSource::HttpDirect,
                relevance_score: 0.8,
            },
        ];

        let ranked = DiscoveryRadar::deduplicate_and_rank(hits);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].source, SearchSource::GoogleApi);
    }

    #[test]
    fn test_discovery_dedup() {
        let hits = vec![
            SearchHit {
                title: "Result A".into(),
                url: "https://example.com/same".into(),
                snippet: "First".into(),
                source: SearchSource::GoogleApi,
                relevance_score: 1.0,
            },
            SearchHit {
                title: "Result B".into(),
                url: "https://example.com/same".into(),
                snippet: "Second".into(),
                source: SearchSource::HttpDirect,
                relevance_score: 0.7,
            },
        ];

        let deduped = DiscoveryRadar::deduplicate_and_rank(hits);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_discovery_filter_noise() {
        let hits = vec![
            SearchHit {
                title: "Good".into(),
                url: "https://ok.com".into(),
                snippet: "Long enough snippet for filtering test".into(),
                source: SearchSource::GoogleApi,
                relevance_score: 0.9,
            },
            SearchHit {
                title: "Bad".into(),
                url: "https://bad.com".into(),
                snippet: "short".into(),
                source: SearchSource::HttpDirect,
                relevance_score: 0.1,
            },
        ];

        let filtered = DiscoveryRadar::filter_noise(hits);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_immunity_sanitize_ampersand() {
        let mut cmd = "echo hello && echo world".to_string();
        ImmunitySystem::sanitize_instruction(&mut cmd);
        assert!(!cmd.contains("&&"));
        assert!(cmd.contains(";"));
    }

    #[test]
    fn test_immunity_pre_check_safe() {
        let safe_cmd = "echo hello ; echo world";
        assert!(ImmunitySystem::pre_check_command(safe_cmd).is_ok());
    }

    #[test]
    fn test_evolve_register() {
        let tmp = env::temp_dir().join("aion_intel_test_evolve");
        let _ = fs::remove_dir_all(&tmp);
        let paths = RouterPaths::for_workspace(&tmp);
        paths.ensure_base_dirs().unwrap();

        let result = Synthesizer::evolve(
            &paths,
            "custom_parser",
            "parse custom",
            "Must handle nested",
        );
        assert!(result.is_ok());

        let skill = result.unwrap();
        assert!(skill.metadata.name.contains("evolved"));
        assert!(skill.root_dir.join("main.rs").exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
