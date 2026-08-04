use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct BreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout_secs: u64,
    pub half_open_max_calls: u32,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_secs: 60,
            half_open_max_calls: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerStatus {
    pub state: String,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure_time: Option<u64>,
    pub last_success_time: Option<u64>,
    pub consecutive_failures: u32,
}

#[derive(Debug)]
pub struct CircuitBreaker {
    config: BreakerConfig,
    data_path: PathBuf,
    state: BreakerState,
    status: BreakerStatus,
    half_open_calls: u32,
    last_state_change: Instant,
}

impl CircuitBreaker {
    pub fn new(config: BreakerConfig, data_path: &PathBuf) -> Self {
        let status = Self::load_status(data_path);
        Self {
            config,
            data_path: data_path.clone(),
            state: BreakerState::Closed,
            status,
            half_open_calls: 0,
            last_state_change: Instant::now(),
        }
    }

    fn load_status(data_path: &PathBuf) -> BreakerStatus {
        let file = data_path.join("circuit_breaker.json");
        if file.exists() {
            match fs::read_to_string(&file) {
                Ok(content) => match serde_json::from_str::<BreakerStatus>(&content) {
                    Ok(s) => s,
                    Err(_) => BreakerStatus {
                        state: "closed".to_string(),
                        failure_count: 0,
                        success_count: 0,
                        last_failure_time: None,
                        last_success_time: None,
                        consecutive_failures: 0,
                    },
                },
                Err(_) => BreakerStatus {
                    state: "closed".to_string(),
                    failure_count: 0,
                    success_count: 0,
                    last_failure_time: None,
                    last_success_time: None,
                    consecutive_failures: 0,
                },
            }
        } else {
            BreakerStatus {
                state: "closed".to_string(),
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
                last_success_time: None,
                consecutive_failures: 0,
            }
        }
    }

    fn save(&self) -> Result<()> {
        fs::create_dir_all(&self.data_path)?;
        fs::write(
            self.data_path.join("circuit_breaker.json"),
            serde_json::to_string_pretty(&self.status)?,
        )?;
        Ok(())
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn allow_call(&mut self) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::Open => {
                if let Some(last_failure) = self.status.last_failure_time {
                    let elapsed = Self::now_secs() - last_failure;
                    if elapsed >= self.config.recovery_timeout_secs {
                        self.state = BreakerState::HalfOpen;
                        self.half_open_calls = 0;
                        self.last_state_change = Instant::now();
                        tracing::info!("Circuit breaker recovered to HalfOpen");
                    }
                    elapsed >= self.config.recovery_timeout_secs
                } else {
                    false
                }
            }
            _ => {
                self.status.failure_count = 0;
                true
            }
        }
    }

    pub fn record_failure(&mut self) {
        self.status.failure_count += 1;
        self.status.consecutive_failures += 1;
        self.status.last_failure_time = Some(Self::now_secs());
        match self.state {
            BreakerState::Closed => {
                if self.status.consecutive_failures >= self.config.failure_threshold {
                    self.state = BreakerState::Open;
                    tracing::warn!(
                        "Circuit breaker: Closed - (threshold {})",
                        self.config.failure_threshold
                    );
                }
            }
            BreakerState::HalfOpen => {
                self.state = BreakerState::Open;
                self.last_state_change = Instant::now();
                tracing::warn!("Circuit breaker: HalfOpen - (test call failed)");
            }
            _ => {}
        }
        let _ = self.save();
    }

    pub fn record_success(&mut self) {
        self.status.success_count += 1;
        self.status.failure_count = 0;
        self.status.consecutive_failures = 0;
        self.status.last_success_time = Some(Self::now_secs());
        match self.state {
            BreakerState::HalfOpen => {
                self.state = BreakerState::Closed;
                self.last_state_change = Instant::now();
                tracing::info!("Circuit breaker: HalfOpen -> Closed (success)");
            }
            _ => {}
        }
        let _ = self.save();
    }

    pub fn reset(&mut self) {
        self.state = BreakerState::Closed;
        self.status.failure_count = 0;
        self.status.success_count = 0;
        self.status.consecutive_failures = 0;
        self.status.last_failure_time = None;
        self.status.last_success_time = None;
        self.half_open_calls = 0;
        let _ = self.save();
    }

    pub fn status_report(&self) -> Value {
        let next_recovery = if let Some(last_failure) = self.status.last_failure_time {
            Some(last_failure + self.config.recovery_timeout_secs - Self::now_secs())
        } else {
            None
        };
        json!({
            "state": format!("{:?}", self.state),
            "failure_count": self.status.failure_count,
            "success_count": self.status.success_count,
            "consecutive_failures": self.status.consecutive_failures,
            "failure_threshold": self.config.failure_threshold,
            "recovery_timeout_secs": self.config.recovery_timeout_secs,
            "next_recovery_in_secs": next_recovery,
            "last_failure_time": self.status.last_failure_time,
            "last_success_time": self.status.last_success_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breaker_open() {
        let tmp = std::env::temp_dir().join("aion-cb-test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let config = BreakerConfig {
            failure_threshold: 3,
            recovery_timeout_secs: 60,
            half_open_max_calls: 2,
        };
        let mut cb = CircuitBreaker::new(config, &tmp);
        assert!(cb.allow_call());
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.allow_call());
        let status = cb.status_report();
        assert_eq!(status["state"], "Open");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_breaker_half_open_success() {
        let tmp = std::env::temp_dir().join("aion-cb-test2");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let config = BreakerConfig {
            failure_threshold: 2,
            recovery_timeout_secs: 60,
            half_open_max_calls: 2,
        };
        let mut cb = CircuitBreaker::new(config, &tmp);
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.allow_call());
        cb.record_success();
        assert!(cb.allow_call());
        let _ = fs::remove_dir_all(&tmp);
    }
}
