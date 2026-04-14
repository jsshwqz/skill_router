# Aion Forge — 项目规则

## 语言规定

**本项目只使用 Rust 语言编写所有代码。**

- 禁止在任何模块中引入 Python、Go、Node.js、Shell 脚本或其他语言的实现
- 示例代码、文档代码块也必须使用 Rust
- 外部 Agent 接入必须通过 Rust trait 实现，不得通过多语言 HTTP 调用绕过类型系统

## 架构规定

- 外部技能/Agent 通过实现 Rust trait（`SkillProvider` 等）接入，编译链接而非网络调用
- 所有新增 crate 必须加入根 `Cargo.toml` 的 `[workspace]` 成员
- 新增依赖优先使用 `{ workspace = true }` 复用已有版本，不重复声明

## 代码质量

- 库代码（`aion-types`、`aion-memory`、`aion-intel`、`aion-router`）禁止使用 `println!`，必须用 `tracing::info!/warn!/error!`
- 所有新增数据结构字段若可能影响向后兼容，必须加 `#[serde(default)]`
- 公开 API 必须有 doc comment（`///`）

## crate 结构

```
aion-types   — 数据结构与协议定义（无 IO，无 HTTP）
aion-memory  — 记忆存储管理
aion-intel   — AI 推断、规划、搜索
aion-router  — 技能路由、执行、协调核心
aion-cli     — 命令行入口
aion-server  — HTTP REST API（纯 Rust，axum）
```
