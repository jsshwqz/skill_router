use crate::models::SkillMetadata;
use chrono::{DateTime, Utc};

pub struct Lifecycle;

impl Lifecycle {
    pub fn decide(skill: &SkillMetadata) -> Option<String> {
        let usage = match &skill.usage {
            Some(u) => u,
            None => return None,
        };

        let last_used = match DateTime::parse_from_rfc3339(&usage.last_used) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => return None,
        };

        let now = Utc::now();
        let days_since_used = (now - last_used).num_days();
        
        let success_rate = if usage.total_calls > 0 {
            (usage.success_calls as f64) / (usage.total_calls as f64)
        } else {
            0.0
        };

        // Decision logic
        if days_since_used >= 180 {
            return Some("purge_candidate".to_string());
        }
        if days_since_used >= 90 {
            return Some("archive_candidate".to_string());
        }
        if usage.total_calls >= 15 && success_rate >= 0.95 {
            return Some("publish_candidate".to_string());
        }
        if usage.total_calls >= 8 {
            return Some("polish".to_string());
        }
        if usage.total_calls >= 3 {
            return Some("keep".to_string());
        }
        if usage.total_calls < 2 && days_since_used >= 30 {
            return Some("archive_candidate".to_string());
        }

        None
    }
}
