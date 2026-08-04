# Forge 可验证重构提案

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
