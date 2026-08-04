use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub name: String,
    pub consecutive_failures: u32,
    pub last_failure_time: Option<u64>,
    pub last_success_time: Option<u64>,
    pub total_failures: u32,
    pub total_successes: u32,
}

impl EngineStatus {
    pub fn is_healthy(&self) -> bool {
        self.consecutive_failures < 3
    }

    pub fn is_degraded(&self) -> bool {
        self.consecutive_failures >= 3 && self.consecutive_failures < 5
    }

    pub fn is_unhealthy(&self) -> bool {
        self.consecutive_failures >= 5
    }
}

#[derive(Debug)]
pub struct HealthManager {
    engines: HashMap<String, EngineStatus>,
    data_path: PathBuf,
}

impl HealthManager {
    pub fn new(data_path: &PathBuf) -> Self {
        let mut mgr = HealthManager {
            engines: HashMap::new(),
            data_path: data_path.clone(),
        };
        let _ = mgr.load();
        mgr
    }

    pub fn record_success(&mut self, engine: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let status = self.engines.entry(engine.to_string()).or_insert_with(|| EngineStatus {
            name: engine.to_string(),
            consecutive_failures: 0,
            last_failure_time: None,
            last_success_time: Some(now),
            total_failures: 0,
            total_successes: 0,
        });
        status.consecutive_failures = 0;
        status.total_successes += 1;
        status.last_success_time = Some(now);
        let _ = self.save();
    }

    pub fn record_failure(&mut self, engine: &str, _error: &str, _latency_ms: f64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let status = self.engines.entry(engine.to_string()).or_insert_with(|| EngineStatus {
            name: engine.to_string(),
            consecutive_failures: 0,
            last_failure_time: None,
            last_success_time: None,
            total_failures: 0,
            total_successes: 0,
        });
        status.consecutive_failures += 1;
        status.total_failures += 1;
        status.last_failure_time = Some(now);
        let _ = self.save();
    }

    pub fn get_status(&self, engine: &str) -> Option<&EngineStatus> {
        self.engines.get(engine)
    }

    pub fn all_statuses(&self) -> &HashMap<String, EngineStatus> {
        &self.engines
    }

    pub fn health_report(&self) -> Value {
        let engines: Vec<Value> = self
            .engines
            .iter()
            .map(|(name, status)| {
                json!({
                    "name": name,
                    "state": if status.is_unhealthy() { "unhealthy" } else if status.is_degraded() { "degraded" } else { "healthy" },
                    "consecutive_failures": status.consecutive_failures,
                    "total_failures": status.total_failures,
                    "total_successes": status.total_successes,
                    "last_failure_time": status.last_failure_time,
                    "last_success_time": status.last_success_time,
                })
            })
            .collect();
        json!({
            "engines": engines,
            "total_engines": engines.len(),
            "unhealthy_count": engines.iter().filter(|e| e["state"] == "unhealthy").count(),
            "degraded_count": engines.iter().filter(|e| e["state"] == "degraded").count(),
            "healthy_count": engines.iter().filter(|e| e["state"] == "healthy").count(),
        })
    }

    fn save(&self) -> Result<()> {
        let file = self.data_path.join("engine_health.json");
        fs::create_dir_all(&self.data_path)?;
        let data: HashMap<String, EngineStatus> = self.engines.clone();
        fs::write(file, serde_json::to_string_pretty(&data)?)?;
        Ok(())
    }

    fn load(&mut self) -> Result<()> {
        let file = self.data_path.join("engine_health.json");
        if file.exists() {
            let content = fs::read_to_string(&file)?;
            let data: HashMap<String, EngineStatus> = serde_json::from_str(&content)?;
            self.engines = data;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_manager() {
        let tmp = std::env::temp_dir().join("aion-health-test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let mut mgr = HealthManager::new(&tmp);
        mgr.record_success("test-engine");
        let status = mgr.get_status("test-engine").unwrap();
        assert!(status.is_healthy());
        assert_eq!(status.consecutive_failures, 0);
        mgr.record_failure("test-engine", "test error", 100.0);
        assert!(mgr.get_status("test-engine").unwrap().is_healthy());
        mgr.record_failure("test-engine", "test error", 100.0);
        mgr.record_failure("test-engine", "test error", 100.0);
        assert!(mgr.get_status("test-engine").unwrap().is_degraded());
        let _ = fs::remove_dir_all(&tmp);
    }
}
