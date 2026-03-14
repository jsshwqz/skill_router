#!/usr/bin/env -S cargo run
//! Encoding Checker Skill (Rust Implementation)
//! 
//! Features:
//! 1. Check file encoding (UTF-8 vs GBK/ANSI)
//! 2. Scan project for encoding issues
//! 3. Generate encoding report
//! 4. Prevent GBK/ANSI encoding in source files

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct EncodingInput {
    #[serde(default)]
    action: String, // "check", "scan", "report"
    #[serde(default)]
    file_path: String,
    #[serde(default)]
    strict_mode: bool,
}

#[derive(Debug, Serialize)]
struct EncodingOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issues: Option<Vec<EncodingIssue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files_scanned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files_with_issues: Option<usize>,
    error: Option<String>,
    duration_ms: u64,
}

#[derive(Debug, Serialize)]
struct EncodingIssue {
    file: String,
    issue: String,
    severity: String, // "warning", "error"
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // Use default action for testing
        process_encoding("scan", "", false, start_time)
    } else {
        // Check if second argument is a valid action or JSON
        let action = &args[1];
        if action.starts_with('{') || action.starts_with('[') {
            // Try to parse as JSON
            match serde_json::from_str::<EncodingInput>(action) {
                Ok(input) => process_encoding(&input.action, &input.file_path, input.strict_mode, start_time),
                Err(e) => EncodingOutput {
                    status: "error".to_string(),
                    skill: "encoding_checker".to_string(),
                    file: None,
                    encoding: None,
                    issues: None,
                    report: None,
                    files_scanned: None,
                    files_with_issues: None,
                    error: Some(format!("JSON parse error: {}", e)),
                    duration_ms: 0,
                },
            }
        } else if ["check", "scan", "report"].contains(&action.as_str()) {
            // Valid action
            process_encoding(action, "", false, start_time)
        } else {
            // Invalid action
            EncodingOutput {
                status: "error".to_string(),
                skill: "encoding_checker".to_string(),
                file: None,
                encoding: None,
                issues: None,
                report: None,
                files_scanned: None,
                files_with_issues: None,
                error: Some(format!("Unknown action: {}", action)),
                duration_ms: 0,
            }
        }
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_encoding(action: &str, file_path: &str, strict_mode: bool, start_time: std::time::Instant) -> EncodingOutput {
    match action {
        "check" => check_file_encoding(file_path, start_time),
        "scan" => scan_project_encoding(file_path, strict_mode, start_time),
        "report" => generate_encoding_report(file_path, start_time),
        _ => EncodingOutput {
            status: "error".to_string(),
            skill: "encoding_checker".to_string(),
            file: None,
            encoding: None,
            issues: None,
            report: None,
            files_scanned: None,
            files_with_issues: None,
            error: Some(format!("Unknown action: {}", action)),
            duration_ms: 0,
        },
    }
}

fn check_file_encoding(file_path: &str, start_time: std::time::Instant) -> EncodingOutput {
    let path = Path::new(file_path);
    
    if !path.exists() {
        return EncodingOutput {
            status: "error".to_string(),
            skill: "encoding_checker".to_string(),
            file: Some(file_path.to_string()),
            encoding: None,
            issues: None,
            report: None,
            files_scanned: None,
            files_with_issues: None,
            error: Some(format!("File not found: {}", file_path)),
            duration_ms: 0,
        };
    }

    // Try to read as UTF-8
    match fs::read_to_string(file_path) {
        Ok(content) => {
            let duration = start_time.elapsed().as_millis() as u64;
            
            // Check for common GBK/ANSI indicators
            let issues = check_encoding_issues(file_path, &content);
            
            EncodingOutput {
                status: if issues.is_empty() { "success".to_string() } else { "warning".to_string() },
                skill: "encoding_checker".to_string(),
                file: Some(file_path.to_string()),
                encoding: Some("UTF-8".to_string()),
                issues: Some(issues),
                report: None,
                files_scanned: None,
                files_with_issues: None,
                error: None,
                duration_ms: duration,
            }
        }
        Err(e) => {
            // File is not valid UTF-8, likely GBK/ANSI
            let duration = start_time.elapsed().as_millis() as u64;
            
            EncodingOutput {
                status: "error".to_string(),
                skill: "encoding_checker".to_string(),
                file: Some(file_path.to_string()),
                encoding: Some("GBK/ANSI detected".to_string()),
                issues: Some(vec![EncodingIssue {
                    file: file_path.to_string(),
                    issue: "File is not valid UTF-8 encoding".to_string(),
                    severity: "error".to_string(),
                }]),
                report: None,
                files_scanned: None,
                files_with_issues: None,
                error: Some(format!("Encoding error: {}", e)),
                duration_ms: duration,
            }
        }
    }
}

fn scan_project_encoding(project_root: &str, strict_mode: bool, start_time: std::time::Instant) -> EncodingOutput {
    let root_path = Path::new(project_root);
    let base_path = if root_path.exists() { root_path } else { Path::new(".") };
    
    let mut files_scanned = 0;
    let mut files_with_issues = 0;
    let mut all_issues = Vec::new();
    
    // Scan specific file types
    let extensions = vec!["rs", "py", "md", "json", "toml", "yaml", "yml"];
    
    // Scan root level files first
    for entry in fs::read_dir(base_path).unwrap_or_else(|_| panic!("Cannot read directory: {:?}", base_path)) {
        let entry = entry.unwrap();
        let file_name = entry.file_name();
        let file_path = entry.path();
        
        if file_path.is_file() {
            if let Some(name_str) = file_name.to_str() {
                // Check extension
                let ext = name_str.split('.').last().unwrap_or("");
                if extensions.contains(&ext) {
                    files_scanned += 1;
                    
                    // Try to read as UTF-8
                    if let Ok(content) = fs::read_to_string(&file_path) {
                        let issues = check_encoding_issues(file_path.to_str().unwrap(), &content);
                        if !issues.is_empty() {
                            files_with_issues += 1;
                            all_issues.extend(issues);
                        }
                    } else {
                        files_with_issues += 1;
                        all_issues.push(EncodingIssue {
                            file: file_path.to_str().unwrap().to_string(),
                            issue: "File is not valid UTF-8 encoding".to_string(),
                            severity: if strict_mode { "error" } else { "warning" }.to_string(),
                        });
                    }
                }
            }
        }
    }
    
    // Scan skills directories
    let skills_dirs = vec!["skills", "skill-router/skills"];
    for skills_dir in skills_dirs {
        let skills_path = base_path.join(skills_dir);
        if skills_path.exists() {
            for entry in fs::read_dir(&skills_path).unwrap_or_else(|_| panic!("Cannot read directory: {:?}", skills_path)) {
                let entry = entry.unwrap();
                let skill_dir = entry.path();
                
                if skill_dir.is_dir() {
                    // Check skill.json
                    let skill_json = skill_dir.join("skill.json");
                    if skill_json.exists() {
                        files_scanned += 1;
                        if let Ok(content) = fs::read_to_string(&skill_json) {
                            let issues = check_encoding_issues(skill_json.to_str().unwrap(), &content);
                            if !issues.is_empty() {
                                files_with_issues += 1;
                                all_issues.extend(issues);
                            }
                        }
                    }
                    
                    // Check main.rs in src
                    let src_main = skill_dir.join("src").join("main.rs");
                    if src_main.exists() {
                        files_scanned += 1;
                        if let Ok(content) = fs::read_to_string(&src_main) {
                            let issues = check_encoding_issues(src_main.to_str().unwrap(), &content);
                            if !issues.is_empty() {
                                files_with_issues += 1;
                                all_issues.extend(issues);
                            }
                        }
                    }
                }
            }
        }
    }
    
    let duration = start_time.elapsed().as_millis() as u64;
    
    EncodingOutput {
        status: if files_with_issues > 0 { "warning".to_string() } else { "success".to_string() },
        skill: "encoding_checker".to_string(),
        file: None,
        encoding: None,
        issues: Some(all_issues),
        report: Some(format!(
            "# Encoding Scan Report

## Summary
- Files Scanned: {}
- Files with Issues: {}
- Strict Mode: {}

## Status: {}",
            files_scanned,
            files_with_issues,
            strict_mode,
            if files_with_issues == 0 { "All files are UTF-8 encoded" } else { "Some files have encoding issues" }
        )),
        files_scanned: Some(files_scanned),
        files_with_issues: Some(files_with_issues),
        error: None,
        duration_ms: duration,
    }
}

fn generate_encoding_report(file_path: &str, start_time: std::time::Instant) -> EncodingOutput {
    let report = r#"
# Encoding Checker Report

## Purpose
This skill checks for encoding issues in the project to prevent GBK/ANSI encoding problems.

## Common Issues
1. **Chinese characters in source files**: Should use English identifiers
2. **Non-UTF-8 files**: All files should be UTF-8 encoded
3. **Mixed encodings**: Inconsistent encoding across files

## Prevention Rules
1. All source code files (.rs, .py) must be UTF-8 encoded
2. All documentation files (.md) must be UTF-8 encoded
3. Use English identifiers in source code
4. Check encoding before committing

## Files to Check
- skills/*/src/main.rs
- skills/*/skill.json
- *.md
- Cargo.toml
- config.json
- registry.json
"#;
    
    let duration = start_time.elapsed().as_millis() as u64;
    
    EncodingOutput {
        status: "success".to_string(),
        skill: "encoding_checker".to_string(),
        file: None,
        encoding: None,
        issues: None,
        report: Some(report.to_string()),
        files_scanned: None,
        files_with_issues: None,
        error: None,
        duration_ms: duration,
    }
}

fn check_encoding_issues(file_path: &str, content: &str) -> Vec<EncodingIssue> {
    let mut issues = Vec::new();
    
    // Check for Chinese characters in source files
    if file_path.ends_with(".rs") || file_path.ends_with(".py") {
        for c in content.chars() {
            if c >= '\u{4e00}' && c <= '\u{9fff}' {
                issues.push(EncodingIssue {
                    file: file_path.to_string(),
                    issue: format!("Chinese character found in source file: '{}'", c),
                    severity: "warning".to_string(),
                });
                break; // Report only first occurrence
            }
        }
    }
    
    issues
}
