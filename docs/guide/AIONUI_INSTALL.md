# AionUI 集成指南

## 当前正式集成

Aion Forge 作为 MCP 工具服务器注入 AionUI 的正常 Agent，不作为独立聊天 Agent 使用。

| 配置项 | 当前值 |
|---|---|
| 命令 | `D:/test/aionui/forge/aion-forge-cli.exe` |
| 参数 | `mcp-server` |
| 模型入口 | `http://127.0.0.1:20128/v1` |
| 默认模型 | `auto/fast` |
| Provider | OmniRoute |

AionUI 中的 MCP 名称为 `aion-forge`。模型地址、模型名和本地访问凭据通过该 MCP 的环境变量配置。

## 验证标准

集成完成后必须同时满足：

1. AionUI 的 MCP 连接测试成功。
2. 工具列表能够返回当前完整能力集合；2026-07-17 的实测数量为 75。
3. `code_generate` 能经 OmniRoute 返回真实代码，而不是 placeholder。
4. AionUI 重启后配置仍然存在并保持启用。

## ACP 说明

`aion-forge-acp` 是可选 ACP 入口，不是当前正式 MCP 入口。独立 Aion Forge ACP 聊天 Agent 与 `aionext-forge` 扩展均保持停用，不应注册到 AionUI 的 Agent 列表。

`aion-cli` 属于 `D:/test/aionui/aion-cli` 的独立 AionUI Agent 项目。Forge 不构建、发布或安装该名称的兼容产物。

## 常见问题

| 现象 | 检查项 |
|---|---|
| MCP 无法连接 | 确认根目录 `aion-forge-cli.exe` 存在，并使用 `mcp-server` 参数 |
| AI 返回空内容 | 确认使用包含 OmniRoute 非流式及 SSE 兼容修复的新构建 |
| 工具存在但模型不可用 | 检查 OmniRoute 是否监听 20128，以及 `auto/fast` 是否有可用 Provider |
| 显示普通 Forge 聊天助手 | 禁用 ACP Agent；正常 Agent 应通过 MCP 调用具体 Forge 工具 |
