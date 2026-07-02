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

    /// 最大允许的 \r 控制字符数（超过此数量触发拦截）
    const MAX_CONTROL_R_COUNT: usize = 100;

    pub fn sanitize_instruction(instruction: &mut String) {
        // Auto-fix simple patterns
        if instruction.contains(" && ") {
            *instruction = instruction.replace(" && ", " ; ");
        }
    }

    /// 检测输入中是否包含过多的 \r 控制字符（CONTROL_CHARACTER_FLOOD 攻击防御）
    ///
    /// 当输入中存在 100 个以上的 `\r`（回车符）时返回错误，
    /// 因为已知这会导致 LLM 注意层崩溃并"遗忘"系统 prompt。
    pub fn check_control_character_flood(input: &str) -> Result<()> {
        let count = input.chars().filter(|&c| c == '\r').count();
        if count > Self::MAX_CONTROL_R_COUNT {
            return Err(anyhow!(
                "CONTROL_CHARACTER_FLOOD blocked: input contains {} \\r characters \
                 (limit: {}). This pattern is known to cause LLM attention collapse.",
                count,
                Self::MAX_CONTROL_R_COUNT,
            ));
        }
        Ok(())
    }
}
