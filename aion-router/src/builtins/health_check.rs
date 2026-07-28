//! Health Check builtin skill
//!
//! Reads `.skill-router/cli_health.json` for per-engine health status
//! and `VERSION.json` (workspace root) for server version information.
//! Returns a unified health report covering all AI engines and the server itself.

use std::time::{Duration, Instant};

use anyhow::Result;
use reqwest::{redirect::Policy, StatusCode, Url};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use crate::{
    config::{configured_ai_endpoints, AiEndpoint, AiProtocol},
    security::AiSecurityReviewer,
};

use super::BuiltinSkill;

const LIVE_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// HealthCheck: reads cli_health.json and VERSION.json, returns unified status
pub struct HealthCheck;

#[async_trait::async_trait]
impl BuiltinSkill for HealthCheck {
    fn name(&self) -> &'static str {
        "health_check"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let live_probe_requested = context
            .context
            .get("live_probe")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // Determine workspace root from context or fall back to current dir
        let workspace_root = context
            .context
            .get("workspace")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let state_dir = workspace_root.join(".skill-router");

        // ── Read cli_health.json ────────────────────────────────────────
        let health_path = state_dir.join("cli_health.json");
        let engines = if health_path.exists() {
            match std::fs::read_to_string(&health_path) {
                Ok(content) => match serde_json::from_str::<Value>(&content) {
                    Ok(parsed) => parsed.get("engines").cloned().unwrap_or(Value::Null),
                    Err(e) => json!({ "error": format!("failed to parse cli_health.json: {}", e) }),
                },
                Err(e) => json!({ "error": format!("failed to read cli_health.json: {}", e) }),
            }
        } else {
            json!({ "error": "cli_health.json not found" })
        };

        // ── Extract per-engine status ───────────────────────────────────
        let engine_names = ["claude", "openai", "gemini", "local"];
        let mut engine_statuses = serde_json::Map::new();
        let mut all_healthy = true;

        for name in &engine_names {
            if let Some(engine_data) = engines.get(name) {
                let status = engine_data.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                let successes = engine_data.get("successes").and_then(|v| v.as_u64()).unwrap_or(0);
                let failures = engine_data.get("failures").and_then(|v| v.as_u64()).unwrap_or(0);
                let consecutive_failures = engine_data
                    .get("consecutive_failures")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let avg_latency_ms = engine_data
                    .get("avg_latency_ms")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let last_error_kind = engine_data.get("last_error_kind").cloned().unwrap_or(Value::Null);
                let last_updated_at = engine_data.get("last_updated_at").cloned().unwrap_or(Value::Null);
                let cooldown_until = engine_data.get("cooldown_until").cloned().unwrap_or(Value::Null);

                if status != "healthy" {
                    all_healthy = false;
                }

                engine_statuses.insert(
                    name.to_string(),
                    json!({
                        "status": status,
                        "successes": successes,
                        "failures": failures,
                        "consecutive_failures": consecutive_failures,
                        "avg_latency_ms": avg_latency_ms,
                        "last_error_kind": last_error_kind,
                        "last_updated_at": last_updated_at,
                        "cooldown_until": cooldown_until,
                        "status_basis": "historical_execution_telemetry",
                    }),
                );
            } else {
                all_healthy = false;
                engine_statuses.insert(
                    name.to_string(),
                    json!({
                        "status": "no_telemetry",
                        "configuration": engine_configuration(name),
                        "status_basis": "historical_execution_telemetry",
                    }),
                );
            }
        }

        // ── Read VERSION.json ───────────────────────────────────────────
        let version_path = workspace_root.join("VERSION.json");
        let server_version = if version_path.exists() {
            match std::fs::read_to_string(&version_path) {
                Ok(content) => match serde_json::from_str::<Value>(&content) {
                    Ok(parsed) => parsed,
                    Err(e) => json!({ "error": format!("failed to parse VERSION.json: {}", e) }),
                },
                Err(e) => json!({ "error": format!("failed to read VERSION.json: {}", e) }),
            }
        } else {
            // Fall back to the invoking Forge entrypoint version.
            json!({
                "version": _skill.metadata.version,
                "source": "skill_metadata"
            })
        };

        // ── Build overall status ────────────────────────────────────────
        let overall_status = if all_healthy { "healthy" } else { "degraded" };

        let live_probes = if live_probe_requested {
            probe_configured_endpoints().await
        } else {
            Value::Null
        };

        Ok(json!({
            "overall_status": overall_status,
            "status_basis": "historical_execution_telemetry",
            "live_probe_performed": live_probe_requested,
            "live_probes": live_probes,
            "notice": if live_probe_requested {
                "Historical telemetry is reported separately from explicit lightweight endpoint probes."
            } else {
                "Engine statuses summarize prior executions and are not live connectivity probes."
            },
            "engines": Value::Object(engine_statuses),
            "server_version": server_version,
            "component_version": {
                "name": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION"),
            },
            "health_file": health_path.to_string_lossy(),
            "timestamp": super::now_epoch_ms(),
        }))
    }
}

async fn probe_configured_endpoints() -> Value {
    let mut probes = serde_json::Map::new();
    for endpoint in configured_ai_endpoints() {
        probes.insert(endpoint.label.clone(), probe_endpoint(&endpoint).await);
    }
    Value::Object(probes)
}

async fn probe_endpoint(endpoint: &AiEndpoint) -> Value {
    let started = Instant::now();
    let probe_url = match probe_url(endpoint) {
        Ok(url) => url,
        Err(error_kind) => return probe_result(false, started, Some(error_kind)),
    };
    let client = match reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(LIVE_PROBE_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(_) => return probe_result(false, started, Some("client_build")),
    };
    let mut request = client.get(probe_url);
    if !endpoint.api_key.is_empty() {
        request = match endpoint.protocol {
            AiProtocol::AnthropicMessages => request
                .header("x-api-key", &endpoint.api_key)
                .header("anthropic-version", "2023-06-01"),
            AiProtocol::OpenAiChat if endpoint.label == "gemini" => request.header("x-goog-api-key", &endpoint.api_key),
            AiProtocol::OpenAiChat => request.bearer_auth(&endpoint.api_key),
        };
    }
    match request.send().await {
        Ok(response) if response.status().is_success() => probe_result(true, started, None),
        Ok(response) => probe_result(false, started, Some(status_error_kind(response.status()))),
        Err(error) if error.is_timeout() => probe_result(false, started, Some("timeout")),
        Err(error) if error.is_connect() => probe_result(false, started, Some("connect")),
        Err(_) => probe_result(false, started, Some("request")),
    }
}

fn probe_url(endpoint: &AiEndpoint) -> std::result::Result<Url, &'static str> {
    let mut url = Url::parse(&endpoint.base_url).map_err(|_| "invalid_url")?;
    if url.username() != "" || url.password().is_some() {
        return Err("embedded_credentials");
    }
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err("insecure_scheme");
    }
    if AiSecurityReviewer::is_private_network_url(url.as_str()) && !loopback {
        return Err("blocked_target");
    }
    url.set_query(None);
    url.set_fragment(None);
    let path = format!("{}/models", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn probe_result(reachable: bool, started: Instant, error_kind: Option<&str>) -> Value {
    json!({
        "configured": true,
        "reachable": reachable,
        "latency_ms": started.elapsed().as_millis() as u64,
        "error_kind": error_kind,
    })
}

fn status_error_kind(status: StatusCode) -> &'static str {
    match status {
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::TOO_MANY_REQUESTS => "rate_limited",
        status if status.is_redirection() => "redirect_blocked",
        _ => "http_status",
    }
}

fn engine_configuration(name: &str) -> &'static str {
    let variables: &[&str] = match name {
        "claude" => &["ANTHROPIC_API_KEY", "AION_HOST_AI_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "gemini" => &["GOOGLE_AI_API_KEY"],
        "local" => &["AI_BASE_URL", "AI_API_KEY", "AI_MODEL"],
        _ => &[],
    };
    if variables
        .iter()
        .any(|variable| std::env::var(variable).is_ok_and(|value| !value.trim().is_empty()))
    {
        "configured"
    } else {
        "not_configured"
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use aion_types::types::{ExecutionContext, PermissionSet, SkillDefinition, SkillMetadata, SkillSource};
    use serde_json::json;

    use reqwest::StatusCode;

    use crate::config::{AiEndpoint, AiProtocol};

    use super::{probe_url, status_error_kind, BuiltinSkill, HealthCheck};

    #[tokio::test]
    async fn reports_entrypoint_version_and_distinguishes_missing_telemetry() {
        let workspace = std::env::temp_dir().join(format!("aion-health-{}", uuid::Uuid::new_v4()));
        let skill = SkillDefinition {
            metadata: SkillMetadata {
                name: "health_check".to_string(),
                version: "9.8.7".to_string(),
                capabilities: vec!["health_check".to_string()],
                entrypoint: "builtin:health_check".to_string(),
                permissions: PermissionSet::default(),
                instruction: None,
                engine_capable: false,
            },
            root_dir: PathBuf::new(),
            source: SkillSource::Local,
        };
        let context = ExecutionContext::new("health", "health_check").with_context(json!({"workspace": workspace}));

        let report = HealthCheck.execute(&skill, &context).await.unwrap();

        assert_eq!(report["server_version"]["version"], "9.8.7");
        assert_eq!(report["server_version"]["source"], "skill_metadata");
        assert_eq!(report["component_version"]["name"], "aion-router");
        assert_eq!(report["engines"]["local"]["status"], "no_telemetry");
        assert_eq!(report["live_probe_performed"], false);
        assert!(report["live_probes"].is_null());
        assert!(
            report["engines"]["local"]["configuration"] == "configured"
                || report["engines"]["local"]["configuration"] == "not_configured"
        );
    }

    fn endpoint(base_url: &str) -> AiEndpoint {
        AiEndpoint {
            label: "local".to_string(),
            base_url: base_url.to_string(),
            api_key: "secret-sentinel".to_string(),
            model: "test-model".to_string(),
            protocol: AiProtocol::OpenAiChat,
        }
    }

    #[test]
    fn live_probe_url_enforces_ssrf_and_secret_boundaries() {
        let safe = probe_url(&endpoint("https://api.example.com/v1?token=secret#fragment")).unwrap();
        assert_eq!(safe.as_str(), "https://api.example.com/v1/models");
        assert!(!safe.as_str().contains("secret"));

        assert_eq!(
            probe_url(&endpoint("http://api.example.com/v1")),
            Err("insecure_scheme")
        );
        assert_eq!(probe_url(&endpoint("https://127.0.0.2/v1")), Err("blocked_target"));
        assert_eq!(
            probe_url(&endpoint("https://user:password@example.com/v1")),
            Err("embedded_credentials")
        );
        assert!(probe_url(&endpoint("http://127.0.0.1:11434/v1")).is_ok());
    }

    #[test]
    fn live_probe_exposes_stable_error_kinds_only() {
        assert_eq!(status_error_kind(StatusCode::UNAUTHORIZED), "unauthorized");
        assert_eq!(status_error_kind(StatusCode::FORBIDDEN), "forbidden");
        assert_eq!(status_error_kind(StatusCode::TOO_MANY_REQUESTS), "rate_limited");
        assert_eq!(status_error_kind(StatusCode::FOUND), "redirect_blocked");
        assert_eq!(status_error_kind(StatusCode::INTERNAL_SERVER_ERROR), "http_status");
    }
}
