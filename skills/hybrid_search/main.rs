#!/usr/bin/env -S cargo run
//! Hybrid Search 技能 (Rust Implementation)
//! 
//! 功能:
//! 1. 智能路由：URL -> Jina Reader, 查询 -> Exa Search
//! 2. 免费无API依赖的完整搜索解决方案
//! 3. 安全可控的混合搜索策略
//! 4. 透明决策日志和用户配置支持

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct HybridInput {
    query: String,
    #[serde(default = "default_strategy")]
    strategy: String,
    #[serde(default)]
    use_jina: bool,
    #[serde(default)]
    use_exa: bool,
}

#[derive(Debug, Serialize)]
struct HybridResult {
    title: String,
    url: String,
    content: String,
}

fn default_strategy() -> String {
    "auto".to_string()
}

#[derive(Debug, Serialize)]
struct SearchDecision {
    query_analyzed_as: String,
    skills_considered: Vec<String>,
    final_choice: String,
    confidence: f32,
    reasoning: Vec<String>,
    alternative_results: Option<Vec<AlternativeResult>>,
}

#[derive(Debug, Serialize)]
struct AlternativeResult {
    skill: String,
    status: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct HybridOutput {
    status: String,
    skill: String,
    strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    results: Option<Vec<HybridResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision_log: Option<SearchDecision>,
}

fn main() -> Result<()> {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    // 读取搜索配置
    let config_path = "search_config.json";
    let use_transparency = match std::fs::read_to_string(config_path) {
        Ok(config_content) => {
            let config: serde_json::Value = serde_json::from_str(&config_content)?;
            config["transparency"]["show_decision_log"].as_bool().unwrap_or(true)
        }
        Err(_) => true, // 默认启用透明度
    };

    let output = if args.len() < 2 {
        // 测试模式
        HybridOutput {
            status: "error".to_string(),
            skill: "hybrid_search".to_string(),
            strategy: "none".to_string(),
            results: None,
            content: None,
            error: Some("Query required".to_string()),
            duration_ms: 0,
            decision_log: if use_transparency {
                Some(SearchDecision {
                    query_analyzed_as: "invalid_input".to_string(),
                    skills_considered: vec![],
                    final_choice: "none".to_string(),
                    confidence: 0.0,
                    reasoning: vec!["Missing query parameter".to_string()],
                    alternative_results: None,
                })
            } else {
                None
            },
        }
    } else {
        let query = &args[1];
        
        // 智能路由决策
        if query.contains("http://") || query.contains("https://") || query.contains("www.") {
            // URL 内容提取 -> 首先尝试 Jina Reader，如果需要 JavaScript 渲染则使用浏览器自动化
            execute_url_extraction_with_transparency(query, start_time, use_transparency)
        } else {
            // 通用查询 -> Exa Search
            execute_exa_search_with_transparency(query, start_time, use_transparency)
        }
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn execute_url_extraction_with_transparency(url: &str, start_time: std::time::Instant, use_transparency: bool) -> HybridOutput {
    let mut reasoning = vec!["Query contains URL pattern (http/https/www)".to_string()];
    let skills_considered = vec!["jina_reader".to_string(), "browser_automation".to_string()];
    
    // 首先尝试 Jina Reader（适用于静态内容）
    let jina_result = execute_jina_reader_raw(url);
    
    let decision_log = if use_transparency {
        Some(SearchDecision {
            query_analyzed_as: "static_url".to_string(),
            skills_considered: skills_considered.clone(),
            final_choice: "jina_reader".to_string(),
            confidence: if jina_result.status == "success" { 0.95 } else { 0.6 },
            reasoning: reasoning.clone(),
            alternative_results: None,
        })
    } else {
        None
    };
    
    // 如果 Jina Reader 成功，直接返回结果
    if jina_result.status == "success" {
        return HybridOutput {
            status: "success".to_string(),
            skill: "hybrid_search".to_string(),
            strategy: "web_reader".to_string(),
            results: None,
            content: jina_result.content,
            error: None,
            duration_ms: start_time.elapsed().as_millis() as u64,
            decision_log,
        };
    }
    
    // 如果 Jina Reader 失败，检查是否需要浏览器自动化
    let browser_available = check_browser_availability();
    let mut alternative_results = Vec::new();
    
    if browser_available {
        // 使用浏览器自动化
        reasoning.push("Jina Reader failed, browser available".to_string());
        let browser_result = execute_browser_automation_raw(url);
        
        if use_transparency {
            alternative_results.push(AlternativeResult {
                skill: "jina_reader".to_string(),
                status: "failed".to_string(),
                reason: jina_result.error.unwrap_or("Unknown error".to_string()),
            });
        }
        
        let final_decision_log = if use_transparency {
            Some(SearchDecision {
                query_analyzed_as: "dynamic_url".to_string(),
                skills_considered,
                final_choice: "browser_automation".to_string(),
                confidence: 0.85,
                reasoning,
                alternative_results: Some(alternative_results),
            })
        } else {
            None
        };
        
        return HybridOutput {
            status: browser_result.status,
            skill: "hybrid_search".to_string(),
            strategy: "browser_automation".to_string(),
            results: None,
            content: browser_result.content,
            error: browser_result.error,
            duration_ms: start_time.elapsed().as_millis() as u64,
            decision_log: final_decision_log,
        };
    } else {
        // 浏览器不可用，返回 Jina Reader 的错误
        reasoning.push("Jina Reader failed, browser not available".to_string());
        
        let final_decision_log = if use_transparency {
            Some(SearchDecision {
                query_analyzed_as: "static_url".to_string(),
                skills_considered,
                final_choice: "jina_reader".to_string(),
                confidence: 0.3, // 低置信度，因为失败了
                reasoning,
                alternative_results: None,
            })
        } else {
            None
        };
        
        return HybridOutput {
            status: "error".to_string(),
            skill: "hybrid_search".to_string(),
            strategy: "web_reader".to_string(),
            results: None,
            content: None,
            error: jina_result.error,
            duration_ms: start_time.elapsed().as_millis() as u64,
            decision_log: final_decision_log,
        };
    }
}

fn execute_jina_reader_raw(url: &str) -> HybridOutput {
    let jina_path = r"C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\jina_reader\target\release\jina_reader.exe";
    
    let input_json = serde_json::json!({
        "url": url,
        "timeout_seconds": 30
    }).to_string();
    
    match Command::new(jina_path).arg(input_json).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                match serde_json::from_str::<serde_json::Value>(&stdout) {
                    Ok(jina_response) => {
                        if jina_response["status"] == "success" {
                            HybridOutput {
                                status: "success".to_string(),
                                skill: "jina_reader".to_string(),
                                strategy: "web_reader".to_string(),
                                results: None,
                                content: jina_response["content"].as_str().map(|s| s.to_string()),
                                error: None,
                                duration_ms: 0,
                                decision_log: None,
                            }
                        } else {
                            HybridOutput {
                                status: "error".to_string(),
                                skill: "jina_reader".to_string(),
                                strategy: "web_reader".to_string(),
                                results: None,
                                content: None,
                                error: Some(format!("Jina Reader error: {}", jina_response["error"])),
                                duration_ms: 0,
                                decision_log: None,
                            }
                        }
                    }
                    Err(e) => HybridOutput {
                        status: "error".to_string(),
                        skill: "jina_reader".to_string(),
                        strategy: "web_reader".to_string(),
                        results: None,
                        content: None,
                        error: Some(format!("Jina response parse error: {}", e)),
                        duration_ms: 0,
                        decision_log: None,
                    },
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                HybridOutput {
                    status: "error".to_string(),
                    skill: "jina_reader".to_string(),
                    strategy: "web_reader".to_string(),
                    results: None,
                    content: None,
                    error: Some(format!("Jina Reader execution failed: {}", stderr)),
                    duration_ms: 0,
                    decision_log: None,
                }
            }
        }
        Err(e) => HybridOutput {
            status: "error".to_string(),
            skill: "jina_reader".to_string(),
            strategy: "web_reader".to_string(),
            results: None,
            content: None,
            error: Some(format!("Failed to start Jina Reader: {}", e)),
            duration_ms: 0,
            decision_log: None,
        },
    }
}

fn execute_browser_automation_raw(url: &str) -> HybridOutput {
    let browser_path = r"C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\browser_automation\target\release\browser_automation.exe";
    
    match Command::new(browser_path).arg(url).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                match serde_json::from_str::<serde_json::Value>(&stdout) {
                    Ok(browser_response) => {
                        if browser_response["status"] == "success" {
                            HybridOutput {
                                status: "success".to_string(),
                                skill: "browser_automation".to_string(),
                                strategy: "browser_automation".to_string(),
                                results: None,
                                content: browser_response["content"].as_str().map(|s| s.to_string()),
                                error: browser_response["error"].as_str().map(|s| s.to_string()),
                                duration_ms: 0,
                                decision_log: None,
                            }
                        } else {
                            HybridOutput {
                                status: "error".to_string(),
                                skill: "browser_automation".to_string(),
                                strategy: "browser_automation".to_string(),
                                results: None,
                                content: None,
                                error: Some(format!("Browser automation error: {}", browser_response["error"])),
                                duration_ms: 0,
                                decision_log: None,
                            }
                        }
                    }
                    Err(e) => HybridOutput {
                        status: "error".to_string(),
                        skill: "browser_automation".to_string(),
                        strategy: "browser_automation".to_string(),
                        results: None,
                        content: None,
                        error: Some(format!("Browser response parse error: {}", e)),
                        duration_ms: 0,
                        decision_log: None,
                    },
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                HybridOutput {
                    status: "error".to_string(),
                    skill: "browser_automation".to_string(),
                    strategy: "browser_automation".to_string(),
                    results: None,
                    content: None,
                    error: Some(format!("Browser automation execution failed: {}", stderr)),
                    duration_ms: 0,
                    decision_log: None,
                }
            }
        }
        Err(e) => HybridOutput {
            status: "error".to_string(),
            skill: "browser_automation".to_string(),
            strategy: "browser_automation".to_string(),
            results: None,
            content: None,
            error: Some(format!("Failed to start browser automation: {}", e)),
            duration_ms: 0,
            decision_log: None,
        },
    }
}

fn check_browser_availability() -> bool {
    // 检查 Chrome
    match Command::new("where").arg("chrome.exe").output() {
        Ok(output) => {
            if output.status.success() && !output.stdout.is_empty() {
                return true;
            }
        }
        Err(_) => {}
    }
    
    // 检查 Edge
    match Command::new("where").arg("msedge.exe").output() {
        Ok(output) => {
            if output.status.success() && !output.stdout.is_empty() {
                return true;
            }
        }
        Err(_) => {}
    }
    
    false
}

fn execute_exa_search_with_transparency(query: &str, start_time: std::time::Instant, use_transparency: bool) -> HybridOutput {
    let exa_result = execute_exa_search_raw(query);
    
    let decision_log = if use_transparency {
        Some(SearchDecision {
            query_analyzed_as: "semantic_query".to_string(),
            skills_considered: vec!["exa_search".to_string()],
            final_choice: "exa_search".to_string(),
            confidence: if exa_result.status == "success" { 0.9 } else { 0.4 },
            reasoning: vec!["Query does not contain URL pattern, treated as semantic search".to_string()],
            alternative_results: None,
        })
    } else {
        None
    };
    
    HybridOutput {
        status: exa_result.status,
        skill: "hybrid_search".to_string(),
        strategy: "ai_search".to_string(),
        results: exa_result.results,
        content: exa_result.content,
        error: exa_result.error,
        duration_ms: start_time.elapsed().as_millis() as u64,
        decision_log,
    }
}

fn execute_exa_search_raw(query: &str) -> HybridOutput {
    let exa_path = r"C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\exa_search\target\release\exa_search.exe";
    
    let input_json = serde_json::json!({
        "query": query,
        "num_results": 5,
        "use_autoprompt": true,
        "timeout_seconds": 30
    }).to_string();
    
    match Command::new(exa_path).arg(input_json).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                match serde_json::from_str::<serde_json::Value>(&stdout) {
                    Ok(exa_response) => {
                        if exa_response["status"] == "success" {
                            let mut results = Vec::new();
                            if let Some(results_array) = exa_response["results"].as_array() {
                                for result in results_array {
                                    if let (Some(title), Some(url), Some(text)) = (
                                        result["title"].as_str(),
                                        result["url"].as_str(), 
                                        result["text"].as_str()
                                    ) {
                                        results.push(HybridResult {
                                            title: title.to_string(),
                                            url: url.to_string(),
                                            content: text.to_string(),
                                        });
                                    }
                                }
                            }
                            
                            HybridOutput {
                                status: "success".to_string(),
                                skill: "exa_search".to_string(),
                                strategy: "ai_search".to_string(),
                                results: if results.is_empty() { None } else { Some(results) },
                                content: None,
                                error: exa_response["error"].as_str().map(|s| s.to_string()),
                                duration_ms: 0,
                                decision_log: None,
                            }
                        } else {
                            HybridOutput {
                                status: "error".to_string(),
                                skill: "exa_search".to_string(),
                                strategy: "ai_search".to_string(),
                                results: None,
                                content: None,
                                error: Some(format!("Exa Search error: {}", exa_response["error"])),
                                duration_ms: 0,
                                decision_log: None,
                            }
                        }
                    }
                    Err(e) => HybridOutput {
                        status: "error".to_string(),
                        skill: "exa_search".to_string(),
                        strategy: "ai_search".to_string(),
                        results: None,
                        content: None,
                        error: Some(format!("Exa response parse error: {}", e)),
                        duration_ms: 0,
                        decision_log: None,
                    },
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                HybridOutput {
                    status: "error".to_string(),
                    skill: "exa_search".to_string(),
                    strategy: "ai_search".to_string(),
                    results: None,
                    content: None,
                    error: Some(format!("Exa Search execution failed: {}", stderr)),
                    duration_ms: 0,
                    decision_log: None,
                }
            }
        }
        Err(e) => HybridOutput {
            status: "error".to_string(),
            skill: "exa_search".to_string(),
            strategy: "ai_search".to_string(),
            results: None,
            content: None,
            error: Some(format!("Failed to start Exa Search: {}", e)),
            duration_ms: 0,
            decision_log: None,
        },
    }
}