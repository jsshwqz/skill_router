use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use aion_types::types::RouterPaths;
use aion_types::parallel::TaskGraph;

pub struct ParallelPlanner;

impl ParallelPlanner {
    pub fn split_task(task: &str, _paths: &RouterPaths) -> Result<TaskGraph> {
        let base_url = std::env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key  = std::env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string());
        let model    = std::env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string());

        let prompt = format!(
            "Task: \"{}\"\n\
            Split this task into smaller parallel sub-tasks. \n\
            Return a JSON object with this structure:\n\
            {{\"instructions\": [ \n\
              {{\"id\": \"unique_id\", \"task\": \"sub_task_description\", \"capability\": \"matched_capability\", \"dependencies\": []}} \n\
            ]}}\n\
            Available capabilities include: yaml_parse, json_parse, web_search, text_summarize, code_generate, etc.\n\
            Return ONLY the valid JSON.", 
            task
        );

        let body = json!({
            "model": model, 
            "messages": [{"role": "system", "content": "You are a task decomposer AI."}, {"role": "user", "content": prompt}], 
            "temperature": 0.0,
            "response_format": { "type": "json_object" }
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        
        let resp: Value = client.post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()?
            .json()?;

        let content = resp["choices"][0]["message"]["content"].as_str()
            .or_else(|| resp["result"].as_str())
            .ok_or_else(|| anyhow!("AI failed to return task graph"))?;

        let graph: TaskGraph = serde_json::from_str(content)?;
        Ok(graph)
    }
}
