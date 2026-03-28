#[cfg(test)]
mod tests {
    use crate::parallel_executor::ParallelExecutor;
    use aion_types::capability_registry::CapabilityRegistry;
    use aion_types::types::RouterPaths;
    use aion_types::parallel::{ParallelInstruction, TaskGraph};
    use std::env;
    use std::fs;

    #[tokio::test]
    async fn test_parallel_5_tasks() {
        let tmp = env::temp_dir().join("aion_router_test_parallel");
        let _ = fs::remove_dir_all(&tmp);
        let paths = RouterPaths::for_workspace(&tmp);
        paths.ensure_base_dirs().unwrap();
        let reg = CapabilityRegistry::builtin();

        let instructions: Vec<ParallelInstruction> = (0..5)
            .map(|i| ParallelInstruction::simple(
                &format!("task_{}", i),
                &format!("echo test {}", i),
                "echo",
            ))
            .collect();

        let graph = TaskGraph { instructions };

        let result = ParallelExecutor::execute_graph(graph, &paths, &reg).await;
        assert!(result.is_ok(), "Parallel execution failed: {:?}", result.err());
        let response = result.unwrap();
        assert_eq!(response.results.len(), 5);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_concurrent_routing() {
        use crate::SkillRouter;
        use std::sync::Arc;

        let tmp = env::temp_dir().join("aion_router_test_concurrent");
        let _ = fs::remove_dir_all(&tmp);
        let paths = RouterPaths::for_workspace(&tmp);
        paths.ensure_base_dirs().unwrap();

        let router = Arc::new(SkillRouter::new(paths).unwrap());
        let mut handles = vec![];

        for i in 0..10 {
            let r = Arc::clone(&router);
            handles.push(tokio::spawn(async move {
                r.route(&format!("echo hello {}", i)).await
            }));
        }

        for h in handles {
            let res = h.await.unwrap();
            assert!(res.is_ok(), "Concurrent routing failed: {:?}", res.err());
        }

        let _ = fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_end_to_end_evolution() {
        use crate::SkillRouter;

        let tmp = env::temp_dir().join("aion_router_test_evolution");
        let _ = fs::remove_dir_all(&tmp);
        let paths = RouterPaths::for_workspace(&tmp);
        paths.ensure_base_dirs().unwrap();

        let router = SkillRouter::new(paths.clone()).unwrap();

        // Task that definitely shouldn't exist locally
        let task = "navigate to the Andromeda galaxy";
        let result = router.route(task).await;

        assert!(result.is_ok(), "Evolution routing failed: {:?}", result.err());
        let res = result.unwrap();

        // Verify it was a generated (synthesized) skill
        assert!(res.skill.metadata.name.contains("placeholder"));

        // Verify files were actually created
        let skill_dir = res.skill.root_dir.clone();
        assert!(skill_dir.exists());
        assert!(skill_dir.join("skill.json").exists());
        assert!(skill_dir.join("README.md").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Security 模块测试
    // ══════════════════════════════════════════════════════════════════════════

    mod security_tests {
        use crate::security::{AiSecurityReviewer, Security, Verdict};
        use aion_types::types::{
            ExecutionContext, ExecutionResponse, PermissionSet, RouterPaths,
            SkillDefinition, SkillMetadata, SkillSource,
        };
        use serde_json::{json, Value};
        use std::path::PathBuf;

        // ── 辅助工厂函数 ─────────────────────────────────────────────────

        fn make_skill(entrypoint: &str) -> SkillDefinition {
            SkillDefinition {
                metadata: SkillMetadata {
                    name: "test_skill".into(),
                    version: "0.1.0".into(),
                    capabilities: vec!["http_fetch".into()],
                    entrypoint: entrypoint.into(),
                    permissions: PermissionSet::default(),
                    instruction: None,
                },
                root_dir: PathBuf::from("/tmp/test_skill"),
                source: SkillSource::Local,
            }
        }

        fn make_skill_with_perms(entrypoint: &str, perms: PermissionSet) -> SkillDefinition {
            let mut skill = make_skill(entrypoint);
            skill.metadata.permissions = perms;
            skill
        }

        fn make_context_with_url(url: &str) -> ExecutionContext {
            ExecutionContext {
                task: "fetch url".into(),
                capability: "http_fetch".into(),
                context: json!({"url": url}),
                artifacts: Value::Object(Default::default()),
            }
        }

        fn make_response(result: Value) -> ExecutionResponse {
            ExecutionResponse {
                status: "success".into(),
                result,
                artifacts: Value::Object(Default::default()),
                error: None,
            }
        }

        // ── is_private_network_url 测试 ──────────────────────────────────

        #[test]
        fn test_private_network_ipv4_loopback() {
            assert!(AiSecurityReviewer::is_private_network_url("http://127.0.0.1/api"));
            assert!(AiSecurityReviewer::is_private_network_url("https://127.0.0.1:8080/path"));
            assert!(AiSecurityReviewer::is_private_network_url("http://localhost/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://localhost:3000"));
        }

        #[test]
        fn test_private_network_ipv4_rfc1918() {
            assert!(AiSecurityReviewer::is_private_network_url("http://10.0.0.1/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://10.255.255.255/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://172.16.0.1/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://172.31.255.255/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://192.168.1.1/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://192.168.0.100:8080/"));
            // 169.254.x.x (link-local)
            assert!(AiSecurityReviewer::is_private_network_url("http://169.254.1.1/"));
        }

        #[test]
        fn test_private_network_ipv6_loopback() {
            assert!(AiSecurityReviewer::is_private_network_url("http://[::1]/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://[::1]:8080/path"));
        }

        #[test]
        fn test_private_network_ipv6_ula() {
            assert!(AiSecurityReviewer::is_private_network_url("http://[fc00::1]/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://[fd12:3456::1]/"));
        }

        #[test]
        fn test_private_network_ipv4_mapped() {
            assert!(AiSecurityReviewer::is_private_network_url("http://[::ffff:127.0.0.1]/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://[::ffff:10.0.0.1]/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://[::ffff:192.168.1.1]/"));
        }

        #[test]
        fn test_private_network_domains() {
            assert!(AiSecurityReviewer::is_private_network_url("http://myapp.local/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://server.corp/api"));
            assert!(AiSecurityReviewer::is_private_network_url("http://db.internal/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://printer.lan/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://gateway.intranet/"));
            assert!(AiSecurityReviewer::is_private_network_url("http://nas.home.arpa/"));
        }

        #[test]
        fn test_private_network_public_allowed() {
            assert!(!AiSecurityReviewer::is_private_network_url("https://google.com/"));
            assert!(!AiSecurityReviewer::is_private_network_url("https://8.8.8.8/"));
            assert!(!AiSecurityReviewer::is_private_network_url("https://api.github.com/repos"));
            assert!(!AiSecurityReviewer::is_private_network_url("https://example.com:443/path"));
        }

        // ── heuristic_pre 测试 ──────────────────────────────────────────

        #[test]
        fn test_heuristic_pre_blocks_sensitive_key() {
            let skill = make_skill("builtin:http_fetch");
            let ctx = ExecutionContext {
                task: "fetch data".into(),
                capability: "http_fetch".into(),
                context: json!({"url": "https://example.com", "api_key": "sk-xxx"}),
                artifacts: Value::Object(Default::default()),
            };
            let result = AiSecurityReviewer::heuristic_pre(&skill, &ctx);
            assert!(result.is_some());
            assert!(result.unwrap().contains("api_key"));
        }

        #[test]
        fn test_heuristic_pre_blocks_shell_exec() {
            let skill = make_skill("builtin:shell_execute");
            let ctx = ExecutionContext::new("run command", "shell_execute");
            let result = AiSecurityReviewer::heuristic_pre(&skill, &ctx);
            assert!(result.is_some());
            assert!(result.unwrap().contains("shell/exec"));
        }

        #[test]
        fn test_heuristic_pre_blocks_process_exec_perm() {
            let perms = PermissionSet {
                network: false,
                filesystem_read: false,
                filesystem_write: false,
                process_exec: true,
                sandboxed_exec: false,
            };
            let skill = make_skill_with_perms("builtin:custom", perms);
            let ctx = ExecutionContext::new("do something", "custom");
            let result = AiSecurityReviewer::heuristic_pre(&skill, &ctx);
            assert!(result.is_some());
            assert!(result.unwrap().contains("process_exec"));
        }

        #[test]
        fn test_heuristic_pre_blocks_private_url() {
            let skill = make_skill("builtin:http_fetch");
            let ctx = make_context_with_url("http://192.168.1.1/admin");
            let result = AiSecurityReviewer::heuristic_pre(&skill, &ctx);
            assert!(result.is_some());
            assert!(result.unwrap().contains("private/internal"));
        }

        #[test]
        fn test_heuristic_pre_blocks_non_https() {
            let skill = make_skill("builtin:http_fetch");
            let ctx = make_context_with_url("http://example.com/data");
            let result = AiSecurityReviewer::heuristic_pre(&skill, &ctx);
            assert!(result.is_some());
            assert!(result.unwrap().contains("HTTPS"));
        }

        #[test]
        fn test_heuristic_pre_allows_safe_request() {
            let skill = make_skill("builtin:text_summarize");
            let ctx = ExecutionContext {
                task: "summarize this text".into(),
                capability: "text_summarize".into(),
                context: json!({"text": "Hello world, this is a test."}),
                artifacts: Value::Object(Default::default()),
            };
            let result = AiSecurityReviewer::heuristic_pre(&skill, &ctx);
            assert!(result.is_none(), "Safe request should pass: {:?}", result);
        }

        // ── heuristic_post 测试 ─────────────────────────────────────────

        #[test]
        fn test_heuristic_post_detects_aws_key() {
            let resp = make_response(json!("Found key: AKIAIOSFODNN7EXAMPLE"));
            let result = AiSecurityReviewer::heuristic_post(&resp);
            assert!(result.is_some());
            assert!(result.unwrap().contains("AWS"));
        }

        #[test]
        fn test_heuristic_post_detects_pem() {
            let resp = make_response(json!("-----BEGIN RSA PRIVATE KEY-----\nMIIE..."));
            let result = AiSecurityReviewer::heuristic_post(&resp);
            assert!(result.is_some());
            assert!(result.unwrap().contains("PEM"));
        }

        #[test]
        fn test_heuristic_post_detects_github_token() {
            let resp = make_response(json!("Token: ghp_ABCDEFghijklmnopqrst1234567890ab"));
            let result = AiSecurityReviewer::heuristic_post(&resp);
            assert!(result.is_some());
            assert!(result.unwrap().contains("GitHub"));
        }

        #[test]
        fn test_heuristic_post_detects_env_leak() {
            let resp = make_response(json!("SERPAPI_KEY=abcdef12345\nAI_BASE_URL=http://localhost"));
            let result = AiSecurityReviewer::heuristic_post(&resp);
            assert!(result.is_some());
            assert!(result.unwrap().contains(".env"));
        }

        #[test]
        fn test_heuristic_post_safe_output() {
            let resp = make_response(json!("The summary of the document is: Rust is a systems programming language."));
            let result = AiSecurityReviewer::heuristic_post(&resp);
            assert!(result.is_none(), "Safe output should pass: {:?}", result);
        }

        // ── Security::validate 测试 ─────────────────────────────────────

        #[test]
        fn test_security_validate_builtin_pass() {
            let skill = make_skill("builtin:echo");
            let tmp = std::env::temp_dir().join("aion_security_test_validate");
            let paths = RouterPaths::for_workspace(&tmp);
            assert!(Security::validate(&skill, &paths).is_ok());
        }

        #[test]
        fn test_security_validate_path_escape() {
            let skill = make_skill("../../etc/passwd");
            let tmp = std::env::temp_dir().join("aion_security_test_escape");
            let paths = RouterPaths::for_workspace(&tmp);
            assert!(Security::validate(&skill, &paths).is_err());
        }

        // ── fail_policy 测试 ────────────────────────────────────────────

        /// 合并为单个测试避免多线程环境变量竞争
        #[test]
        fn test_fail_policy_closed_and_open() {
            // 使用互斥锁序列化环境变量访问
            use std::sync::Mutex;
            static ENV_LOCK: Mutex<()> = Mutex::new(());
            let _guard = ENV_LOCK.lock().unwrap();

            let old = std::env::var("AI_SECURITY_FAIL_POLICY").ok();

            // closed → Deny
            std::env::set_var("AI_SECURITY_FAIL_POLICY", "closed");
            let verdict = AiSecurityReviewer::fail_policy_verdict("test");
            assert!(matches!(verdict, Verdict::Deny(_)), "closed policy should deny");

            // open → Allow
            std::env::set_var("AI_SECURITY_FAIL_POLICY", "open");
            let verdict = AiSecurityReviewer::fail_policy_verdict("test");
            assert_eq!(verdict, Verdict::Allow, "open policy should allow");

            // 恢复
            match old {
                Some(v) => std::env::set_var("AI_SECURITY_FAIL_POLICY", v),
                None => std::env::remove_var("AI_SECURITY_FAIL_POLICY"),
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // MessageBus 模块测试
    // ══════════════════════════════════════════════════════════════════════════

    mod message_bus_tests {
        use crate::message_bus::MessageBus;
        use aion_types::agent_message::{AgentMessage, AgentMessageType};

        fn make_task_msg(from: &str, to: &str, task_id: &str) -> AgentMessage {
            AgentMessage::new(
                from,
                to,
                AgentMessageType::TaskAssignment {
                    task_id: task_id.to_string(),
                    task: "test task".to_string(),
                    capability: "echo".to_string(),
                },
            )
        }

        #[tokio::test]
        async fn test_message_bus_publish_subscribe() {
            let bus = MessageBus::new(64);
            let mut rx = bus.subscribe();

            let msg = make_task_msg("agent_a", "agent_b", "t1");
            let count = bus.publish(msg);
            assert_eq!(count, 1);

            let received = rx.recv().await.unwrap();
            assert_eq!(received.from_agent, "agent_a");
            assert_eq!(received.to_agent, "agent_b");
        }

        #[tokio::test]
        async fn test_message_bus_multiple_subscribers() {
            let bus = MessageBus::new(64);
            let mut rx1 = bus.subscribe();
            let mut rx2 = bus.subscribe();
            let mut rx3 = bus.subscribe();

            let msg = make_task_msg("orchestrator", "", "broadcast_t1");
            let count = bus.publish(msg);
            assert_eq!(count, 3);

            let r1 = rx1.recv().await.unwrap();
            let r2 = rx2.recv().await.unwrap();
            let r3 = rx3.recv().await.unwrap();
            assert_eq!(r1.from_agent, "orchestrator");
            assert_eq!(r2.from_agent, "orchestrator");
            assert_eq!(r3.from_agent, "orchestrator");
        }

        #[tokio::test]
        async fn test_message_bus_no_subscriber() {
            let bus = MessageBus::new(64);
            // 不创建任何订阅者
            let msg = make_task_msg("a", "b", "t1");
            let count = bus.publish(msg);
            assert_eq!(count, 0);
        }

        #[tokio::test]
        async fn test_message_bus_subscriber_count() {
            let bus = MessageBus::new(64);
            assert_eq!(bus.subscriber_count(), 0);

            let _rx1 = bus.subscribe();
            assert_eq!(bus.subscriber_count(), 1);

            let _rx2 = bus.subscribe();
            assert_eq!(bus.subscriber_count(), 2);

            drop(_rx1);
            assert_eq!(bus.subscriber_count(), 1);

            drop(_rx2);
            assert_eq!(bus.subscriber_count(), 0);
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Coordinator + ExpertOpinion 模块测试
    // ══════════════════════════════════════════════════════════════════════════

    mod coordinator_tests {
        use crate::coordinator::{ExpertOpinion, MultiAgentCoordinator};
        use crate::message_bus::MessageBus;
        use aion_types::agent_message::{AgentRef, AgentRole};
        use serde_json::json;
        use std::sync::Arc;
        use std::time::Duration;

        #[test]
        fn test_coordinator_register_agent() {
            let bus = Arc::new(MessageBus::new(64));
            let mut coord = MultiAgentCoordinator::new(bus);

            let agent = AgentRef {
                id: "executor_1".into(),
                role: AgentRole::Executor,
                endpoint: None,
                capabilities: vec!["text_summarize".into()],
            };
            coord.register_agent(agent);

            // 注册第二个
            let agent2 = AgentRef {
                id: "executor_2".into(),
                role: AgentRole::Executor,
                endpoint: None,
                capabilities: vec!["code_generate".into()],
            };
            coord.register_agent(agent2);

            // 验证通过 consult_experts 需要 agents，这里通过内部字段不可直接访问
            // 但可以验证不 panic 且协调器正常创建
        }

        #[test]
        fn test_coordinator_timeout_builder() {
            let bus = Arc::new(MessageBus::new(64));
            let coord = MultiAgentCoordinator::new(bus)
                .with_timeout(Duration::from_secs(60));
            // 验证 builder 模式不 panic
            let _ = coord;
        }

        #[test]
        fn test_expert_opinion_success_rate() {
            let opinions = vec![
                ExpertOpinion {
                    agent_id: "a1".into(),
                    task_id: "t1".into(),
                    success: true,
                    result: json!("ok"),
                    error: None,
                },
                ExpertOpinion {
                    agent_id: "a2".into(),
                    task_id: "t2".into(),
                    success: true,
                    result: json!("fine"),
                    error: None,
                },
                ExpertOpinion {
                    agent_id: "a3".into(),
                    task_id: "t3".into(),
                    success: false,
                    result: json!(null),
                    error: Some("timeout".into()),
                },
            ];

            let rate = ExpertOpinion::success_rate(&opinions);
            assert!((rate - 0.667).abs() < 0.01, "Expected ~0.667, got {}", rate);
        }

        #[test]
        fn test_expert_opinion_majority_result() {
            let opinions = vec![
                ExpertOpinion {
                    agent_id: "a1".into(),
                    task_id: "t1".into(),
                    success: false,
                    result: json!(null),
                    error: Some("failed".into()),
                },
                ExpertOpinion {
                    agent_id: "a2".into(),
                    task_id: "t2".into(),
                    success: true,
                    result: json!("the answer"),
                    error: None,
                },
            ];

            let result = ExpertOpinion::majority_result(&opinions);
            assert!(result.is_some());
            assert_eq!(result.unwrap(), &json!("the answer"));
        }

        #[test]
        fn test_expert_opinion_empty() {
            let rate = ExpertOpinion::success_rate(&[]);
            assert_eq!(rate, 0.0);

            let result = ExpertOpinion::majority_result(&[]);
            assert!(result.is_none());
        }
    }
}
