use std::path::Path;

use aion_router::builtins::BuiltinRegistry;
use aion_types::{
    capability_registry::CapabilityRegistry,
    types::{ExecutionContext, PermissionSet, SkillDefinition, SkillMetadata, SkillSource},
};
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::catalog::{is_planner_callable, CapabilityCatalog};

/// Provider-neutral interface used by the ACP planning loop to execute Forge capabilities.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute one exact capability name with a JSON object in the supplied working directory.
    async fn execute(&self, name: &str, arguments: Value, cwd: &Path) -> Result<Value>;
}

/// Production tool executor backed by Forge's builtin registry.
pub struct ForgeToolExecutor {
    registry: BuiltinRegistry,
    catalog: CapabilityCatalog,
}

impl ForgeToolExecutor {
    /// Build an executor from explicit registries, primarily for deterministic tests.
    pub fn from_registries(registry: BuiltinRegistry, metadata: CapabilityRegistry) -> Self {
        let catalog = CapabilityCatalog::from_registries(&registry, &metadata);
        Self { registry, catalog }
    }

    /// Return the exact live capability catalog enforced by this executor.
    pub fn catalog(&self) -> &CapabilityCatalog {
        &self.catalog
    }
}

impl Default for ForgeToolExecutor {
    fn default() -> Self {
        Self::from_registries(BuiltinRegistry::default_registry(), CapabilityRegistry::builtin())
    }
}

#[async_trait]
impl ToolExecutor for ForgeToolExecutor {
    async fn execute(&self, name: &str, arguments: Value, cwd: &Path) -> Result<Value> {
        if !is_planner_callable(name) {
            bail!("Forge capability '{name}' is not callable from ACP planner");
        }
        if !arguments.is_object() {
            bail!("arguments for Forge capability '{name}' must be a JSON object");
        }

        let entry = self
            .catalog
            .entry(name)
            .ok_or_else(|| anyhow::anyhow!("unknown Forge capability '{name}'"))?;
        if !entry.planner_callable {
            bail!("Forge capability '{name}' is not callable from ACP planner");
        }
        let builtin = self
            .registry
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Forge capability '{name}' has no builtin executor"))?;

        let task = arguments
            .get("task")
            .or_else(|| arguments.get("text"))
            .or_else(|| arguments.get("input"))
            .and_then(Value::as_str)
            .unwrap_or(name)
            .to_string();
        let skill = SkillDefinition {
            metadata: SkillMetadata {
                name: name.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: vec![name.to_string()],
                entrypoint: format!("builtin:{name}"),
                permissions: PermissionSet::default(),
                instruction: None,
                engine_capable: false,
            },
            root_dir: cwd.to_path_buf(),
            source: SkillSource::Local,
        };
        let context = ExecutionContext::new(&task, name).with_context(arguments);

        builtin.execute(&skill, &context).await
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use aion_router::builtins::{BuiltinRegistry, BuiltinSkill};
    use aion_types::{
        capability_registry::{CapabilityDefinition, CapabilityRegistry},
        types::{ExecutionContext, SkillDefinition},
    };
    use anyhow::Result;
    use serde_json::{json, Value};

    use super::{ForgeToolExecutor, ToolExecutor};

    struct FakeBuiltin;

    #[async_trait::async_trait]
    impl BuiltinSkill for FakeBuiltin {
        fn name(&self) -> &'static str {
            "fake_builtin"
        }

        async fn execute(&self, skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
            Ok(json!({
                "name": skill.metadata.name,
                "cwd": skill.root_dir,
                "arguments": context.context,
            }))
        }
    }

    fn fake_executor() -> ForgeToolExecutor {
        let mut registry = BuiltinRegistry::new();
        registry.register(Box::new(FakeBuiltin));
        let mut metadata = CapabilityRegistry::default();
        metadata.register(CapabilityDefinition {
            name: "fake_builtin".to_string(),
            description: "Fake builtin for executor tests".to_string(),
            inputs: vec!["value".to_string()],
            outputs: vec!["arguments".to_string()],
            parameters_schema: json!({"type": "object"}),
            examples: Vec::new(),
            requires_approval: false,
            category: "test".to_string(),
        });
        ForgeToolExecutor::from_registries(registry, metadata)
    }

    #[tokio::test]
    async fn executor_rejects_recursive_planner_entry() {
        let executor = ForgeToolExecutor::default();

        let error = executor
            .execute("ai_task", json!({"task": "loop"}), Path::new("."))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("not callable from ACP planner"));
    }

    #[tokio::test]
    async fn executor_rejects_unknown_tools_and_non_object_arguments() {
        let executor = fake_executor();

        assert!(executor
            .execute("missing", json!({}), Path::new("."))
            .await
            .unwrap_err()
            .to_string()
            .contains("unknown Forge capability"));
        assert!(executor
            .execute("fake_builtin", json!(["not", "object"]), Path::new("."))
            .await
            .unwrap_err()
            .to_string()
            .contains("JSON object"));
    }

    #[tokio::test]
    async fn valid_arguments_reach_the_registered_builtin() {
        let executor = fake_executor();
        let result = executor
            .execute(
                "fake_builtin",
                json!({"value": "seen"}),
                Path::new("D:/test/aionui/forge"),
            )
            .await
            .unwrap();

        assert_eq!(result["name"], "fake_builtin");
        assert_eq!(result["arguments"]["value"], "seen");
        assert!(result["cwd"].as_str().unwrap().ends_with("forge"));
    }
}
