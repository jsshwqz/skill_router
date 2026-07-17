# Aion Forge AI 协作规范

## 协作方式

当前助手负责读取源码、修改、测试和评审。需要 Forge 能力时，通过正式 MCP 调用具体工具，并明确区分 Forge 输出与宿主助手判断。

Forge 不作为独立聊天 Agent 使用。`aion-forge-acp` 和历史扩展不得自动注册或替代正常 Agent。

## 工作目录

项目固定在 `D:/test/aionui/forge`。开发产物和临时文件不得写入 C 盘；临时审计材料使用 D 盘临时目录。

## 构建与部署

| 操作 | 目标 |
|---|---|
| Release 构建 | workspace 中的 `aion-cli` package |
| 正式二进制 | `D:/test/aionui/forge/aion-cli.exe` |
| AionUI MCP 参数 | `mcp-server` |
| 模型入口 | OmniRoute 20128，`auto/fast` |

覆盖正式二进制前应结束占用它的旧 MCP 进程；覆盖后必须比较构建产物与正式二进制的 SHA256，并执行真实工具调用。

## 提交规范

1. 纯格式化与功能修改不得混在同一个提交。
2. 运行时 registry、日志、缓存和模型输出不得提交。
3. MCP 核心修复、ACP 历史兼容层和文档分别提交。
4. 每个功能提交至少运行相关 crate 测试；涉及正式入口时运行全 workspace 测试。

## 安全约束

- 不在日志、提交或提示词中泄露真实凭据。
- 生产使用前评估将 AI 安全审查策略设为 closed。
- ACP 历史适配器不得写入 AionUI 数据库或自动启用 Agent。
