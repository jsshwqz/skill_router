use std::collections::BTreeMap;

use serde_json::{json, Value};

/// One public Aion Forge tool exposed by both direct CLI and MCP catalogs.
pub(crate) struct ToolCatalogEntry {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) input_schema: Value,
    pub(crate) requires_approval: bool,
}

/// Build the single authoritative public tool catalog.
pub(crate) fn entries() -> Vec<ToolCatalogEntry> {
    let registry = aion_types::capability_registry::CapabilityRegistry::builtin();
    let mut entries: BTreeMap<String, ToolCatalogEntry> = registry
        .definitions()
        .map(|capability| {
            let input_schema = if capability.parameters_schema.is_null() || capability.parameters_schema == json!({}) {
                json!({
                    "type": "object",
                    "properties": properties_from_inputs(&capability.inputs),
                    "required": capability.inputs,
                })
            } else {
                capability.parameters_schema.clone()
            };
            (
                capability.name.clone(),
                ToolCatalogEntry {
                    name: capability.name.clone(),
                    description: capability.description.clone(),
                    input_schema,
                    requires_approval: capability.requires_approval,
                },
            )
        })
        .collect();

    entries
        .entry("sanitize".to_string())
        .or_insert_with(|| ToolCatalogEntry {
            name: "sanitize".to_string(),
            description: "Remove dangerous control characters and report sanitization details.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {"text": {"type": "string", "description": "Text to sanitize."}},
                "required": ["text"],
            }),
            requires_approval: false,
        });

    entries.into_values().collect()
}

fn properties_from_inputs(inputs: &[String]) -> Value {
    let properties = inputs
        .iter()
        .map(|input| (input.clone(), json!({"type": "string", "description": input})))
        .collect();
    Value::Object(properties)
}
