//! 技能格式转换 builtin：SKILL.md ↔ forge skill.json
//!
//! 解析社区标准 SKILL.md（YAML frontmatter + Markdown）并转换为 forge 可注册的技能。
//! 也支持反向：将 forge skill 导出为 SKILL.md 格式。

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

use aion_types::types::{ExecutionContext, PermissionSet, RouterPaths, SkillDefinition, SkillMetadata, SkillSource};

use super::BuiltinSkill;

/// SKILL.md frontmatter 字段
struct SkillMdFrontmatter {
    name: String,
    description: String,
    _license: Option<String>,
    compatibility: Option<String>,
    metadata: HashMap<String, String>,
}

/// 从文本中解析 SKILL.md frontmatter
fn parse_frontmatter(raw: &str) -> Result<(SkillMdFrontmatter, String)> {
    let trimmed = raw.trim();
    // 必须以 --- 开头
    if !trimmed.starts_with("---") {
        return Err(anyhow!("SKILL.md must start with `---` frontmatter"));
    }
    let after_first = &trimmed[3..];
    // 找到结束的 ---
    let end_idx = after_first
        .find("\n---")
        .ok_or_else(|| anyhow!("SKILL.md frontmatter: missing closing `---`"))?;
    let yaml_block = &after_first[..end_idx];
    let body = after_first[end_idx + 4..].trim().to_string();

    // 手动解析简单 YAML key-value
    let mut name = String::new();
    let mut description = String::new();
    let mut license = None;
    let mut compatibility = None;
    let mut metadata = HashMap::new();
    let mut in_metadata = false;

    for line in yaml_block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "metadata:" {
            in_metadata = true;
            continue;
        }
        if in_metadata {
            if trimmed.starts_with('-') || !trimmed.contains(':') {
                in_metadata = false;
            } else if let Some((k, v)) = trimmed.split_once(':') {
                metadata.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
                continue;
            }
        }
        if in_metadata {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let val = v.trim().trim_matches('"').to_string();
            match k.trim() {
                "name" => name = val,
                "description" => description = val,
                "license" => license = Some(val),
                "compatibility" => compatibility = Some(val),
                _ => {}
            }
        }
    }

    if name.is_empty() {
        return Err(anyhow!("SKILL.md frontmatter: `name` is required"));
    }
    if description.is_empty() {
        return Err(anyhow!("SKILL.md frontmatter: `description` is required"));
    }

    Ok((
        SkillMdFrontmatter {
            name,
            description,
            _license: license,
            compatibility,
            metadata,
        },
        body,
    ))
}

/// 将 SKILL.md 内容转换为 forge SkillDefinition
fn skillmd_to_forge(raw: &str, root_dir: &Path) -> Result<SkillDefinition> {
    let (front, body) = parse_frontmatter(raw)?;

    // 判断是否需要网络（description 或 body 中有 http/https）
    let needs_network = front.description.contains("http")
        || front.compatibility.as_deref().unwrap_or("").contains("internet")
        || body.contains("http://")
        || body.contains("https://");

    let instruction = if body.is_empty() {
        Some(front.description.clone())
    } else {
        Some(format!("{}\n\n{}", front.description, body))
    };

    // 版本号从 metadata 中取，没有则默认
    let version = front
        .metadata
        .get("version")
        .cloned()
        .unwrap_or_else(|| "0.1.0".to_string());

    let mut permissions = PermissionSet::default_deny();
    if needs_network {
        permissions = permissions.with_network(true);
    }

    Ok(SkillDefinition {
        metadata: SkillMetadata {
            name: format!("{}_from_skillmd", front.name),
            version,
            capabilities: vec![front.name.clone()],
            entrypoint: "builtin:ai_task".to_string(),
            permissions,
            instruction,
            engine_capable: false,
        },
        root_dir: root_dir.join(&front.name),
        source: SkillSource::Generated,
    })
}

pub struct SkillConvert;

#[async_trait::async_trait]
impl BuiltinSkill for SkillConvert {
    fn name(&self) -> &'static str {
        "skill_convert"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        // 输入：source（SKILL.md 原始内容 或 文件路径）
        let source = ctx.context["source"]
            .as_str()
            .or_else(|| ctx.context["text"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();

        // 检测是文件路径还是原始内容
        let content: String = if Path::new(&source).exists() {
            std::fs::read_to_string(&source).map_err(|e| anyhow!("Failed to read SKILL.md file: {}", e))?
        } else {
            source.clone()
        };

        // 解析
        let workspace = std::env::current_dir().unwrap_or_default();
        let paths = RouterPaths::for_workspace(&workspace);

        match skillmd_to_forge(&content, &paths.generated_skills_dir) {
            Ok(def) => {
                // 写入 skill.json
                let skill_dir = def.root_dir.clone();
                std::fs::create_dir_all(&skill_dir)?;
                let mut map = serde_json::Map::new();
                map.insert("name".into(), json!(def.metadata.name));
                map.insert("version".into(), json!(def.metadata.version));
                map.insert("capabilities".into(), json!(def.metadata.capabilities));
                map.insert("entrypoint".into(), json!(def.metadata.entrypoint));
                map.insert("permissions".into(), json!(def.metadata.permissions));
                if let Some(ref instr) = def.metadata.instruction {
                    map.insert("instruction".into(), json!(instr));
                }
                std::fs::write(
                    skill_dir.join("skill.json"),
                    serde_json::to_vec_pretty(&serde_json::Value::Object(map))?,
                )?;

                Ok(json!({
                    "status": "converted",
                    "name": def.metadata.name,
                    "capability": def.metadata.capabilities[0],
                    "entrypoint": def.metadata.entrypoint,
                    "instruction_length": def.metadata.instruction.as_ref().map(|i| i.len()).unwrap_or(0),
                    "skill_path": skill_dir.to_string_lossy().to_string(),
                    "needs_network": def.metadata.permissions.network,
                }))
            }
            Err(e) => {
                // 尝试作为 forge skill.json 反向转换
                let skill_def = std::fs::read_to_string(&source).unwrap_or_else(|_| content.clone());
                if let Ok(json_val) = serde_json::from_str::<Value>(&skill_def) {
                    let name = json_val["name"].as_str().unwrap_or("unnamed");
                    let _capability = json_val["capabilities"]
                        .as_array()
                        .and_then(|a| a[0].as_str())
                        .unwrap_or(name);
                    let instruction = json_val["instruction"].as_str().unwrap_or("");
                    let description = json_val["description"].as_str().unwrap_or(instruction);

                    // 生成 SKILL.md
                    let skillmd = format!(
                        "---\nname: {}\ndescription: {}\n---\n\n{}",
                        name, description, instruction
                    );

                    return Ok(json!({
                        "status": "converted",
                        "format": "skill.json_to_skillmd",
                        "name": name,
                        "skillmd": skillmd,
                    }));
                }
                Err(e)
            }
        }
    }
}
