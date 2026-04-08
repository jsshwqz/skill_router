//! Core engine — holds router + memory + AI config

use aion_memory::memory::{MemoryManager, MemoryCategory};
use aion_router::SkillRouter;
use aion_types::types::RouterPaths;


pub struct Engine {
    pub router: SkillRouter,
    pub memory: MemoryManager,
    pub http: reqwest::Client,
    pub ai_base_url: String,
    pub ai_api_key: String,
    pub ai_model: String,
}

impl Engine {
    pub fn new() -> anyhow::Result<Self> {
        let workspace = std::env::current_dir()?;
        let paths = RouterPaths {
            workspace_root: workspace.clone(),
            skills_dir: workspace.join("skills"),
            state_dir: workspace.join(".skill-router"),
            generated_skills_dir: workspace.join(".skill-router/generated"),
            registry_path: workspace.join(".skill-router/registry.json"),
            executions_log: workspace.join(".skill-router/executions.log"),
            trusted_sources_path: workspace.join(".skill-router/trusted_sources.json"),
            capabilities_dir: workspace.join(".skill-router/capabilities"),
        };

        // Ensure state dirs exist
        std::fs::create_dir_all(&paths.state_dir)?;
        std::fs::create_dir_all(&paths.generated_skills_dir)?;
        std::fs::create_dir_all(&paths.capabilities_dir)?;

        let router = SkillRouter::new(paths)?;
        let memory = MemoryManager::new(&workspace);

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(
                std::env::var("REQUEST_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(120),
            ))
            .build()?;

        let ai_base_url = std::env::var("AI_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434/v1".into());
        let ai_api_key = std::env::var("AI_API_KEY").unwrap_or_default();
        let ai_model = std::env::var("AI_MODEL")
            .unwrap_or_else(|_| "qwen2.5:7b".into());

        Ok(Self { router, memory, http, ai_base_url, ai_api_key, ai_model })
    }

    /// Store a lesson via aion-memory
    pub fn remember(&self, content: &str, category: MemoryCategory) -> anyhow::Result<String> {
        self.memory.remember(category, content, "aion-zl", 7)
    }

    /// Recall memories
    pub fn recall(&self, query: &str) -> anyhow::Result<Vec<aion_memory::memory::MemoryEntry>> {
        self.memory.recall(query, 5)
    }

    /// Route a task through aion-router (in-process, no HTTP)
    pub async fn route(&self, task: &str) -> anyhow::Result<aion_types::types::RouteResult> {
        self.router.route(task).await
    }
}
