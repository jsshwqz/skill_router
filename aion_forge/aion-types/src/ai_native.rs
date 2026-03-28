use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::SystemTime;

use crate::types::ExecutionContext;

// ---------------------------------------------------------------------------
// Priority levels for payload scheduling
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Background = 0,
    Low = 1,
    #[default]
    Normal = 2,
    High = 3,
    Critical = 4,
}

// ---------------------------------------------------------------------------
// AI Backend enumeration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiBackend {
    #[default]
    Ollama,
    OpenAi,
    GoogleAi,
    Custom(String),
}

impl AiBackend {
    /// Resolve the base URL for this backend.
    pub fn base_url(&self) -> String {
        match self {
            AiBackend::Ollama => {
                std::env::var("AI_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434/v1".to_string())
            }
            AiBackend::OpenAi => {
                std::env::var("OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
            }
            AiBackend::GoogleAi => {
                std::env::var("GOOGLE_AI_BASE_URL")
                    .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta".to_string())
            }
            AiBackend::Custom(url) => url.clone(),
        }
    }

    /// Resolve the API key for this backend.
    pub fn api_key(&self) -> String {
        match self {
            AiBackend::Ollama => std::env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string()),
            AiBackend::OpenAi => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            AiBackend::GoogleAi => std::env::var("GOOGLE_AI_API_KEY").unwrap_or_default(),
            AiBackend::Custom(_) => std::env::var("AI_API_KEY").unwrap_or_default(),
        }
    }

    /// Resolve the default model for this backend.
    pub fn default_model(&self) -> String {
        match self {
            AiBackend::Ollama => std::env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string()),
            AiBackend::OpenAi => std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string()),
            AiBackend::GoogleAi => std::env::var("GOOGLE_AI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string()),
            AiBackend::Custom(_) => std::env::var("AI_MODEL").unwrap_or_else(|_| "default".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Payload metadata (caller tracing)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadMeta {
    /// Unique ID of the calling agent
    #[serde(default)]
    pub agent_id: String,
    /// Session identifier for cross-session tracing
    #[serde(default)]
    pub session_id: String,
    /// Timestamp (UNIX seconds)
    #[serde(default)]
    pub timestamp: u64,
    /// Preferred AI backend
    #[serde(default)]
    pub backend: AiBackend,
    /// Custom model override (empty = use backend default)
    #[serde(default)]
    pub model: String,
}

impl Default for PayloadMeta {
    fn default() -> Self {
        Self {
            agent_id: String::new(),
            session_id: String::new(),
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            backend: AiBackend::default(),
            model: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Autonomous behavior & error recovery configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousConfig {
    /// If true, router will attempt to align `parameters` to skill schema automatically
    #[serde(default)]
    pub auto_align_params: bool,
    /// Strategy to use when execution fails
    #[serde(default)]
    pub recovery_strategy: RecoveryStrategy,
    /// Maximum retries for autonomous recovery
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_max_retries() -> u32 { 1 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStrategy {
    /// Just return the error
    #[default]
    None,
    /// Automatically trigger DiscoveryRadar and re-synthesize the skill
    ReSynthesize,
    /// Fallback to a predefined generic capability
    Fallback(String),
}

impl Default for AutonomousConfig {
    fn default() -> Self {
        Self {
            auto_align_params: false,
            recovery_strategy: RecoveryStrategy::None,
            max_retries: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// AiNativePayload — the unified, dehumanized invocation contract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiNativePayload {
    /// Semantic intent in snake_case (e.g. "yaml_parse", "web_search")
    pub intent: String,
    /// If provided, skip the Planner and route directly to this capability
    #[serde(default)]
    pub capability: Option<String>,
    /// Structured parameters (replaces natural language)
    #[serde(default)]
    pub parameters: Value,
    /// Scheduling priority
    #[serde(default)]
    pub priority: Priority,
    /// Autonomous behavior configuration
    #[serde(default)]
    pub autonomous: AutonomousConfig,
    /// Caller metadata
    #[serde(default)]
    pub metadata: PayloadMeta,
}

impl AiNativePayload {
    /// Create a new payload with minimal required fields.
    pub fn new(intent: &str) -> Self {
        Self {
            intent: intent.to_string(),
            capability: None,
            parameters: Value::Object(Default::default()),
            priority: Priority::Normal,
            autonomous: AutonomousConfig::default(),
            metadata: PayloadMeta::default(),
        }
    }

    /// Builder: set autonomous configuration.
    pub fn with_autonomous(mut self, config: AutonomousConfig) -> Self {
        self.autonomous = config;
        self
    }

    /// Builder: set capability directly (skips Planner inference).
    pub fn with_capability(mut self, cap: &str) -> Self {
        self.capability = Some(cap.to_string());
        self
    }

    /// Builder: attach structured parameters.
    pub fn with_parameters(mut self, params: Value) -> Self {
        self.parameters = params;
        self
    }

    /// Builder: set priority.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: set agent ID.
    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.metadata.agent_id = agent_id.to_string();
        self
    }

    /// Builder: set session ID.
    pub fn with_session(mut self, session_id: &str) -> Self {
        self.metadata.session_id = session_id.to_string();
        self
    }

    /// Builder: set AI backend.
    pub fn with_backend(mut self, backend: AiBackend) -> Self {
        self.metadata.backend = backend;
        self
    }

    // -----------------------------------------------------------------------
    // Conversion: AiNativePayload → ExecutionContext
    // -----------------------------------------------------------------------

    /// Convert this native payload into an ExecutionContext for the Executor.
    pub fn to_execution_context(&self) -> ExecutionContext {
        let capability = self
            .capability
            .clone()
            .unwrap_or_else(|| self.intent.clone());

        let mut context = self.parameters.clone();
        // Inject metadata into context for downstream consumers
        if let Value::Object(ref mut map) = context {
            map.insert(
                "_meta".to_string(),
                json!({
                    "agent_id": self.metadata.agent_id,
                    "session_id": self.metadata.session_id,
                    "timestamp": self.metadata.timestamp,
                    "backend": self.metadata.backend,
                    "priority": self.priority,
                    "autonomous": self.autonomous,
                }),
            );
        }

        ExecutionContext {
            task: self.intent.clone(),
            capability,
            context,
            artifacts: Value::Object(Default::default()),
        }
    }

    // -----------------------------------------------------------------------
    // Conversion: ExecutionContext → AiNativePayload
    // -----------------------------------------------------------------------

    /// Reconstruct a payload from an ExecutionContext (best-effort).
    pub fn from_execution_context(ctx: &ExecutionContext) -> Self {
        let mut parameters = ctx.context.clone();
        let mut meta = PayloadMeta::default();
        let mut priority = Priority::Normal;
        let mut autonomous = AutonomousConfig::default();

        // Extract metadata if embedded
        if let Value::Object(ref mut map) = parameters {
            if let Some(meta_val) = map.remove("_meta") {
                if let Ok(m) = serde_json::from_value::<PayloadMeta>(meta_val.clone()) {
                    meta = m;
                }
                if let Some(p) = meta_val.get("priority") {
                    if let Ok(pr) = serde_json::from_value::<Priority>(p.clone()) {
                        priority = pr;
                    }
                }
                if let Some(a) = meta_val.get("autonomous") {
                    if let Ok(auto) = serde_json::from_value::<AutonomousConfig>(a.clone()) {
                        autonomous = auto;
                    }
                }
            }
        }

        Self {
            intent: ctx.task.clone(),
            capability: Some(ctx.capability.clone()),
            parameters,
            priority,
            autonomous,
            metadata: meta,
        }
    }

    // -----------------------------------------------------------------------
    // Serialization helpers
    // -----------------------------------------------------------------------

    /// Serialize to JSON string.
    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| anyhow!("serialize error: {}", e))
    }

    /// Deserialize from JSON string.
    pub fn from_json_str(s: &str) -> Result<Self> {
        serde_json::from_str(s).map_err(|e| anyhow!("deserialize error: {}", e))
    }

    /// Sort a batch of payloads by priority (Critical first).
    pub fn sort_by_priority(payloads: &mut [AiNativePayload]) {
        payloads.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
}
