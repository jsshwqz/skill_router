# Aion Forge v0.7.x 项目总览

## 项目定位

Aion Forge 是纯 Rust 实现的 MCP 工具服务器，为 AionUI 和其他 MCP 客户端提供代码、解析、搜索、记忆、RAG、治理及多模型编排能力。

当前正式启动入口为 `aion-cli.exe mcp-server`。AI 请求通过本机 OmniRoute 的 OpenAI-compatible 接口路由，默认模型为 `auto/fast`。

## Workspace

| crate | 职责 |
|---|---|
| `aion-types` | 数据结构与协议定义 |
| `aion-memory` | 记忆存储与蒸馏 |
| `aion-intel` | 推断、规划、搜索和 RAG |
| `aion-router` | 技能路由、执行、治理和 MCP 核心 |
| `aion-sandbox` | 命令策略、隔离与审计 |
| `glitch-filter` | 控制字符及异常输入过滤 |
| `aion-cli` | CLI 与正式 MCP server 入口 |
| `aion-server` | axum HTTP API |
| `aion-forge-cli-gen` | CLI 适配器生成 |
| `aion-zl` | 辩证分析与战略规划 |
| `aion-forge-acp` | 停用的历史 ACP 兼容适配器 |

## 运行链路

AionUI 正常 Agent → Aion Forge MCP → `aion-router` builtin → OmniRoute → 可用模型 Provider。

ACP 独立聊天壳不在正式链路中，也不应自动注册。

## 当前验证基线

截至 2026-07-17：

- AionUI MCP 连接测试成功，识别 75 个工具。
- `code_generate` 经 OmniRoute 返回有效 Rust 代码。
- 全 workspace 单元、集成和文档测试约 192 项通过，0 失败。
- OpenAI-compatible 响应同时支持显式非流式请求和 SSE 回退解析。
- AionUI 重启后 MCP 配置保持有效。

## 配置原则

- 进程环境变量优先于项目 `.env`，便于 AionUI 注入当前配置。
- `.skill-router/registry.json` 是运行时历史，不进入版本控制。
- 安全审查策略由 `AI_SECURITY_FAIL_POLICY` 控制；生产环境应评估使用 closed。
- Provider 可用性由 `AI_PROVIDERS_DISABLED` 明确约束。

## 已知债务

1. `route_task` 对 workspace 分析类中文任务仍可能产生语义误判。
2. `aion-forge-acp`、`aion-cli` 和 `aion-zl` 的入口级集成测试仍需补充。
3. Forge 能力发现偶尔先走动态发现，再命中已有 builtin，存在额外延迟。
4. crate 版本目前独立演进，发布前需要明确版本策略。
