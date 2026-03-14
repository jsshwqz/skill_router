#!/usr/bin/env -S cargo run
//! Context Manager Skill (Rust Implementation)
//! 
//! Features:
//! 1. Read project file status
//! 2. Generate context summaries
//! 3. Update CONTEXT.md
//! 4. Scan project structure
//! 5. Multi-context cube management (MemOS inspired)

mod context_cube;
mod multicontext;

use anyhow::Result;
use multicontext::MultiContextManager;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct ContextInput {
    #[serde(default)]
    action: String, // "read", "update", "summarize", "scan", "create-context", "load-context", "save-file", "search-context", "list-contexts"
    #[serde(default)]
    context_id: String,
    #[serde(default)]
    file_path: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    max_summary_length: usize,
    #[serde(default)]
    keyword: String,
}

#[derive(Debug, Serialize)]
struct ContextOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files_scanned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contexts: Option<Vec<ContextInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_results: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

#[derive(Debug, Serialize)]
struct ContextInfo {
    context_id: String,
    context_name: String,
    file_count: usize,
    is_shared: bool,
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // Use default action for testing
        process_context("scan", "", "", "", "", 100, "", start_time)
    } else if args.len() == 2 {
        // Second argument is action
        let action = &args[1];
        process_context(action, "", "", "", "", 100, "", start_time)
    } else {
        // Parse JSON input
        match serde_json::from_str::<ContextInput>(&args[1]) {
            Ok(input) => process_context(
                &input.action,
                &input.context_id,
                &input.file_path,
                &input.content,
                &input.keyword,
                input.max_summary_length,
                "",
                start_time,
            ),
            Err(e) => ContextOutput {
                status: "error".to_string(),
                skill: "context_manager".to_string(),
                content: None,
                summary: None,
                files_scanned: None,
                updated_file: None,
                context_id: None,
                contexts: None,
                search_results: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_context(
    action: &str,
    context_id: &str,
    file_path: &str,
    content: &str,
    keyword: &str,
    max_length: usize,
    _dir_path: &str,
    start_time: std::time::Instant,
) -> ContextOutput {
    match action {
        "read" => read_file(file_path, start_time),
        "update" => update_context_md(file_path, start_time),
        "summarize" => generate_summary(file_path, max_length, start_time),
        "scan" => scan_project(start_time),
        "create-context" => create_context(context_id, file_path, start_time), // file_path used as context_name
        "load-context" => load_context(context_id, start_time),
        "save-file" => save_file_to_context(context_id, file_path, content, start_time),
        "search-context" => search_context(context_id, keyword, start_time),
        "list-contexts" => list_contexts(start_time),
        "scan-context" => scan_context_dir(context_id, file_path, start_time), // file_path used as dir_path
        _ => ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Unknown action: {}", action)),
            duration_ms: 0,
        },
    }
}

fn read_file(file_path: &str, start_time: std::time::Instant) -> ContextOutput {
    let path = Path::new(file_path);
    
    if !path.exists() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("File not found: {}", file_path)),
            duration_ms: 0,
        };
    }

    match fs::read_to_string(file_path) {
        Ok(content) => {
            let duration = start_time.elapsed().as_millis() as u64;
            ContextOutput {
                status: "success".to_string(),
                skill: "context_manager".to_string(),
                content: Some(content),
                summary: None,
                files_scanned: None,
                updated_file: None,
                context_id: None,
                contexts: None,
                search_results: None,
                error: None,
                duration_ms: duration,
            }
        }
        Err(e) => ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Read error: {}", e)),
            duration_ms: 0,
        },
    }
}

fn update_context_md(file_path: &str, start_time: std::time::Instant) -> ContextOutput {
    let context_md_path = "CONTEXT.md";
    
    // If file_path is provided, use it as update content
    let update_content = if !file_path.is_empty() {
        format!("\n\n=== Updated at {} ===\n{}\n---", 
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                file_path)
    } else {
        String::new()
    };

    // Check if CONTEXT.md exists
    let content = if Path::new(context_md_path).exists() {
        let mut content = fs::read_to_string(context_md_path).unwrap_or_default();
        content.push_str(&update_content);
        content
    } else {
        format!("# Project Context\n\n## Last Updated: {}\n\n## Status\n\n- Project: Skill Router\n- Version: 0.2.0{}\n", 
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                update_content)
    };

    match fs::write(context_md_path, &content) {
        Ok(_) => {
            let duration = start_time.elapsed().as_millis() as u64;
            ContextOutput {
                status: "success".to_string(),
                skill: "context_manager".to_string(),
                content: None,
                summary: None,
                files_scanned: None,
                updated_file: Some(context_md_path.to_string()),
                context_id: None,
                contexts: None,
                search_results: None,
                error: None,
                duration_ms: duration,
            }
        }
        Err(e) => ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Write error: {}", e)),
            duration_ms: 0,
        },
    }
}

fn generate_summary(file_path: &str, max_length: usize, start_time: std::time::Instant) -> ContextOutput {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return ContextOutput {
                status: "error".to_string(),
                skill: "context_manager".to_string(),
                content: None,
                summary: None,
                files_scanned: None,
                updated_file: None,
                context_id: None,
                contexts: None,
                search_results: None,
                error: Some(format!("Read error: {}", e)),
                duration_ms: 0,
            };
        }
    };

    let original_length = content.len();
    
    // Generate summary (simplified: truncate + key info extraction)
    let summary = if original_length <= max_length {
        Some(content.clone())
    } else {
        // Extract first N lines and last N lines
        let lines: Vec<&str> = content.lines().collect();
        let mid = lines.len() / 2;
        let excerpt: Vec<String> = lines[..mid.min(10)]
            .iter()
            .chain(&lines[mid.min(mid)..(mid + 10).min(lines.len())])
            .map(|s| s.to_string())
            .collect();
        
        Some(format!(
            "Summary (original: {} chars, summary: {} chars)\n\n{}",
            original_length,
            excerpt.join("\n").len(),
            excerpt.join("\n")
        ))
    };

    let duration = start_time.elapsed().as_millis() as u64;
    
    ContextOutput {
        status: "success".to_string(),
        skill: "context_manager".to_string(),
        content: Some(content),
        summary,
        files_scanned: None,
        updated_file: None,
        context_id: None,
        contexts: None,
        search_results: None,
        error: None,
        duration_ms: duration,
    }
}

fn scan_project(start_time: std::time::Instant) -> ContextOutput {
    let mut files_scanned = 0;
    let mut project_files = Vec::new();
    
    // Scan key files
    let key_files = vec![
        "README.md",
        "CHANGELOG.md",
        "CONFIG_SUMMARY.md",
        "CONTEXT_SUMMARY.md",
        "PROJECT_SUMMARY.md",
    ];
    
    for file in key_files {
        if Path::new(file).exists() {
            files_scanned += 1;
            project_files.push(file.to_string());
        }
    }
    
    // Scan skills directory
    let skills_path = Path::new("skills");
    if skills_path.exists() {
        if let Ok(entries) = fs::read_dir(skills_path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    files_scanned += 1;
                    if let Some(name) = entry.file_name().to_str() {
                        project_files.push(format!("skills/{}", name));
                    }
                }
            }
        }
    }

    // Generate project scan summary
    let summary = format!(
        "# Project Scan Summary\n\n## Scanned At: {}\n## Files Scanned: {}\n\n## Key Files Found:\n{}\n\n## Skills Directory:\n{}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
        files_scanned,
        project_files.iter()
            .filter(|s| !s.starts_with("skills/"))
            .map(|s| format!("- {}", s))
            .collect::<Vec<_>>()
            .join("\n"),
        project_files.iter()
            .filter(|s| s.starts_with("skills/"))
            .map(|s| format!("- {}", s))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let duration = start_time.elapsed().as_millis() as u64;
    
    ContextOutput {
        status: "success".to_string(),
        skill: "context_manager".to_string(),
        content: Some(summary.clone()),
        summary: Some(summary),
        files_scanned: Some(files_scanned),
        updated_file: None,
        context_id: None,
        contexts: None,
        search_results: None,
        error: None,
        duration_ms: duration,
    }
}

// ===== 新增的多上下文功能 =====

fn create_context(context_id: &str, context_name: &str, start_time: std::time::Instant) -> ContextOutput {
    if context_id.is_empty() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some("context_id is required".to_string()),
            duration_ms: 0,
        };
    }

    let context_name = if context_name.is_empty() { context_id } else { context_name };

    let mut manager = MultiContextManager::load_all().unwrap_or_else(|_| MultiContextManager::new());
    if let Err(e) = manager.create_context(context_id, context_name) {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Create context error: {}", e)),
            duration_ms: 0,
        };
    }

    if let Err(e) = manager.save_all() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Save context error: {}", e)),
            duration_ms: 0,
        };
    }

    let duration = start_time.elapsed().as_millis() as u64;
    ContextOutput {
        status: "success".to_string(),
        skill: "context_manager".to_string(),
        content: None,
        summary: None,
        files_scanned: None,
        updated_file: None,
        context_id: Some(context_id.to_string()),
        contexts: None,
        search_results: None,
        error: None,
        duration_ms: duration,
    }
}

fn load_context(context_id: &str, start_time: std::time::Instant) -> ContextOutput {
    if context_id.is_empty() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some("context_id is required".to_string()),
            duration_ms: 0,
        };
    }

    let manager = MultiContextManager::load_all().unwrap_or_else(|_| MultiContextManager::new());
    match manager.get_context_summary(context_id) {
        Ok(summary) => {
            let duration = start_time.elapsed().as_millis() as u64;
            ContextOutput {
                status: "success".to_string(),
                skill: "context_manager".to_string(),
                content: Some(summary.clone()),
                summary: Some(summary),
                files_scanned: None,
                updated_file: None,
                context_id: Some(context_id.to_string()),
                contexts: None,
                search_results: None,
                error: None,
                duration_ms: duration,
            }
        }
        Err(e) => ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Load context error: {}", e)),
            duration_ms: 0,
        },
    }
}

fn save_file_to_context(context_id: &str, file_path: &str, content: &str, start_time: std::time::Instant) -> ContextOutput {
    if context_id.is_empty() || file_path.is_empty() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some("context_id and file_path are required".to_string()),
            duration_ms: 0,
        };
    }

    let mut manager = MultiContextManager::load_all().unwrap_or_else(|_| MultiContextManager::new());
    if !manager.contexts.contains_key(context_id) {
        // Auto-create context if it doesn't exist
        if let Err(e) = manager.create_context(context_id, context_id) {
            return ContextOutput {
                status: "error".to_string(),
                skill: "context_manager".to_string(),
                content: None,
                summary: None,
                files_scanned: None,
                updated_file: None,
                context_id: None,
                contexts: None,
                search_results: None,
                error: Some(format!("Auto-create context error: {}", e)),
                duration_ms: 0,
            };
        }
    }

    if let Err(e) = manager.add_file_to_context(context_id, file_path, content) {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Save file error: {}", e)),
            duration_ms: 0,
        };
    }

    if let Err(e) = manager.save_all() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Save context error: {}", e)),
            duration_ms: 0,
        };
    }

    let duration = start_time.elapsed().as_millis() as u64;
    ContextOutput {
        status: "success".to_string(),
        skill: "context_manager".to_string(),
        content: None,
        summary: None,
        files_scanned: None,
        updated_file: Some(file_path.to_string()),
        context_id: Some(context_id.to_string()),
        contexts: None,
        search_results: None,
        error: None,
        duration_ms: duration,
    }
}

fn search_context(context_id: &str, keyword: &str, start_time: std::time::Instant) -> ContextOutput {
    if context_id.is_empty() || keyword.is_empty() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some("context_id and keyword are required".to_string()),
            duration_ms: 0,
        };
    }

    let manager = MultiContextManager::load_all().unwrap_or_else(|_| MultiContextManager::new());
    match manager.search_context(context_id, keyword) {
        Ok(results) => {
            let duration = start_time.elapsed().as_millis() as u64;
            ContextOutput {
                status: "success".to_string(),
                skill: "context_manager".to_string(),
                content: None,
                summary: None,
                files_scanned: None,
                updated_file: None,
                context_id: Some(context_id.to_string()),
                contexts: None,
                search_results: Some(results),
                error: None,
                duration_ms: duration,
            }
        }
        Err(e) => ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Search context error: {}", e)),
            duration_ms: 0,
        },
    }
}

fn list_contexts(start_time: std::time::Instant) -> ContextOutput {
    let manager = MultiContextManager::load_all().unwrap_or_else(|_| MultiContextManager::new());
    let contexts: Vec<ContextInfo> = manager
        .list_contexts()
        .iter()
        .map(|(id, context)| ContextInfo {
            context_id: id.to_string(),
            context_name: context.cube_name.clone(),
            file_count: context.metadata.file_count,
            is_shared: context.metadata.is_shared,
        })
        .collect();

    let duration = start_time.elapsed().as_millis() as u64;
    ContextOutput {
        status: "success".to_string(),
        skill: "context_manager".to_string(),
        content: None,
        summary: None,
        files_scanned: None,
        updated_file: None,
        context_id: None,
        contexts: Some(contexts),
        search_results: None,
        error: None,
        duration_ms: duration,
    }
}

fn scan_context_dir(context_id: &str, dir_path: &str, start_time: std::time::Instant) -> ContextOutput {
    if context_id.is_empty() || dir_path.is_empty() {
        return ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some("context_id and dir_path are required".to_string()),
            duration_ms: 0,
        };
    }

    let mut manager = MultiContextManager::load_all().unwrap_or_else(|_| MultiContextManager::new());
    if !manager.contexts.contains_key(context_id) {
        if let Err(e) = manager.create_context(context_id, context_id) {
            return ContextOutput {
                status: "error".to_string(),
                skill: "context_manager".to_string(),
                content: None,
                summary: None,
                files_scanned: None,
                updated_file: None,
                context_id: None,
                contexts: None,
                search_results: None,
                error: Some(format!("Auto-create context error: {}", e)),
                duration_ms: 0,
            };
        }
    }

    match manager.scan_context(context_id, dir_path) {
        Ok(files_scanned) => {
            if let Err(e) = manager.save_all() {
                return ContextOutput {
                    status: "error".to_string(),
                    skill: "context_manager".to_string(),
                    content: None,
                    summary: None,
                    files_scanned: None,
                    updated_file: None,
                    context_id: None,
                    contexts: None,
                    search_results: None,
                    error: Some(format!("Save context error: {}", e)),
                    duration_ms: 0,
                };
            }

            let duration = start_time.elapsed().as_millis() as u64;
            ContextOutput {
                status: "success".to_string(),
                skill: "context_manager".to_string(),
                content: None,
                summary: None,
                files_scanned: Some(files_scanned),
                updated_file: None,
                context_id: Some(context_id.to_string()),
                contexts: None,
                search_results: None,
                error: None,
                duration_ms: duration,
            }
        }
        Err(e) => ContextOutput {
            status: "error".to_string(),
            skill: "context_manager".to_string(),
            content: None,
            summary: None,
            files_scanned: None,
            updated_file: None,
            context_id: None,
            contexts: None,
            search_results: None,
            error: Some(format!("Scan context error: {}", e)),
            duration_ms: 0,
        },
    }
}