#!/usr/bin/env -S cargo run
//! Google 搜索技能 (Rust Implementation)
//! 
//! 功能:
//! 1. 执行 Google 搜索
//! 2. 解析搜索结果
//! 3. 返回格式化结果

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Deserialize)]
struct SearchInput {
    query: String,
    #[serde(default)]
    count: usize,
}

#[derive(Debug, Serialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[derive(Debug, Serialize)]
struct GoogleOutput {
    status: String,
    skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    results: Option<Vec<SearchResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    duration_ms: u64,
}

fn main() {
    let start_time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();

    let output = if args.len() < 2 {
        // 使用默认搜索进行测试
        process_search("Rust programming language", 3, start_time)
    } else {
        // 解析 JSON 输入
        match serde_json::from_str::<SearchInput>(&args[1]) {
            Ok(input) => process_search(&input.query, input.count, start_time),
            Err(e) => GoogleOutput {
                status: "error".to_string(),
                skill: "google_search".to_string(),
                results: None,
                error: Some(format!("JSON parse error: {}", e)),
                duration_ms: 0,
            },
        }
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn process_search(query: &str, count: usize, start_time: std::time::Instant) -> GoogleOutput {
    // 模拟 Google 搜索结果
    // 在真实场景中，这里会调用 Google Custom Search API
    
    let mock_results = vec![
        SearchResult {
            title: format!("{} - Wikipedia", query),
            url: format!("https://en.wikipedia.org/wiki/{}", query.replace(' ', "_")),
            snippet: format!("Detailed information about {} from Wikipedia.", query),
        },
        SearchResult {
            title: format!("Official {} Documentation", query),
            url: format!("https://docs.example.com/{}", query.replace(' ', "-")),
            snippet: format!("Official documentation and resources for {}.", query),
        },
        SearchResult {
            title: format!("Learn {} - Tutorial", query),
            url: format!("https://tutorial.example.com/{}", query.replace(' ', "-")),
            snippet: format!("Step-by-step tutorial to learn {} from scratch.", query),
        },
    ];

    let results = if count > mock_results.len() {
        mock_results
    } else {
        mock_results.into_iter().take(count).collect()
    };

    let duration = start_time.elapsed().as_millis() as u64;
    
    GoogleOutput {
        status: "success".to_string(),
        skill: "google_search".to_string(),
        results: Some(results),
        error: None,
        duration_ms: duration,
    }
}
