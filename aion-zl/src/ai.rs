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
    let body = build_chat_body(model, system_prompt, user_prompt, 0.7, 4096);

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
    let body = build_chat_body(model, system_prompt, user_prompt, 0.2, 4096);

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

    parse_chat_content(&text)
}

/// Build an OpenAI-compatible `/chat/completions` request body.
/// Exposed as a private helper so request construction can be unit-tested
/// without any network access.
fn build_chat_body(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f64,
    max_tokens: u32,
) -> serde_json::Value {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt },
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
    })
}

/// Extract the assistant message content from a `/chat/completions` response.
fn parse_chat_content(text: &str) -> Result<String> {
    let parsed: serde_json::Value = serde_json::from_str(text)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── request-body construction (no network) ──

    #[test]
    fn build_chat_body_has_openai_compatible_shape() {
        let body = build_chat_body("model-x", "be strict", "do the task", 0.7, 4096);
        assert_eq!(body["model"], "model-x");
        assert_eq!(body["temperature"], 0.7);
        assert_eq!(body["max_tokens"], 4096);

        let msgs = body["messages"].as_array().expect("messages array");
        assert_eq!(msgs.len(), 2, "system + user messages");
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "be strict");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "do the task");
    }

    #[test]
    fn build_chat_body_respects_temperature_and_tokens() {
        let body = build_chat_body("m", "s", "u", 0.2, 512);
        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["max_tokens"], 512);
    }

    // ── JSON extraction ──

    #[test]
    fn extract_json_strips_surrounding_text() {
        let raw = "Here is the result:\n{\"a\": 1, \"b\": [2]}\nHope this helps.";
        assert_eq!(extract_json(raw).unwrap(), json!({"a": 1, "b": [2]}));
    }

    #[test]
    fn extract_json_extracts_array() {
        let raw = "Output: [1, 2, 3] done";
        assert_eq!(extract_json(raw).unwrap(), json!([1, 2, 3]));
    }

    #[test]
    fn extract_json_plain_json_passes_through() {
        let raw = r#"{"a":{"b":2}}"#;
        assert_eq!(extract_json(raw).unwrap()["a"]["b"], 2);
    }

    #[test]
    fn extract_json_invalid_input_errors() {
        assert!(extract_json("no json here").is_err());
    }

    // ── response parsing ──

    #[test]
    fn parse_chat_content_extracts_assistant_message() {
        let text = r#"{"choices":[{"message":{"content":"the answer"}}]}"#;
        assert_eq!(parse_chat_content(text).unwrap(), "the answer");
    }

    #[test]
    fn parse_chat_content_missing_choices_yields_empty_string() {
        assert_eq!(parse_chat_content(r#"{"choices":[]}"#).unwrap(), "");
        assert_eq!(parse_chat_content(r#"{}"#).unwrap(), "");
    }

    #[test]
    fn parse_chat_content_invalid_json_errors() {
        assert!(parse_chat_content("not json").is_err());
    }

    // ── loopback mock HTTP (no external network) ──

    /// Spawn a tiny HTTP/1.1 server on loopback that records the raw request
    /// and replies with a fixed status line + JSON body.
    async fn spawn_mock_server(
        status_line: &'static str,
        body: &'static str,
    ) -> (
        String,
        tokio::task::JoinHandle<()>,
        std::sync::Arc<tokio::sync::Mutex<Vec<u8>>>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let captured = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<u8>::new()));
        let cap = captured.clone();

        let handle = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept connection");
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.expect("read request");
            cap.lock().await.extend_from_slice(&buf[..n]);
            let resp = format!(
                "{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status_line,
                body.len(),
                body
            );
            sock.write_all(resp.as_bytes()).await.expect("write response");
            let _ = sock.shutdown().await;
        });

        (format!("http://{}", addr), handle, captured)
    }

    #[tokio::test]
    async fn chat_posts_to_mock_server_and_parses_content() {
        let (base, handle, captured) =
            spawn_mock_server("HTTP/1.1 200 OK", r#"{"choices":[{"message":{"content":"mock reply"}}]}"#).await;

        let client = reqwest::Client::new();
        let out = chat(&client, &base, "secret-key", "mock-model", "sys", "usr")
            .await
            .expect("chat should succeed");
        assert_eq!(out, "mock reply");

        handle.await.expect("server task finished");

        let req = String::from_utf8_lossy(&captured.lock().await).to_lowercase();
        assert!(
            req.starts_with("post /chat/completions http/1.1"),
            "unexpected request line: {req}"
        );
        assert!(req.contains("authorization: bearer secret-key"), "missing auth header");
        assert!(req.contains("\"model\":\"mock-model\""), "missing model in body");
        assert!(req.contains("\"role\":\"system\""), "missing system message");
        assert!(req.contains("\"temperature\":0.7"), "missing temperature");
    }

    #[tokio::test]
    async fn chat_non_success_status_returns_error() {
        let (base, handle, _captured) =
            spawn_mock_server("HTTP/1.1 500 Internal Server Error", r#"{"error":"boom"}"#).await;

        let client = reqwest::Client::new();
        let err = chat(&client, &base, "k", "m", "s", "u")
            .await
            .expect_err("chat should fail on 500");
        assert!(err.to_string().contains("500"), "unexpected error: {err}");

        handle.await.expect("server task finished");
    }
}
