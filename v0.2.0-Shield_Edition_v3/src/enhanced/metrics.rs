use crate::models::{Metrics, SkillMetrics};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct MetricPoint {
    pub name: String,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
    pub labels: HashMap<String, String>,
}

pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, u64>>>,
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<f64>>>>,
    skill_metrics: Arc<RwLock<HashMap<String, SkillMetrics>>>,
    start_time: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            skill_metrics: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }
    
    pub async fn increment_counter(&self, name: &str, delta: u64) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += delta;
    }
    
    pub async fn decrement_counter(&self, name: &str, delta: u64) {
        let mut counters = self.counters.write().await;
        let entry = counters.entry(name.to_string()).or_insert(0);
        *entry = entry.saturating_sub(delta);
    }
    
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }
    
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write().await;
        histograms.entry(name.to_string()).or_insert_with(Vec::new).push(value);
    }
    
    pub async fn record_task_start(&self, task_id: &str) {
        self.increment_counter("tasks_started", 1).await;
        self.set_gauge(&format!("task_{}_active", task_id), 1.0).await;
    }
    
    pub async fn record_task_complete(&self, task_id: &str, duration_ms: u64, success: bool) {
        self.increment_counter("tasks_completed", 1).await;
        self.set_gauge(&format!("task_{}_active", task_id), 0.0).await;
        self.record_histogram("task_duration_ms", duration_ms as f64).await;
        
        if success {
            self.increment_counter("tasks_successful", 1).await;
        } else {
            self.increment_counter("tasks_failed", 1).await;
        }
    }
    
    pub async fn record_skill_execution(
        &self,
        skill_name: &str,
        duration_ms: u64,
        success: bool,
    ) {
        let mut skills = self.skill_metrics.write().await;
        
        let entry = skills.entry(skill_name.to_string()).or_insert_with(|| SkillMetrics {
            executions: 0,
            successes: 0,
            failures: 0,
            avg_latency_ms: 0.0,
            last_used: Utc::now().to_rfc3339(),
        });
        
        entry.executions += 1;
        if success {
            entry.successes += 1;
        } else {
            entry.failures += 1;
        }
        
        let total = (entry.successes + entry.failures) as f64;
        entry.avg_latency_ms = (entry.avg_latency_ms * (total - 1.0) + duration_ms as f64) / total;
        entry.last_used = Utc::now().to_rfc3339();
        
        self.record_histogram(&format!("skill_{}_latency_ms", skill_name), duration_ms as f64).await;
    }
    
    pub async fn record_cache_hit(&self) {
        self.increment_counter("cache_hits", 1).await;
    }
    
    pub async fn record_cache_miss(&self) {
        self.increment_counter("cache_misses", 1).await;
    }
    
    pub async fn get_summary(&self) -> Metrics {
        let counters = self.counters.read().await;
        let skills = self.skill_metrics.read().await;
        
        let total = counters.get("tasks_completed").copied().unwrap_or(0);
        let successful = counters.get("tasks_successful").copied().unwrap_or(0);
        let failed = counters.get("tasks_failed").copied().unwrap_or(0);
        let cache_hits = counters.get("cache_hits").copied().unwrap_or(0);
        let cache_misses = counters.get("cache_misses").copied().unwrap_or(0);
        
        Metrics {
            total_tasks: total,
            successful_tasks: successful,
            failed_tasks: failed,
            cache_hits,
            cache_misses,
            avg_task_duration_ms: self.calculate_avg_duration().await,
            skills_executed: skills.clone(),
        }
    }
    
    async fn calculate_avg_duration(&self) -> f64 {
        let histograms = self.histograms.read().await;
        if let Some(durations) = histograms.get("task_duration_ms") {
            if !durations.is_empty() {
                return durations.iter().sum::<f64>() / durations.len() as f64;
            }
        }
        0.0
    }
    
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
    
    pub async fn export_prometheus(&self) -> String {
        let counters = self.counters.read().await;
        let gauges = self.gauges.read().await;
        let skills = self.skill_metrics.read().await;
        
        let mut output = String::new();
        
        output.push_str("# HELP skill_router_tasks_total Total number of tasks processed\n");
        output.push_str("# TYPE skill_router_tasks_total counter\n");
        if let Some(v) = counters.get("tasks_completed") {
            output.push_str(&format!("skill_router_tasks_total {}\n", v));
        }
        
        output.push_str("\n# HELP skill_router_tasks_successful Total successful tasks\n");
        output.push_str("# TYPE skill_router_tasks_successful counter\n");
        if let Some(v) = counters.get("tasks_successful") {
            output.push_str(&format!("skill_router_tasks_successful {}\n", v));
        }
        
        output.push_str("\n# HELP skill_router_tasks_failed Total failed tasks\n");
        output.push_str("# TYPE skill_router_tasks_failed counter\n");
        if let Some(v) = counters.get("tasks_failed") {
            output.push_str(&format!("skill_router_tasks_failed {}\n", v));
        }
        
        output.push_str("\n# HELP skill_router_cache_hits Cache hit count\n");
        output.push_str("# TYPE skill_router_cache_hits counter\n");
        if let Some(v) = counters.get("cache_hits") {
            output.push_str(&format!("skill_router_cache_hits {}\n", v));
        }
        
        output.push_str("\n# HELP skill_router_uptime_seconds Service uptime\n");
        output.push_str("# TYPE skill_router_uptime_seconds gauge\n");
        output.push_str(&format!("skill_router_uptime_seconds {}\n", self.uptime_seconds()));
        
        output.push_str("\n# HELP skill_executions_total Total skill executions\n");
        output.push_str("# TYPE skill_executions_total counter\n");
        for (name, metrics) in skills.iter() {
            output.push_str(&format!(
                "skill_executions_total{{skill=\"{}\"}} {}\n",
                name, metrics.executions
            ));
        }
        
        output
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}