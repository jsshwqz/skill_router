//! AI chat calls (OpenAI-compatible API)

use anyhow::Result;
use serde_json::json;

/// Creative/exploratory variant — allows moderate output variance.
#[allow(dead_code)]
pub async fn chat(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let url = format!("{}/chat/completions", base_url);
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt },
        ],
        "temperature": 0.7,
        "max_tokens": 4096,
    });

    chat_inner(http, &url, &body, api_key).await
}

/// Deterministic variant for structured tasks (contract, sensor, classification).
/// Uses low temperature to minimize output variance.
pub async fn chat_deterministic(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let url = format!("{}/chat/completions", base_url);
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt },
        ],
        "temperature": 0.2,
        "max_tokens": 4096,
    });

    chat_inner(http, &url, &body, api_key).await
}

async fn chat_inner(http: &reqwest::Client, url: &str, body: &serde_json::Value, api_key: &str) -> Result<String> {
    let resp = http
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("AI API error ({}): {}", status, text);
    }

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    Ok(content)
}

/// Creative variant for JSON tasks.
#[allow(dead_code)]
pub async fn chat_json(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<serde_json::Value> {
    let raw = chat(http, base_url, api_key, model, system_prompt, user_prompt).await?;
    extract_json(&raw)
}

/// Deterministic variant for structured JSON tasks.
pub async fn chat_json_deterministic(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<serde_json::Value> {
    let raw = chat_deterministic(http, base_url, api_key, model, system_prompt, user_prompt).await?;
    extract_json(&raw)
}

/// Extract JSON object/array from raw text (strips surrounding text).
fn extract_json(raw: &str) -> Result<serde_json::Value> {
    let json_str = if let Some(start) = raw.find('{') {
        let end = raw.rfind('}').unwrap_or(raw.len() - 1);
        &raw[start..=end]
    } else if let Some(start) = raw.find('[') {
        let end = raw.rfind(']').unwrap_or(raw.len() - 1);
        &raw[start..=end]
    } else {
        raw
    };
    let parsed = serde_json::from_str(json_str)?;
    Ok(parsed)
}
