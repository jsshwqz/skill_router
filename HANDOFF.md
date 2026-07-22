# Aion Forge 项目交接

最后更新：2026-07-17。

## 当前状态

| 项目 | 状态 |
|---|---|
| 正式入口 | `D:/test/aionui/forge/aion-forge-cli.exe mcp-server` |
| AionUI 集成 | MCP 已启用，ACP 独立聊天 Agent 已禁用 |
| 模型路由 | OmniRoute `http://127.0.0.1:20128/v1`，模型 `auto/fast` |
| 工具数量 | AionUI 实测 75 |
| 端到端调用 | `code_generate` 已成功 |
| 测试 | 全 workspace 通过，0 失败 |

## 重要决策

- Forge 是由正常 Agent 按需调用的 MCP 工具集，不是独立聊天模型。
- `aion-forge-acp` 仅保留为可选 ACP 入口，不注册、不部署为默认扩展。
- `aion-cli` 属于 `D:/test/aionui/aion-cli` 的独立 AionUI Agent 项目，不是 Forge 入口或兼容产物。
- `aionext-forge` 已从活跃工作区隔离。
- `.skill-router/registry.json` 属于运行时历史，在本地保留但不再跟踪。
- AionUI 注入的环境变量优先于项目 `.env`。

## 本轮关键修复

- OmniRoute 请求显式设置非流式，并兼容 SSE 回退响应。
- placeholder 不再伪装成功，而是返回明确错误。
- health check 明确区分历史遥测与实时探测。
- `strategic_plan` 的中文截断改为按字符处理，避免 UTF-8 panic。

## 后续优先事项

1. 修复 `route_task` 对 workspace 分析任务的误判。
2. 为 CLI、ACP 兼容层和 aion-zl 增加入口级测试。
3. 处理生产环境安全审查 fail-open 风险。
