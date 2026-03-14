mod memcube;
mod multicube;
mod skill_memory;

use anyhow::{Context, Result};
use memcube::MemoryEntry;
use multicube::MultiCubeManager;
use skill_memory::{SkillMemoryStore, SkillUsageRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!(
            "Usage: memory_manager <command> [args...]\n\nCommands:\n  save <content> [tags...] [key=value...]       - Save memory to default cube\n  save-cube <cube_id> <content> [tags...]       - Save memory to specific cube\n  load                                          - Load and display memories from default cube\n  load-cube <cube_id>                           - Load memories from specific cube\n  search <keyword>                              - Search memories in default cube\n  search-cube <cube_id> <keyword>               - Search in specific cube\n  search-global <keyword>                       - Search across all cubes\n  create-cube <cube_id> <cube_name>             - Create new memory cube\n  delete-cube <cube_id>                         - Delete a memory cube\n  list-cubes                                    - List all memory cubes\n  set-shared <cube_id> <true|false>             - Set cube as shared\n  clear                                         - Clear all memories in default cube\n  clear-cube <cube_id>                          - Clear all memories in specific cube\n  summary                                       - Show memory summary\n  enable-long-memory                            - Enable long memory mode\n  disable-long-memory                           - Disable long memory mode\n  long-memory-status                            - Show long memory status\n  # ===== 技能记忆命令 =====\n  record-skill-usage <skill_id> <input> <output> <success> <time_ms> [tags...] - Record skill usage\n  get-skill-stats <skill_id>                    - Get skill statistics\n  search-skill-usage <skill_id>                 - Search skill usage by skill ID\n  search-skill-tag <tag>                        - Search skill usage by tag\n  search-skill-keyword <keyword>                - Search skill usage by keyword\n  help                                          - Show this help message"
        );
        return Ok(());
    }

    let command = &args[1];

    match command.as_str() {
        // ===== 传统命令（向后兼容） =====
        "enable-long-memory" => {
            let mut storage = load_legacy_storage("MEMORY/memory.json")?;
            storage.long_memory_enabled = true;
            save_legacy_storage(&storage, "MEMORY/memory.json")?;
            let output =
                format!(r#"{{\"status\":\"success\",\"enabled\":true,\"message\":\"Long memory enabled\"}}"#);
            println!("{}", output);
        }

        "disable-long-memory" => {
            let mut storage = load_legacy_storage("MEMORY/memory.json")?;
            storage.long_memory_enabled = false;
            save_legacy_storage(&storage, "MEMORY/memory.json")?;
            let output = format!(
                r#"{{\"status\":\"success\",\"enabled\":false,\"message\":\"Long memory disabled\"}}"#
            );
            println!("{}", output);
        }

        "long-memory-status" => {
            let storage = load_legacy_storage("MEMORY/memory.json")?;
            let status = if storage.long_memory_enabled {
                "enabled"
            } else {
                "disabled"
            };
            let output = format!(
                r#"{{\"status\":\"success\",\"enabled\":{},\"message\":\"Long memory is {}\"}}"#,
                storage.long_memory_enabled, status
            );
            println!("{}", output);
        }

        "save" => {
            if args.len() < 3 {
                eprintln!("Error: content is required");
                return Ok(());
            }

            // content 从 args[2] 开始，但实际上应该是 args[2]
            let content = &args[2];
            let args_after_content: Vec<String> = if args.len() > 3 { args[3..].to_vec() } else { vec![] };

            let mut storage = load_legacy_storage("MEMORY/memory.json")?;

            let (tags, metadata) = parse_args(&args_after_content);

            storage.add_entry(&content, &tags, metadata);
            save_legacy_storage(&storage, "MEMORY/memory.json")?;

            let id = storage.entries.last().unwrap().id.clone();
            let output = format!(
                r#"{{\"status\":\"success\",\"message\":\"Memory saved\",\"id\":\"{}\"}}"#,
                id
            );
            println!("{}", output);
        }

        "load" => {
            let storage = load_legacy_storage("MEMORY/memory.json")?;
            let output = format!(
                r#"{{\"status\":\"success\",\"count\":{},\"summary\":\"{}\"}}"#,
                storage.entries.len(),
                escape_json_string(&storage.summary)
            );
            println!("{}", output);

            for entry in &storage.entries {
                let output = format!(
                    r#"{{\"id\":\"{}\",\"timestamp\":\"{}\",\"content\":\"{}\",\"tags\":[{}]}}"#,
                    entry.id,
                    entry.timestamp,
                    escape_json_string(&entry.content),
                    entry.tags.join(", ")
                );
                println!("{}", output);
            }
        }

        "search" => {
            if args.len() < 3 {
                eprintln!("Error: keyword is required");
                return Ok(());
            }
            let keyword = &args[2];
            let storage = load_legacy_storage("MEMORY/memory.json")?;
            let results = storage.search_by_keyword(keyword);

            let output = format!(
                r#"{{\"status\":\"success\",\"found\":{},\"keyword\":\"{}\"}}"#,
                results.len(),
                keyword
            );
            println!("{}", output);
            for entry in &results {
                let output = format!(
                    r#"{{\"id\":\"{}\",\"content\":\"{}\",\"score\":100}}"#,
                    entry.id,
                    escape_json_string(&entry.content)
                );
                println!("{}", output);
            }
        }

        "search-tag" => {
            if args.len() < 3 {
                eprintln!("Error: tag is required");
                return Ok(());
            }
            let tag = &args[2];
            let storage = load_legacy_storage("MEMORY/memory.json")?;
            let results = storage.search_by_tag(tag);

            let output = format!(
                r#"{{\"status\":\"success\",\"found\":{},\"tag\":\"{}\"}}"#,
                results.len(),
                tag
            );
            println!("{}", output);
            for entry in &results {
                let output = format!(
                    r#"{{\"id\":\"{}\",\"content\":\"{}\"}}"#,
                    entry.id,
                    escape_json_string(&entry.content)
                );
                println!("{}", output);
            }
        }

        "clear" => {
            let mut storage = load_legacy_storage("MEMORY/memory.json")?;
            let count = storage.entries.len();
            storage.entries.clear();
            storage.summary = "No memories".to_string();
            save_legacy_storage(&storage, "MEMORY/memory.json")?;
            let output = format!(
                r#"{{\"status\":\"success\",\"cleared\":{},\"message\":\"All memories cleared\"}}"#,
                count
            );
            println!("{}", output);
        }

        "summary" => {
            let storage = load_legacy_storage("MEMORY/memory.json")?;
            let output = format!(
                r#"{{\"status\":\"success\",\"count\":{},\"summary\":\"{}\"}}"#,
                storage.entries.len(),
                escape_json_string(&storage.summary)
            );
            println!("{}", output);
        }

        // ===== 新增立方体命令 =====
        "save-cube" => {
            if args.len() < 4 {
                eprintln!("Error: cube_id and content are required");
                return Ok(());
            }

            let cube_id = &args[2];
            let content = &args[3];

            let mut manager =
                MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());
            let (tags, _metadata) = if args.len() > 4 {
                // 解析 tags，从 args[4] 开始
                let args_for_tags: Vec<String> = args[4..].to_vec();
                parse_args(&args_for_tags)
            } else {
                // 没有 tags
                (Vec::new(), HashMap::new())
            };

            let entry = MemoryEntry::new(
                &format!(
                    "mem_{}_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
                    manager.cubes.len()
                ),
                &content,
                tags,
            );
            manager.save_memory(cube_id, entry)?;
            manager.save_all()?;

            let output = format!(
                r#"{{\"status\":\"success\",\"message\":\"Memory saved to cube {}\",\"cube_id\":\"{}\"}}"#,
                cube_id, cube_id
            );
            println!("{}", output);
        }

        "load-cube" => {
            if args.len() < 3 {
                eprintln!("Error: cube_id is required");
                return Ok(());
            }

            let cube_id = &args[2];
            let manager = MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            if let Some(cube) = manager.cubes.get(cube_id) {
                let output = format!(
                    r#"{{\"status\":\"success\",\"cube_id\":\"{}\",\"count\":{},\"cube_name\":\"{}\"}}"#,
                    cube_id,
                    cube.memories.len(),
                    cube.cube_name
                );
                println!("{}", output);

                for entry in &cube.memories {
                    let output = format!(
                        r#"{{\"id\":\"{}\",\"timestamp\":\"{}\",\"content\":\"{}\",\"tags\":[{}]}}"#,
                        entry.id,
                        entry.timestamp,
                        escape_json_string(&entry.content),
                        entry.tags.join(", ")
                    );
                    println!("{}", output);
                }
            } else {
                let output = format!(
                    r#"{{\"status\":\"error\",\"message\":\"Cube not found: {}\"}}"#,
                    cube_id
                );
                println!("{}", output);
            }
        }

        "search-cube" => {
            if args.len() < 4 {
                eprintln!("Error: cube_id and keyword are required");
                return Ok(());
            }

            let cube_id = &args[2];
            let keyword = &args[3];
            let manager = MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            if let Some(cube) = manager.cubes.get(cube_id) {
                let results = cube.retrieve(keyword, 10);
                let output = format!(
                    r#"{{\"status\":\"success\",\"cube_id\":\"{}\",\"found\":{},\"keyword\":\"{}\"}}"#,
                    cube_id,
                    results.len(),
                    keyword
                );
                println!("{}", output);
                for entry in &results {
                    let output = format!(
                        r#"{{\"id\":\"{}\",\"content\":\"{}\",\"score\":100}}"#,
                        entry.id,
                        escape_json_string(&entry.content)
                    );
                    println!("{}", output);
                }
            } else {
                let output = format!(
                    r#"{{\"status\":\"error\",\"message\":\"Cube not found: {}\"}}"#,
                    cube_id
                );
                println!("{}", output);
            }
        }

        "search-global" => {
            if args.len() < 3 {
                eprintln!("Error: keyword is required");
                return Ok(());
            }
            let keyword = &args[2];
            let manager = MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            let results = manager.global_retrieve(keyword, 10);
            let total: usize = results.iter().map(|(_, r)| r.len()).sum();
            let output = format!(
                r#"{{\"status\":\"success\",\"found_total\":{},\"keyword\":\"{}\",\"cubes_searched\":{}}}"#,
                total,
                keyword,
                results.len()
            );
            println!("{}", output);

            for (cube_id, cube_results) in &results {
                for entry in cube_results {
                    let output = format!(
                        r#"{{\"cube_id\":\"{}\",\"id\":\"{}\",\"content\":\"{}\",\"score\":100}}"#,
                        cube_id,
                        entry.id,
                        escape_json_string(&entry.content)
                    );
                    println!("{}", output);
                }
            }
        }

        "create-cube" => {
            if args.len() < 4 {
                eprintln!("Error: cube_id and cube_name are required");
                return Ok(());
            }

            let cube_id = &args[2];
            let cube_name = &args[3];
            let mut manager =
                MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            manager.create_cube(cube_id, cube_name, Default::default(), Default::default());

            manager.save_all()?;

            let output = format!(
                r#"{{\"status\":\"success\",\"message\":\"Cube created\",\"cube_id\":\"{}\",\"cube_name\":\"{}\"}}"#,
                cube_id, cube_name
            );
            println!("{}", output);
        }

        "delete-cube" => {
            if args.len() < 3 {
                eprintln!("Error: cube_id is required");
                return Ok(());
            }

            let cube_id = &args[2];
            let mut manager =
                MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            if manager.delete_cube(cube_id) {
                manager.save_all()?;
                let output = format!(
                    r#"{{\"status\":\"success\",\"message\":\"Cube deleted\",\"cube_id\":\"{}\"}}"#,
                    cube_id
                );
                println!("{}", output);
            } else {
                let output = format!(
                    r#"{{\"status\":\"error\",\"message\":\"Cube not found: {}\"}}"#,
                    cube_id
                );
                println!("{}", output);
            }
        }

        "list-cubes" => {
            let manager = MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            let output = format!(r#"{{\"status\":\"success\",\"count\":{}}}"#, manager.cubes.len());
            println!("{}", output);

            for (cube_id, cube) in &manager.cubes {
                let shared = if cube.metadata.is_shared {
                    "shared"
                } else {
                    "private"
                };
                let output = format!(
                    r#"{{\"cube_id\":\"{}\",\"cube_name\":\"{}\",\"memories\":{},\"status\":\"{}\"}}"#,
                    cube_id,
                    cube.cube_name,
                    cube.memories.len(),
                    shared
                );
                println!("{}", output);
            }
        }

        "set-shared" => {
            if args.len() < 4 {
                eprintln!("Error: cube_id and shared (true|false) are required");
                return Ok(());
            }

            let cube_id = &args[2];
            let shared = args[3].parse::<bool>().unwrap_or(false);
            let mut manager =
                MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            manager.set_shared(cube_id, shared);
            manager.save_all()?;

            let status = if shared { "shared" } else { "private" };
            let output = format!(
                r#"{{\"status\":\"success\",\"message\":\"Cube set to {}\",\"cube_id\":\"{}\"}}"#,
                status, cube_id
            );
            println!("{}", output);
        }

        "clear-cube" => {
            if args.len() < 3 {
                eprintln!("Error: cube_id is required");
                return Ok(());
            }

            let cube_id = &args[2];
            let mut manager =
                MultiCubeManager::load_all().unwrap_or_else(|_| MultiCubeManager::new());

            if let Some(cube) = manager.cubes.get_mut(cube_id) {
                let count = cube.memories.len();
                cube.memories.clear();
                cube.search_index.clear();
                manager.save_all()?;
                let output = format!(
                    r#"{{\"status\":\"success\",\"cleared\":{},\"cube_id\":\"{}\"}}"#,
                    count, cube_id
                );
                println!("{}", output);
            } else {
                let output = format!(
                    r#"{{\"status\":\"error\",\"message\":\"Cube not found: {}\"}}"#,
                    cube_id
                );
                println!("{}", output);
            }
        }

        // ===== 技能记忆命令 =====
        "record-skill-usage" => {
            if args.len() < 6 {
                eprintln!("Error: skill_id, input, output, success, and time_ms are required");
                return Ok(());
            }

            let skill_id = &args[2];
            let input = &args[3];
            let output = &args[4];
            let success = args[5].parse::<bool>().unwrap_or(false);
            let time_ms = args[6].parse::<u64>().unwrap_or(0);
            
            let tags = if args.len() > 7 {
                let tag_args: Vec<String> = args[7..].to_vec();
                let (parsed_tags, _) = parse_args(&tag_args);
                parsed_tags
            } else {
                Vec::new()
            };

            let mut store = SkillMemoryStore::load("MEMORY/skill_memory.json")?;
            let record = SkillUsageRecord::new(skill_id, input, output, success, time_ms, tags);
            store.add_record(record);
            store.save("MEMORY/skill_memory.json")?;

            let output = format!(
                r#"{{\"status\":\"success\",\"message\":\"Skill usage recorded\",\"skill_id\":\"{}\"}}"#,
                skill_id
            );
            println!("{}", output);
        }

        "get-skill-stats" => {
            if args.len() < 3 {
                eprintln!("Error: skill_id is required");
                return Ok(());
            }

            let skill_id = &args[2];
            let store = SkillMemoryStore::load("MEMORY/skill_memory.json")?;

            if let Some(stats) = store.get_skill_stats(skill_id) {
                let output = format!(
                    r#"{{\"status\":\"success\",\"skill_id\":\"{}\",\"total_calls\":{},\"success_calls\":{},\"avg_latency_ms\":{:.2}}}"#,
                    skill_id,
                    stats.total_calls,
                    stats.success_calls,
                    stats.avg_latency_ms
                );
                println!("{}", output);
            } else {
                let output = format!(
                    r#"{{\"status\":\"error\",\"message\":\"No stats found for skill: {}\"}}"#,
                    skill_id
                );
                println!("{}", output);
            }
        }

        "search-skill-usage" => {
            if args.len() < 3 {
                eprintln!("Error: skill_id is required");
                return Ok(());
            }

            let skill_id = &args[2];
            let store = SkillMemoryStore::load("MEMORY/skill_memory.json")?;
            let results = store.search_by_skill(skill_id);

            let output = format!(
                r#"{{\"status\":\"success\",\"skill_id\":\"{}\",\"found\":{}}}"#,
                skill_id,
                results.len()
            );
            println!("{}", output);

            for record in &results {
                let output = format!(
                    r#"{{\"skill_id\":\"{}\",\"timestamp\":\"{}\",\"input\":\"{}\",\"output\":\"{}\",\"success\":{},\"execution_time_ms\":{}}}"#,
                    record.skill_id,
                    record.timestamp,
                    escape_json_string(&record.input),
                    escape_json_string(&record.output),
                    record.success,
                    record.execution_time_ms
                );
                println!("{}", output);
            }
        }

        "search-skill-tag" => {
            if args.len() < 3 {
                eprintln!("Error: tag is required");
                return Ok(());
            }

            let tag = &args[2];
            let store = SkillMemoryStore::load("MEMORY/skill_memory.json")?;
            let results = store.search_by_tag(tag);

            let output = format!(
                r#"{{\"status\":\"success\",\"tag\":\"{}\",\"found\":{}}}"#,
                tag,
                results.len()
            );
            println!("{}", output);

            for record in &results {
                let output = format!(
                    r#"{{\"skill_id\":\"{}\",\"timestamp\":\"{}\",\"input\":\"{}\",\"success\":{}}}"#,
                    record.skill_id,
                    record.timestamp,
                    escape_json_string(&record.input),
                    record.success
                );
                println!("{}", output);
            }
        }

        "search-skill-keyword" => {
            if args.len() < 3 {
                eprintln!("Error: keyword is required");
                return Ok(());
            }

            let keyword = &args[2];
            let store = SkillMemoryStore::load("MEMORY/skill_memory.json")?;
            let results = store.search_by_keyword(keyword);

            let output = format!(
                r#"{{\"status\":\"success\",\"keyword\":\"{}\",\"found\":{}}}"#,
                keyword,
                results.len()
            );
            println!("{}", output);

            for record in &results {
                let output = format!(
                    r#"{{\"skill_id\":\"{}\",\"timestamp\":\"{}\",\"input\":\"{}\",\"success\":{}}}"#,
                    record.skill_id,
                    record.timestamp,
                    escape_json_string(&record.input),
                    record.success
                );
                println!("{}", output);
            }
        }

        "help" | _ => {
            println!(
                "Usage: memory_manager <command> [args...]\n\nCommands:\n  save <content> [tags...] [key=value...]       - Save memory to default cube\n  save-cube <cube_id> <content> [tags...]       - Save memory to specific cube\n  load                                          - Load and display memories from default cube\n  load-cube <cube_id>                           - Load memories from specific cube\n  search <keyword>                              - Search memories in default cube\n  search-cube <cube_id> <keyword>               - Search in specific cube\n  search-global <keyword>                       - Search across all cubes\n  create-cube <cube_id> <cube_name>             - Create new memory cube\n  delete-cube <cube_id>                         - Delete a memory cube\n  list-cubes                                    - List all memory cubes\n  set-shared <cube_id> <true|false>             - Set cube as shared\n  clear                                         - Clear all memories in default cube\n  clear-cube <cube_id>                          - Clear all memories in specific cube\n  summary                                       - Show memory summary\n  enable-long-memory                            - Enable long memory mode\n  disable-long-memory                           - Disable long memory mode\n  long-memory-status                            - Show long memory status\n  # ===== 技能记忆命令 =====\n  record-skill-usage <skill_id> <input> <output> <success> <time_ms> [tags...] - Record skill usage\n  get-skill-stats <skill_id>                    - Get skill statistics\n  search-skill-usage <skill_id>                 - Search skill usage by skill ID\n  search-skill-tag <tag>                        - Search skill usage by tag\n  search-skill-keyword <keyword>                - Search skill usage by keyword\n  help                                          - Show this help message"
            );
        }
    }

    Ok(())
}

// ===== 传统存储兼容函数 =====
#[derive(Debug, Serialize, Deserialize)]
struct LegacyMemoryStorage {
    entries: Vec<memcube::MemoryEntry>,
    summary: String,
    last_updated: String,
    #[serde(default)]
    long_memory_enabled: bool,
}

impl LegacyMemoryStorage {
    fn new() -> Self {
        LegacyMemoryStorage {
            entries: Vec::new(),
            summary: String::new(),
            last_updated: chrono::Utc::now().to_rfc3339(),
            long_memory_enabled: true,
        }
    }

    fn add_entry(&mut self, content: &str, tags: &[String], metadata: HashMap<String, String>) {
        let entry = memcube::MemoryEntry::new(
            &format!(
                "mem_{}_{}",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
                self.entries.len()
            ),
            content,
            tags.to_vec(),
        );
        self.entries.push(entry);
        self.last_updated = chrono::Utc::now().to_rfc3339();
        self.update_summary();
    }

    fn update_summary(&mut self) {
        if self.entries.is_empty() {
            self.summary = "No memories".to_string();
            return;
        }

        let recent_count = self.entries.len().min(3);
        let recent_entries: Vec<&memcube::MemoryEntry> =
            self.entries.iter().rev().take(recent_count).collect();

        let joined: String = recent_entries
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<&str>>()
            .join("; ");

        self.summary = format!(
            "Total {} memories, recent {} : {}",
            self.entries.len(),
            recent_count,
            joined
        );
    }

    fn search_by_tag(&self, tag: &str) -> Vec<&memcube::MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    fn search_by_keyword(&self, keyword: &str) -> Vec<&memcube::MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.content.contains(keyword))
            .collect()
    }
}

fn load_legacy_storage(path: &str) -> Result<LegacyMemoryStorage> {
    if Path::new(path).exists() {
        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content).context("Failed to parse memory storage")
    } else {
        Ok(LegacyMemoryStorage::new())
    }
}

fn save_legacy_storage(storage: &LegacyMemoryStorage, path: &str) -> Result<()> {
    let content = serde_json::to_string_pretty(storage)?;
    fs::write(path, content).context("Failed to write memory storage")
}

// ===== 参数解析 =====
fn parse_args(args: &[String]) -> (Vec<String>, HashMap<String, String>) {
    let mut tags: Vec<String> = Vec::new();
    let mut metadata: HashMap<String, String> = HashMap::new();

    for arg in args {
        if arg.starts_with('#') {
            tags.push(arg[1..].to_string());
        } else if arg.contains('=') {
            let parts: Vec<&str> = arg.splitn(2, '=').collect();
            if parts.len() == 2 {
                metadata.insert(parts[0].to_string(), parts[1].to_string());
            }
        }
    }

    (tags, metadata)
}