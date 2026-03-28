//! # aion-cli-gen
//!
//! CLI 包装器自动生成管道——分析外部工具并生成 AI Agent 可用的技能包。
//!
//! ## 5 阶段管道
//!
//! 1. **分析**：运行 `tool --help`，提取命令结构
//! 2. **生成**：产出 SKILL.md + sandbox-policy.json + skill.json
//! 3. **测试**：验证生成物合法性
//! 4. **安全审查**：检验沙箱策略安全性
//! 5. **发布**：写入目标目录
//!
//! ## 使用
//!
//! ```text
//! aion-cli tool generate <tool-name> --output ./skills/
//! ```

pub mod analyzer;
pub mod generator;
pub mod pipeline;

pub use analyzer::{ToolAnalyzer, ToolAnalysis};
pub use generator::SkillGenerator;
pub use pipeline::{GenerationPipeline, PipelineResult};
