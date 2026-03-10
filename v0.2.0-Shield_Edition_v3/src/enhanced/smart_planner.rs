use crate::models::{ComplexityLevel, ExecutionStrategy, SubTask, TaskPlan, TaskStatus};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashSet;

lazy_static! {
    static ref MULTI_TASK_PATTERNS: Vec<(&'static str, Vec<&'static str>)> = vec![
        (
            r"然后|接着|之后|并且|同时|也",
            vec!["sequential", "parallel"]
        ),
        (r"先.*再|首先.*然后|第一步.*第二步", vec!["sequential"]),
        (r"并行|同时|一起|并发", vec!["parallel"]),
        (r"处理所有|批量|多个|全部", vec!["parallel"]),
    ];
    static ref CAPABILITY_PATTERNS: Vec<(&'static str, &'static str)> = vec![
        (r"(?i)\b(yaml|yml)\b|\b解析\s*yaml", "yaml_parse"),
        (r"(?i)\b(json)\b|\b解析\s*json", "json_parse"),
        (r"(?i)\b(pdf)\b|\b解析\s*pdf", "pdf_parse"),
        (r"(?i)\b(excel|xlsx|csv)\b|\b解析\s*excel", "excel_parse"),
        (r"(?i)\b(search|搜索|查找|检索|google)\b", "web_search"),
        (r"(?i)\b(summarize|摘要|总结|概括|汇总)\b", "text_summarize"),
        (r"(?i)\b(translate|翻译)\b", "translation"),
        (r"(?i)\b(analyze|分析|提取|extract)\b", "data_analysis"),
        (r"(?i)\b(convert|转换|transform)\b", "data_conversion"),
        (r"(?i)\b(download|下载|fetch)\b", "file_download"),
        (r"(?i)\b(upload|上传)\b", "file_upload"),
        (r"(?i)\b(email|邮件|send\s*mail)\b", "email_send"),
        (r"(?i)\b(image|图片|图片处理)\b", "image_process"),
        (r"(?i)\b(video|视频)\b", "video_process"),
        (r"(?i)\b(audio|音频|语音)\b", "audio_process"),
        (r"(?i)\b(code|代码|生成代码|generate)\b", "code_generation"),
        (r"(?i)\b(test|测试|单元测试)\b", "test_generation"),
        (r"(?i)\b(api|接口)\b", "api_call"),
        (r"(?i)\b(database|数据库|sql)\b", "database_query"),
        (r"(?i)\b(scraper|爬虫|crawl|spider)\b", "web_scraping"),
    ];
}

pub struct SmartPlanner;

impl SmartPlanner {
    pub fn analyze_task(task: &str) -> TaskPlan {
        let task_id = format!("task_{}", chrono::Utc::now().timestamp_millis());

        let security_check = Self::security_filter(task);
        if !security_check.is_safe {
            return TaskPlan {
                task_id,
                original_task: task.to_string(),
                subtasks: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                estimated_complexity: ComplexityLevel::Simple,
            };
        }

        let subtasks = Self::decompose_task(task);
        let strategy = Self::determine_strategy(&subtasks, task);
        let complexity = Self::estimate_complexity(&subtasks);

        TaskPlan {
            task_id,
            original_task: task.to_string(),
            subtasks,
            execution_strategy: strategy,
            estimated_complexity: complexity,
        }
    }

    fn security_filter(task: &str) -> SecurityCheckResult {
        let task_lower = task.to_lowercase();

        let critical_blacklist = [
            "ignore previous",
            "forget instruction",
            "sudo",
            "format c:",
            "rm -rf",
            "delete root",
            "overwrite system",
            "bypass security",
            "disable guardian",
            "reveal secret",
            "dump registry",
            "drop table",
            "truncate",
            "exec(",
            "eval(",
            "__import__",
            "subprocess.popen.*shell=true",
            "os.system(",
            "base64.b64decode",
        ];

        let warning_patterns = [
            "delete all",
            "remove all",
            "wipe",
            "destroy",
            "password",
            "credential",
            "secret key",
            "api key",
        ];

        for pattern in critical_blacklist {
            if task_lower.contains(pattern) {
                eprintln!(
                    "[SECURITY CRITICAL] Blocked malicious intent: '{}'",
                    pattern
                );
                return SecurityCheckResult {
                    is_safe: false,
                    warnings: vec![],
                };
            }
        }

        let mut warnings = Vec::new();
        for pattern in warning_patterns {
            if task_lower.contains(pattern) {
                warnings.push(format!("Warning: Sensitive keyword '{}' detected", pattern));
            }
        }

        SecurityCheckResult {
            is_safe: true,
            warnings,
        }
    }

    fn decompose_task(task: &str) -> Vec<SubTask> {
        let mut subtasks = Vec::new();
        let capabilities = Self::infer_capabilities(task);

        if Self::is_multi_step_task(task) {
            let segments = Self::split_task_segments(task);

            for (idx, segment) in segments.iter().enumerate() {
                let segment_caps = Self::infer_capabilities(segment);
                let subtask_id = format!("subtask_{}", idx);

                let dependencies = if idx > 0 {
                    vec![format!("subtask_{}", idx - 1)]
                } else {
                    vec![]
                };

                subtasks.push(SubTask {
                    id: subtask_id,
                    description: segment.clone(),
                    required_capabilities: segment_caps,
                    dependencies,
                    status: TaskStatus::Pending,
                    assigned_skill: None,
                    retry_count: 0,
                });
            }
        } else {
            subtasks.push(SubTask {
                id: "subtask_0".to_string(),
                description: task.to_string(),
                required_capabilities: capabilities,
                dependencies: vec![],
                status: TaskStatus::Pending,
                assigned_skill: None,
                retry_count: 0,
            });
        }

        subtasks
    }

    fn is_multi_step_task(task: &str) -> bool {
        for (pattern, _) in MULTI_TASK_PATTERNS.iter() {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(task) {
                    return true;
                }
            }
        }

        let sentence_count = task
            .matches(|c| c == '。' || c == '.' || c == '；' || c == ';')
            .count();
        sentence_count > 1
    }

    fn split_task_segments(task: &str) -> Vec<String> {
        let mut segments = Vec::new();

        let delimiters = ["然后", "接着", "之后", "并且", "。", ".", "；", ";", "\n"];
        let mut current_segment = String::new();
        let mut chars = task.chars().peekable();

        while let Some(ch) = chars.next() {
            current_segment.push(ch);

            let is_delimiter = delimiters.iter().any(|d| {
                if d.len() == 1 {
                    d.chars().next() == Some(ch)
                } else {
                    let remaining: String = chars.clone().take(d.len() - 1).collect();
                    let combined = format!("{}{}", ch, remaining);
                    combined == *d
                }
            });

            if is_delimiter && !current_segment.trim().is_empty() {
                segments.push(current_segment.trim().to_string());
                current_segment = String::new();
            }
        }

        if !current_segment.trim().is_empty() {
            segments.push(current_segment.trim().to_string());
        }

        if segments.is_empty() {
            segments.push(task.to_string());
        }

        segments
    }

    fn infer_capabilities(task: &str) -> Vec<String> {
        let mut caps = Vec::new();
        let mut seen = HashSet::new();

        for (pattern, capability) in CAPABILITY_PATTERNS.iter() {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(task) && !seen.contains(*capability) {
                    caps.push(capability.to_string());
                    seen.insert(capability.to_string());
                }
            }
        }

        if caps.is_empty() {
            caps.push("generic_parse".to_string());
        }

        caps
    }

    fn determine_strategy(subtasks: &[SubTask], task: &str) -> ExecutionStrategy {
        let task_lower = task.to_lowercase();

        if task_lower.contains("并行") || task_lower.contains("同时") || task_lower.contains("并发")
        {
            return ExecutionStrategy::Parallel;
        }

        if subtasks.len() > 1 {
            let has_dependencies = subtasks.iter().any(|s| !s.dependencies.is_empty());

            if has_dependencies {
                return ExecutionStrategy::Pipeline;
            }

            let unique_caps: HashSet<_> = subtasks
                .iter()
                .flat_map(|s| s.required_capabilities.iter())
                .collect();

            if unique_caps.len() == subtasks.len() {
                return ExecutionStrategy::Parallel;
            }

            return ExecutionStrategy::Sequential;
        }

        ExecutionStrategy::Sequential
    }

    fn estimate_complexity(subtasks: &[SubTask]) -> ComplexityLevel {
        match subtasks.len() {
            0 => ComplexityLevel::Simple,
            1 => {
                let caps = &subtasks[0].required_capabilities;
                if caps.len() <= 1 {
                    ComplexityLevel::Simple
                } else {
                    ComplexityLevel::Medium
                }
            }
            2..=3 => ComplexityLevel::Medium,
            4..=6 => ComplexityLevel::Complex,
            _ => ComplexityLevel::MultiStage,
        }
    }
}

struct SecurityCheckResult {
    is_safe: bool,
    warnings: Vec<String>,
}
