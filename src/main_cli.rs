#!/usr/bin/env -S cargo run
//! 智能搜索 CLI 工具
//! 
//! 功能特性:
//! - 透明决策日志显示
//! - 实时进度指示器
//! - 彩色输出和格式化
//! - 用户配置支持
//! - 错误恢复和降级

use anyhow::Result;
use clap::{Arg, Command};
use serde_json::Value;
use std::env;
use std::process::Command as ProcessCommand;

fn main() -> Result<()> {
    let matches = Command::new("智能搜索")
        .version("1.0")
        .about("智能混合搜索引擎 - 免费、无API依赖")
        .arg(
            Arg::new("query")
                .help("搜索查询或URL")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("显示详细决策日志"),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("静默模式，仅输出结果"),
        )
        .arg(
            Arg::new("raw")
                .short('r')
                .long("raw")
                .help("输出原始JSON响应"),
        )
        .get_matches();

    let query = matches.get_one::<String>("query").unwrap();
    let verbose = matches.get_flag("verbose");
    let quiet = matches.get_flag("quiet");
    let raw = matches.get_flag("raw");

    // 如果启用了静默模式，禁用透明度
    if quiet {
        // 创建临时配置文件
        std::fs::write(
            "search_config_temp.json",
            r#"{"transparency": {"show_decision_log": false}}"#,
        )?;
        env::set_var("SEARCH_CONFIG", "search_config_temp.json");
    }

    // 调用混合搜索技能
    let hybrid_path = r"C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\hybrid_search	argetelease\hybrid_search.exe";
    
    println!("{}", "🔍 正在处理搜索请求...".green());
    
    match ProcessCommand::new(hybrid_path).arg(query).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if raw {
                    println!("{}", stdout);
                } else {
                    match serde_json::from_str::<Value>(&stdout) {
                        Ok(response) => {
                            display_formatted_response(&response, verbose)?;
                        }
                        Err(_) => {
                            println!("❌ 响应解析失败");
                            println!("{}", stdout);
                        }
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("❌ 执行失败: {}", stderr);
            }
        }
        Err(e) => {
            println!("❌ 启动失败: {}", e);
        }
    }

    // 清理临时配置
    if quiet {
        let _ = std::fs::remove_file("search_config_temp.json");
    }

    Ok(())
}

fn display_formatted_response(response: &Value, verbose: bool) -> Result<()> {
    if let Some(status) = response["status"].as_str() {
        if status == "success" {
            println!("{}", "✅ 搜索成功!".green());
            
            // 显示内容或结果
            if let Some(content) = response["content"].as_str() {
                println!("
{}", content);
            } else if let Some(results) = response["results"].as_array() {
                println!("
找到 {} 个结果:", results.len());
                for (i, result) in results.iter().enumerate() {
                    if let (Some(title), Some(url)) = (
                        result["title"].as_str(),
                        result["url"].as_str(),
                    ) {
                        println!("
{}.", i + 1);
                        println!("   标题: {}", title.bold());
                        println!("   链接: {}", url.blue());
                        if let Some(text) = result["content"].as_str() {
                            println!("   摘要: {}", text);
                        }
                    }
                }
            }
        } else {
            println!("{}", "❌ 搜索失败!".red());
            if let Some(error) = response["error"].as_str() {
                println!("错误: {}", error.red());
            }
        }

        // 显示决策日志（如果启用）
        if verbose || response["decision_log"].is_object() {
            if let Some(decision_log) = response["decision_log"].as_object() {
                println!("
{}", "📊 决策分析:".cyan().bold());
                println!("   查询类型: {}", decision_log["query_analyzed_as"].as_str().unwrap_or("unknown"));
                println!("   最终选择: {}", decision_log["final_choice"].as_str().unwrap_or("unknown"));
                println!("   置信度: {:.1}%", decision_log["confidence"].as_f64().unwrap_or(0.0) * 100.0);
                
                if let Some(reasoning) = decision_log["reasoning"].as_array() {
                    println!("   推理过程:");
                    for reason in reasoning {
                        if let Some(r) = reason.as_str() {
                            println!("     • {}", r);
                        }
                    }
                }
                
                if let Some(alternatives) = decision_log["alternative_results"].as_array() {
                    println!("   备选方案:");
                    for alt in alternatives {
                        if let (Some(skill), Some(status), Some(reason)) = (
                            alt["skill"].as_str(),
                            alt["status"].as_str(),
                            alt["reason"].as_str(),
                        ) {
                            println!("     • {}: {} ({})", skill, status, reason);
                        }
                    }
                }
            }
        }

        // 显示执行时间
        if let Some(duration) = response["duration_ms"].as_u64() {
            println!("
⏱️  执行时间: {}ms", duration);
        }
    }

    Ok(())
}

trait Colorize {
    fn red(&self) -> String;
    fn green(&self) -> String;
    fn blue(&self) -> String;
    fn cyan(&self) -> String;
    fn bold(&self) -> String;
}

impl Colorize for str {
    fn red(&self) -> String {
        format!("\x1b[31m{}\x1b[0m", self)
    }
    
    fn green(&self) -> String {
        format!("\x1b[32m{}\x1b[0m", self)
    }
    
    fn blue(&self) -> String {
        format!("\x1b[34m{}\x1b[0m", self)
    }
    
    fn cyan(&self) -> String {
        format!("\x1b[36m{}\x1b[0m", self)
    }
    
    fn bold(&self) -> String {
        format!("\x1b[1m{}\x1b[0m", self)
    }
}