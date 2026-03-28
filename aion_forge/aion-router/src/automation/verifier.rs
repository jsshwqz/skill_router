use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub success: bool,
    pub message: String,
    pub metrics: Value,
}

pub trait Verifier: Send + Sync {
    fn name(&self) -> &str;
    fn verify(&self, task: &str, output: &Value, context: &Value) -> Result<VerificationReport>;
}

pub struct CoreVerifiers;

impl CoreVerifiers {
    /// Provide a default resolver for core verifiers built into the router
    pub fn resolve(id: &str, working_dir: std::path::PathBuf) -> Option<Box<dyn Verifier>> {
        match id {
            "cargo_check" => Some(Box::new(CargoCheckVerifier { working_dir })),
            "syntax_check" => None, // Implementation specific
            "output_format" => None,
            _ => None,
        }
    }
}

pub struct CargoCheckVerifier {
    pub working_dir: std::path::PathBuf,
}

impl Verifier for CargoCheckVerifier {
    fn name(&self) -> &str {
        "cargo_check"
    }

    fn verify(&self, task: &str, _output: &Value, _context: &Value) -> Result<VerificationReport> {
        let output = Command::new("cargo")
            .arg("check")
            .current_dir(&self.working_dir)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        // cargo check usually writes diagnostics to stderr
        let success = output.status.success();
        let message = if success {
            format!("cargo check passed for {}", task)
        } else {
            format!("cargo check failed. Output:\n{}", stderr)
        };

        Ok(VerificationReport {
            success,
            message,
            metrics: json!({
                "exit_code": output.status.code(),
                "stdout_len": stdout.len(),
                "stderr_len": stderr.len(),
            }),
        })
    }
}
