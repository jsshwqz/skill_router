use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureSample {
    pub capability: String,
    pub error_class: String,
    pub reproduction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureCluster {
    pub key: String,
    pub samples: Vec<FailureSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionBudget {
    pub max_candidates: usize,
    pub max_gate_seconds: u64,
    pub max_changed_files: usize,
    pub max_diff_bytes: usize,
}

impl Default for EvolutionBudget {
    fn default() -> Self {
        Self {
            max_candidates: 3,
            max_gate_seconds: 900,
            max_changed_files: 12,
            max_diff_bytes: 128 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub mandatory: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayMetrics {
    pub passed: u32,
    pub failed: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateEvidence {
    pub id: String,
    pub worktree: PathBuf,
    pub changed_files: usize,
    pub diff_bytes: usize,
    pub gates: Vec<GateResult>,
    pub baseline: ReplayMetrics,
    pub candidate: ReplayMetrics,
}

impl CandidateEvidence {
    pub fn is_eligible(&self, budget: &EvolutionBudget) -> bool {
        self.changed_files <= budget.max_changed_files
            && self.diff_bytes <= budget.max_diff_bytes
            && self.gates.iter().all(|gate| !gate.mandatory || gate.passed)
            && self.candidate.failed <= self.baseline.failed
            && self.candidate.passed >= self.baseline.passed
    }

    pub fn fitness(&self) -> i64 {
        let fixed_failures = self.baseline.failed.saturating_sub(self.candidate.failed) as i64;
        let added_passes = self.candidate.passed.saturating_sub(self.baseline.passed) as i64;
        let optional_gates = self.gates.iter().filter(|gate| !gate.mandatory && gate.passed).count() as i64;
        let latency_penalty = self
            .candidate
            .duration_ms
            .saturating_sub(self.baseline.duration_ms)
            .div_ceil(100) as i64;
        fixed_failures * 10_000 + added_passes * 1_000 + optional_gates * 100
            - latency_penalty
            - self.changed_files as i64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionRequest {
    pub repository: PathBuf,
    pub worktree_root: PathBuf,
    pub dry_run: bool,
    #[serde(default)]
    pub budget: EvolutionBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchCandidate {
    pub id: String,
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GateSpec {
    Format,
    FocusedTest {
        package: String,
        #[serde(default)]
        filter: Option<String>,
    },
    Clippy {
        package: String,
    },
    DiffCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionOutcome {
    pub dry_run: bool,
    pub selected: Option<CandidateEvidence>,
    pub evaluated: Vec<CandidateEvidence>,
    pub rejected: Vec<String>,
}

pub struct EvolutionRunner;

impl EvolutionRunner {
    pub fn run(
        request: &EvolutionRequest,
        candidates: &[PatchCandidate],
        gates: &[GateSpec],
    ) -> Result<EvolutionOutcome> {
        request.validate()?;
        validate_candidates(candidates, &request.budget)?;
        if gates.is_empty() {
            bail!("at least one verification gate is required");
        }
        if request.dry_run {
            return Ok(EvolutionOutcome {
                dry_run: true,
                selected: None,
                evaluated: Vec::new(),
                rejected: Vec::new(),
            });
        }

        fs::create_dir_all(&request.worktree_root)?;
        let baseline = run_replay(&request.repository, gates, request.budget.max_gate_seconds)?;
        let mut evaluated = Vec::new();
        let mut rejected = Vec::new();

        for candidate in candidates {
            let worktree = request.worktree_root.join(safe_candidate_id(&candidate.id)?);
            if worktree.exists() {
                bail!("candidate worktree already exists: {}", worktree.display());
            }
            add_worktree(&request.repository, &worktree)?;
            let evaluation = evaluate_candidate(request, candidate, gates, &worktree, &baseline);
            match evaluation {
                Ok(evidence) => evaluated.push(evidence),
                Err(error) => rejected.push(format!("{}: {error}", candidate.id)),
            }
            remove_worktree(&request.repository, &worktree)?;
        }

        let selected = select_candidate(&evaluated, &request.budget).cloned();
        Ok(EvolutionOutcome {
            dry_run: false,
            selected,
            evaluated,
            rejected,
        })
    }
}

impl EvolutionRequest {
    pub fn validate(&self) -> Result<()> {
        let repository = canonical_existing_directory(&self.repository)?;
        let worktree_root = absolute_path(&self.worktree_root)?;
        let worktree_ancestor = nearest_existing_ancestor(&worktree_root)?;
        if worktree_ancestor.starts_with(&repository) {
            bail!("evolution worktrees must be outside the production repository");
        }
        if self.budget.max_candidates == 0
            || self.budget.max_gate_seconds == 0
            || self.budget.max_changed_files == 0
            || self.budget.max_diff_bytes == 0
        {
            bail!("evolution budgets must be greater than zero");
        }
        Ok(())
    }
}

pub fn cluster_failures(samples: Vec<FailureSample>) -> Vec<FailureCluster> {
    let mut clusters = BTreeMap::<String, Vec<FailureSample>>::new();
    for sample in samples {
        let key = format!("{}:{}", normalize(&sample.capability), normalize(&sample.error_class));
        clusters.entry(key).or_default().push(sample);
    }
    clusters
        .into_iter()
        .map(|(key, samples)| FailureCluster { key, samples })
        .collect()
}

pub fn select_candidate<'a>(
    candidates: &'a [CandidateEvidence],
    budget: &EvolutionBudget,
) -> Option<&'a CandidateEvidence> {
    candidates
        .iter()
        .filter(|candidate| candidate.is_eligible(budget))
        .max_by(|left, right| {
            left.fitness()
                .cmp(&right.fitness())
                .then_with(|| right.id.cmp(&left.id))
        })
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn canonical_existing_directory(path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()?;
    if !canonical.is_dir() {
        bail!("repository must be a directory");
    }
    Ok(canonical)
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn validate_candidates(candidates: &[PatchCandidate], budget: &EvolutionBudget) -> Result<()> {
    if candidates.is_empty() {
        bail!("at least one patch candidate is required");
    }
    if candidates.len() > budget.max_candidates {
        bail!("candidate count exceeds evolution budget");
    }
    for candidate in candidates {
        safe_candidate_id(&candidate.id)?;
        if candidate.patch.trim().is_empty() {
            bail!("candidate patch must not be empty");
        }
        if candidate.patch.len() > budget.max_diff_bytes {
            bail!("candidate patch exceeds diff budget");
        }
    }
    Ok(())
}

fn safe_candidate_id(id: &str) -> Result<&str> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("candidate id must contain only ASCII letters, digits, '-' or '_'");
    }
    Ok(id)
}

fn add_worktree(repository: &Path, worktree: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(worktree)
        .arg("HEAD")
        .current_dir(repository)
        .output()?;
    require_success("git worktree add", output)
}

fn remove_worktree(repository: &Path, worktree: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree)
        .current_dir(repository)
        .output()?;
    require_success("git worktree remove", output)
}

fn evaluate_candidate(
    request: &EvolutionRequest,
    candidate: &PatchCandidate,
    gates: &[GateSpec],
    worktree: &Path,
    baseline: &ReplayMetrics,
) -> Result<CandidateEvidence> {
    let patch_path = worktree.join(".aion-evolution.patch");
    fs::write(&patch_path, &candidate.patch)?;
    let check = Command::new("git")
        .args(["apply", "--check"])
        .arg(&patch_path)
        .current_dir(worktree)
        .output()?;
    require_success("git apply --check", check)?;
    let apply = Command::new("git")
        .arg("apply")
        .arg(&patch_path)
        .current_dir(worktree)
        .output()?;
    require_success("git apply", apply)?;
    fs::remove_file(&patch_path)?;

    let names = command_output(Command::new("git").args(["diff", "--name-only"]).current_dir(worktree))?;
    let diff = command_output(Command::new("git").args(["diff", "--binary"]).current_dir(worktree))?;
    let changed_files = names.lines().filter(|line| !line.trim().is_empty()).count();
    if changed_files > request.budget.max_changed_files || diff.len() > request.budget.max_diff_bytes {
        bail!("candidate exceeds changed-file or diff-size budget");
    }
    let (gate_results, replay) = run_gates(worktree, gates, request.budget.max_gate_seconds)?;
    Ok(CandidateEvidence {
        id: candidate.id.clone(),
        worktree: worktree.to_path_buf(),
        changed_files,
        diff_bytes: diff.len(),
        gates: gate_results,
        baseline: baseline.clone(),
        candidate: replay,
    })
}

fn run_replay(repository: &Path, gates: &[GateSpec], max_seconds: u64) -> Result<ReplayMetrics> {
    Ok(run_gates(repository, gates, max_seconds)?.1)
}

fn run_gates(directory: &Path, gates: &[GateSpec], max_seconds: u64) -> Result<(Vec<GateResult>, ReplayMetrics)> {
    let started = Instant::now();
    let mut results = Vec::new();
    for gate in gates {
        if started.elapsed().as_secs() >= max_seconds {
            bail!("verification gate budget exhausted");
        }
        let gate_started = Instant::now();
        let output = run_gate(directory, gate)?;
        results.push(GateResult {
            name: gate_name(gate),
            passed: output.status.success(),
            mandatory: true,
            duration_ms: gate_started.elapsed().as_millis() as u64,
        });
    }
    let passed = results.iter().filter(|result| result.passed).count() as u32;
    let failed = results.len() as u32 - passed;
    Ok((
        results,
        ReplayMetrics {
            passed,
            failed,
            duration_ms: started.elapsed().as_millis() as u64,
        },
    ))
}

fn run_gate(directory: &Path, gate: &GateSpec) -> Result<Output> {
    let mut command = match gate {
        GateSpec::Format => {
            let mut command = Command::new("cargo");
            command.args(["fmt", "--all", "--", "--check"]);
            command
        }
        GateSpec::FocusedTest { package, filter } => {
            validate_package(package)?;
            let mut command = Command::new("cargo");
            command.args(["test", "-p", package]);
            if let Some(filter) = filter {
                command.arg(filter);
            }
            command
        }
        GateSpec::Clippy { package } => {
            validate_package(package)?;
            let mut command = Command::new("cargo");
            command.args([
                "clippy",
                "-p",
                package,
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ]);
            command
        }
        GateSpec::DiffCheck => {
            let mut command = Command::new("git");
            command.args(["diff", "--check"]);
            command
        }
    };
    Ok(command.current_dir(directory).output()?)
}

fn validate_package(package: &str) -> Result<()> {
    if package.is_empty()
        || !package
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("invalid Cargo package name");
    }
    Ok(())
}

fn gate_name(gate: &GateSpec) -> String {
    match gate {
        GateSpec::Format => "cargo fmt".to_string(),
        GateSpec::FocusedTest { package, .. } => format!("cargo test -p {package}"),
        GateSpec::Clippy { package } => format!("cargo clippy -p {package}"),
        GateSpec::DiffCheck => "git diff --check".to_string(),
    }
}

fn command_output(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        bail!("command failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn require_success(label: &str, output: Output) -> Result<()> {
    if !output.status.success() {
        bail!("{label} failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

fn nearest_existing_ancestor(path: &Path) -> Result<PathBuf> {
    let mut current = path;
    loop {
        if current.exists() {
            return Ok(current.canonicalize()?);
        }
        current = current
            .parent()
            .ok_or_else(|| anyhow::anyhow!("path has no existing ancestor"))?;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;

    fn candidate(id: &str, failed: u32, passed: u32, mandatory_passed: bool) -> CandidateEvidence {
        CandidateEvidence {
            id: id.to_string(),
            worktree: PathBuf::from(id),
            changed_files: 2,
            diff_bytes: 512,
            gates: vec![GateResult {
                name: "cargo test".to_string(),
                passed: mandatory_passed,
                mandatory: true,
                duration_ms: 10,
            }],
            baseline: ReplayMetrics {
                passed: 10,
                failed: 2,
                duration_ms: 100,
            },
            candidate: ReplayMetrics {
                passed,
                failed,
                duration_ms: 100,
            },
        }
    }

    #[test]
    fn clusters_equivalent_failures_deterministically() {
        let clusters = cluster_failures(vec![
            FailureSample {
                capability: "MCP_CALL".to_string(),
                error_class: " runtime_error ".to_string(),
                reproduction: "first".to_string(),
            },
            FailureSample {
                capability: "mcp_call".to_string(),
                error_class: "runtime_error".to_string(),
                reproduction: "second".to_string(),
            },
        ]);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].samples.len(), 2);
    }

    #[test]
    fn selects_only_non_regressing_candidates() {
        let budget = EvolutionBudget::default();
        let candidates = vec![
            candidate("regression", 3, 10, true),
            candidate("failed-gate", 0, 12, false),
            candidate("improved", 0, 12, true),
            candidate("partial", 1, 11, true),
        ];
        assert_eq!(select_candidate(&candidates, &budget).unwrap().id, "improved");
    }

    #[test]
    fn rejects_worktrees_inside_production_repository() {
        let repository = std::env::current_dir().unwrap();
        let request = EvolutionRequest {
            worktree_root: repository.join("evolution-worktrees"),
            repository,
            dry_run: true,
            budget: EvolutionBudget::default(),
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn applies_candidate_only_in_an_isolated_worktree() {
        let root = std::env::temp_dir().join(format!("aion-evolution-{}", uuid::Uuid::new_v4()));
        let repository = root.join("repository");
        let worktrees = root.join("worktrees");
        fs::create_dir_all(&repository).unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repository)
            .status()
            .unwrap()
            .success());
        fs::write(repository.join("value.txt"), "old\n").unwrap();
        assert!(Command::new("git")
            .args(["add", "value.txt"])
            .current_dir(&repository)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args([
                "-c",
                "user.name=Aion Forge",
                "-c",
                "user.email=forge@example.invalid",
                "commit",
                "-q",
                "-m",
                "baseline",
            ])
            .current_dir(&repository)
            .status()
            .unwrap()
            .success());

        let request = EvolutionRequest {
            repository: repository.clone(),
            worktree_root: worktrees,
            dry_run: false,
            budget: EvolutionBudget::default(),
        };
        let outcome = EvolutionRunner::run(
            &request,
            &[PatchCandidate {
                id: "candidate-1".to_string(),
                patch: "diff --git a/value.txt b/value.txt\nindex 3367afd..3e75765 100644\n--- a/value.txt\n+++ b/value.txt\n@@ -1 +1 @@\n-old\n+new\n"
                    .to_string(),
            }],
            &[GateSpec::DiffCheck],
        )
        .unwrap();

        assert_eq!(fs::read_to_string(repository.join("value.txt")).unwrap(), "old\n");
        assert_eq!(outcome.selected.unwrap().id, "candidate-1");
        assert!(!root.join("worktrees").join("candidate-1").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
