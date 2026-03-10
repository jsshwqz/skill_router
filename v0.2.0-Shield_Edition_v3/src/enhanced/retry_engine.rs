use crate::models::{SkillMetadata, Config, SubTask, TaskStatus, SubTaskResult};
use anyhow::{Result, anyhow};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum RetryStrategy {
    Fixed(Duration),
    Exponential { base: Duration, max: Duration },
    Immediate,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub strategy: RetryStrategy,
    pub retryable_errors: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            strategy: RetryStrategy::Exponential {
                base: Duration::from_millis(100),
                max: Duration::from_secs(30),
            },
            retryable_errors: vec![
                "timeout".to_string(),
                "connection refused".to_string(),
                "temporary".to_string(),
                "rate limit".to_string(),
                "503".to_string(),
                "502".to_string(),
            ],
        }
    }
}

pub struct RetryEngine {
    config: RetryConfig,
    fallback_chain: VecDeque<String>,
}

impl RetryEngine {
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            fallback_chain: VecDeque::new(),
        }
    }
    
    pub fn with_fallbacks(fallbacks: Vec<String>) -> Self {
        let mut chain = VecDeque::new();
        for f in fallbacks {
            chain.push_back(f);
        }
        Self {
            config: RetryConfig::default(),
            fallback_chain: chain,
        }
    }
    
    pub fn add_fallback(&mut self, skill_name: String) {
        self.fallback_chain.push_back(skill_name);
    }
    
    pub async fn execute_with_retry<F, Fut>(
        &self,
        skill: &SkillMetadata,
        mut executor: F,
    ) -> Result<SubTaskResult>
    where
        F: FnMut(&SkillMetadata) -> Fut,
        Fut: std::future::Future<Output = Result<SubTaskResult>>,
    {
        let mut last_error: Option<String> = None;
        let start = Instant::now();
        
        for attempt in 0..=self.config.max_retries {
            match executor(skill).await {
                Ok(result) if result.status == TaskStatus::Completed => {
                    return Ok(result);
                }
                Ok(result) => {
                    if let Some(err) = &result.error {
                        if !self.is_retryable(err) {
                            return Ok(result);
                        }
                        last_error = Some(err.clone());
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    if !self.is_retryable(&err_msg) {
                        return Err(e);
                    }
                    last_error = Some(err_msg);
                }
            }
            
            if attempt < self.config.max_retries {
                let delay = self.calculate_delay(attempt);
                tokio::time::sleep(delay).await;
            }
        }
        
        Ok(SubTaskResult {
            subtask_id: format!("retry_{}", skill.name),
            status: TaskStatus::Failed,
            output: None,
            error: last_error,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
    
    pub async fn execute_with_fallback<F, Fut>(
        &mut self,
        primary_skill: &SkillMetadata,
        registry: &crate::models::Registry,
        mut executor: F,
    ) -> Result<SubTaskResult>
    where
        F: FnMut(&SkillMetadata) -> Fut,
        Fut: std::future::Future<Output = Result<SubTaskResult>>,
    {
        let result = self.execute_with_retry(primary_skill, &mut executor).await?;
        
        if result.status == TaskStatus::Completed {
            return Ok(result);
        }
        
        for fallback_name in self.fallback_chain.iter() {
            if let Some(fallback_skill) = registry.skills.get(fallback_name) {
                let fallback_result = self.execute_with_retry(fallback_skill, &mut executor).await?;
                if fallback_result.status == TaskStatus::Completed {
                    return Ok(fallback_result);
                }
            }
        }
        
        Ok(result)
    }
    
    fn is_retryable(&self, error: &str) -> bool {
        let error_lower = error.to_lowercase();
        self.config.retryable_errors.iter().any(|e| {
            error_lower.contains(&e.to_lowercase())
        })
    }
    
    fn calculate_delay(&self, attempt: u32) -> Duration {
        match &self.config.strategy {
            RetryStrategy::Fixed(d) => *d,
            RetryStrategy::Exponential { base, max } => {
                let multiplier = 2u32.saturating_pow(attempt);
                let delay = base.saturating_mul(multiplier);
                delay.min(*max)
            }
            RetryStrategy::Immediate => Duration::ZERO,
        }
    }
}

pub struct CircuitBreaker {
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            failure_threshold,
            success_threshold: 2,
            timeout,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
        }
    }
    
    pub fn is_available(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last) = self.last_failure {
                    if last.elapsed() >= self.timeout {
                        self.state = CircuitState::HalfOpen;
                        self.success_count = 0;
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }
    
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                }
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }
    
    pub fn record_failure(&mut self) {
        self.last_failure = Some(Instant::now());
        
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
            }
            CircuitState::Open => {}
        }
    }
}