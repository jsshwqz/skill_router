# Forge 当前状态模型

## 2026-08-11 最终复核元信息

- **分析范围**：当前 `HEAD=fbcf581a74af731b353bf7229e96685c12b433af`，复核基线 `c0c7a498ba67ba16453e2932837c00b58ed3a2d9..fbcf581` 的 5 个新增提交、此前全部现状结论、五份交接文档和当前本地门禁；未修改业务代码。本文后续历史快照与本节冲突时，以本节为准。
- **已读取的代码和文档**：新增提交及 3 个变更文件；直接 CLI/ACP/MCP/HTTP 路径；memory semantic recall；WorkflowConfig、AiParallelSolve、AiTripleVote、AiTriangleReview、AiResearch、AiSerialOptimize；Agent delegate/broadcast/gather；Pipeline DAG；熔断器与测试；CI；`docs/refactor/01` 至 `05`。
- **未读取或无法确认的内容**：未逐项执行全部 77 项公开能力；未连接真实模型、NATS、AionUI、GitHub Actions 或生产数据；未完成 workspace lib/doc/全部 integration 测试和跨平台、性能实验；direct contract 在 249 秒内未完成。因此不能确认生产指标、外部兼容性或全部能力行为。
- **结论的证据**：`git diff c0c7a49..fbcf581`、当前源码定位，以及 2026-08-11 fresh 执行的 rustfmt、workspace check、strict clippy、`aion-intel` lib、`aion-router` lib 和 direct contract 测试。
- **当前置信度**：新增提交的静态效果和当前门禁 **高（0.97）**；本文列出的未接入缺口 **高（0.95）**；定向重构方向 **高（0.88）**；生产影响、性能和工期 **低（0.35）**。

## 0. 2026-08-11 最终复核结论（当前有效）

### 0.1 当前快照与门禁

**事实**：`c0c7a49..fbcf581` 包含 5 个提交，仅变更 `agent.rs`、`orchestrator.rs` 和 `05-implementation-report.md`，合计 53 行新增、15 行删除。提交标题声称使用 ReviewMerger、trust_weight、parallel 和 Agent 改进，但代码只完成了其中一部分表面接线。

| 检查项 | Fresh 结果 | 结论 |
|---|---|---|
| `cargo fmt --all -- --check` | **失败** | `orchestrator.rs` 存在缩进、长行和尾随空格；阶段 0B 当前不成立 |
| `cargo check --workspace` | 通过，2 warnings | deprecated `AiSmartCollaborate`；新增 `merged_result` 未使用 |
| strict clippy | **失败：14 errors** | 仍为 `aion-intel` 的 11 个 `collapsible_if`、2 个 `needless_borrow`、1 个 `new_without_default`；workspace 不是零 warning |
| `aion-intel` lib | 27 passed，0 failed | 该 crate 的现有单测保持通过 |
| `aion-router` lib | **94 passed，1 failed** | `circuit_breaker::tests::test_breaker_half_open_success` 仍稳定失败 |
| direct contract | **249 秒超时** | 没有最终通过/失败汇总；不能据此声称 77 项运行契约已通过 |
| CI integration | 仍可吞失败 | [.github/workflows/ci.yml:106](../../.github/workflows/ci.yml#L106) 仍有 `cargo test ... || true` |

### 0.2 新提交的实际效果

| 声明或参数 | 当前事实 | 判定 |
|---|---|---|
| WorkflowConfig 已使用 | [orchestrator.rs:2693](../../aion-router/src/builtins/orchestrator.rs#L2693) 只把 phase 名拼入 `workflow` 字符串；engines、timeout、parallel、retry 不驱动调度 | **仍未真正接入** |
| `trust_weight` 已修复 | [orchestrator.rs:2844](../../aion-router/src/builtins/orchestrator.rs#L2844) 读取后只用于日志，未进入投票或合并输入 | **未实现行为** |
| ReviewMerger 已接入 | [orchestrator.rs:2931](../../aion-router/src/builtins/orchestrator.rs#L2931) 计算 `merged_result`，但响应不包含它，编译器报告 unused | **结果被丢弃** |
| serial optimize parallel | 只有声称 `tokio::join!` 的 doc comment；执行仍顺序 await analyze、optimize、verify | **未实现** |
| research depth | 计算 `engine_count` 并写入 input，但闭包仍固定 `cycle_to_fill(..., 3)` | **未实现行为** |
| Agent timeout/ack/gather | timeout 仅回显；ack 用 delivery count 近似；gather 返回 placeholder；新增重复 `note` key，前值会被覆盖 | **未实现真实协议** |
| semantic recall | [memory.rs:75](../../aion-router/src/builtins/memory.rs#L75) 仍明确使用 keyword fallback | **未实现 semantic** |
| Pipeline DAG | 只有类型和 `topological_sort`，生产执行仍未引用 | **未接入** |

**事实**：这些缺口跨越 orchestrator、Agent、pipeline 和 memory，不是一个统一执行链可以同时解决的问题。

### 0.3 最终重构必要性判断

**结论**：Forge **需要重构**，但证据支持的是“按领域定向重构”，不支持全 workspace 重写，也不支持现在就把 direct CLI/ACP 强制迁入统一执行链。

推荐顺序：

1. 先恢复 fmt、零 warning、strict clippy、熔断器测试和可信 CI，建立可归因基线。
2. 对未实现能力立即收缩契约：真实实现前返回 `unsupported` 或明确 `*_applied: false`，停止静默成功。
3. 对 orchestrator 的配置/调度/结果合并、Agent 消息协作、Pipeline DAG、semantic recall 分别做定向重构和行为测试。
4. direct CLI/ACP 执行链是否收敛继续由 E1—E6 决定，不与上述功能修复捆绑。

这不是“在旧代码上无限打补丁”：目标是以行为契约为边界替换未完成实现；每个领域独立合并、独立回滚。完整候选比较和迁移路线见 `02-refactor-proposal.md` 当前决策。

## 2026-08-04 历史增量复核元信息（已由上节取代）

- **分析范围**：当前 `HEAD=c0c7a498ba67ba16453e2932837c00b58ed3a2d9`，重点复核基线 `9ba54ed6e0499c95d3f6b8c6bd1a2fc820b39b4f..c0c7a49` 的 5 个提交及其对原结论的影响；未修改业务代码。若本节与后文基线描述冲突，以本节为准。
- **已读取的代码和文档**：上述提交列表与 diff、`docs/refactor/03-adversarial-review.md` 至 `05-implementation-report.md`、直接 CLI/ACP/MCP/HTTP 执行入口、memory semantic 分支、WorkflowConfig 与协作工作流、pipeline DAG、Agent delegate/broadcast/gather、AiResearch/AiSerialOptimize、熔断器实现和测试、CI workflow。
- **未读取或无法确认的内容**：未逐行复核全部 builtin 和所有历史提交；未连接真实模型、NATS、AionUI、GitHub Actions 或生产数据；workspace lib 测试在 309 秒内未完成；未运行 doc test、全部集成测试、跨平台发布与性能实验；因此不能确认生产成功率、P95、外部兼容性或当前契约列出的全部 77 项公开能力行为。项目规则中的“70 个能力”与代码契约的 77 项口径不一致，权威口径仍待确认。
- **结论的证据**：当前代码位置、`git diff 9ba54ed..c0c7a49`、文档 03—05，以及本机重新执行的 fmt/check/clippy、`aion-intel` lib 测试、`aion-router` lib 测试。命令结果和失败项记录在下节。
- **当前置信度**：静态架构与本文列出的实现缺口 **高（0.92）**；当前本地门禁状态 **高（0.95）**；熔断器失败根因 **高（0.95）**；真实外部服务、并发性能和生产影响 **低（0.30）**。

## 2026-08-04 历史增量复核结论

### 0.1 快照与门禁

**事实**：从基线到当前 HEAD 共 5 个提交，变更 44 个文件（约 `+1782/-500`），并新增/更新了 `03`—`05` 阶段文档。

| 检查项 | 当前结果 | 证据与解释 |
|---|---|---|
| UTF-8 | 已恢复 | `aion-intel` 可被 rustc/clippy 读取，原非法 UTF-8 阻断不再出现 |
| `cargo fmt --all -- --check` | **失败** | 当前差异位于 `aion-router/src/builtins/orchestrator.rs`；因此 `05-implementation-report.md` 中阶段 0B 的成功记录只代表当时快照，不能代表当前 HEAD |
| `cargo check --workspace` | 通过但有 1 个 warning | `AiSmartCollaborate` deprecated warning，约位于 `orchestrator.rs:3112` |
| strict clippy | **失败：14 errors** | 11 个 `collapsible_if`、2 个 `needless_borrow`、1 个 `new_without_default`，集中于 `aion-intel`；另有 deprecated warning |
| `aion-intel` lib 测试 | 通过 | 27 passed，0 failed |
| `aion-router` lib 测试 | **失败** | 94 passed，1 failed：`circuit_breaker::tests::test_breaker_half_open_success` |
| workspace lib 测试 | 未完成 | `cargo test --workspace --lib --no-fail-fast` 在 309 秒超时，无最终汇总 |
| CI integration gate | 仍可吞失败 | [.github/workflows/ci.yml:106](../../.github/workflows/ci.yml#L106) 仍使用 `cargo test ... || true` |

**事实**：熔断器测试失败是确定性的状态机/测试契约矛盾，不是并行测试偶发错误。测试在 [circuit_breaker.rs:238](../../aion-router/src/circuit_breaker.rs#L238) 把状态推进到 `Open` 后直接调用 `record_success()`，随后期望 `allow_call()` 为真；实现 [circuit_breaker.rs:164](../../aion-router/src/circuit_breaker.rs#L164) 只在 `HalfOpen` 状态收到 success 时关闭熔断器，`Open` 状态保持不变。需要产品/设计选择“修测试以先等待进入 HalfOpen”或“允许 Open 被外部成功直接关闭”，本次现状分析不替代该决策。

### 0.2 原问题的最新状态

| 问题 | 当前判定 | 证据 |
|---|---|---|
| direct CLI/ACP 绕过 `SkillRouter`/`Executor` | **事实仍成立；是否为缺陷待实验** | [direct.rs:41](../../aion-forge-cli/src/direct.rs#L41)、[direct.rs:61](../../aion-forge-cli/src/direct.rs#L61)、[ACP executor.rs:42](../../aion-forge-acp/src/executor.rs#L42)、[ACP executor.rs:90](../../aion-forge-acp/src/executor.rs#L90)。对抗审查正确指出：在威胁模型、性能和兼容性证据不足前，不能直接推出必须统一执行链 |
| semantic recall | **仍是可观测日志中的 keyword fallback** | [memory.rs:75](../../aion-router/src/builtins/memory.rs#L75)；两个分支均调用 `manager.recall`，embedding provider 未接入该路径 |
| WorkflowConfig | **从完全 stub 变为部分解析，但仍未驱动声明的工作流** | [orchestrator.rs:2516](../../aion-router/src/builtins/orchestrator.rs#L2516) 使用手写逐行解析而非 YAML parser；[orchestrator.rs:2693](../../aion-router/src/builtins/orchestrator.rs#L2693) 仅把 phase 名拼入 workflow 字符串。phase engines/timeout/parallel 及全局 timeout/retry 未用于实际调度 |
| Pipeline DAG | **类型存在，执行路径未接入** | [pipeline.rs:29](../../aion-router/src/builtins/pipeline.rs#L29) 定义 `TaskPipelineDAG` 和拓扑排序，但 `TaskPipeline::execute` 仍串行读取 `context["steps"]`；仓库中无生产调用或相应测试 |
| Agent timeout/retry/ack | **retry 有限实现；timeout/ack 仍主要是报告字段** | [agent.rs:29](../../aion-router/src/builtins/agent.rs#L29) 读取 timeout 但没有超时控制；`ack_required` 使用投递数量近似 ack，`wait_timeout_ms` 未参与等待；gather/reduce 仍为占位聚合 |
| Q7 research depth | **参数没有控制真实 engine 数量** | [orchestrator.rs:3160](../../aion-router/src/builtins/orchestrator.rs#L3160) 计算 `engine_count`，但 [orchestrator.rs:3181](../../aion-router/src/builtins/orchestrator.rs#L3181) 固定 `cycle_to_fill(..., 3)` |
| Q6 serial optimize parallel | **未实现** | 文档注释声称读取 `parallel` 并使用 `tokio::join!`，但执行体未读取该参数，仍顺序 await analyze、optimize、verify |

### 0.3 重构必要性修正

**事实**：当前最紧急问题是基线不可信与多个“声明已完成、实际未接入”的功能缺口，不是 crate 边界或代码风格。

**推断**：在 fmt、strict clippy、熔断器契约和 required CI 尚未稳定前开始执行链统一或 orchestrator 拆分，会把已有失败与迁移回归混在一起，降低可归因性。

**当前推荐**：采用 `04-final-design.md` 的 **A+V（最小基线修复 + 行为验证）**，原文后续“直接推荐方案 B”降级为待实验候选。先恢复可重复门禁并补入口行为刻画；仅在 E1—E6 证明 direct CLI/ACP 与 MCP/HTTP 的差异确实造成安全、兼容性或维护成本后，再单独批准执行链收敛。WorkflowConfig、semantic、Agent、DAG、Q6/Q7 属于产品正确性修复，应拆成独立决策与测试，不包装成结构重构。

### 0.4 当前停止条件

- fmt、strict clippy 或 required 测试仍失败时，不进入生产执行路径迁移。
- 无法定义 direct CLI/ACP 的威胁模型和必须一致的 gate 时，不统一执行链。
- WorkflowConfig、semantic、ack、depth/parallel 的产品契约未确定时，不根据提交标题补写实现。
- workspace 测试继续超时，应先分层计时、隔离慢测或外部依赖，而不是把“未完成”记为通过。

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
