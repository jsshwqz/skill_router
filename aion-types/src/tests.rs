#[cfg(test)]
mod ai_native_tests {
    use crate::ai_native::{AiBackend, AiNativePayload, Priority};
    use crate::capability_registry::CapabilityRegistry;
    use serde_json::json;

    #[test]
    fn test_ai_native_roundtrip() {
        let payload = AiNativePayload::new("yaml_parse")
            .with_capability("yaml_parse")
            .with_parameters(json!({"text": "key: value"}))
            .with_priority(Priority::High)
            .with_agent("agent_007")
            .with_session("session_42")
            .with_backend(AiBackend::Ollama);

        let ctx = payload.to_execution_context();
        assert_eq!(ctx.capability, "yaml_parse");
        assert_eq!(ctx.task, "yaml_parse");
        assert_eq!(ctx.context["text"], "key: value");

        let restored = AiNativePayload::from_execution_context(&ctx);
        assert_eq!(restored.intent, "yaml_parse");
        assert_eq!(restored.capability, Some("yaml_parse".to_string()));
        assert_eq!(restored.parameters["text"], "key: value");
    }

    #[test]
    fn test_ai_native_serialization() {
        let payload = AiNativePayload::new("web_search")
            .with_parameters(json!({"query": "rust programming"}));

        let json_str = payload.to_json_string().unwrap();
        let restored = AiNativePayload::from_json_str(&json_str).unwrap();
        assert_eq!(restored.intent, "web_search");
        assert_eq!(restored.parameters["query"], "rust programming");
    }

    #[test]
    fn test_ai_native_priority_sort() {
        let mut payloads = vec![
            AiNativePayload::new("low_task").with_priority(Priority::Low),
            AiNativePayload::new("critical_task").with_priority(Priority::Critical),
            AiNativePayload::new("normal_task").with_priority(Priority::Normal),
            AiNativePayload::new("bg_task").with_priority(Priority::Background),
            AiNativePayload::new("high_task").with_priority(Priority::High),
        ];

        AiNativePayload::sort_by_priority(&mut payloads);
        assert_eq!(payloads[0].intent, "critical_task");
        assert_eq!(payloads[1].intent, "high_task");
        assert_eq!(payloads[2].intent, "normal_task");
        assert_eq!(payloads[3].intent, "low_task");
        assert_eq!(payloads[4].intent, "bg_task");
    }

    #[test]
    fn test_async_task_query_capability_contract() {
        let registry = CapabilityRegistry::builtin();
        let definition = registry
            .get("async_task_query")
            .expect("async_task_query must be declared for its builtin implementation");
        assert_eq!(definition.category, "orchestration");
        assert!(!definition.requires_approval);
        assert!(definition.inputs.contains(&"task_id".to_string()));
        assert_eq!(definition.parameters_schema["type"], "object");
        assert!(definition.parameters_schema["required"].is_null());
    }

    #[test]
    fn test_ai_backend_defaults() {
        let ollama = AiBackend::Ollama;
        let url = ollama.base_url();
        // 环境变量 AI_BASE_URL 会覆盖默认 URL，只在未设置时检查默认值
        if std::env::var("AI_BASE_URL").is_err() {
            assert!(url.contains("11434"), "expected default Ollama URL, got: {}", url);
        }

        let openai = AiBackend::OpenAi;
        assert!(openai.base_url().contains("openai.com"));

        let google = AiBackend::GoogleAi;
        assert!(google.base_url().contains("googleapis"));

        let custom = AiBackend::Custom("https://my-ai.local".into());
        assert_eq!(custom.base_url(), "https://my-ai.local");
    }
}
