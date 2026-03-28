use std::fs;

use anyhow::Result;
use serde_json::json;

use aion_types::capability_registry::CapabilityRegistry;
use aion_types::types::{PermissionSet, RouterPaths, SkillDefinition, SkillMetadata, SkillSource};

/// Default AI instruction templates for known AI capabilities.
/// Fallback when the registry knows a capability requires AI but no custom instruction exists.
// code_lint 和 code_test 已有专用 Rust builtin，不再需要 AI 模板
const DEFAULT_AI_INSTRUCTIONS: &[(&str, &str)] = &[
    ("code_generate", "Generate code for the given requirement. Return only the code."),
    ("text_summarize", "Summarize the given text concisely in 2-3 sentences."),
    ("text_translate", "Translate the given text. If Chinese, translate to English; if English, translate to Chinese. Return only the translation."),
    ("text_classify", "Classify the given text into a single category label. Return only the label."),
    ("text_extract", "Extract key entities (names, organizations, terms, dates) from the text. Return as JSON array."),
    ("image_describe", "Describe the image at the given path or URL."),
    ("pdf_parse", "Extract and structure text content from the given PDF."),
];

/// Look up the default AI instruction template for a known AI capability.
/// Returns `None` if the capability is not AI-dependent or has no template.
pub fn ai_instruction_for(capability: &str) -> Option<&'static str> {
    DEFAULT_AI_INSTRUCTIONS
        .iter()
        .find(|(name, _)| *name == capability)
        .map(|(_, instr)| *instr)
}

pub struct Synthesizer;

impl Synthesizer {
    /// Create a placeholder skill definition.
    /// Uses registry metadata to determine if this capability needs AI or a direct builtin.
    pub fn placeholder_definition(
        paths: &RouterPaths,
        capability: &str,
        _task: &str,
        registry: Option<&CapabilityRegistry>,
    ) -> Result<SkillDefinition> {
        let root_dir = paths
            .generated_skills_dir
            .join(format!("{capability}_placeholder"));

        // Determine type from registry metadata (not hardcoded list)
        let needs_ai = registry
            .map(|r| r.capability_requires_ai(capability))
            .unwrap_or(false);
        let needs_network = registry
            .map(|r| r.capability_requires_network(capability))
            .unwrap_or(false);

        // AI capabilities that have their own dedicated builtin (not generic ai_task)
        const DEDICATED_AI_BUILTINS: &[&str] = &[
            "ai_parallel_solve", "ai_triple_vote", "ai_triangle_review",
            "ai_code_generate", "ai_smart_collaborate", "ai_research",
            "ai_serial_optimize", "ai_long_context", "ai_cross_review",
            "code_lint", "code_test", "pdf_parse", "spec_driven",
        ];

        let (entrypoint, instruction) = if DEDICATED_AI_BUILTINS.contains(&capability) {
            // 有专用 builtin 的 AI 能力，直接路由到自己的 builtin
            (format!("builtin:{capability}"), None)
        } else if needs_ai {
            let instr = DEFAULT_AI_INSTRUCTIONS
                .iter()
                .find(|(name, _)| *name == capability)
                .map(|(_, i)| i.to_string())
                .unwrap_or_else(|| {
                    registry
                        .and_then(|r| r.get(capability))
                        .map(|def| format!("{}. Process the input and return the result.", def.description))
                        .unwrap_or_else(|| format!("Execute '{}' on the given input.", capability))
                });
            ("builtin:ai_task".to_string(), Some(instr))
        } else {
            (format!("builtin:{capability}"), None)
        };

        let mut permissions = PermissionSet::default_deny();
        if needs_network || needs_ai {
            permissions = permissions.with_network(true);
        }

        Ok(SkillDefinition {
            metadata: SkillMetadata {
                name: format!("{capability}_placeholder"),
                version: "0.1.0".to_string(),
                capabilities: vec![capability.to_string()],
                entrypoint,
                permissions,
                instruction,
            },
            root_dir,
            source: SkillSource::Generated,
        })
    }

    /// Backward-compatible: no registry awareness.
    pub fn create_placeholder(
        paths: &RouterPaths,
        capability: &str,
        task: &str,
    ) -> Result<SkillDefinition> {
        Self::create_placeholder_with_context(paths, capability, task, None, None)
    }

    /// Registry-aware version (preferred).
    pub fn create_placeholder_aware(
        paths: &RouterPaths,
        capability: &str,
        task: &str,
        registry: Option<&CapabilityRegistry>,
    ) -> Result<SkillDefinition> {
        Self::create_placeholder_with_context(paths, capability, task, None, registry)
    }

    pub fn create_placeholder_with_context(
        paths: &RouterPaths,
        capability: &str,
        task: &str,
        discovery_context: Option<serde_json::Value>,
        registry: Option<&CapabilityRegistry>,
    ) -> Result<SkillDefinition> {
        let definition = Self::placeholder_definition(paths, capability, task, registry)?;
        Self::persist_definition(&definition)?;

        let mut readme_content = format!(
            "# {}\n\nGenerated locally for capability `{}` from task `{}`.\n",
            definition.metadata.name, capability, task
        );

        if let Some(ctx) = discovery_context {
            readme_content.push_str("\n## Discovery Intelligence\n");
            readme_content.push_str("Found related knowledge during evolution phase:\n\n");
            if let Some(hits) = ctx["hits"].as_array() {
                for hit in hits.iter().take(3) {
                    readme_content.push_str(&format!(
                        "- **{}**: {} (Source: {:?})\n",
                        hit["title"].as_str().unwrap_or("Untitled"),
                        hit["snippet"].as_str().unwrap_or("..."),
                        hit["source"]
                    ));
                }
            }
        }

        fs::write(definition.root_dir.join("README.md"), readme_content)?;
        Ok(definition)
    }

    pub fn evolve(
        paths: &RouterPaths,
        capability: &str,
        task: &str,
        requirement: &str,
    ) -> Result<SkillDefinition> {
        let name = format!("{}_evolved", capability);
        let root_dir = paths.generated_skills_dir.join(&name);
        let definition = SkillDefinition {
            metadata: SkillMetadata {
                name,
                version: "0.1.0".to_string(),
                capabilities: vec![capability.to_string()],
                entrypoint: "main.rs".to_string(),
                permissions: PermissionSet::default_deny(),
                instruction: None,
            },
            root_dir,
            source: SkillSource::Generated,
        };
        Self::persist_definition(&definition)?;
        let code = format!(
            "// Automatically evolved skill for {}\nfn main() {{\n    println!(\"Task: {}\");\n    // Requirement: {}\n}}",
            capability, task, requirement
        );
        fs::write(definition.root_dir.join("main.rs"), code)?;
        Ok(definition)
    }

    fn persist_definition(definition: &SkillDefinition) -> Result<()> {
        fs::create_dir_all(&definition.root_dir)?;
        let mut map = serde_json::Map::new();
        map.insert("name".into(), json!(definition.metadata.name));
        map.insert("version".into(), json!(definition.metadata.version));
        map.insert("capabilities".into(), json!(definition.metadata.capabilities));
        map.insert("entrypoint".into(), json!(definition.metadata.entrypoint));
        map.insert("permissions".into(), json!(definition.metadata.permissions));
        if let Some(ref instr) = definition.metadata.instruction {
            map.insert("instruction".into(), json!(instr));
        }
        fs::write(
            definition.root_dir.join("skill.json"),
            serde_json::to_vec_pretty(&serde_json::Value::Object(map))?,
        )?;
        Ok(())
    }
}
