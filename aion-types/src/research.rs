//! 研发助手工具链的数据类型定义。
//!
//! 提供模型谱分析、质量评估、迭代精炼、超参优化等核心能力的
//! 数据结构，供 `aion-analyze`、`aion-eval`、`aion-refine`、`aion-optimize`
//! 等 crate 共享使用。

pub mod evaluation;
pub mod optimize;
pub mod refine;
pub mod spectral;
