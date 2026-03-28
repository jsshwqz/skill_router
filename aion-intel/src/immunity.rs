use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSignature {
    pub id: String,
    pub pattern: String,
    pub fix_suggestion: String,
    pub description: String,
}

pub struct ImmunitySystem;

impl ImmunitySystem {
    pub fn pre_check_command(command: &str) -> Result<()> {
        // Rule 1: PowerShell && connector
        if command.contains("&&") {
            return Err(anyhow!(
                "Immunity Violation [ERR-PS-CONJ]: PowerShell does not support '&&'. \
                Please use ';' instead. Instruction: {}", 
                command
            ));
        }

        Ok(())
    }

    pub fn sanitize_instruction(instruction: &mut String) {
        // Auto-fix simple patterns
        if instruction.contains(" && ") {
            *instruction = instruction.replace(" && ", " ; ");
        }
    }
}
