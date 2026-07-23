use aion_router::builtins::BuiltinRegistry;
use aion_types::capability_registry::CapabilityRegistry;
use serde_json::Value;

/// Metadata for one live Forge capability exposed to the ACP planner.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityEntry {
    /// Exact builtin registry name.
    pub name: String,
    /// Public description sourced from the capability registry.
    pub description: String,
    /// JSON Schema for the capability argument object.
    pub parameters_schema: Value,
    /// Whether the client should ask for approval before execution.
    pub requires_approval: bool,
    /// Whether the ACP planner may call this capability directly.
    pub planner_callable: bool,
}

/// Deterministic intersection of executable builtins and public capability metadata.
#[derive(Debug, Clone, Default)]
pub struct CapabilityCatalog {
    entries: Vec<CapabilityEntry>,
}

impl CapabilityCatalog {
    /// Build a catalog containing only capabilities present in both registries.
    pub fn from_registries(builtins: &BuiltinRegistry, metadata: &CapabilityRegistry) -> Self {
        let entries = builtins
            .list_skills()
            .into_iter()
            .filter_map(|name| {
                metadata.get(name).map(|definition| CapabilityEntry {
                    name: name.to_string(),
                    description: definition.description.clone(),
                    parameters_schema: definition.parameters_schema.clone(),
                    requires_approval: definition.requires_approval,
                    planner_callable: is_planner_callable(name),
                })
            })
            .collect();
        Self { entries }
    }

    /// Build the live catalog from Forge's default builtin and metadata registries.
    pub fn live() -> Self {
        Self::from_registries(&BuiltinRegistry::default_registry(), &CapabilityRegistry::builtin())
    }

    /// Return all live entries in stable builtin-name order.
    pub fn entries(&self) -> &[CapabilityEntry] {
        &self.entries
    }

    /// Find a live capability by its exact registry name.
    pub fn entry(&self, name: &str) -> Option<&CapabilityEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }
}

pub(crate) fn is_planner_callable(name: &str) -> bool {
    !matches!(name, "ai_task" | "autonomous_agent")
}

#[cfg(test)]
mod tests {
    use aion_router::builtins::BuiltinRegistry;
    use aion_types::capability_registry::CapabilityRegistry;

    use super::CapabilityCatalog;

    #[test]
    fn live_catalog_advertises_only_registered_builtins() {
        let registry = BuiltinRegistry::default_registry();
        let metadata = CapabilityRegistry::builtin();
        let catalog = CapabilityCatalog::from_registries(&registry, &metadata);

        assert!(catalog
            .entries()
            .iter()
            .all(|entry| registry.get(&entry.name).is_some()));
        assert!(catalog.entries().iter().any(|entry| entry.name == "yaml_parse"));
        assert!(!catalog.entries().iter().any(|entry| entry.name == "text_summarize"));
    }

    #[test]
    fn descriptions_and_schemas_come_from_capability_metadata() {
        let registry = BuiltinRegistry::default_registry();
        let metadata = CapabilityRegistry::builtin();
        let catalog = CapabilityCatalog::from_registries(&registry, &metadata);
        let entry = catalog.entry("yaml_parse").unwrap();
        let definition = metadata.get("yaml_parse").unwrap();

        assert_eq!(entry.description, definition.description);
        assert_eq!(entry.parameters_schema, definition.parameters_schema);
        assert_eq!(entry.requires_approval, definition.requires_approval);
    }

    #[test]
    fn recursive_planner_entries_are_not_callable() {
        let registry = BuiltinRegistry::default_registry();
        let catalog = CapabilityCatalog::from_registries(&registry, &CapabilityRegistry::builtin());

        assert!(!catalog.entry("autonomous_agent").unwrap().planner_callable);
    }
}
