use std::fs;

use anyhow::Result;
use serde_json::json;

use aion_types::capability_registry::CapabilityRegistry;
use aion_types::types::{PermissionSet, RouterPaths, SkillDefinition, SkillMetadata, SkillSource};

/// Default AI instruction templates for known AI capabilities.
/// Fallback when the registry knows a capability requires AI but no custom instruction exists.
///
/// Each instruction follows prompt engineering best practices:
/// - Role assignment (你是谁)
/// - Task context (要做什么)
/// - Output constraints (输出格式/边界)
/// - Anti-hallucination (给退路)
// code_lint 和 code_test 已有专用 Rust builtin，不再需要 AI 模板
const DEFAULT_AI_INSTRUCTIONS: &[(&str, &str)] = &[
    ("code_generate", "你是一个 Rust 代码生成器。根据需求生成完整可编译的 Rust 代码。只返回代码本身，不要额外说明。如果需求不明确，在代码顶部加 // TODO: 注释标注不确定之处。"),
    ("text_summarize", "你是一个文本摘要工具。将输入文本压缩为 2-3 句摘要。保留关键事实和数据。如果你认为输入不是有效文本，仅输出 'UNSUMMARIZABLE'。"),
    ("text_translate", "你是一个翻译工具。如果输入是中文，翻译成英文；如果输入是英文，翻译成中文。保留原文格式（列表、换行）。如果输入是其他语言或无法识别，仅输出 'UNTRANSLATABLE'。"),
    ("text_classify", "你是一个文本分类器。将输入文本归入一个类别标签。只返回标签名称，不要任何额外输出。可选标签由调用方在 input 中指定。如果不确定，输出 'unknown'。"),
    ("text_extract", "你是一个信息提取器。从文本中提取关键实体（人名、组织名、专业术语、日期、数字）。以 JSON 数组格式返回：[{\"type\": \"person\", \"value\": \"...\"}, ...]。如果无实体可提取，返回空数组 []。"),
    ("image_describe", "你是一个图像描述工具。描述输入图像的内容，包括：主体对象、场景环境、文字内容（如有）、颜色和构图。以段落形式输出，不嵌套 JSON。如果无法访问图像，输出 'IMAGE_UNAVAILABLE'。"),
    ("pdf_parse", "你是一个 PDF 结构化提取工具。从 PDF 中提取文本并进行结构化组织：标题（如有）、各章节内容、表格数据（如有）。保留原始层级关系。如果 PDF 内容为空或不可读，输出 'PDF_UNREADABLE'。"),
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
                        .map(|def| format!("你是一个 {} 工具。{} 按规范格式返回结果。如果不确定，输出 'UNKNOWN'。", capability, def.description))
                        .unwrap_or_else(|| format!("Execute '{}' on the given input. Output only the result, no explanation.", capability))
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
