use std::path::PathBuf;

use anyhow::{bail, Result};
use glitch_filter::{GlitchFilter, Sanitizer};
use serde_json::{json, Value};

use aion_types::types::ExecutionContext;

/// Return the direct CLI tool catalog.
pub fn list_tools() -> Value {
    let tools: Vec<Value> = crate::catalog::entries()
        .into_iter()
        .map(|entry| {
            json!({
                "name": entry.name,
                "description": entry.description,
            })
        })
        .collect();
    json!({"total": tools.len(), "tools": tools})
}

/// Execute one built-in tool with the supplied direct CLI parameters.
pub async fn execute(tool_name: &str, raw_params: Option<&str>, quiet: bool) -> Result<Value> {
    if tool_name.is_empty() {
        bail!("No tool provided. Use --tool <name> or --list.");
    }

    let mut params: Value = match raw_params {
        Some(raw) => serde_json::from_str(raw).unwrap_or_else(|_| json!({"text": raw})),
        None => json!({}),
    };

    sanitize_text(&mut params, quiet);

    let registry = aion_router::builtins::BuiltinRegistry::default_registry();
    let Some(builtin) = registry.get(tool_name) else {
        bail!("Tool '{}' not found. Use --list to view available tools.", tool_name);
    };

    let context = ExecutionContext::new(tool_name, tool_name).with_context(params);
    let skill = aion_types::types::SkillDefinition {
        metadata: aion_types::types::SkillMetadata {
            name: tool_name.to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![tool_name.to_string()],
            entrypoint: format!("builtin:{tool_name}"),
            permissions: aion_types::types::PermissionSet::default_deny().with_network(true),
            instruction: None,
            engine_capable: false,
        },
        root_dir: PathBuf::new(),
        source: aion_types::types::SkillSource::Local,
    };

    builtin.execute(&skill, &context).await
}

fn sanitize_text(params: &mut Value, quiet: bool) {
    let Some(text) = params.get("text").and_then(Value::as_str) else {
        return;
    };

    let filter = GlitchFilter::new();
    let alerts = filter.check(text);
    if !alerts.is_empty() {
        tracing::warn!(count = alerts.len(), "glitch tokens detected in direct input");
    }

    let mut sanitizer = Sanitizer::new();
    let clean_text = sanitizer.sanitize(text);
    if sanitizer.sanitized_count() > 0 {
        if !quiet {
            tracing::warn!(
                count = sanitizer.sanitized_count(),
                "dangerous control characters removed from direct input"
            );
        }
        params["text"] = json!(clean_text);
    }
}
