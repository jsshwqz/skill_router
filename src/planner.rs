pub struct Planner;

impl Planner {
    pub fn infer_capabilities(task: &str) -> Vec<String> {
        let task_lower = task.to_lowercase();

        // RULE 1: Intent Firewall - Prevent Prompt Injection & Malicious Intent
        let blacklist = [
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
        ];

        for forbidden in blacklist {
            if task_lower.contains(forbidden) {
                eprintln!(
                    "[SECURITY ALERT] Malicious intent detected in task: '{}'. Blocking execution.",
                    forbidden
                );
                return Vec::new(); // Return no capabilities to halt the flow
            }
        }

        let mut caps = Vec::new();

        if task_lower.contains("yaml") || task_lower.contains("解析yaml") {
            caps.push("yaml_parse".to_string());
        }
        if task_lower.contains("json") || task_lower.contains("解析json") {
            caps.push("json_parse".to_string());
        }
        if task_lower.contains("pdf") || task_lower.contains("解析pdf") {
            caps.push("pdf_parse".to_string());
        }
        if task_lower.contains("search")
            || task_lower.contains("web")
            || task_lower.contains("google")
            || task_lower.contains("搜索")
            || task_lower.contains("查找")
        {
            caps.push("web_search".to_string());
        }
        if task_lower.contains("parse")
            || task_lower.contains("extract")
            || task_lower.contains("analyze")
            || task_lower.contains("解析")
            || task_lower.contains("分析")
            || task_lower.contains("提取")
        {
            caps.push("generic_parse".to_string());
        }
        if task_lower.contains("summarize")
            || task_lower.contains("tl;dr")
            || task_lower.contains("汇总")
            || task_lower.contains("统计")
            || task_lower.contains("概括")
        {
            caps.push("text_summarize".to_string());
        }
        if task_lower.contains("synthesize")
            || task_lower.contains("synth")
            || task_lower.contains("合成")
            || task_lower.contains("生成")
        {
            caps.push("skill_synthesize".to_string());
        }

        // Memory management capabilities
        if task_lower.contains("save") || task_lower.contains("保存") || task_lower.contains("存储") {
            caps.push("memory_management".to_string());
        }
        if task_lower.contains("load") || task_lower.contains("加载") || task_lower.contains("读取") {
            caps.push("memory_management".to_string());
        }
        if task_lower.contains("memory") || task_lower.contains("记忆") || task_lower.contains("context") {
            caps.push("memory_management".to_string());
        }

        caps
    }
}
