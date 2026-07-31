use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Lifecycle of a durable error record.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorLifecycle {
    /// The failure was seen but has not been reproduced.
    #[default]
    Observed,
    /// The failure has a stable reproduction.
    Reproduced,
    /// A matching execution succeeded after the failure.
    Fixed,
    /// The fix and regression test were verified for a version.
    Verified,
    /// A verified failure fingerprint appeared again.
    Regressed,
}

/// Verified fix evidence bound to an error fingerprint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedFix {
    /// Git commit containing the fix.
    pub commit: String,
    /// Pull request identifier when available.
    #[serde(default)]
    pub pull_request: Option<String>,
    /// Regression test that guards the fix.
    pub regression_test: String,
    /// Safe mitigation that may be applied before execution.
    #[serde(default)]
    pub mitigation: Option<String>,
}

/// Version history used to replay verified failures after upgrades.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayMetadata {
    /// Versions where the fingerprint was observed.
    #[serde(default)]
    pub observed_versions: Vec<String>,
    /// Versions where the fix passed verification.
    #[serde(default)]
    pub verified_versions: Vec<String>,
    /// Most recent version associated with this record.
    #[serde(default)]
    pub latest_version: String,
    /// Number of verification replays.
    #[serde(default)]
    pub replay_count: u32,
}

/// One durable error and its prevention evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorRecord {
    /// Stable deterministic fingerprint.
    pub fingerprint: String,
    /// Forge capability that failed.
    pub capability: String,
    /// Coarse error taxonomy.
    pub error_class: String,
    /// Normalized error text.
    pub normalized_error: String,
    /// Canonical JSON execution context.
    pub context_signature: String,
    /// Current error lifecycle.
    #[serde(default)]
    pub lifecycle: ErrorLifecycle,
    /// Priority, increased when a verified error regresses.
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Number of recurrences after verification.
    #[serde(default)]
    pub recurrence_count: u32,
    /// Verified fix evidence.
    #[serde(default)]
    pub fix: Option<VerifiedFix>,
    /// Upgrade replay metadata.
    #[serde(default)]
    pub replay: ReplayMetadata,
    /// First observation timestamp.
    #[serde(default)]
    pub first_seen: u64,
    /// Last update timestamp.
    #[serde(default)]
    pub updated_at: u64,
}

/// Failure input used to produce and update a fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorObservation {
    /// Forge capability that failed.
    pub capability: String,
    /// Coarse error taxonomy.
    pub error_class: String,
    /// Raw error text.
    pub error: String,
    /// Execution context used for matching.
    pub context: Value,
    /// Forge or component version.
    pub version: String,
    /// Observation timestamp.
    pub observed_at: u64,
}

impl ErrorObservation {
    /// Compute the stable fingerprint for this observation.
    pub fn fingerprint(&self) -> String {
        fingerprint(
            &self.capability,
            &self.error_class,
            &normalize_error(&self.error),
            &canonical_json(&self.context),
        )
    }
}

/// Result of checking the error knowledge base before execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum GateDecision {
    /// No known error prevents execution.
    Allow,
    /// An unresolved matching error prevents execution.
    Block {
        /// Matching fingerprint.
        fingerprint: String,
        /// Human-readable reason.
        reason: String,
    },
    /// A verified mitigation should run before execution.
    ApplyKnownMitigation {
        /// Matching fingerprint.
        fingerprint: String,
        /// Verified mitigation instruction.
        mitigation: String,
    },
}

/// Durable local error knowledge base.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorKnowledgeBase {
    #[serde(default)]
    records: BTreeMap<String, ErrorRecord>,
}

impl ErrorKnowledgeBase {
    /// Load records from a JSON array or an empty missing file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let records: Vec<ErrorRecord> = serde_json::from_str(&fs::read_to_string(path)?)?;
        Ok(Self {
            records: records
                .into_iter()
                .map(|record| (record.fingerprint.clone(), record))
                .collect(),
        })
    }

    /// Persist records as stable pretty JSON.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let records: Vec<_> = self.records.values().collect();
        fs::write(path, serde_json::to_vec_pretty(&records)?)?;
        Ok(())
    }

    /// Return one record by fingerprint.
    pub fn get(&self, fingerprint: &str) -> Option<&ErrorRecord> {
        self.records.get(fingerprint)
    }

    /// Record an observation and return its fingerprint.
    pub fn observe(&mut self, observation: ErrorObservation) -> String {
        let normalized_error = normalize_error(&observation.error);
        let context_signature = canonical_json(&observation.context);
        let fingerprint = fingerprint(
            &observation.capability,
            &observation.error_class,
            &normalized_error,
            &context_signature,
        );
        let record = self.records.entry(fingerprint.clone()).or_insert_with(|| ErrorRecord {
            fingerprint: fingerprint.clone(),
            capability: observation.capability.clone(),
            error_class: observation.error_class.clone(),
            normalized_error,
            context_signature,
            lifecycle: ErrorLifecycle::Observed,
            priority: default_priority(),
            recurrence_count: 0,
            fix: None,
            replay: ReplayMetadata::default(),
            first_seen: observation.observed_at,
            updated_at: observation.observed_at,
        });
        if record.lifecycle == ErrorLifecycle::Verified {
            record.lifecycle = ErrorLifecycle::Regressed;
            record.priority = record.priority.saturating_add(1);
            record.recurrence_count = record.recurrence_count.saturating_add(1);
        }
        push_unique(&mut record.replay.observed_versions, &observation.version);
        record.replay.latest_version = observation.version;
        record.updated_at = observation.observed_at;
        fingerprint
    }

    /// Mark a fingerprint as reproducible.
    pub fn mark_reproduced(&mut self, fingerprint: &str) -> Result<()> {
        self.transition(fingerprint, ErrorLifecycle::Reproduced)
    }

    /// Attach fix evidence and mark a fingerprint fixed.
    pub fn mark_fixed(&mut self, fingerprint: &str, fix: VerifiedFix) -> Result<()> {
        if fix.commit.trim().is_empty() || fix.regression_test.trim().is_empty() {
            bail!("verified fix requires commit and regression_test");
        }
        let record = self
            .records
            .get_mut(fingerprint)
            .ok_or_else(|| anyhow::anyhow!("unknown error fingerprint"))?;
        if !matches!(record.lifecycle, ErrorLifecycle::Reproduced | ErrorLifecycle::Regressed) {
            bail!("error must be reproduced or regressed before attaching a fix");
        }
        record.fix = Some(fix);
        record.lifecycle = ErrorLifecycle::Fixed;
        Ok(())
    }

    /// Verify a fixed fingerprint for a component version.
    pub fn mark_verified(&mut self, fingerprint: &str, version: &str, timestamp: u64) -> Result<()> {
        let record = self
            .records
            .get_mut(fingerprint)
            .ok_or_else(|| anyhow::anyhow!("unknown error fingerprint"))?;
        if record.lifecycle != ErrorLifecycle::Fixed || record.fix.is_none() {
            bail!("error must have fix evidence before verification");
        }
        record.lifecycle = ErrorLifecycle::Verified;
        push_unique(&mut record.replay.verified_versions, version);
        record.replay.latest_version = version.to_string();
        record.replay.replay_count = record.replay.replay_count.saturating_add(1);
        record.updated_at = timestamp;
        Ok(())
    }

    /// Resolve only the record whose fingerprint matches a successful execution.
    pub fn resolve_success(&mut self, observation: &ErrorObservation, version: &str, timestamp: u64) {
        let fingerprint = observation.fingerprint();
        if let Some(record) = self.records.get_mut(&fingerprint) {
            if matches!(record.lifecycle, ErrorLifecycle::Observed | ErrorLifecycle::Reproduced) {
                record.lifecycle = ErrorLifecycle::Fixed;
                record.replay.latest_version = version.to_string();
                record.updated_at = timestamp;
            }
        }
    }

    /// Resolve exactly one explicitly supplied fingerprint after a successful replay.
    pub fn resolve_fingerprint_success(&mut self, fingerprint: &str, version: &str, timestamp: u64) -> Result<()> {
        let record = self
            .records
            .get_mut(fingerprint)
            .ok_or_else(|| anyhow::anyhow!("unknown error fingerprint"))?;
        if matches!(record.lifecycle, ErrorLifecycle::Observed | ErrorLifecycle::Reproduced) {
            record.lifecycle = ErrorLifecycle::Fixed;
            record.replay.latest_version = version.to_string();
            record.updated_at = timestamp;
        }
        Ok(())
    }

    /// Check whether a known matching error should alter execution.
    pub fn pre_execution(&self, capability: &str, context: &Value) -> GateDecision {
        let context_signature = canonical_json(context);
        let matching = self
            .records
            .values()
            .filter(|record| record.capability == capability && record.context_signature == context_signature)
            .max_by_key(|record| record.priority);
        match matching {
            Some(record)
                if matches!(
                    record.lifecycle,
                    ErrorLifecycle::Observed | ErrorLifecycle::Reproduced | ErrorLifecycle::Regressed
                ) =>
            {
                GateDecision::Block {
                    fingerprint: record.fingerprint.clone(),
                    reason: format!("known unresolved error: {}", record.normalized_error),
                }
            }
            Some(record) if record.lifecycle == ErrorLifecycle::Verified => record
                .fix
                .as_ref()
                .and_then(|fix| fix.mitigation.as_ref())
                .map(|mitigation| GateDecision::ApplyKnownMitigation {
                    fingerprint: record.fingerprint.clone(),
                    mitigation: mitigation.clone(),
                })
                .unwrap_or(GateDecision::Allow),
            _ => GateDecision::Allow,
        }
    }

    fn transition(&mut self, fingerprint: &str, target: ErrorLifecycle) -> Result<()> {
        let record = self
            .records
            .get_mut(fingerprint)
            .ok_or_else(|| anyhow::anyhow!("unknown error fingerprint"))?;
        let allowed = matches!(
            (record.lifecycle, target),
            (ErrorLifecycle::Observed, ErrorLifecycle::Reproduced)
                | (ErrorLifecycle::Regressed, ErrorLifecycle::Reproduced)
        );
        if !allowed {
            bail!("invalid error lifecycle transition");
        }
        record.lifecycle = target;
        Ok(())
    }
}

fn default_priority() -> u32 {
    1
}

fn normalize_error(error: &str) -> String {
    error
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let sorted: BTreeMap<_, _> = object.iter().collect();
            let body = sorted
                .into_iter()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical_json(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
        Value::Array(values) => format!("[{}]", values.iter().map(canonical_json).collect::<Vec<_>>().join(",")),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn fingerprint(capability: &str, error_class: &str, error: &str, context: &str) -> String {
    let canonical = format!("{capability}\u{1f}{error_class}\u{1f}{error}\u{1f}{context}");
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in canonical.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !value.is_empty() && !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}
