//! JSON Schema 数据结构定义
//!
//! 提供 `JsonSchema` 结构体和 `SchemaError` 错误类型。
//! 校验逻辑实现在 `aion-router` 的 `verifier.rs` 中。

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// JSON Schema 校验错误
#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Schema 解析失败: {0}")]
    Parse(String),
    #[error("校验失败: {0}")]
    Validation(String),
}

/// 编译期加载的 JSON Schema，附带一个唯一标识名称
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    /// Schema 名称（如 "text_toon_output"）
    pub name: String,
    /// 原始 JSON Schema 字符串
    pub schema: String,
    /// 人类可读描述
    pub description: String,
}

impl JsonSchema {
    /// 创建一个新的 JsonSchema
    pub fn new(name: impl Into<String>, schema: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schema: schema.into(),
            description: description.into(),
        }
    }
}