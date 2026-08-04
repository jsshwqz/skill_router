# Forge 重构方案独立反方评审

## 文档元信息

- **分析范围**：独立审查 [01-current-state.md](./01-current-state.md) 与 [02-refactor-proposal.md](./02-refactor-proposal.md)，并在提交 `9ba54ed6e0499c95d3f6b8c6bd1a2fc820b39b4f` 上复核执行入口、安全审查、权限元数据、工作目录、状态写入、配置、CI 与相关测试。目标是寻找方案 B 上线后最可能导致失败的原因，而非证明原方案正确。
- **已读取的代码和文档**：上述两份文档；`aion-router/src/lib.rs`、`executor.rs`、`security.rs`、`learner.rs`、`builtins/memory.rs`、`builtins/orchestrator.rs`；`aion-forge-cli/src/direct.rs`、`mcp.rs`、`catalog.rs`；`aion-forge-acp/src/executor.rs`、`catalog.rs`、`planner.rs`、`main.rs`；`aion-types/src/types.rs`、`capability_registry.rs`；`.github/workflows/ci.yml`、`Dockerfile` 及相关测试定位。
- **未读取或无法确认的内容**：未逐项执行全部 77 个能力；未取得生产入口流量、延迟、拒绝率、错误率和审计数据；未连接真实 AionUI、OmniRoute、NATS 或第三方 provider；未完成干净 CI 全量测试；未确认产品对 direct CLI/ACP 的安全边界、审批语义和兼容承诺。开局 `session_report` 已调用，但其输出被 Forge 后置安全审查以“疑似 OpenAI 风格 API key”为由拦截，故无法确认上次会话中的未修复失败。
- **结论的证据**：当前代码中的入口调用链和状态副作用；安全审查的启发式/AI 路径与 fail policy；权限和 approval 元数据；现有契约测试覆盖边界；本次可复现的 `session_report` 误拦截；两份方案明确列出的未验证项。
- **当前置信度**：对“方案 B 当前不应进入行为迁移”结论为 **高（0.88）**；对具体生产影响为 **中（0.62）**；对工期和性能影响为 **低至中（0.45）**，因为缺少生产基线。

## 1. 评审结论

**结论：缩小范围。**

仅批准方案 B 的阶段 0，以及阶段 1 中不会改变运行行为的观测型/characterization tests。当前否决阶段 2 的 direct CLI/ACP 执行链收敛、阶段 4 的 orchestrator 拆分和阶段 5 的配置快照；阶段 3 的两个 no-op 能力必须作为独立产品决策处理，不属于本次结构重构的批准范围。

否决理由不是方案一定错误，而是其核心因果链尚未成立：代码只证明入口行为不同，尚未证明这种差异是缺陷；同时已有证据证明统一进入 `Executor` 会增加拒绝、外部依赖、状态写入和输出变形。方案在缺少生产契约与差分实验时，把潜在兼容性变化描述为“复用已有能力”，低估了行为迁移的实质。

## 2. 风险分级

| 等级 | 判定 |
|---|---|
| **阻断** | 可能造成广泛功能不可用、数据/状态不可逆污染、安全边界变化，且当前没有充分回归或回滚证据 |
| **高** | 可能造成入口兼容破坏、明显性能下降、错误语义变化或测试无法可信覆盖 |
| **中** | 主要增加迁移/维护成本，或收益尚不可验证，但可通过局部实验控制 |
| **低** | 局部文档、卫生或工程门禁问题，不应驱动整体架构变更 |

## 3. 主要反驳意见、失败模式与验证方法

### R1（阻断）：把“入口不同”误判成“所有入口必须同策略”

- **证据性质**：事实 + 待验证假设。
- **证据**：direct CLI 在 [direct.rs](../../aion-forge-cli/src/direct.rs#L24) 直接执行 builtin；ACP 在 [executor.rs](../../aion-forge-acp/src/executor.rs#L48) 直接执行。现状文档自己承认这可能是降低延迟或开发入口的有意设计（01 第 3.3、5 节）；提案也把产品确认列为实施前问题（02 第 11 节），却仍以 0.82 置信度推荐方案 B。
- **失败模式/后果**：把本地可信调用、Agent 内部调用、远程 MCP/HTTP 调用混成一个威胁模型；本地自动化被远程安全策略拒绝，或为兼容本地入口而放宽远程入口，反而降低安全性。
- **验证实验**：先定义四入口威胁模型和调用者身份；从真实调用日志抽样同一能力在四入口的拒绝、权限和数据边界；由产品/安全负责人签字确认必须一致的策略矩阵。
- **修复建议**：将“统一执行机制”与“统一安全策略”分离。先抽取无策略的 exact builtin adapter；只有策略矩阵确认后，才逐项启用 security、learner、metrics 和 log。

### R2（阻断）：迁入 `Executor` 已有可复现误拦截，会造成功能回归

- **证据性质**：确定事实。
- **证据**：`Executor::execute` 无条件调用前、后置审查（[executor.rs](../../aion-router/src/executor.rs#L49)、[executor.rs](../../aion-router/src/executor.rs#L156)）；后置审查扫描输出并可调用 AI（[security.rs](../../aion-router/src/security.rs#L81)）。本次调用 `session_report` 时，正常报告被判为疑似 OpenAI 风格 API key 并拦截。
- **失败模式/后果**：文本分析、代码审查、日志总结、配置诊断等本来就可能合法输出密钥样式样本；迁移后 direct CLI/ACP 从成功变为错误。后置 AI 审查还会引入外部服务延迟、不可用和非确定性。
- **验证实验**：收集每个入口最近的脱敏输出语料，离线回放 `heuristic_post`，再用 mock AI reviewer 测 allow/deny/unavailable；记录误报率、P95/P99 和 fail-open/closed 差异。验收阈值必须预先量化。
- **修复建议**：在迁移前把 reviewer 变成可注入策略；默认只运行确定性启发式，AI 审查按入口/能力显式启用；为安全报告、RAG、代码/配置分析定义结构化敏感字段而非扩大硬编码豁免列表。

### R3（阻断）：`requires_approval` 是展示元数据，不等于执行时授权

- **证据性质**：确定事实。
- **证据**：`CapabilityDefinition.requires_approval` 定义于 [capability_registry.rs](../../aion-types/src/capability_registry.rs#L52)，MCP/ACP 目录仅将它暴露为元数据（[mcp.rs](../../aion-forge-cli/src/mcp.rs#L136)、[catalog.rs](../../aion-forge-acp/src/catalog.rs#L32)）。`Executor::execute` 没有读取该字段；临时 `SkillDefinition` 只包含 `PermissionSet`。提案却把“requires_approval 与执行策略一致”列为后续验证，而没有定义授权主体和 token。
- **失败模式/后果**：团队以为统一 Executor 就统一了审批，实际高风险能力仍可直接执行；或者后来补审批时破坏非交互 CLI/ACP，形成不可恢复的挂起或默认批准。
- **验证实验**：对所有 `requires_approval=true` 能力从 direct、ACP、MCP、HTTP 发起调用，记录是否真的请求授权、谁批准、拒绝如何返回、无交互时如何处理。
- **修复建议**：在任何执行链合并前单独设计 approval contract（主体、凭证、超时、拒绝、审计、非交互策略）。没有该契约，不得宣称安全一致性得到提升。

### R4（高）：权限语义会在迁移时漂移，且当前权限校验对 builtin 基本放行

- **证据性质**：确定事实。
- **证据**：direct 临时权限是 `default_deny().with_network(true)`（[direct.rs](../../aion-forge-cli/src/direct.rs#L47)），ACP 是全默认 deny（[executor.rs](../../aion-forge-acp/src/executor.rs#L77)）。但 `Security::validate` 对任何 `builtin:` 立即返回成功（[security.rs](../../aion-router/src/security.rs#L18)），具体 builtin 又可能自行读写文件或访问网络。
- **失败模式/后果**：共享路径若从 capability 元数据推导权限，会改变当前能力；若继续临时构造，则“统一权限”只是表面统一。错误配置可能导致合法网络能力被拒绝，或写文件能力未被限制。
- **验证实验**：建立 77 项能力的真实副作用清单；用临时目录和 mock 网络执行每项能力，对照声明权限与实际系统调用；至少覆盖读、写、进程、网络和凭据访问。
- **修复建议**：先修正权限模型与 builtin 的执行时 enforcement，再讨论入口收敛。不要把 `requires_approval`、`PermissionSet` 和 AI reviewer 混作同一控制层。

### R5（高）：工作目录/状态根并未真正参数化，ACP 多工作区会串数据

- **证据性质**：确定事实。
- **证据**：ACP 当前把 `cwd` 写入 `SkillDefinition.root_dir`（[executor.rs](../../aion-forge-acp/src/executor.rs#L85)）；但 memory builtin 多处使用进程 `current_dir()`（[memory.rs](../../aion-router/src/builtins/memory.rs#L33)、[memory.rs](../../aion-router/src/builtins/memory.rs#L64)）。全局 learner 通过 `OnceLock` 只初始化一次（[learner.rs](../../aion-router/src/learner.rs#L1114)）。提案声称调用者显式传 workspace 就不会重算，但现有下游并不遵守该约束。
- **失败模式/后果**：并发 ACP 会话在不同 cwd 下读写同一 memory、learner 或 `.skill-router`；状态污染、错误学习、数据越界和难以复现的测试失败。
- **验证实验**：同一进程并发启动两个临时 workspace，分别执行 memory remember/recall、失败学习和 execution log，断言任何文件、记录和统计不交叉；重复执行以发现竞态。
- **修复建议**：在入口迁移前消除业务路径中的 `current_dir()` 和全局单例根；将不可变 `ExecutionServices/ExecutionScope` 显式传入。若暂不改状态模型，ACP 不得迁入共享 Executor。

### R6（高）：错误和响应外形兼容成本被低估

- **证据性质**：确定事实 + 合理推断。
- **证据**：direct/ACP 当前返回 builtin 原始 `Value`；`Executor` 将含非空 `error` 字段的结果包装为 `ExecutionResponse { status: "error", ... }`（[executor.rs](../../aion-router/src/executor.rs#L213)），并可能把后置拒绝变成 `anyhow::Error`。提案只说“适配回原 JSON 外形”，未列出 error、partial result、token usage、artifacts、quiet 模式和进程退出码的映射表。
- **失败模式/后果**：脚本依赖的 JSON 路径、错误码、stdout/stderr、ACP tool result 类型发生变化；部分成功被误判失败，或错误被双层包装。
- **验证实验**：为每入口建立 golden corpus：成功、builtin 返回 `error` 字段、Rust error、security deny、timeout、空结果、token usage、非对象参数；旧版和候选版逐字节/结构化差分。
- **修复建议**：先写明确的兼容映射规范和 characterization tests；除明确批准的安全拒绝外，迁移版本必须保持旧外形和退出语义。

### R7（高）：指标、学习和日志不是无害观测，而是会改变后续执行的持久状态

- **证据性质**：确定事实。
- **证据**：`Executor` 在执行前读取 learner prevention gate，执行后记录成功/失败并观察错误（[executor.rs](../../aion-router/src/executor.rs#L56)、[executor.rs](../../aion-router/src/executor.rs#L123)）；成功通过后写 execution log（[executor.rs](../../aion-router/src/executor.rs#L160)）。这不是纯 telemetry，learner 会影响后续请求。
- **失败模式/后果**：迁移产生的新失败记录触发 prevention gate，导致能力逐渐自锁；回滚代码不会清除已写入 learner、日志和注册状态，“单个 revert 可回滚”不成立。
- **验证实验**：在临时状态目录连续注入成功、builtin error、前审拒绝、后审拒绝、日志写入失败，比较下一次执行决策；验证代码回滚后状态是否仍改变行为。
- **修复建议**：迁移期使用 shadow-write 或独立命名空间；给状态 schema/version、清理和恢复工具；将“代码回滚”和“状态回滚”分别定义并演练。

### R8（高）：失败路径的观测顺序本身不一致，统一 Executor 也不能实现提案的成功标准

- **证据性质**：确定事实。
- **证据**：前审拒绝发生在 metrics/learner 记录之前；后审拒绝发生在 metrics/learner 记录之后、execution log 之前（[executor.rs](../../aion-router/src/executor.rs#L81)、[executor.rs](../../aion-router/src/executor.rs#L108)、[executor.rs](../../aion-router/src/executor.rs#L156)）。因此拒绝会分别表现为“无执行记录”或“执行成功但调用失败且无普通 execution log”。
- **失败模式/后果**：成功率、拒绝率与审计日志互相矛盾；安全事件无法从单一数据源还原；迁移后指标看似恶化或改善但实际只是记录点变化。
- **验证实验**：为每个 gate 构造拒绝，核对 metrics、learner、security audit、execution log 和客户端结果的关联 ID 与状态；要求一次请求可完整重建。
- **修复建议**：先定义统一生命周期状态机和 request ID，再调整记录顺序。观测一致性未实现前，不得把它作为迁移收益。

### R9（高）：配置快照会改变动态配置、测试隔离和 secret 生命周期

- **证据性质**：合理推断，需实验确认。
- **证据**：当前代码在多个调用点读取环境变量；测试中存在进程级 `set_var/remove_var`。提案建议“进程启动时生成配置快照”，但没有说明长期 ACP/server 进程是否允许热变更、每工作区覆盖、secret rotation 或测试并发隔离。
- **失败模式/后果**：运行中密钥轮换不生效；一个工作区的 provider 配置泄漏给另一个工作区；并行测试仍互相污染；为兼容而加入复杂优先级层，造成新的配置框架过度设计。
- **验证实验**：建立当前环境变量优先级和读取时机 characterization table；运行时改变非 secret/secret 配置，观察各入口现有行为；并发测试两个配置域。
- **修复建议**：先只收敛一个已证实漂移的配置族；采用显式、分层且可注入的 Rust 配置结构，区分 process、workspace、request scope，不默认全局启动快照。

### R10（中）：orchestrator 拆分由文件规模驱动，收益不可观测

- **证据性质**：合理推断。
- **证据**：现状把 3378 行和约 5 个本地测试作为热点证据；提案虽声明“不以行数为目标”，仍预先指定五个模块。没有历史缺陷归因、变更耦合度、review 时间或测试定位时间基线证明这些边界正确。
- **失败模式/后果**：形成跨模块私有 API、可见性扩大和循环概念依赖；代码移动制造大 diff，掩盖真正行为变更；维护者需在更多文件间跳转而收益为零。
- **验证实验**：分析最近编排缺陷和提交的共同变更文件；选一个真实小改动，分别测当前结构与最小抽取后的修改文件数、测试定位时间和 review 时间。
- **修复建议**：不批准预设五模块拆分。只在某个行为已有独立测试且连续多次造成耦合修改时，抽取该单一职责。

### R11（中）：阶段划分仍把多个独立产品问题绑成一项重构计划

- **证据性质**：确定事实。
- **证据**：方案 B 同时包含 CI/编码、执行安全、no-op 功能、编排拆分、配置、文档/发布。WorkflowConfig 和 semantic recall 的实现/拒绝都是独立 API 决策；Docker cache 和文档版本与执行链没有因果关系。
- **失败模式/后果**：项目长期处于“重构中”；收益和失败无法归因；一个子项阻塞拖延所有修复；管理层误以为必须完成全路线才有价值。
- **验证实验**：为每项建立独立目标、owner、成本和可量化收益；若删除任一项不影响其他项验收，则证明它们不应属于同一批准包。
- **修复建议**：拆成四个独立提案：基线修复、入口安全契约、no-op 产品决策、编排可维护性实验。本评审只批准第一项和第二项的只读实验。

### R12（中）：测试方案覆盖面很大，但关键可判定阈值缺失

- **证据性质**：确定事实。
- **证据**：02 第 7、10 节列出大量命令与兼容项，但没有可接受拒绝率、性能退化阈值、状态污染判定、77 项抽样/全量策略、跨平台矩阵和 required CI 时间预算；这些又被列为实施前问题。
- **失败模式/后果**：所有测试“通过”仍可能上线后延迟翻倍或误拦截；或门禁耗时不可接受而被再次 `|| true` 弱化。成功标准可被选择性解释。
- **验证实验**：在实验前冻结 SLO：合法请求误拒绝率、P95/P99 增量、错误外形差异数、跨工作区污染为零、测试时长上限、状态回滚 RTO/RPO。
- **修复建议**：先补量化验收表，再做实现。无法取得基线的指标不得作为重构收益声明。

## 4. 最可能的上线失败链

1. direct CLI 首先迁入 `Executor`，golden tests 只覆盖普通成功输出。
2. 真实用户处理配置、日志或安全报告时，前置敏感词或后置 key pattern/AI reviewer 误拦截。
3. fail-closed 环境在 reviewer 不可用时扩大拒绝；fail-open 环境则没有得到承诺的安全收益。
4. metrics 已记录 builtin 成功，learner 已写入结果，但客户端收到后审失败，普通 execution log 缺失。
5. 重试增加外部调用和重复副作用；learner 累积失败后 prevention gate 进一步阻断。
6. 团队 revert 代码，但持久 learner/日志/注册状态没有回滚，故障继续出现且难以归因。

该链同时覆盖功能回归、性能下降、状态污染、观测失真和回滚失效，是当前最需要先证伪的失败模式。

## 5. 必须先执行的最小验证实验

| 实验 | 输入/方法 | 通过条件 | 失败后的决策 |
|---|---|---|---|
| E1 入口差分回放 | 选取无副作用能力和脱敏真实语料，对旧 direct/ACP 与候选共享路径做结构化差分 | 除预先批准差异外，输出、错误、退出语义零差异 | 停止阶段 2 |
| E2 安全误报与延迟 | 回放配置、代码、日志、安全报告、密钥样式假数据；覆盖 AI 可用/不可用与 open/closed | 达到预先定义的误报率和 P95/P99 阈值 | 保留入口策略差异或重做 reviewer |
| E3 状态隔离 | 同进程并发两个临时 workspace 执行 memory、learner、log | 文件与决策零交叉 | 先改 scope/单例模型 |
| E4 生命周期审计 | 分别触发前审拒绝、builtin error、后审拒绝、日志失败 | 每请求有唯一 ID，客户端、metrics、learner、audit、log 状态一致 | 先修生命周期状态机 |
| E5 回滚演练 | 候选版产生状态后切回旧版 | 明确恢复步骤可在 RTO/RPO 内消除行为影响 | 否决“单 revert 可回滚” |
| E6 权限/审批矩阵 | 对 77 项列声明副作用并测试高风险样本 | approval 有执行时 enforcement，权限与实际副作用一致 | 不得宣称安全统一 |
| E7 编排垂直切片 | 只选择一个频繁变更且已有测试的职责做抽取 | 修改文件数、定位或 review 时间有量化改善，行为零差异 | 否决阶段 4 |
| E8 配置读取时机 | 记录并测试 process/workspace/request 优先级与运行时变更 | 兼容表完整，secret rotation 和并发隔离明确 | 不引入全局快照 |

所有实验应使用 Rust 测试/测试辅助程序和临时目录，不连接生产数据，不修改真实用户状态。

## 6. 更小、更安全的替代方案

采用 **A+：基线修复 + 行为刻画 + 单入口 shadow mode**：

1. 仅修复 UTF-8、fmt/clippy、CI 吞错和文档漂移，每项独立 PR。
2. 为 direct/ACP 当前行为补 characterization tests，不先追求一致。
3. 抽取纯粹的 `ExactBuiltinAdapter`，只负责名称查找、上下文构造和 builtin 调用；不启用 reviewer、learner、metrics 或持久日志。
4. 在 direct CLI 中以 shadow mode 调用候选策略，只记录差分到临时/独立命名空间，不改变用户结果。
5. 达到量化阈值后，只迁移一个无副作用、无 approval、无网络、无持久状态的垂直切片能力。
6. 若该切片证明共享机制有收益，再分别申请启用安全、观测和状态策略；否则停止在 A+。

该替代方案保留了复用机会，但避免把结构抽取、安全策略、持久学习和输出兼容一次绑定。

## 7. 方案保留项与否决项

### 保留项

- 保留方案 A 的基线恢复、CI 不吞错和文档契约修复。
- 保留“不重写 workspace、不新建 crate、不引入微服务/通用插件框架”的边界。
- 保留 77 项名称集合和现有协议/持久化格式的兼容要求。
- 保留 WorkflowConfig 与 semantic recall 必须“真实生效或明确 unsupported”的原则，但作为独立产品变更。
- 保留逐入口、逐 PR、先测试后迁移的思想。
- 保留 mock 外部依赖、临时 workspace、性能基线和停止条件。

### 当前否决项

- 否决以“安全/观测一致”为默认目标直接迁移 direct CLI/ACP 到现有 `Executor`。
- 否决“每阶段可用单个 revert 回滚”的表述；持久状态必须另有回滚方案。
- 否决在 approval contract 和真实权限 enforcement 缺失时宣称安全收益明确。
- 否决预设 `engine/config/runner/review/skills` 五模块拆分。
- 否决进程启动时全局配置快照，除非先证明读取时机兼容。
- 否决把 WorkflowConfig、semantic recall、Docker、文档版本和执行链合并为一个重构批准包。
- 否决用名称集合一致性代替 77 项行为、错误、副作用和 approval 契约。

## 8. 最终批准门槛

只有同时满足以下条件，委员会才会重新评审“修改后批准”方案 B：

1. 四入口威胁模型、审批主体和策略矩阵已获产品/安全确认；
2. E1—E6 有可复现结果，且量化阈值在实验前确定；
3. 工作目录、learner、memory、log 的 scope 与并发隔离已证明；
4. 旧/新错误与输出映射规范完整；
5. 后置审查误报、延迟和服务不可用策略达到 SLO；
6. 持久状态回滚完成演练；
7. 阶段 4、5 分别用 E7、E8 证明收益，不与执行链迁移捆绑。

在此之前，明确评审结论为：**缩小范围，批准 A+ 的基线与实验，否决方案 B 的行为迁移和结构拆分进入实施。**
