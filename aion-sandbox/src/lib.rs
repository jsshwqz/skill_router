//! # aion-sandbox
//!
//! 沙箱化 CLI 执行层——在白名单策略约束下安全执行外部命令。
//!
//! ## 核心组件
//!
//! - [`policy::SandboxPolicy`] — 白名单策略定义
//! - [`executor::SandboxedExecutor`] — 沙箱化命令执行器
//! - [`jail::ResourceLimits`] — 资源限制（超时、输出上限）
//! - [`audit::AuditLog`] — 执行审计日志
//!
//! ## 安全设计
//!
//! - **白名单优先**：只有策略中显式声明的命令才能执行
//! - **参数过滤**：正则白名单/黑名单控制命令参数
//! - **环境隔离**：默认清空环境变量，仅透传白名单变量
//! - **资源约束**：超时自动 kill，输出截断防止 OOM
//! - **完整审计**：每次执行记录到审计日志

pub mod policy;
pub mod executor;
pub mod jail;
pub mod audit;

pub use policy::{SandboxPolicy, CommandRule, WorkDirPolicy};
pub use executor::{SandboxedExecutor, SandboxedCommand, SandboxOutput};
pub use jail::ResourceLimits;
pub use audit::{AuditLog, AuditEntry};
