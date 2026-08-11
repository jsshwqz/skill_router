# Forge 可验证重构提案

## 2026-08-11 最终复核元信息

- **分析范围**：以当前 `HEAD=fbcf581a74af731b353bf7229e96685c12b433af` 为最终提案快照，吸收新增 5 个提交的静态效果和 fresh 门禁结果；只更新提案文档，不修改 Rust 业务代码。后续历史方案与本节冲突时，以本节为准。
- **已读取的代码和文档**：继承 `01-current-state.md` 的 2026-08-11 读取范围，重点复核未实现能力是否共享根因、最新 ReviewMerger/trust_weight/WorkflowConfig/Agent 改动，以及 `03`—`05` 的决策和实施证据。
- **未读取或无法确认的内容**：生产入口占比、正式威胁模型、真实 provider/NATS 行为、远端 CI、完整 workspace/doc/integration 测试、跨平台与性能基线均未确认；因此不批准全量重写或入口执行链迁移。
- **结论的证据**：当前代码位置、`c0c7a49..fbcf581` 差异、fresh fmt/check/clippy/分层测试，以及 `01-current-state.md` 当前第 0 节。
- **当前置信度**：定向重构必要性 **高（0.88）**；全 workspace 重写收益 **低（0.25）**；立即统一执行链收益 **低到中（0.45）**；分阶段路线可回滚性 **高（0.90）**。

## 0. 当前最终决策：能力收缩 + 定向重构

推荐 **先恢复可信基线和收缩虚假能力承诺，再按领域替换未完成实现**。A+V 是必须完成的阶段 0/1，不是长期终点；原“方案 B：执行链收敛”继续保留为实验后的可选项，不与功能重构绑定。

### 0.1 候选方案比较

| 方案 | 收益 | 风险/成本 | 当前决定 |
|---|---|---|---|
| A. 原地逐项修补 | 短期改动最少 | 延续“字段读取/日志输出即完成”的模式，半实现继续堆积 | **不推荐** |
| B. 全 workspace 或描述符驱动重写 | 可重新统一结构 | 触及 77 项能力和所有入口；产品语义、测试和生产数据不足 | **否决** |
| C. 立即统一 direct CLI/ACP 执行链 | 可能统一安全和观测副作用 | 威胁模型、兼容性、延迟和状态污染尚未验证 | **暂缓，等待 E1—E6** |
| D. 能力收缩 + 按领域定向重构 | 直接解决已证实的未接入问题；范围清楚，可独立回滚 | 需要先定义行为契约，不能一次性宣称全部完成 | **推荐** |

### 0.2 定向重构边界

| 领域 | 允许重构的责任 | 必须先固定的契约 |
|---|---|---|
| orchestrator | WorkflowConfig 解析/校验、phase 调度、engine 数量、parallel、vote/review merge | phase 顺序、超时、重试、权重、降级、结果外形 |
| Agent | delegate timeout/retry、broadcast ack、gather/reduce 响应收集 | ack 主体、correlation、超时语义、部分成功与重复消息 |
| Pipeline | DAG 反序列化、依赖校验、拓扑执行与失败传播 | 环检测、并发规则、输出引用、取消和重试 |
| Memory | semantic 索引、provider、持久化与显式 fallback | 相似度、索引生命周期、provider 失败和兼容响应 |

不新增通用框架或新 crate；优先从现有文件提取只服务上述行为的模块。若某能力近期没有产品承诺，先返回明确 `unsupported`，而不是补建抽象。

### 0.3 分阶段迁移路线

1. **阶段 0：恢复工程基线**
   - rustfmt、workspace check 零 warning、strict clippy 全通过。
   - 先确定熔断器 Open/HalfOpen 契约，再修实现或测试。
   - CI required job 移除吞失败；分层记录测试终态和耗时。
2. **阶段 1：能力真实性门禁**
   - 为 WorkflowConfig、trust_weight、ReviewMerger、depth、parallel、Agent、DAG、semantic 增加失败的行为刻画测试。
   - 未准备实现的参数返回显式 unsupported/fallback 状态。
3. **阶段 2A：orchestrator 定向重构**
   - 先让配置字段和 engine/merge 参数真实影响执行；再按配置、runner、merge 的现有责任拆分。
   - 不同时迁移入口，不改变公共 JSON 外形。
4. **阶段 2B—2D：Agent、Pipeline、Memory 独立重构**
   - 每个领域单独提交、单独测试、单独回滚；不得跨领域共享未经证明的抽象。
5. **阶段 3：E1—E6 入口实验**
   - 比较输入输出、延迟、权限、安全 gate、metrics、learner、日志和状态目录。
   - 只有实验超过预先批准的收益阈值，才另行设计单能力执行链试点。

### 0.4 成功标准与不可破坏行为

- fmt、workspace check、strict clippy、workspace lib/doc/required integration 测试均获得完整成功终态；不得超时后推定通过。
- 每个已声明参数都有行为证据：输入变化导致对应调度、权重、合并、超时、ack、DAG 或检索变化；否则明确 unsupported。
- 公开能力名称、CLI 命令、MCP schema/protocol、HTTP route/status/auth/CORS、持久化格式和环境变量优先级保持兼容。
- 现有 77 项代码契约与项目规则“70 个能力”的口径先确认；未经批准不增删公共能力。
- 不迁移 direct CLI/ACP 执行链，不改变当前安全语义，除非 E1—E6 和新的设计评审均通过。
- 每一领域可用单独 revert 回滚；回滚不得要求同时撤销其他领域修复。

### 0.5 必须进一步确认

1. WorkflowConfig、DAG、semantic、Agent gather 是否属于稳定承诺；若不是，是否接受立即返回 unsupported？
2. `trust_weight` 是全局权重、单引擎权重还是最低信任阈值？
3. Agent ack 的确认主体、消息格式、最小确认数和超时后的返回状态是什么？
4. serial optimize 的 analyze 与 optimize 存在数据依赖；所谓 parallel 是否实际指多候选并行，而非两个阶段并行？
5. 代码契约 77 项与项目规则 70 项，哪个是发布权威口径？

## 2026-08-04 历史增量复核元信息（已由上节取代）

- **分析范围**：以当前 `HEAD=c0c7a498ba67ba16453e2932837c00b58ed3a2d9` 为提案快照，吸收 `03-adversarial-review.md`、`04-final-design.md`、`05-implementation-report.md` 和最新代码/门禁结果；不修改业务代码。若本节与后文旧推荐冲突，以本节为准。
- **已读取的代码和文档**：继承 `01-current-state.md` 的增量读取范围，重点复核执行入口、WorkflowConfig、semantic recall、pipeline DAG、Agent 协作能力、Q6/Q7、熔断器、CI，以及五份 refactor 文档之间的结论一致性。
- **未读取或无法确认的内容**：生产入口占比、正式威胁模型、真实 provider/NATS 行为、远端 CI、完整 workspace/doc/integration 测试、跨平台与性能基线均未确认；因此本提案不批准生产执行链迁移。
- **结论的证据**：`9ba54ed..c0c7a49` 代码差异、当前源码定位、对抗审查的反例、最终设计的停止条件，以及 2026-08-04 本机 fmt/check/clippy/分层测试结果；详见 `01-current-state.md` 第 0 节。
- **当前置信度**：推荐先做 A+V **高（0.91）**；直接统一执行链的净收益 **低到中（0.45）**；各功能缺口的静态判定 **高（0.90）**；工期和生产影响 **低（0.35）**。

## 2026-08-04 历史决策（已由当前最终决策取代）

推荐 **A+V：最小基线修复 + 行为验证**。这是对后文基线提案经过对抗审查和最新实现复核后的收窄，不批准立即进行执行链统一、orchestrator 拆分或新抽象引入。

| 候选 | 当前选择 | 原因 |
|---|---|---|
| **A+V：基线修复 + 刻画/实验** | **推荐** | 直接解决当前 fmt/clippy/test/CI 失败；以最小变更获得后续决策证据；每步可独立回滚 |
| **B：执行链收敛** | 有条件保留 | 代码差异确定，但安全/兼容性/性能收益尚未由 E1—E6 证明；当前实施会混入不可信基线 |
| **C：描述符驱动全面重构** | 不选 | 迁移面覆盖入口、schema 和全部能力，当前没有与风险相称的收益证据 |
| **停止结构重构，仅修明确缺陷** | 可接受退路 | 若实验表明入口差异是刻意契约，或收益不足，则完成基线与产品缺陷修复后停止 |

### 0.1 分阶段路线

1. **阶段 0A—0D：恢复可信基线**
   - 保留已完成的 UTF-8 恢复；修复当前 rustfmt 差异。
   - 清理 strict clippy 的 14 个 error 与 deprecated warning，不改变业务语义。
   - 先决定熔断器契约，再修正实现或测试；要求该失败可单独复现并消失。
   - required CI 不得使用 `|| true`；慢测/外部集成可拆为明确的 optional job，但必须如实显示失败或未运行。
2. **阶段 1：行为刻画**
   - 固化当前代码契约中的 77 项公开能力名称/路由、MCP/HTTP/CLI/ACP 外形、错误映射、权限与审计副作用；同时确认项目规则“70 个能力”与契约 77 项之间的权威口径。
   - 对 workspace 测试按 crate 分层计时，记录 passed/failed/ignored/timeout，不以总命令超时替代结论。
3. **阶段 2：独立修复产品契约**
   - WorkflowConfig、semantic recall、Agent timeout/ack/gather、Pipeline DAG、research depth、serial optimize parallel 各自建立 failing characterization test 和明确契约。
   - 每项独立 PR/提交；不得仅让字段被读取或写入响应就声称功能生效。
4. **阶段 3：执行 E1—E6 决策实验**
   - 比较各入口的输入、输出、延迟、权限、安全 gate、metrics、learner 与 execution log。
   - 只有差异违反明确契约且共享路径能在兼容预算内解决时，才批准方案 B；否则停止。
5. **阶段 4：可选迁移**
   - 若获批准，先迁移 direct CLI，再迁移 ACP；MCP/HTTP 保持现状。
   - 每迁移一个入口后重跑同一套契约测试；任何未解释的输出或安全语义变化立即回滚该阶段。

### 0.2 当前成功标准

- `cargo fmt --all -- --check`、strict clippy、workspace check 全部无 warning 通过。
- `aion-router` 当前 95 个 lib 测试全部通过；workspace lib/doc/required integration 测试获得完整终态，不超时、不吞失败。
- 远端 required CI 与本地门禁一致，任何 optional 失败清楚标记。
- 上述 6 类产品参数有行为级测试：输入变化必须导致声明的调度/检索/超时/ack/并发变化，或明确返回 unsupported/fallback 状态。
- 公开能力名称、CLI 命令、MCP schema/protocol、HTTP route/status/auth/CORS、持久化格式和环境变量优先级保持兼容，除非另行批准 breaking change。
- 在 E1—E6 完成前，生产入口执行链和安全语义保持现状。

### 0.3 为什么不再直接推荐方案 B

后文原方案把“代码路径不同”进一步推断为“已证明的安全/观测缺陷”。最新对抗审查指出这一推断缺少入口威胁模型、兼容性与性能实验；当前代码又新增多个未真正接入执行路径的功能声明，并重新引入 fmt 失败。继续按 B 迁移会扩大变量数量。A+V 能先修复确定问题，再用可复现实验决定是否存在值得迁移的净收益。

## 文档元信息

- **分析范围**：以 [01-current-state.md](./01-current-state.md) 的 `9ba54ed6e0499c95d3f6b8c6bd1a2fc820b39b4f` 快照为依据，设计不修改业务代码的候选重构路线。
- **已读取的代码和文档**：继承现状文档的读取范围，重点复核 `SkillRouter`、`Executor`、直接 CLI、ACP executor、MCP/HTTP 入口、能力目录契约、orchestrator、配置读取、CI/发布和现有测试。
- **未读取或无法确认的内容**：真实生产入口占比、外部模型/NATS/AionUI 行为、干净 CI 的完整测试结果、所有 77 个能力的逐项兼容契约、产品对 WorkflowConfig/semantic fallback 的正式承诺。
- **结论的证据**：入口调用链、统一 Executor 已有职责、77 工具契约测试、当前 CI/编码失败、no-op 参数、热点文件规模和文档漂移；详见现状文档第 3—6 节。
- **当前置信度**：推荐方向 **中高（0.82）**；具体 API 形态 **中（0.65）**；工期估计 **中低（0.50）**，需先取得干净构建和测试计时。

## 1. 决策摘要

推荐 **方案 B：执行链纵向收敛 + 编排模块渐进拆分**。

这不是 workspace 重写。保留现有 crate、77 项公开能力目录、SkillRouter、Executor、协议入口和持久化格式；先恢复可信基线，再让直接 CLI 与 ACP 复用统一执行内核，随后只拆分已证明高风险的 `orchestrator.rs` 职责，并选择性收敛关键配置。

重构必要性：**有条件成立**。如果团队不打算保证 direct CLI/ACP 与 MCP/HTTP 的安全和观测一致性，也不承诺 WorkflowConfig/semantic 参数，则应选择方案 A 并缩小范围；如果这些是稳定产品能力，则方案 B 的收益明确。

## 2. 目标与非目标

### 2.1 目标

1. 同一 builtin 在各入口共享明确的权限、安全、指标、学习和日志策略。
2. 对外参数要么真实生效，要么返回可观察的 unsupported/fallback 状态，禁止静默成功。
3. 建立当前快照可重复、不会吞错的格式/编译/测试门禁。
4. 降低编排核心的改动半径，使 workflow、引擎调用、投票合并可独立测试。
5. 统一关键配置的解析和默认值，同时保留环境变量名称及优先级兼容性。
6. 每一阶段可独立合并、可用单个 revert 回滚。

### 2.2 非目标

1. 不重写全部 crate，不把现有 Rust trait 体系替换成网络微服务。
2. 不为“架构漂亮”增加通用插件框架、事件溯源或新协议层。
3. 不在没有数据时优化多模型性能或替换 provider。
4. 不删除 ACP、ZL、CLI generator、备份或运行输出；是否保留需单独决策。
5. 不改变 77 个公开名称、MCP/HTTP 协议、持久化格式或命令名称，除非单独批准 breaking change。
6. 不把功能补全伪装成纯重构；WorkflowConfig 和 semantic recall 的行为修复必须单独标注。

## 3. 候选方案

| 方案 | 内容 | 主要收益 | 成本与风险 | 适用条件 |
|---|---|---|---|---|
| **A. 最小修复，不做结构重构** | 修复 UTF-8、fmt/clippy、unused/deprecated warning；让 no-op 参数明确报错或标注 fallback；修正 CI `|| true` 和文档 | 最快恢复可信基线，回归风险最低 | direct CLI/ACP 旁路、热点文件和配置漂移继续存在；后续每次安全修复需多入口同步 | 产品暂不要求入口一致，或近期只能投入很小成本 |
| **B. 执行链纵向收敛（推荐）** | 包含 A；在 router 内提供“精确能力执行”共享路径，使 direct CLI/ACP 经过 Executor；再按现有职责拆 orchestrator；选择性收敛关键配置 | 直接解决已证实的安全/观测不一致，复用已有能力，不改变 crate 图；可逐入口迁移 | 安全审查可能暴露历史上被旁路的请求；需要输出兼容与 mock 测试 | 77 项能力需作为稳定产品维护，多个入口都继续支持 |
| **C. 描述符驱动的全面重构** | 每个 builtin 同时提供名称、schema、权限、执行实现；自动生成 capability/MCP/ACP/manifest；重划 router 子 crate | 理论上最大限度消除元数据重复和大文件 | 触及 77 项公共 schema、所有入口和发布产物；迁移面大，当前测试基线不足；现有名称一致性测试已覆盖核心漂移 | 只有在发现大量 schema/实现漂移并有完整契约测试后才考虑 |

## 4. 推荐方案 B 的设计

### 4.1 先稳定，再重构

在任何模块移动前建立“基线修复 PR”：

- 恢复所有 Rust 源文件为有效 UTF-8。
- 运行 rustfmt，并把纯格式化与行为改动分开提交/PR，便于审查。
- 消除当前 warning，使 CI 的 `-D warnings` 有实际约束力。
- 将 integration job 的允许失败改成显式的 required/optional 两类；optional job 可不阻断，但不得显示为“测试通过”。
- 在干净 checkout 上记录按 crate 的 build/test 时间和失败测试。

此阶段不改变路由、输出或 provider 行为。若基线无法在干净 CI 恢复，应暂停后续重构。

### 4.2 建立共享的“精确能力执行”路径

目标不是新建一层通用框架，而是把 direct CLI/ACP 已重复的 4 个步骤收回 router：能力元数据查找、临时 SkillDefinition 构造、ExecutionContext 构造、`Executor::execute` 调用。

设计约束：

1. 接口接受精确 capability 名称，不触发自然语言重新分类或动态发现。
2. 调用者显式传入 workspace、source 和 JSON context；不从当前目录偷偷重算已有参数。
3. 复用 `CapabilityRegistry` 和 `BuiltinRegistry`，不建立第三套注册表。
4. 返回现有 `ExecutionResponse`，由 direct CLI/ACP 适配回各自原有 JSON 外形。
5. 安全、metrics、learner、post-review 和 execution log 默认统一；若某入口确需差异，用窄的策略参数表达并写契约测试，不用入口内旁路。

迁移顺序：先 direct CLI，验证稳定后再 ACP。MCP/HTTP 已走 `SkillRouter`，本阶段不改。

### 4.3 明确修复两个 no-op 契约

这是功能正确性工作，应与结构移动分开：

**WorkflowConfig** 有两个可选落地，需产品确认后选择：

- 若为稳定能力：定义 YAML schema，使用已有 Rust YAML 依赖解析；校验 phase 名称、engine、timeout、retry；实际驱动 workflow；非法配置返回错误。
- 若为实验/未承诺能力：暂时从公开 schema 移除或返回 `unsupported`，而不是返回默认配置并声称加载成功。

**semantic recall** 同样二选一：

- 接入同一 embedding provider 和可持久化索引，响应标明 provider/是否降级；
- 或在未实现前对 `semantic: true` 返回明确 unsupported。若保留 fallback，必须返回 `semantic_applied: false` 与 fallback 原因。

不建议继续保留不可观察的关键词回退。

### 4.4 渐进拆分 orchestrator，不新建 crate

只在行为契约通过后，在 `aion-router/src/builtins/orchestrator/` 内按已有职责移动代码：

| 建议模块 | 只负责 | 首要契约 |
|---|---|---|
| `engine.rs` | Engine 标签、可用性、provider/CLI 调用 | engine label、禁用列表、fallback 顺序 |
| `config.rs` | OrchestratorConfig、WorkflowConfig 解析/校验 | env 默认值、YAML 错误、phase 顺序 |
| `runner.rs` | 并行/串行执行、timeout、async task | 超时、并发上限、结果顺序 |
| `review.rs` | vote、merge、triangle/cross review | 权重、冲突、降级状态 |
| `skills.rs` | BuiltinSkill 适配与输入/输出 JSON | 77 工具中的既有 schema/响应 |

拆分只做移动和可见性收窄，不同时重写算法。每个移动 PR 前后运行相同契约测试；任何输出差异都视为行为变更并单独评审。

### 4.5 选择性收敛配置

不一次性替换 30 个文件的 env 读取。先处理会改变入口语义的四类：

1. workspace/path；
2. provider/base URL/model/key 的解析顺序；
3. timeout/retry/concurrency；
4. security、passthrough、MCP mode。

进程启动时生成不可泄密的配置快照或窄配置结构，传给共享执行路径。保留现有环境变量名和优先级；secret 不进入 Debug、日志或报告。server 的 host/port/CORS 等入口专属配置可继续留在 server。

### 4.6 能力元数据暂不重写

保留 `CapabilityRegistry::builtin()`、`BuiltinRegistry::default_registry()` 和现有 77 项集合契约。先增加以下验证即可：

- 每个公开 schema 的 required 字段能被对应 builtin 接受；
- `requires_approval` 与执行策略一致；
- manifests 是生成物还是源文件必须明确，并加 drift check。

只有出现持续的 schema/实现漂移证据，才重新评估方案 C。

## 5. 为什么不选其他方案

### 不推荐仅停在方案 A

方案 A 能恢复 CI，却不会解决 direct CLI/ACP 绕过 Executor 的结构性差异。只要这些入口继续支持，安全、学习和观测修复就要重复实现或继续遗漏。它适合作为方案 B 的阶段 0，而不是长期终点。

若产品明确声明 direct CLI/ACP 是不受统一安全策略约束的开发入口，则方案 A 反而应成为最终方案；这是需要确认的停止条件。

### 不推荐方案 C

当前已有 77 名称一致性契约，未发现足够证据证明注册表设计是主要故障源。方案 C 会同时触及元数据、执行、协议、生成物和全部能力，且当前连全量测试基线都不可信。收益主要是架构整洁，风险远高于已证实收益。

## 6. 不重构与重构的成本/风险

| 维度 | 不重构/仅修复 | 推荐重构 |
|---|---|---|
| 短期交付 | 最快 | 较慢，需先补基线 |
| 回归风险 | 单次低 | 每阶段中低；可通过逐入口迁移控制 |
| 安全一致性 | 旁路长期存在 | 可统一到 Executor |
| 维护成本 | 每个入口重复适配和审查 | 共享路径一次修复，多入口受益 |
| 编排改动风险 | 3378 行热点继续扩大 | 拆分后局部测试和审查更清晰 |
| 兼容风险 | 最低 | 安全审查可能改变过去旁路成功的调用 |
| 停止成本 | 无 | 任一阶段可停止，已完成阶段仍有独立价值 |

## 7. 成功标准

### 7.1 必须全部满足的工程门禁

1. `cargo fmt --all -- --check` 通过。
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
3. `cargo test --lib --workspace`、`cargo test --doc --workspace` 和入口契约测试通过；不得用 `|| true` 掩盖 required job 失败。
4. 测试结果记录 commit、平台、Rust 版本、测试数量、忽略数量、耗时和失败项。
5. Rust 源文件 UTF-8 检查加入门禁。

### 7.2 行为成功标准

1. direct CLI、MCP、ACP 三者公开能力名称集合仍为 77，且与 builtin routability 一致。
2. direct CLI 与 ACP 的精确工具调用不会被自然语言重新路由。
3. 对同一 mock builtin，各入口执行相同的权限、安全、metrics/learner 和日志策略；允许差异必须有命名策略和测试。
4. WorkflowConfig 的有效配置会改变可观察 phase/engine 执行，非法配置失败；或该参数明确返回 unsupported。
5. `semantic: true` 能证明真实语义路径被调用，或明确返回未应用状态；不再静默等同关键词搜索。
6. MCP initialize、protocol version、tool schema、async polling和 error result 外形保持兼容。
7. HTTP 路径、状态码、health 匿名访问、Bearer 认证、CORS 默认行为保持兼容。

### 7.3 可维护性成功标准

1. `orchestrator.rs` 的拆分以职责和测试为依据；目标不是任意行数，但单个模块不再同时拥有配置解析、engine I/O、workflow、review 和 skill adapter。
2. 新增/修改一个编排 workflow 时，不需要编辑超过一个实现模块和一个目录注册点。
3. 关键 provider/timeout/security 配置只有一个解析规则，入口只负责注入。
4. README、CHANGELOG、项目总览、HANDOFF 与测试中的版本/能力数量不再互相矛盾。

## 8. 不可破坏的现有行为与兼容性

1. 保留 `aion-forge` 主命令和 `aion-forge-cli` 兼容命令。
2. 保留无参数默认 ACP、`acp`、`mcp-server`、`setup`、`--tool/--params/--list/--quiet` 的命令契约。
3. 保留 77 个公开能力名称、输入 schema、approval 元数据；删除或重命名需另行走 breaking-change 决策。
4. 保留 `ai_smart_collaborate` 兼容别名，直到有明确弃用期限和迁移说明。
5. 保留 MCP `2025-11-25` 协议兼容、stdio 纯协议 stdout 和紧凑 JSON 行行为。
6. 保留 HTTP endpoint、响应字段、WebSocket 路径和可选 Bearer auth；`/v1/health` 继续公开。
7. 保留 `.skill-router` 路径语义、registry/execution log、redb 数据和旧 JSON 迁移。
8. 保留现有环境变量名和优先级；任何默认值变化单独列为行为变更。
9. 保留 sandbox approval 与外部 sibling `aion-cli` 的边界，不把它纳入 Forge workspace。
10. 保留 fail-open/fail-closed 的现有可配置性；默认策略是否改变需安全评审。

## 9. 分阶段迁移路线

| 阶段 | 交付 | 验证 | 回滚边界 |
|---|---|---|---|
| 0. 基线恢复 | UTF-8、fmt、warnings、CI 不吞错、按 crate 测试画像 | 全部工程门禁；无业务输出变化 | 单独 PR/revert |
| 1. 契约加固 | direct/ACP/MCP/HTTP 的 exact-execution、security、schema、日志 golden tests | mock builtin，不依赖外网 | 纯测试 PR，可独立回滚 |
| 2. 执行链收敛 | router 共享精确执行路径；先 direct，后 ACP | 每迁移一个入口跑原契约 + 新一致性契约 | 每入口独立 PR |
| 3. no-op 行为修复 | WorkflowConfig 与 semantic 的“实现或明确拒绝” | valid/invalid/fallback/mocked-provider tests | 两项分开 PR |
| 4. orchestrator 拆分 | 按 engine/config/runner/review/skills 移动 | 前后 JSON golden、timeout/concurrency tests | 每个模块移动独立 PR |
| 5. 关键配置收敛 | provider、timeout、workspace、security 配置快照 | env precedence table tests、secret redaction | 配置类别分 PR |
| 6. 发布与文档闭环 | 版本/工具数量、Docker cache、manifest drift、实施报告 | release contract、容器 build、文档检查 | 与业务代码分离 |

### 停止条件

任一条件满足时应停止或缩小范围：

- 干净 CI 无法建立稳定基线；
- 产品确认 direct CLI/ACP 应永久旁路统一策略；
- 入口迁移导致大量依赖未记录的隐式行为，且无法通过契约锁定；
- WorkflowConfig/semantic 并非公共承诺，删除/明确拒绝比实现更便宜；
- 模块拆分没有减少变更范围或测试定位时间。

## 10. 初步测试策略

### 10.1 快速层（每次提交）

- UTF-8、rustfmt、clippy。
- 改动 crate 的 unit tests。
- 77 工具目录/路由/schema drift tests。
- 不访问网络，不修改真实用户目录。

### 10.2 workspace 层（每个 PR）

- 全 workspace lib、doc 和非 ignored integration tests。
- direct CLI 输出、MCP handshake/tools call、ACP tool executor、HTTP router/auth 契约。
- 使用临时 workspace，验证 `.skill-router`、memory 和 execution log，不污染项目根。

### 10.3 模拟外部依赖层

- Rust mock HTTP server 覆盖 provider success、timeout、429、invalid JSON、SSE/non-stream。
- fake embedding provider 验证 semantic 路径确实被调用及降级标记。
- fake engine runner 验证 WorkflowConfig phase、parallel、timeout、retry。
- fake security reviewer/策略验证各入口一致性。

### 10.4 可选 live 层

- 真实 AionUI MCP 发现与一次无副作用工具调用。
- 真实 OmniRoute/provider smoke test，凭据由 CI secret 注入。
- NATS/distributed feature 单独 job。
- live job 可以 optional，但结果必须显示 passed/failed/skipped，不能把失败改成成功。

### 10.5 回归与性能

- 在阶段 0 记录 clean build、incremental check、各 crate tests、MCP list 和单次 echo 的耗时。
- 只有存在同环境基线时才宣称性能改善。
- 对 `orchestrator` 重点记录 timeout、并行上限、任务完成顺序和降级结果，不以代码行数作为成功指标。

## 11. 实施前必须回答的问题

1. 是否批准 direct CLI/ACP 进入统一 Executor，即接受过去被旁路的请求可能被安全策略拒绝？
2. WorkflowConfig 与 semantic recall 选择“实现”还是“明确 unsupported”？
3. 77 项是否全部属于稳定公共 API？若不是，需要 experimental 列表。
4. required CI 的最大可接受时长是多少，哪些 live tests 可为 optional？
5. 对外版本口径采用 README 的 v0.3.0、Cargo 的 0.7.0，还是 tag v0.7.1？
6. 是否授权后续清理已跟踪的 backup/output 文件？本提案不默认删除。

在这 6 个问题确认前，可以执行阶段 0 和纯测试的阶段 1；不应开始会改变入口行为的阶段 2/3。
