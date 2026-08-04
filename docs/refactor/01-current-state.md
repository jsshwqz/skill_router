# Forge 当前状态模型

## 文档元信息

- **分析范围**：`9ba54ed6e0499c95d3f6b8c6bd1a2fc820b39b4f`（2026-08-04 07:52:40 +08:00）所指向的 Forge workspace；覆盖 crate 结构、正式与兼容入口、核心路由/执行链、配置、依赖、测试、CI、发布脚本和主要项目文档。分析期间仓库曾从 `7776c46` 前进到 `9ba54ed`，因此本文只对上述快照负责。
- **已读取的代码和文档**：根与各 crate 的 `Cargo.toml`、`Cargo.lock` 元数据、CLI/MCP/ACP/HTTP/ZL 入口、`SkillRouter`、`Executor`、builtin 与 capability 注册表、WorkflowConfig、EmbeddingProvider、MemoryRecall、RAG、memory 路径、核心测试、`.github/workflows/*.yml`、Dockerfile、README、CHANGELOG、HANDOFF、项目总览、发布门槛、版本重排方案、升级核验与近期提交记录。完整清单见“附录 A”。
- **未读取或无法确认的内容**：没有逐行阅读全部约 77 个 builtin、automation 子系统和所有历史归档；没有连接真实 AionUI、OmniRoute、第三方模型、NATS、搜索服务或生产数据；没有取得当前 GitHub Actions 远端运行记录；忽略测试和全 workspace 测试未成功跑完；没有测量生产成功率、P95、资源占用或跨平台发布产物。
- **结论的证据**：当前源码定位、Cargo 依赖、契约测试、Git 状态/提交记录，以及本地可复现实验：`cargo check --workspace`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --all-features -- -D warnings`、分组测试尝试。每项重要结论均在正文给出文件和行号或实验结果。
- **当前置信度**：架构和静态事实 **高（0.90）**；本地构建/门禁状态 **高（0.90）**；无外部服务的运行行为 **中（0.65）**；真实多模型、分布式和生产表现 **低（0.30）**。

## 1. 判定口径

本文使用三类标签，避免把推断写成事实：

| 标签 | 含义 |
|---|---|
| **事实** | 可由当前源码、配置、测试或本次命令直接复现 |
| **推断** | 由多个事实支持，但尚无运行实验完整验证 |
| **待验证假设** | 可能影响方案，现有证据不足 |

## 2. 项目职责与边界

### 2.1 主要职责

**事实**：Forge 是纯 Rust workspace，向 CLI、MCP、ACP 和 HTTP 客户端暴露解析、文本、搜索、记忆、RAG、安全治理、Agent 协作和多模型编排能力。根 workspace 当前包含 11 个成员（[Cargo.toml](../../Cargo.toml)）。

**事实**：项目规则定义的核心分层仍成立：

| crate | 当前职责 | 直接内部依赖 |
|---|---|---|
| `aion-types` | 协议、数据结构、能力元数据、路径 | 无 |
| `aion-memory` | redb 记忆存储、迁移、蒸馏、命名空间 | `aion-types` |
| `aion-intel` | 规划、发现、RAG、embedding、指令合成 | `aion-types` |
| `aion-sandbox` | 命令策略、隔离、资源限制、审计 | 无 Forge crate |
| `glitch-filter` | 异常/控制字符过滤 | 无 Forge crate |
| `aion-router` | 能力路由、选择、执行、安全、学习、builtin、编排 | types、memory、intel、sandbox、glitch-filter |
| `aion-forge-acp` | 可选 ACP 会话、规划与工具适配 | types、router |
| `aion-forge-cli` | `aion-forge` 与兼容命令、MCP server、setup、直接调用 | acp、intel、router、types、glitch-filter |
| `aion-server` | axum REST/WebSocket/metrics | router、types、memory |
| `aion-zl` | 独立辩证策略 CLI/MCP | types、router、intel、memory |
| `aion-forge-cli-gen` | 外部 CLI 分析与技能生成库 | types、sandbox |

**事实**：当前 Cargo 内部依赖是自底向上的，没有发现 crate 级循环依赖。`aion-cli` 不在 workspace 中，属于 sibling 项目；不应在本次重构中重新合并。

### 2.2 入口

| 入口 | 代码证据 | 当前行为 |
|---|---|---|
| `aion-forge` | [aion-forge-cli/src/main.rs](../../aion-forge-cli/src/main.rs)、[lib.rs:17](../../aion-forge-cli/src/lib.rs#L17) | 统一主命令；无参数默认启动 ACP |
| `aion-forge-cli` | [aion-forge-cli/Cargo.toml](../../aion-forge-cli/Cargo.toml)、[src/bin/aion-forge-cli.rs](../../aion-forge-cli/src/bin/aion-forge-cli.rs) | 兼容命令，复用同一 `main_entry` |
| MCP stdio | [aion-forge-cli/src/mcp.rs:83](../../aion-forge-cli/src/mcp.rs#L83) | rmcp server；按精确工具名进入 `SkillRouter` |
| ACP stdio | [aion-forge-acp/src/lib.rs](../../aion-forge-acp/src/lib.rs) | 可选 Agent 会话和规划入口 |
| HTTP/WebSocket | [aion-server/src/main.rs:59](../../aion-server/src/main.rs#L59)、[main.rs:168](../../aion-server/src/main.rs#L168) | 1 个公开 health、其余可选 Bearer 认证；10 个 REST/metrics 路由和 1 个 WebSocket 路由 |
| ZL CLI/MCP | [aion-zl/src/main.rs:44](../../aion-zl/src/main.rs#L44) | 独立辩证策略入口，内部可调用 `SkillRouter` |

## 3. 当前模块与数据流

### 3.1 依赖关系

当前依赖关系可压缩为以下有向关系：

| 上游入口/模块 | 依赖方向 | 下游模块 |
|---|---|---|
| CLI/MCP/ACP/HTTP/ZL | → | router 与协议适配 |
| router | → | types、intel、memory、sandbox、glitch-filter |
| intel、memory | → | types |
| CLI generator | → | types、sandbox |
| router builtin | → | 外部模型/搜索/MCP/NATS（按配置可选） |
| router/memory/intel | → | `.skill-router/`、`memory_store.redb`、RAG store、执行日志 |

### 3.2 MCP 和 HTTP 的标准执行流

**事实**：MCP `tools/call` 和 HTTP route 走标准路由链：

客户端请求 → 协议参数解析 → `SkillRouter::route_with_capability` / `route_with_context` → capability 校验或 Planner 推断 → Loader/Matcher 选择 skill → `Executor::execute` → 权限校验 → prevention gate → 可选治理 → AI 前置安全审查 → immunity 检查 → builtin 或 sandbox → metrics/learner → AI 后置安全审查 → execution log → 协议响应。

核心证据：

- 路由三阶段为关键词、AI 分类、动态发现：[aion-router/src/lib.rs:86](../../aion-router/src/lib.rs#L86)。
- 精确能力入口先校验名称：[aion-router/src/lib.rs:190](../../aion-router/src/lib.rs#L190)。
- skill 选择与执行发生在 [aion-router/src/lib.rs:205](../../aion-router/src/lib.rs#L205) 至 [lib.rs:275](../../aion-router/src/lib.rs#L275)。
- 安全、学习、指标和日志链集中于 [aion-router/src/executor.rs:49](../../aion-router/src/executor.rs#L49)。

### 3.3 直接 CLI 与 ACP 的旁路执行流

**事实**：直接 CLI 和 ACP 工具执行没有经过 `SkillRouter`/`Executor`，而是各自创建 `BuiltinRegistry`、`SkillDefinition` 和 `ExecutionContext` 后直接调用 `builtin.execute(...)`：

- CLI：[aion-forge-cli/src/direct.rs:41](../../aion-forge-cli/src/direct.rs#L41)、[direct.rs:61](../../aion-forge-cli/src/direct.rs#L61)。
- ACP：[aion-forge-acp/src/executor.rs:42](../../aion-forge-acp/src/executor.rs#L42)、[executor.rs:90](../../aion-forge-acp/src/executor.rs#L90)。

因此这两个入口不会自然获得 `Executor` 中的权限校验、前/后安全审查、prevention gate、统一 metrics、learner 记录和 execution log。

**推断**：相同工具通过 MCP/HTTP 与直接 CLI/ACP 调用时，安全拒绝、观测记录和失败学习可能不一致。这不是代码风格问题，而是入口行为和安全边界差异。

### 3.4 能力目录

**事实**：公开目录由 `CapabilityRegistry::builtin()` 生成，CLI 与 MCP 共用 [aion-forge-cli/src/catalog.rs:14](../../aion-forge-cli/src/catalog.rs#L14)。builtin 实现由独立的 [aion-router/src/builtins/mod.rs:84](../../aion-router/src/builtins/mod.rs#L84) 注册。

**事实**：契约测试要求 77 个公开名称无重复，并验证公开声明、builtin 路由、直接 CLI 与 ACP 名称集合一致：[aion-forge-cli/tests/direct_contract.rs:6](../../aion-forge-cli/tests/direct_contract.rs#L6)。这是现有架构的有效保护，不应为了“更统一”而立即替换。

## 4. 当前优势

1. **事实**：crate 责任总体清晰，依赖方向合理；没有证据支持重写整个 workspace。
2. **事实**：MCP 已迁移到 rmcp，HTTP 使用 axum，协议入口有契约测试。
3. **事实**：标准 `Executor` 已包含权限、安全、隔离、指标、学习与日志，缺的不是重新设计这些能力，而是让入口一致使用它。
4. **事实**：77 项公开能力已有名称级一致性测试；MCP 和直接 CLI 目录共享同一元数据来源。
5. **事实**：memory 使用 redb 并保留旧 JSON 迁移；RAG、sandbox、server、ACP、ZL 均有一定单元或契约测试。
6. **推断**：采用增量收敛方案比重新划分 crate 更容易保留行为和逐阶段回滚。

## 5. 问题清单与证据

### P0：当前快照不能通过仓库声明的 CI 质量门禁

**事实**：

- `cargo check --workspace` 通过，但报告 2 个警告：`AiSmartCollaborate` 弃用实现仍触发警告；`workflow_config` 加载结果未使用。
- `cargo fmt --all -- --check` 失败，差异跨 memory、router、intel、server、ZL 等多个 crate。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 失败：`aion-intel/src/rag.rs` 含非法 UTF-8，编译器无法读取。
- `.github/workflows/ci.yml` 明确把 fmt 与 `-D warnings` clippy 作为第一层硬门禁：[ci.yml:37](../../.github/workflows/ci.yml#L37)、[ci.yml:40](../../.github/workflows/ci.yml#L40)。

**影响**：可靠性、开发效率、发布可信度。缓存下的 `cargo check` 可成功，而全量 clippy 读源码失败，容易造成“本地看似可编译”的错误判断。

### P0：两个对外参数/能力仍是静默空操作

**事实 A — WorkflowConfig**：

- `WorkflowConfig::load_from_yaml` 的 doc comment 明示 stub，忽略路径并返回默认配置：[orchestrator.rs:2514](../../aion-router/src/builtins/orchestrator.rs#L2514)。
- `AiParallelSolve` 接收 `workflow_config`，但变量未参与后续输入或 `run_collaboration_workflow`：[orchestrator.rs:2542](../../aion-router/src/builtins/orchestrator.rs#L2542)。
- 最近提交 `1e4bf31` 标题为“integrate WorkflowConfig loading”，但实际仍产生 unused-variable 警告。

**事实 B — semantic memory recall**：

- `semantic: true` 分支明确记录“using keyword fallback”，两分支都调用 `MemoryManager::recall`：[aion-router/src/builtins/memory.rs:62](../../aion-router/src/builtins/memory.rs#L62)。
- `EmbeddingProvider` 已存在，但没有被 MemoryRecall 引用：[aion-intel/src/embedding.rs:7](../../aion-intel/src/embedding.rs#L7)。
- 当前只有 TF-IDF provider 的向量长度单测，没有 WorkflowConfig 或 MemoryRecall semantic 行为测试。

**影响**：接口声明与真实行为不一致，会让调用者得到“成功但未应用配置/语义检索”的结果。这比明确报错更难发现。

### P0：执行安全与观测链按入口分裂

**事实**：MCP/HTTP 进入 `Executor`；直接 CLI/ACP 直接执行 builtin，见 3.3。

**影响**：同一能力存在不同的权限、安全、学习、指标和日志语义。任何只在 `Executor` 添加的可靠性修复都不会自动覆盖所有入口。

**待验证假设**：历史上可能有意让直接 CLI/ACP 绕过部分审查以降低延迟；在改变行为前必须用现有调用和产品决策确认。

### P1：CI 的集成测试阶段允许失败

**事实**：CI 中被忽略测试使用 `cargo test --all -- --include-ignored 2>/dev/null || true`：[ci.yml:106](../../.github/workflows/ci.yml#L106)。任何失败都会被转为成功。

**影响**：集成 job 的绿色状态不能证明 ignored/integration tests 通过。发布 workflow 也没有显式依赖一次测试成功的 workflow。

### P1：配置读取分散且默认值可能漂移

**事实**：`aion-router/src/config.rs` 提供集中读取函数，但源码中有 30 个 Rust 文件直接调用 `std::env::var`。`orchestrator.rs`、`aion-intel`、server、CLI、ACP、ZL 各自定义超时、provider、workspace 和模式默认值。

**影响**：同一环境变量在不同入口的默认值、优先级和生命周期难以确认；测试还会直接修改进程全局环境，增加并发测试干扰风险。

**推断**：不需要一次性建立复杂配置框架；应先收敛直接影响执行语义的 provider、timeout、workspace 和 security 配置。

### P1：核心热点文件扩大了改动半径

**事实**：当前行数约为：`orchestrator.rs` 3378、`capability_registry.rs` 1365、`learner.rs` 1399、`rag.rs` 约 800。`orchestrator.rs` 同时包含引擎发现、进程/HTTP 调用、缓存、并发、异步任务、workflow、投票/评审和多个 builtin；对应文件仅检索到 5 个本地测试。

**影响**：WorkflowConfig 的“接入但未使用”就是热点文件中局部变更未闭环的直接例子。问题在职责与验证半径，而不是文件长本身。

### P1：版本、能力数量和交接文档漂移

**事实**：

- README 宣称公开稳定版 `v0.3.0`、29+ 能力：[README.md:3](../../README.md#L3)、[README.md:7](../../README.md#L7)。
- CLI/ACP Cargo 版本为 `0.7.0`，最新 tag 为 `v0.7.1`。
- 项目总览和 HANDOFF 记录 75 工具；当前契约测试固定为 77。
- CHANGELOG 当前顶部仍是 `v0.3.0`。

**影响**：用户、发布者和后续 AI 无法仅凭官方文档判断当前产品契约。

### P2：仓库卫生和编码债务

**事实**：仓库跟踪了 `*.rs.bak`、`sr_out*.json`、`mcp_out3.json` 等备份/运行输出；`aion-router/src/builtins/ai.rs` 存在乱码注释，并在 library 路径使用调试 `eprintln!`：[ai.rs:170](../../aion-router/src/builtins/ai.rs#L170)、[ai.rs:233](../../aion-router/src/builtins/ai.rs#L233)。

**影响**：源码来源不清、审查噪声增加，并违反库 crate 应使用 tracing 的项目规则。删除这些文件属于破坏性操作，必须单独确认，本任务不处理。

### P2：Docker 依赖缓存层与 workspace 清单不一致

**事实**：Dockerfile 初始只复制 6 个 member 的 Cargo.toml，而根 workspace 有 11 个；缓存预构建失败被 `|| true` 吞掉：[Dockerfile:12](../../Dockerfile#L12) 至 [Dockerfile:28](../../Dockerfile#L28)。最终 COPY 后的真实构建仍是硬失败，因此这是缓存/诊断问题，不足以断言发布镜像一定失败。

## 6. 测试与构建现状

| 检查 | 结果 | 可得结论 |
|---|---|---|
| `cargo check --workspace` | 通过，2 warnings | 增量 check 可完成；不是零警告 |
| `cargo fmt --all -- --check` | 失败 | 当前 HEAD 不满足 CI format 门禁 |
| workspace test binaries (`--no-run`) | 约 249 秒后未成功完成 | 不能声明测试可运行或全绿 |
| types/memory/intel 分组 lib tests | 约 189 秒超时 | 不能声明这些测试失败；验证链路未完成 |
| strict clippy | 失败，RAG 非 UTF-8 | 当前全量源码不能通过 CI lint/编译路径 |
| 历史文档“约 192 项、0 失败” | 2026-07-17 历史记录 | 不能替代当前快照证据 |

**待验证假设**：测试慢可能受到 Windows 链接、依赖缓存、文件编码或并发外部进程影响；需要在干净 checkout 和 CI runner 上拆包计时后才能归因。

## 7. 是否需要重构

**结论**：需要，但只建议**中等范围、以执行一致性为中心的增量重构**；在修复当前构建与契约阻断前，不应开始大规模拆分。

理由：

1. 入口绕过统一执行链是明确的可靠性/安全问题，不能靠格式化解决。
2. no-op 参数已造成“提交称已集成、代码实际未使用”，需要更强的边界与契约测试。
3. 当前 crate 分层和能力目录契约已有可保留价值，重写收益不足。
4. CI、编码和测试基线尚不可信，直接大拆会失去回归判断依据。

## 8. 仍需验证的问题

1. 直接 CLI 和 ACP 绕过安全审查是否是明确产品需求？哪些检查必须一致，哪些允许入口差异？
2. `workflow_config` 是已承诺的公开能力，还是实验参数？配置 schema、错误策略和 phase 语义是什么？
3. `semantic: true` 的兼容承诺是什么：必须真实向量检索，还是允许显式标注 fallback？
4. 当前 77 个工具中哪些属于稳定公共 API，哪些属于 internal/experimental？
5. 生产实际使用的入口占比：MCP、ACP、HTTP、direct CLI 各是多少？
6. 干净 Linux CI 上各 crate 的编译、测试时间和失败项是什么？
7. 最新 tag、公开稳定版本、Cargo 版本应采用哪一个口径？
8. distributed feature、NATS、live provider 和 ignored tests 是否有可重复的测试环境？
9. `aion-zl` 与 `aion-forge-cli-gen` 是否属于正式发布产物，还是 workspace 内部工具？
10. 跟踪的备份/输出文件是否仍承担恢复或审计用途？删除前需要所有者确认。

## 附录 A：本轮直接读取范围

- 配置与构建：根及 11 个 member 的 Cargo.toml、Cargo metadata、CI、release workflow、Dockerfile、docker-compose、router/capability manifests、build/install 文件清单。
- 入口：aion-forge-cli 的 main/lib/cli/direct/mcp/setup/catalog，aion-server 的 main/handlers/error/tests，aion-forge-acp 的 lib/executor/catalog 与入口定位，aion-zl 的 main/engine，CLI generator 的 lib。
- 核心：router lib/executor/config/registry/loader/builtins mod/orchestrator/memory/ai，types/types/capability_registry，intel lib/embedding/rag/planner，memory lib/memory。
- 测试：CLI direct/MCP/release/setup/ACP 契约，server tests，测试分布检索；未逐行读取每个测试文件。
- 文档：README、CHANGELOG、HANDOFF、PROJECT_OVERVIEW、RELEASE_CRITERIA、VERSION_REBASELINE_PLAN、FORGE_ACTUAL_ANALYSIS、UPGRADE_VERIFICATION，以及相关设计/计划文件目录。
