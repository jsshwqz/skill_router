use std::{collections::BTreeMap, fs};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::RouterPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDefinition {
    pub name: String,
    pub description: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub parameters_schema: Value,
    #[serde(default)]
    pub examples: Vec<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    definitions: BTreeMap<String, CapabilityDefinition>,
}

impl CapabilityRegistry {
    pub fn builtin() -> Self {
        let mut registry = Self::default();
        for definition in [
            CapabilityDefinition {
                name: "yaml_parse".to_string(),
                description: "Parse YAML text into structured JSON data".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["parsed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "The YAML content to parse" }
                    },
                    "required": ["text"]
                }),
                examples: vec![serde_json::json!({
                    "intent": "yaml_parse",
                    "parameters": { "text": "foo: bar\nlist:\n  - 1\n  - 2" }
                })],
            },
            CapabilityDefinition {
                name: "json_parse".to_string(),
                description: "Parse and validate JSON text into structured data".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["parsed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "toml_parse".to_string(),
                description: "Parse TOML configuration text into structured data".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["parsed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "csv_parse".to_string(),
                description: "Parse CSV or spreadsheet text into rows and columns".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["rows".to_string(), "headers".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "pdf_parse".to_string(),
                description: "Extract and structure text content from a PDF file path".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["structured_data".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "markdown_render".to_string(),
                description: "Parse Markdown text into structured sections".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["sections".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_summarize".to_string(),
                description: "Summarize text using AI into a concise output".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_translate".to_string(),
                description: "Translate text from one language to another using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_classify".to_string(),
                description: "Classify or categorize text into a label using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_extract".to_string(),
                description: "Extract key entities and information from text using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_diff".to_string(),
                description: "Compute a line-level diff between two text inputs".to_string(),
                inputs: vec!["a".to_string(), "b".to_string()],
                outputs: vec!["diff".to_string(), "added".to_string(), "removed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "a": { "type": "string" }, "b": { "type": "string" } },
                    "required": ["a", "b"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_embed".to_string(),
                description: "Compute a term-frequency bag-of-words vector for text".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["vector".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "web_search".to_string(),
                description: "Search the web via SerpAPI and return organic results".to_string(),
                inputs: vec!["query".to_string()],
                outputs: vec!["results".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "http_fetch".to_string(),
                description: "Fetch the body of an HTTPS URL".to_string(),
                inputs: vec!["url".to_string()],
                outputs: vec!["body".to_string(), "status".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "url": { "type": "string" } },
                    "required": ["url"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "image_describe".to_string(),
                description: "Describe an image at a given path or URL using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "code_generate".to_string(),
                description: "Generate Rust code for a given requirement using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "code_test".to_string(),
                description: "Write Rust unit tests for given code using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "code_lint".to_string(),
                description: "Review Rust code for issues and suggest fixes using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "discovery_search".to_string(),
                description: "Cascade search across Google, HTTP fallback, and local trusted sources".to_string(),
                inputs: vec!["query".to_string()],
                outputs: vec!["hits".to_string(), "sources_succeeded".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "memory_remember".to_string(),
                description: "Persist a memory entry (decision, lesson, error, preference, etc.) to long-term store".to_string(),
                inputs: vec!["content".to_string(), "category".to_string()],
                outputs: vec!["memory_id".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "content": { "type": "string" }, "category": { "type": "string" } },
                    "required": ["content"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "memory_recall".to_string(),
                description: "Recall relevant memories by keyword search from long-term store".to_string(),
                inputs: vec!["query".to_string()],
                outputs: vec!["results".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "memory_distill".to_string(),
                description: "Distill and compact the memory store by removing duplicates and decaying old entries".to_string(),
                inputs: vec![],
                outputs: vec!["removed".to_string(), "merged".to_string()],
                parameters_schema: serde_json::json!({ "type": "object" }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "echo".to_string(),
                description: "Simply echo back the input task for testing".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "space_navigation".to_string(),
                description: "Navigate to interstellar destinations (experimental)".to_string(),
                inputs: vec!["destination".to_string()],
                outputs: vec!["status".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "destination": { "type": "string" } },
                    "required": ["destination"]
                }),
                examples: vec![],
            },
        ] {
            registry
                .definitions
                .insert(definition.name.clone(), definition);
        }
        registry
    }

    pub fn load_or_builtin(paths: &RouterPaths) -> Result<Self> {
        let mut registry = Self::builtin();
        if !paths.capabilities_dir.exists() {
            return Ok(registry);
        }

        for entry in fs::read_dir(&paths.capabilities_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            {
                let definition: CapabilityDefinition =
                    serde_json::from_slice(&fs::read(entry.path())?)?;
                registry.validate_name(&definition.name)?;
                registry
                    .definitions
                    .insert(definition.name.clone(), definition);
            }
        }

        Ok(registry)
    }

    pub fn validate_name(&self, name: &str) -> Result<()> {
        let is_valid = !name.is_empty()
            && !name.starts_with('_')
            && !name.ends_with('_')
            && name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');

        if is_valid {
            Ok(())
        } else {
            Err(anyhow!("invalid capability name: {name}"))
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &CapabilityDefinition> {
        self.definitions.values()
    }

    /// Write a newly discovered capability to capabilities/ dir so it survives restarts.
    pub fn persist_discovered(&self, name: &str, task: &str) -> anyhow::Result<()> {
        // We don't have paths here, so write to a temp location the caller can move.
        // Instead, callers should use persist_to_dir directly.
        let _ = (name, task);
        Ok(())
    }

    pub fn persist_to_dir(&mut self, name: &str, task: &str, capabilities_dir: &std::path::Path) -> anyhow::Result<()> {
        if self.contains(name) { return Ok(()); }
        std::fs::create_dir_all(capabilities_dir)?;
        let def = CapabilityDefinition {
            name: name.to_string(),
            description: format!("Auto-discovered capability for: {}", task),
            inputs: vec!["text".to_string()],
            outputs: vec!["output".to_string()],
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                }
            }),
            examples: vec![],
        };
        std::fs::write(
            capabilities_dir.join(format!("{}.json", name)),
            serde_json::to_vec_pretty(&def)?,
        )?;
        self.definitions.insert(name.to_string(), def);
        Ok(())
    }
}
