pub struct Planner;

impl Planner {
    pub fn infer_capabilities(task: &str) -> Vec<String> {
        let mut caps = Vec::new();
        let task_lower = task.to_lowercase();
        
        if task_lower.contains("yaml") {
            caps.push("yaml_parse".to_string());
        }
        if task_lower.contains("json") {
            caps.push("json_parse".to_string());
        }
        if task_lower.contains("pdf") {
            caps.push("pdf_parse".to_string());
        }
        if task_lower.contains("search") || task_lower.contains("web") || task_lower.contains("google") {
            caps.push("web_search".to_string());
        }
        if task_lower.contains("parse") || task_lower.contains("extract") || task_lower.contains("analyze") {
            caps.push("generic_parse".to_string());
        }
        if task_lower.contains("summarize") || task_lower.contains("tl;dr") {
            caps.push("text_summarize".to_string());
        }
        if task_lower.contains("synthesize") || task_lower.contains("synth") {
            caps.push("skill_synthesize".to_string());
        }

        caps
    }
}
