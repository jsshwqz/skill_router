# Aion Forge 入口与 AionUI CLI 分离设计

日期：2026-07-22
状态：待用户书面复核

## 背景

当前仓库把两个不同产品的身份混在了一起：

- `aion-cli` 原本是供 AionUI Agent 使用的独立 CLI。它目前被复制进 Forge workspace，并被 Forge 的构建、发布、文档和 MCP 配置引用。
- `aion-forge-cli` 至少自 2026-06-24 起已作为 Forge 自己的工具入口存在。2026-07-17，它被整体改名为 `aion-forge-acp`，导致 Forge 主 CLI 消失，而 MCP 继续借用 `aion-cli`。

这不是单纯的文件命名错误，而是产品边界、协议入口和部署身份同时错位。

## 目标

1. 恢复 `aion-forge-cli` 作为 Aion Forge 的正式命令行入口。
2. 让 `aion-forge-cli` 自己提供 MCP server，不再依赖 `aion-cli` 承载 Forge MCP。
3. 让 `aion-forge-acp` 只负责 ACP 协议适配。
4. 将 `aion-cli` 完整迁移到 `D:/test/aionui/aion-cli`，作为 AionUI Agent 的独立项目保留，不删除、不覆盖其源码或二进制。
5. 清除 Forge 活跃构建、发布、安装、配置和正式文档中对 `aion-cli` 的产品依赖。
6. 在迁移期间保持 AionUI 可回滚，最终状态不保留伪装成 Forge 的 `aion-cli` 入口。

## 非目标

- 不重写 `aion-router`、`aion-intel`、`aion-memory` 的既有能力。
- 不改变 75 个 Forge MCP 工具的协议名称和参数结构。
- 不删除历史归档中用于说明过去状态的 `aion-cli` 记录。
- 不把 ACP 恢复为独立聊天模型或默认 Agent。
- 不直接编辑 AionUI 后端数据库。

## 产品边界

| 组件 | 产品归属 | 最终职责 |
|---|---|---|
| `aion-forge-cli` | Aion Forge | 直接工具调用、工具列表、MCP server、Forge 安装配置 |
| `aion-forge-acp` | Aion Forge | 仅提供 ACP 协议适配 |
| `aion-router` 等核心 crate | Aion Forge | 工具注册、执行、治理、记忆、推断和编排 |
| `aion-cli` | AionUI Agent | 独立保留在 Forge 仓库之外，不参与 Forge 构建或发布 |
| `aion-server` | Aion Forge | 可选 HTTP API 入口 |

`aion-cli-gen` 是通用 CLI 适配器生成器。为消除产品命名歧义，它随本次迁移重命名为 `aion-forge-cli-gen`，行为保持不变。

## 最终目录与二进制

Forge 仓库 `D:/test/aionui/forge`：

- `aion-forge-cli/`：Forge CLI 与 MCP server 源码。
- `aion-forge-acp/`：纯 ACP 适配器源码。
- `aion-forge-cli-gen/`：Forge CLI 适配器生成器。
- `aion-forge-cli.exe`：正式 CLI/MCP 发布物。
- `aion-forge-acp.exe`：可选 ACP 发布物。

AionUI 独立目录 `D:/test/aionui/aion-cli`：

- 完整保留原 `aion-cli/` 源码和本地历史材料。
- 不由 Forge workspace 构建。
- 不被 Forge 安装脚本覆盖。

## 命令职责

### `aion-forge-cli`

- `--tool <名称> --params <JSON>`：直接执行一个 Forge builtin，保持 2026-07-17 版本的参数形式。
- `--list`：列出当前能力注册表中的工具，保持 2026-07-17 版本的参数形式。
- `mcp-server`：通过标准输入输出运行 Forge MCP JSON-RPC 服务。
- `setup`：生成并验证 Forge 的 MCP 配置和部署材料；只通过 AionUI 官方配置接口应用 MCP 配置，不注册 ACP 聊天 Agent，也不直接修改数据库。

CLI 延续 2026-07-17 版本的直接工具调用和列表能力，并新增正式 MCP 模式。ACP 逻辑不再编译进这个二进制。

### `aion-forge-acp`

- 只接受 ACP 服务器模式。
- 不再提供直接工具调用、工具列表或 Setup。
- 内部仍可调用同一 `aion-router` builtin 注册表，但协议身份保持独立。
- 默认不注册、不启用；只有明确需要 ACP 客户端兼容时才部署。

## 数据流

### MCP 正式链路

AionUI 正常 Agent 调用 `aion-forge-cli.exe mcp-server`。CLI 将 MCP 请求交给 Forge MCP 层，Forge MCP 层从 `aion-router` 获取 builtin，并按当前进程配置调用 OmniRoute 或其他可用 Provider。

### CLI 直接调用

用户调用 `aion-forge-cli.exe` 并指定工具与参数。CLI 进行输入清理，构造 `ExecutionContext`，再直接调用 `aion-router` builtin。

### ACP 兼容链路

ACP 客户端调用 `aion-forge-acp.exe`。ACP 适配器将会话或工具协议转换为同一 Forge builtin 调用。该链路不经过 MCP，也不冒充正常 AionUI Agent。

### AionUI CLI 链路

属于 AionUI Agent 的 `aion-cli` 从独立目录运行。它不再进入 Forge 的 MCP、发布包、技能配置或文档。

## 安全迁移顺序

1. 在删除任何 Forge 仓库内引用前，将 `aion-cli/` 完整复制到 `D:/test/aionui/aion-cli`。
2. 比较源目录与目标目录的文件清单和内容哈希；验证目标项目能够独立读取和构建。
3. 先为新的包名、命令解析、MCP 初始化、工具列表和协议隔离编写失败测试。
4. 建立 `aion-forge-cli`，迁移直接工具和 MCP 能力，使测试转绿。
5. 收紧 `aion-forge-acp`，确保它不再承担 CLI、MCP 或 Setup 职责。
6. 更新 workspace、锁文件、发布工作流、安装脚本、安全清单、适配器生成和活跃文档。
7. 构建并部署 `aion-forge-cli.exe`，更新项目级和 AionUI 技能级 MCP 配置。
8. 通过 AionUI 官方配置接口把持久化 MCP 命令切换到 `aion-forge-cli.exe`。如果官方接口在当前会话不可用，停止持久化配置写入，不直接操作数据库。
9. 重启 AionUI，验证它实际启动的新进程路径和哈希，并执行真实 Forge 工具调用。
10. 只有在新链路验证通过后，才从 Forge workspace 和版本控制中移除 `aion-cli/`，并清除迁移期间的旧名称兼容入口。

每一步失败时都保留上一步的可运行状态。迁移过程中不覆盖独立目录中的 AionUI CLI。

## 配置与兼容策略

- 活跃配置最终只允许引用 `aion-forge-cli.exe`。
- 迁移期间可以保留旧文件备份，但备份只能位于 D 盘隔离目录，不能参与自动发现或启动。
- 旧 AionUI 会话可能持有 MCP 快照。切换配置后必须重启并新建会话验证，不能仅凭配置保存成功宣称生效。
- 不在 Forge 最终目录保留名为 `aion-cli.exe` 的转发器或符号链接，避免再次占用另一个产品的身份。
- 历史归档可以保留旧命令文本，但必须继续位于 archive 范围，不作为当前安装说明。

## 错误处理

- MCP stdout 只输出 JSON-RPC；日志统一进入 stderr。
- 未知工具返回明确协议错误，不返回 placeholder 成功结果。
- OmniRoute 或 Provider 不可用时保留真实错误和后端信息，不伪造模型输出。
- `setup` 无法获得 AionUI 官方配置上下文时返回稳定错误，不降级为数据库直写。
- ACP 收到非 ACP 模式参数时返回用法错误，避免重新承担 CLI 职责。
- 独立 `aion-cli` 迁移校验失败时立即停止，不从 Forge 仓库移除原目录。

## 测试设计

### 包与身份测试

- 新 CLI 的 Cargo 包名和 clap 命令名必须为 `aion-forge-cli`。
- Forge workspace 不再包含 `aion-cli` 成员。
- 活跃适配器和配置生成结果不得输出 `aion-cli` 作为 Forge 入口。

### CLI 测试

- `list` 返回非空工具集合，并包含已知 builtin。
- 直接调用一个无网络 builtin，结果与注册表执行结果一致。
- 未知工具返回失败状态和明确错误。

### MCP 测试

- `initialize` 返回服务器名 `aion-forge`。
- `tools/list` 返回当前完整工具集合。
- `tools/call` 能执行一个确定性的本地 builtin。
- MCP stdout 中不存在 tracing 日志。

### ACP 隔离测试

- ACP 二进制只暴露 ACP 命令。
- ACP 不接受 `mcp-server`、`setup`、直接工具或列表参数。
- ACP 的初始化、会话和工具转换继续通过已有协议测试。

### 迁移与部署测试

- 独立 AionUI CLI 目录与迁移前源目录内容哈希一致。
- Release 构建只发布 `aion-forge-cli`、`aion-forge-acp` 和既定 Forge 服务。
- AionUI 实际子进程路径为 `aion-forge-cli.exe`。
- 真实 MCP 调用经 OmniRoute 返回有效结果和 token 用量。
- 全 workspace 单元、集成和文档测试通过。

## 完成标准

只有同时满足以下条件，迁移才算完成：

1. Forge 正式链路不再启动或引用 `aion-cli`。
2. `aion-forge-cli` 同时具备直接工具和 MCP 能力。
3. `aion-forge-acp` 只承担 ACP。
4. AionUI CLI 在 `D:/test/aionui/aion-cli` 完整存在且未被修改为 Forge。
5. AionUI 重启后的真实 MCP 进程和调用均使用新 Forge 二进制。
6. 全量测试、发布检查和工作树检查均有新鲜通过证据。
