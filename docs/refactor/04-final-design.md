# Forge 最终重构设计与实施计划

## 文档元信息

- **分析范围**：提交 `9ba54ed6e0499c95d3f6b8c6bd1a2fc820b39b4f` 所指向的 Forge workspace；综合 [01-current-state.md](./01-current-state.md)、[02-refactor-proposal.md](./02-refactor-proposal.md) 与 [03-adversarial-review.md](./03-adversarial-review.md)，并复核执行入口、安全审查、审批元数据、状态根、no-op 行为、CI 和入口契约测试。本设计只批准文档所列的 A+ 范围，不批准代码行为迁移。
- **已读取的代码和文档**：上述三份交接文档；根与 crate 结构；`aion-forge-cli/src/direct.rs`、`mcp.rs`、`catalog.rs` 及 `tests/direct_contract.rs`、`mcp_contract.rs`、`cli_contract.rs`、`acp_contract.rs`；`aion-forge-acp/src/executor.rs`、`catalog.rs` 及测试目录；`aion-router/src/executor.rs`、`security.rs`、`learner.rs`、`builtins/memory.rs`、`builtins/orchestrator.rs`；`aion-types/src/types.rs`、`capability_registry.rs`；`aion-server/src/tests.rs`；`.github/workflows/ci.yml`；README、CHANGELOG、HANDOFF、项目总览、版本标签与 Cargo 版本。
- **未读取或无法确认的内容**：未逐项执行 77 个能力；未取得生产入口流量、合法请求语料、拒绝率、P95/P99、状态恢复目标和 CI 时间预算；未连接真实 AionUI、OmniRoute、NATS 或第三方 provider；未确认 direct CLI/ACP 的正式威胁模型、审批主体、稳定/实验能力清单和对外版本口径。开局 `session_report` 的输出再次因疑似 API Key 被后置安全审查拦截，因此上次会话的未修复失败状态未知。
- **结论的证据**：当前 HEAD 与三份文档快照一致；direct/ACP 直接执行 builtin 的调用链；`Executor` 的前后审查、learner、metrics 与日志副作用；approval 仅作为目录元数据暴露；memory 的进程 cwd 与 learner 全局单例；WorkflowConfig/semantic recall 的静默降级；CI 的 `|| true`；本轮 `cargo fmt --all -- --check` 失败并确认 `aion-intel/src/rag.rs` 非法 UTF-8；定向 77 项目录契约测试在 189 秒内未完成。
- **当前置信度**：对“当前不应整体重构或迁移入口行为”为 **高（0.93）**；对 A+ 工程基线与行为刻画计划为 **高（0.88）**；对未来是否需要共享执行路径为 **中低（0.45）**；对生产性能和安全收益为 **低（0.30）**。

## 1. 最终设计摘要

最终选择 **A+V：基线恢复 + 兼容性刻画 + 证据实验 + 关闭式决策门**。

这不是原方案 B 的缩小实现，而是撤回其尚未成立的运行时目标。Forge 保留现有 crate 图、入口调用链、77 项名称目录、协议和持久化格式；当前只实施不改变产品行为的工程修复与 Rust 测试。direct CLI/ACP 是否进入共享执行路径，必须在 E1—E6 完成、量化阈值获批、产品与安全负责人确认策略矩阵后，作为新的独立设计重新评审。

### 实施顺序

1. **阶段 0A：UTF-8 阻断修复**——只修复无法解析的 Rust 源文件编码。
2. **阶段 0B：纯格式化**——只处理 rustfmt 已报告的文件，不混入逻辑变化。
3. **阶段 0C：零警告恢复**——只消除已确认 warning，保持 alias 与 no-op 行为不变。
4. **阶段 0D：CI 结果可信化**——分离 required offline 与显式触发的 live tests，移除吞错。
5. **阶段 1：现有行为契约冻结**——补齐 direct、ACP、MCP、HTTP 的输出、错误、退出、cwd 和状态副作用 characterization tests。
6. **阶段 2：E1—E6 独立实验**——每个实验单独验证、单独报告、无生产流量和真实用户状态。
7. **阶段 3：关闭式决策门**——任一阻断项未通过即停止；通过也只允许提出单能力试点的新设计，不自动批准迁移。

阶段 4（orchestrator 拆分）、阶段 5（配置收敛）、WorkflowConfig、semantic recall、Docker、版本文档和仓库清理不属于本批准包。

## 2. 判定口径

| 标签 | 本设计中的含义 |
|---|---|
| **确定事实** | 当前代码、配置、测试或本轮命令可直接复现 |
| **合理推断** | 有多项事实支撑，但缺少生产或差分实验 |
| **待验证假设** | 会影响后续选择，现有证据不足；在验证前对应门保持关闭 |

### 2.1 已确认事实

1. direct CLI 在 [direct.rs:24](../../aion-forge-cli/src/direct.rs#L24) 至 [direct.rs:61](../../aion-forge-cli/src/direct.rs#L61) 自行构造上下文并直接调用 builtin；ACP 在 [executor.rs:48](../../aion-forge-acp/src/executor.rs#L48) 至 [executor.rs:90](../../aion-forge-acp/src/executor.rs#L90) 采用相同类型的旁路。
2. `Executor::execute` 在 [aion-router/src/executor.rs:49](../../aion-router/src/executor.rs#L49) 后执行 prevention、前后安全审查、learner 记录和 execution log；后审拒绝发生在 learner 记录之后、普通 execution log 之前。
3. `Security::validate` 对 `builtin:` 直接放行：[security.rs:18](../../aion-router/src/security.rs#L18)。因此“经过 Executor”等同于“执行时权限已强制”不成立。
4. `requires_approval` 定义在 [capability_registry.rs:52](../../aion-types/src/capability_registry.rs#L52)，MCP/ACP 只把它暴露为目录元数据；`Executor` 不读取它。
5. memory builtin 使用进程 `current_dir()`：[memory.rs:33](../../aion-router/src/builtins/memory.rs#L33)、[memory.rs:64](../../aion-router/src/builtins/memory.rs#L64)；learner 使用进程级 `OnceLock`：[learner.rs:1114](../../aion-router/src/learner.rs#L1114)。`ExecutionContext` 本身没有 workspace 字段：[types.rs:97](../../aion-types/src/types.rs#L97)。
6. WorkflowConfig 仍返回默认值：[orchestrator.rs:2516](../../aion-router/src/builtins/orchestrator.rs#L2516)，加载结果未被执行逻辑使用：[orchestrator.rs:2542](../../aion-router/src/builtins/orchestrator.rs#L2542)；semantic recall 两个分支仍调用相同关键词检索：[memory.rs:62](../../aion-router/src/builtins/memory.rs#L62)。
7. CI 的 fmt/clippy 是硬门禁，但 ignored tests 失败被 `|| true` 转成成功：[ci.yml:36](../../.github/workflows/ci.yml#L36)、[ci.yml:106](../../.github/workflows/ci.yml#L106)。
8. 当前目录契约只证明 77 个名称、声明和 routability 集合一致：[direct_contract.rs:6](../../aion-forge-cli/tests/direct_contract.rs#L6)，不证明副作用、审批、错误外形或安全策略一致。

### 2.2 合理推断

1. direct/ACP 进入现有 `Executor` 很可能改变合法请求的成功率、错误外形、延迟和持久状态，但影响大小未知。
2. 在 workspace scope 和生命周期事件未显式化前，共享执行机制会扩大状态串扰和回滚困难，而不是自动提高一致性。
3. 仅因 `orchestrator.rs` 较大而预设五模块边界，不能保证降低修改或测试半径。

### 2.3 待验证假设

1. direct CLI/ACP 的旁路是设计缺陷，还是本地可信入口的有意行为。
2. 哪些能力必须审批、由谁审批、非交互入口如何拒绝或超时。
3. 77 项中哪些是稳定 API，以及各项真实副作用和权限需求。
4. 合法语料上的安全误报率和 AI reviewer 的可接受延迟。
5. process、workspace、request 三层配置是否需要运行时变更和 secret rotation。

## 3. 方案比较与最终选择

| 方案 | 收益 | 主要风险 | 决策 |
|---|---|---|---|
| A：只恢复基线 | 最快恢复可构建性和 CI 可信度 | 无法获得是否需要后续重构的证据 | 不足以作为最终闭环 |
| **A+V：基线 + characterization + E1—E6** | 不改变产品行为即可量化兼容、安全、状态和回滚问题；每项可停止 | 不能立即消除重复入口代码 | **批准** |
| B-R：直接抽取共享 adapter 并 shadow/迁移 | 可能减少重复并统一部分机制 | shadow 仍可能触发 reviewer、learner、日志或外部调用；当前 scope/approval/错误映射均未成立 | 当前否决 |
| C：描述符驱动全面重构 | 理论上减少元数据重复 | 触及 77 项 schema、入口和发布产物，当前无收益证据 | 否决 |

选择 A+V 的原因是：目前能证明的是“行为不同”和“现有 Executor 有副作用”，不能证明“所有入口必须使用相同策略”。先收集判定证据比先建立生产抽象更小、更可回滚。

## 4. 最终架构设计

### 4.1 当前批准的运行时架构

| 边界 | 批准后的状态 | 本计划允许的变化 |
|---|---|---|
| direct CLI | 继续精确名称查找并直接执行 builtin | 仅增加测试；不接入 `Executor` |
| ACP | 继续使用 `ForgeToolExecutor` 直接执行 builtin | 仅增加测试；不改变 cwd、权限或输出 |
| MCP/HTTP | 继续通过 `SkillRouter`/`Executor` | 仅刻画现有契约；不改变策略 |
| capability/builtin registry | 保留两套现有注册源与 77 项集合测试 | 增加行为/副作用验证，不重写注册模型 |
| security/approval | 保留现有 reviewer 和展示元数据 | 只做离线回放；不宣称 approval 已执行 |
| learner/memory/log | 保留现有持久状态模型 | 实验只用临时目录/独立进程；不迁移真实状态 |
| orchestrator/config | 保留单文件和现有 env 读取时机 | 本批准包不拆分、不快照化 |

不新增 crate、协议层、通用插件框架、事件总线或新的公共 trait。测试辅助类型只放在测试模块或集成测试文件中，不成为公共 API。

### 4.2 条件性未来架构

本设计不预先规定未来共享 adapter、策略对象或模块拆分形态。只有阶段 3 的门全部通过后，才允许提出一个仅覆盖单个无副作用能力的独立设计；该设计必须重新列出 API、文件、状态迁移和回滚方案。通过实验不等于自动采用 `Executor`。

## 5. 对反方评审的逐条回应

| 评审项 | 最终处理 |
|---|---|
| R1 入口不同不等于同策略 | 接受。撤回“默认统一安全/观测”的目标；先产出四入口威胁模型和策略矩阵。 |
| R2 `Executor` 已有误拦截 | 接受。禁止 direct/ACP 迁移；E2 离线回放合法语料与合成密钥样本，生产默认值不变。 |
| R3 approval 未执行 | 接受。approval contract 是迁移前置产品设计；本计划只验证现状，不把元数据当授权。 |
| R4 builtin 权限基本放行 | 接受。先完成 77 项副作用/权限矩阵；不从现有 `PermissionSet` 推导新的入口行为。 |
| R5 cwd/全局状态串扰 | 接受。E3 使用独立进程和临时 workspace 复现；未证明零交叉前 ACP 不进入共享路径。 |
| R6 错误和响应兼容被低估 | 接受。阶段 1 冻结成功、错误、超时、空结果、token usage、quiet、stderr 和退出码外形。 |
| R7 learner/log 会改变后续执行 | 接受。取消“单 revert 足以回滚”；E5 分别验证代码回滚与状态恢复。 |
| R8 生命周期记录顺序不一致 | 接受。E4 先刻画各 gate 的当前事件序列；不先修改生产状态机。 |
| R9 全局配置快照改变生命周期 | 接受。删除全局启动快照；E8 退出本批准包，作为未来独立配置提案的证据前置。 |
| R10 orchestrator 拆分收益不可观测 | 接受。删除预设五模块拆分；只有真实缺陷/变更耦合数据支持时才做单职责垂直切片。 |
| R11 多个产品问题被捆绑 | 接受。WorkflowConfig、semantic recall、Docker、版本文档、仓库清理均拆出独立决策。 |
| R12 缺少量化阈值 | 接受。能客观定义的阈值在本设计中冻结；需要产品输入的阈值未获授权时，后续门默认关闭。 |

## 6. 明确删除和保留的改动

### 6.1 从原提案删除

1. direct CLI/ACP 直接迁入现有 `Executor`。
2. 在生产代码中预先创建 `ExactBuiltinAdapter` 或 shadow mode。
3. 预设 `engine/config/runner/review/skills` 五模块拆分。
4. 进程启动时生成全局配置快照。
5. 将 WorkflowConfig、semantic recall、Docker、版本文档和执行链作为同一重构路线交付。
6. 以 77 项名称一致性证明行为、安全、权限或审批一致性。
7. “代码 revert 即完成回滚”的承诺。

### 6.2 保留

1. 不重写 workspace、不新建 crate、不改变 crate 依赖方向。
2. 恢复 UTF-8、rustfmt、clippy 和可信 CI。
3. 保留 77 项名称、命令、协议、配置名称和持久化格式。
4. 使用 Rust mock、临时目录、独立进程和离线语料完成验证。
5. 每个阶段独立变更、独立验收、独立回滚；实现阶段更新 `05-implementation-report.md`。

## 7. 分阶段实施计划

### 7.1 通用前置规则

- 每个阶段开始时记录 HEAD、OS、`rustc`/Cargo 版本、命令、耗时、测试数量、忽略数量和失败项到 `docs/refactor/05-implementation-report.md`。
- 测试只使用 Rust 测试代码、mock 服务、临时目录和独立子进程；不连接生产数据，不修改真实用户目录。
- 不提交代码、不删除文件、不修改外部服务；是否创建提交由用户另行授权。
- 任一阶段出现范围外文件变化、无法解释的输出差异或持久状态写入，立即停止该阶段。

### 阶段 0A：修复 UTF-8 编译阻断

**修改文件**：`aion-intel/src/rag.rs`。

**不修改**：算法、公开类型、测试期望、其他 Rust 文件、Cargo 依赖。

**前置条件**：记录原文件 blob/hash；确认修复目标是编码而非恢复其他历史实现。

**实施**：将非法字节恢复为有效 UTF-8，只修正文案/注释中不可解码内容；任何可执行 token 差异必须单独审查。

**完成条件与测试**：编译器可解析 `rag.rs`；`cargo check -p aion-intel` 不再报告非法 UTF-8；`cargo fmt --all -- --check` 即使仍因格式失败，也不再因解析 `rag.rs` 终止。

**回滚**：恢复记录的原 blob。该阶段不写业务状态，代码回滚即完整回滚。

### 阶段 0B：纯格式化

**修改文件**：仅限本轮 rustfmt 已报告的以下集合：

- `aion-memory/src/memory.rs`
- `aion-router/src/lib.rs`、`mcp_client.rs`、`security.rs`、`circuit_breaker.rs`、`engine_health.rs`
- `aion-router/src/builtins/agent.rs`、`ai.rs`、`circuit_breaker.rs`、`engine_health.rs`、`mcp.rs`、`memory.rs`、`mod.rs`、`orchestrator.rs`、`parsing.rs`、`pipeline.rs`、`text.rs`、`web.rs`
- `aion-forge-cli/src/catalog.rs`、`direct.rs`、`mcp.rs`、`setup.rs`
- `aion-forge-cli/tests/acp_default_contract.rs`、`direct_contract.rs`、`mcp_contract.rs`
- `aion-server/src/error.rs`、`handlers.rs`、`main.rs`、`tests.rs`
- `aion-zl/src/ai.rs`、`dialectic.rs`、`retry.rs`

阶段 0A 后若 rustfmt 报告新的文件，停止并更新影响清单，不自动扩大范围。

**不修改**：字符串、schema、逻辑、配置、测试断言和依赖。

**前置条件**：阶段 0A 完成；工作树中用户已有变更已单独识别。

**完成条件与测试**：`cargo fmt --all -- --check` 通过；格式变更的 token diff 不包含非空白/导入排序之外的变化；随后运行受影响 crate 的编译检查。

**回滚**：整体回滚纯格式化 change set；不与后续逻辑修复混合。

### 阶段 0C：恢复零警告

**修改文件**：`aion-router/src/builtins/orchestrator.rs`；只有新鲜 `cargo check`/clippy 指向其他文件时，停止并先更新本设计影响清单。

**不修改**：`ai_smart_collaborate` 名称与兼容 alias；WorkflowConfig 的当前返回、日志和执行语义；semantic recall；入口调用链。

**前置条件**：0A/0B 完成，能够获得非缓存的编译诊断。

**实施**：以最窄方式处理 `workflow_config` 未使用和 deprecated compatibility implementation 的 warning；不借机补全功能或移除兼容元数据。

**完成条件与测试**：`cargo check --workspace` 零 warning；`cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过；direct/ACP/MCP 目录仍为 77 项，alias 仍存在。

**回滚**：回滚该单文件逻辑无关变更；无数据回滚。

### 阶段 0D：CI 结果可信化

**修改文件**：`.github/workflows/ci.yml`；实施证据写入 `docs/refactor/05-implementation-report.md`。

**不修改**：release workflow、Dockerfile、外部服务、生产代码、测试实现。

**前置条件**：0A—0C 通过；已记录当前 offline 和 ignored/live 测试的实际耗时与失败项。

**实施**：保留 fmt/clippy/lib/doc 为 required；新增或明确 non-ignored integration 的 required 步骤；ignored/live 测试只在显式启用的环境运行，命令本身不得吞错，未启用时显示 skipped。由于 required CI 时间预算尚未获授权，live tests 不得升级为 required。

**完成条件与测试**：workflow 中不再出现将测试失败转成功的 `|| true`；离线失败导致 required job 失败；live 未配置显示 skipped，已配置时保留真实 passed/failed；报告记录每个 job 的命令和耗时。

**回滚**：只回滚 CI 文件；不会影响运行时和持久数据。

### 阶段 1：冻结现有入口兼容契约

**修改文件**：

- `aion-forge-cli/tests/direct_contract.rs`
- `aion-forge-cli/tests/mcp_contract.rs`
- `aion-forge-cli/tests/cli_contract.rs`
- `aion-forge-cli/tests/acp_contract.rs`
- `aion-forge-acp/src/executor.rs` 的测试模块
- `aion-server/src/tests.rs`
- `aion-router/src/executor.rs`、`security.rs`、`learner.rs` 的测试模块
- `docs/refactor/05-implementation-report.md`

**不修改**：上述 Rust 文件的非测试路径；公开 API/schema；reviewer、learner、memory、日志和入口行为。

**前置条件**：阶段 0 全部通过；所有测试目录均指向临时路径，环境变量测试使用独立子进程，避免进程全局污染。

**测试矩阵**：

1. 每个入口的成功、未知能力、参数解析失败、builtin `error` 字段、Rust error、空结果、timeout 和部分结果。
2. direct 的 `--quiet` 单行 JSON、stdout/stderr、进程退出码；ACP tool result；MCP error result；HTTP 状态码与响应字段。
3. cwd/root_dir、`.skill-router`、memory、learner 和 execution log 的实际落点。
4. reviewer allow/deny/unavailable 与 fail-open/fail-closed 的当前外形。
5. 77 项名称集合继续作为目录契约，但不替代行为测试。

**完成条件**：characterization tests 精确描述当前行为且在无外网环境稳定重复三次；同一测试的临时目录间无残留；报告列出已覆盖和未覆盖能力。当前缺陷可作为“现状”被测试锁定，但必须在报告中明确，不得被描述为期望产品行为。

**回滚**：回滚测试 change set；无生产状态回滚。

### 阶段 2：E1—E6 独立验证实验

**前置条件**：阶段 1 的 characterization tests 已稳定重复三次；测试语料完成脱敏分类；所有状态路径均指向临时目录；实验不依赖真实外部服务。

每个实验是独立 change set；前一个实验失败不阻止记录结果，但会关闭阶段 3 的迁移门。

| 实验 | 修改文件 | 完成条件 | 明确不修改 | 回滚 |
|---|---|---|---|---|
| E1 入口差分 | 新建 `aion-forge-cli/tests/execution_diff.rs`；更新实施报告 | 对 `yaml_parse`、`json_parse`、`text_extract` 等无副作用样本，旧 direct/ACP 与测试内候选路径除批准差异外，结果、错误、stdout/stderr、退出语义差异为 0 | 不新增生产 adapter，不切换入口 | 删除测试 change set |
| E2 安全误报/延迟 | 新建 `aion-router/tests/security_replay.rs`；必要测试 helper 留在 `security.rs` 测试模块 | 合法配置、代码、日志、安全报告语料误拒绝为 0；合成 key 样本按规则拒绝；记录 heuristic 与 mock AI 的 P50/P95/P99。产品未批准延迟上限前迁移门保持关闭 | 不调用真实 AI，不改 fail policy 和豁免列表 | 回滚测试；无状态 |
| E3 状态隔离 | 新建 `aion-forge-acp/tests/workspace_isolation.rs` | 两个独立子进程/临时 workspace 的 memory、learner、log 文件和决策交叉为 0；同进程并发结果单独记录 | 不改 `OnceLock`、`current_dir()` 或真实用户目录 | 回滚测试；临时目录由测试生命周期回收 |
| E4 生命周期审计 | 新建 `aion-router/tests/execution_lifecycle.rs` | 前审拒绝、builtin error、后审拒绝、日志失败均有可复现事件表；客户端、metrics、learner、audit、log 能否关联必须逐项给出。缺少 request ID 或状态不一致即实验结论为不通过 | 不先修改生产记录顺序或添加公共字段 | 回滚测试；无状态 |
| E5 回滚演练 | 新建 `aion-forge-cli/tests/state_rollback.rs` | 候选执行产生的临时 learner/log 状态在切回旧路径后不再改变决策；若需清理步骤，必须证明可重复且不触及范围外路径。RTO/RPO 未获授权时迁移门保持关闭 | 不操作真实状态、不宣称代码 revert 足够 | 回滚测试和临时状态 |
| E6 权限/审批 | 新建 `aion-types/tests/capability_effects_contract.rs` 与 `aion-forge-cli/tests/approval_contract.rs` | 77 项均有读/写/进程/网络/凭据/持久状态分类；所有 `requires_approval=true` 项有可验证的执行时主体、凭证、拒绝、超时和非交互策略。当前实现缺失 enforcement 时结论为不通过 | 不增加默认批准，不改变公共 schema | 回滚测试矩阵 |

所有实验命令、样本分类、耗时和结果写入 `05-implementation-report.md`。测试中使用的密钥样本必须是明确不可用的合成字符串，报告只记录样本类别，不复制完整模式。

### 阶段 3：关闭式决策门

**修改文件**：只更新 `docs/refactor/05-implementation-report.md` 和本设计的决策状态；不修改 Rust 代码。

**不修改**：生产入口、`Executor`、安全/审批策略、状态 schema、配置或协议。

**前置条件**：E1—E6 均已有可复现结果；每项结果包含环境、输入类别、命令、耗时、通过/失败和未验证限制。

**允许提出单能力试点的必要条件**：

1. 产品/安全负责人确认四入口威胁模型、审批主体和策略矩阵。
2. E1 的未批准输出/错误/退出差异为 0。
3. E2 合法语料误拒绝为 0，且产品已批准 P95/P99 上限与 reviewer unavailable 策略。
4. E3 跨 workspace 污染为 0。
5. E4 每个请求可完整关联，或已先通过独立生命周期修复设计。
6. E5 已批准并达成 RTO/RPO；代码和状态回滚均可复现。
7. E6 完成 77 项矩阵，approval 在执行时真正生效。
8. required CI 时间预算已获批准，测试在该预算内稳定三次。

任一条件不满足，最终状态为“停止在 A+V”，不创建生产共享 adapter、不迁移入口。全部满足时，也只允许起草一个无副作用、无网络、无 approval、无持久状态能力的试点设计；该设计需单独批准。

**完成条件与验收**：对八项必要条件逐项给出通过/不通过和证据链接；只允许“停止在 A+V”或“允许起草单能力试点”两个结论，不得把“允许起草”写成实现批准。

**回滚**：该阶段只有文档决策，可回滚文档 change set；由于没有运行时改动，不涉及代码或状态恢复。

## 8. API、配置、数据与行为兼容策略

### 8.1 API/协议

- 保留 `aion-forge`、`aion-forge-cli`、无参数默认 ACP、`acp`、`mcp-server`、`setup`、`--tool/--params/--list/--quiet`。
- 保留 77 个公开名称、现有输入 schema 与 approval 元数据；不新增或删除公共字段。
- 保留 MCP `2025-11-25`、stdio 纯协议 stdout、HTTP 路径/状态码、公开 `/v1/health`、Bearer 与 CORS 当前默认行为。
- characterization tests 先锁定当前错误外形；任何未来差异都按 breaking behavior 单独审批。

### 8.2 配置

- 本批准包不改变环境变量名、默认值、优先级或读取时机。
- 不创建全局启动快照，不改变 secret rotation。
- 测试通过独立进程注入环境；日志和报告只记录变量是否设置，不记录值。

### 8.3 数据

- 不修改 redb schema、旧 JSON 迁移、`.skill-router`、registry、learner 或 execution log 格式。
- 实验只写临时目录或显式独立命名空间；任何真实状态写入视为阶段失败。
- 回滚分为代码回滚与状态恢复；没有 E5 证据前不宣称运行时迁移可回滚。

### 8.4 行为

- direct/ACP 保持直接 builtin 执行；MCP/HTTP 保持当前 router/Executor 路径。
- WorkflowConfig 和 semantic recall 的静默行为在本批准包中保持不变，但列为独立产品缺陷，不得借本次重构补全或移除。
- `ai_smart_collaborate` 兼容 alias 保留。

## 9. 观测、日志、指标与诊断

### 9.1 当前批准范围

- 不增加生产 metrics、日志字段或外部 telemetry。
- 每个阶段在 `05-implementation-report.md` 记录：commit、平台、Rust 版本、命令、退出状态、测试/忽略数量、耗时、失败项、临时状态根和是否访问网络。
- E1 记录结构化差异类别；E2 记录误拒绝计数和延迟分位；E3 记录跨目录文件/记录计数；E4 记录 gate × client/metrics/learner/audit/log 事件表；E5 记录恢复步骤和耗时；E6 记录能力副作用与审批覆盖率。

### 9.2 未来试点的前置观测要求

若阶段 3 通过，试点必须在不改变公共响应的前提下拥有测试可见的 request ID、阶段状态和独立临时状态根；正式字段设计仍需新提案。没有这些诊断信息，不进入生产试点。

## 10. 测试与验收总表

| 层级 | 必须通过 | 不能据此声称 |
|---|---|---|
| 编码/格式 | Rust 源可解析；`cargo fmt --all -- --check` | 业务正确 |
| 静态质量 | workspace check；全 targets/features clippy 且 `-D warnings` | 外部服务可用 |
| 离线测试 | workspace lib/doc/non-ignored integration；入口 characterization | live provider/NATS 正常 |
| 目录契约 | 77 名称唯一、声明与 builtin routability 一致 | 77 项副作用/approval 正确 |
| 差分实验 | 未批准的输出、错误、退出差异为 0 | 全入口应采用同一策略 |
| 隔离/回滚 | 跨 workspace 污染为 0；代码与状态恢复可复现 | 单 revert 足够 |
| 性能/安全 | 误拒绝与延迟按批准阈值验收 | 无阈值时宣称改善 |

本轮定向目录契约测试在 189 秒内未完成，因此当前不能声称该测试通过或失败；实施报告必须记录 clean/cached 两类耗时，避免再次用历史结果替代当前证据。

## 11. 风险、未决问题与停止条件

### 11.1 未解决风险

1. 上次会话报告被安全审查拦截，未知失败可能未被继承。
2. 非法 UTF-8 的正确字节恢复来源尚未确认；错误修复可能改变源码 token。
3. 全 workspace 测试耗时和干净 Linux CI 结果未知。
4. direct/ACP 的旁路可能是有意产品边界，也可能是遗漏。
5. approval 无执行时契约；权限元数据与真实 builtin 副作用可能漂移。
6. learner 全局单例和 memory cwd 可能造成多工作区串扰。
7. README v0.3.0/29+、Cargo 0.7.0、tag v0.7.1、文档 75 与测试 77 的权威口径未确定。
8. WorkflowConfig 与 semantic recall 是实现还是明确 unsupported，仍需独立产品决策。
9. required CI 最大时长、live test 策略、误拒绝率、P95/P99、RTO/RPO 尚未获授权。

### 11.2 停止条件

- 0A 无法证明只修编码而未改变可执行 token。
- 0B 出现范围外文件或非格式差异。
- 0C/0D 无法恢复零 warning 和不吞错的离线门禁。
- characterization tests 受真实用户状态、外网或非确定性影响而无法稳定重复。
- E1—E6 任一阻断条件失败，或所需量化阈值未获授权。
- 单能力试点的收益不能用修改文件数、缺陷定位时间、误拒绝率、延迟或维护成本中的至少一项量化。

停止在任一阶段不视为失败；已完成的编码、格式、CI 和行为契约仍有独立价值。

## 12. 本任务交付边界

本任务只创建 `docs/refactor/04-final-design.md`，不修改 Rust 代码、CI、配置、外部服务或持久状态，也不提交代码。后续实施每完成一个阶段，必须更新 `docs/refactor/05-implementation-report.md`，诚实记录失败测试、未完成项和未验证假设。
