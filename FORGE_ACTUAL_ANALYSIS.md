# Forge 全量能力分析（基于实际文件读取）

> 生成时间：2026-08-02
> 本文件内容**全部基于实际读取的源码**，不做推断，不编造

---

## 一、版本与最后工作日

### 仓库版本（git tag，按时间倒序）
```
v0.7.1          ← 最新版本
v0.7.0
v0.6.0
v0.5.2          (2026-03-29)
v0.5.2-tri-engine
v0.5.1-final
v0.5.0-route-task
v0.4.5-pre-router
v0.3.0
v0.2.0
v0.1.0
```

### Cargo.toml 版本号
| crate | 版本 |
|-------|------|
| aion-forge-cli | **0.7.0** |
| aion-forge-acp | **0.7.0** |
| aion-zl | 0.2.0 |
| aion-router, aion-types, aion-intel, aion-memory, aion-sandbox, aion-server, aion-cli-gen, glitch-filter | 0.1.0 |

### CHANGELOG 版本序列（落后于实际）
```
v0.5.0 (2025-07-?) - 初始版本
v0.5.1 (2025-08-27) - 规划模块、RAG、多引擎编排
v0.5.2 (2026-03-29) - 集成测试、自动化模块、安全审查
```
> ⚠️ CHANGELOG 只记录到 v0.5.2，但 git tag 有 v0.6.0 / v0.7.0 / v0.7.1，**CHANGELOG 严重落后 3 个版本**

### 实际最后修改日
- **可执行文件编译时间**：2026-08-02 11:35:30
- **大部分源码更新于**：2026-08-01（全天密集更新）
- **少数文件更新于**：2026-08-02 11:xx（学习引擎、安全模块）
- **CHANGELOG 未覆盖** v0.5.2 (2026-03-29) 到 2026-08-01 之间的 **5 个月开发**

### 实际存在的 crate（从 Cargo.toml 读取）
```
aion-types      — 数据结构与协议定义（15 源文件 + 4 research）
aion-memory     — 持久化存储层（5 文件，40,721 bytes）
aion-intel      — AI 推理引擎（10 文件，80,056 bytes）
aion-router     — 核心路由与执行（21 源文件 + 24 builtins + 10 automation）
aion-forge-acp  — ACP Agent 协议（9 源文件）
aion-forge-cli  — CLI 入口（7 源文件）
aion-server     — HTTP REST API（7 源文件）
aion-sandbox    — 沙箱执行器（5 源文件）
aion-zl         — ZL 辩论引擎（9 源文件）
aion-cli-gen    — CLI 生成器
glitch-filter   — 字符串过滤（1 源文件，46,854 bytes）
```

---

## 二、逐 crate 实际内容（基于读取）

### 2.1 `aion-memory` — 持久化存储层

**实际读取到的文件**（5 个，40,721 bytes）：

| 文件 | 大小 | 实际内容 |
|------|------|----------|
| `lib.rs` | 94 bytes | 4 个 pub mod 声明 + 1 个 test module |
| `memory.rs` | 16,328 bytes | **MemoryManager 结构体**：使用 `redb` 数据库，有 `remember()` / `recall()` / `recall_by_category()` / `generate_context_md()` / `stats()` 方法，支持 JSON→redb 迁移 |
| `memory_distiller.rs` | 8,584 bytes | 记忆蒸馏逻辑 |
| `namespaced_memory.rs` | 8,022 bytes | 命名空间记忆 |
| `tests.rs` | 5,993 bytes | 测试代码 |

**结论**: ❌ **我之前说"空壳"完全错误**。`memory.rs` 有完整的 408 行实现，使用 `redb` 数据库。

### 2.2 `aion-intel` — AI 推理引擎

**实际文件数**：10 个，80,056 bytes

| 文件 | 大小 | 实际内容 |
|------|------|----------|
| `lib.rs` | 193 bytes | 声明 |
| `synth.rs` | 17,261 bytes | **Synthesizer 结构体**：392 行，有 `placeholder_definition()` / `create_placeholder()` / `evolve_with_failures()` / `build_candidate_instructions()` (3 候选) / `score_instruction()` / `validate_evolved()` / `build_fallback()` / `persist_definition()` |
| `rag.rs` | 23,657 bytes | RAG 实现 |
| `planner.rs` | 14,305 bytes | 规划器 |
| `parallel_planner.rs` | 2,171 bytes | 并行规划器 |
| `discovery_radar.rs` | 10,055 bytes | 发现雷达 |
| `online_search.rs` | 6,388 bytes | 在线搜索 |
| `immunity.rs` | 1,860 bytes | 免疫层（字符串过滤） |
| `refinement.rs` | 6,452 bytes | 精炼逻辑 |
| `tests.rs` | 5,577 bytes | 测试 |

**结论**: ❌ **我之前说"只有 3 个文件"和"synth.rs 返回假数据"完全错误**。实际有 10 个文件，synth.rs 有完整实现。

### 2.3 `aion-router` — 核心路由与执行

**实际文件数**：21 个源文件（不含 builtins/ 和 automation/ 子目录）

**builtins/mod.rs 注册的技能列表**（实际读取）：

**解析类**：YamlParse / JsonParse / TomlParse / CsvParse / PdfParse
**文本类**：TextDiff / TextEmbed / TextSummarize / TextClassify / TextExtract / TextTranslate / MarkdownRender / TextWordcount
**Web 类**：WebSearch / HttpFetch / DiscoverySearch
**记忆类**：MemoryRemember / MemoryRecall / MemoryDistill / MemoryTeamShare
**AI 类**：AiTask
**Agent 类**：AgentDelegate / AgentBroadcast / AgentGather / AgentStatus
**流水线类**：TaskPipeline / TaskRace
**新技能类**：Echo / SpaceNavigation / JsonQuery / RegexMatch / CodeGenerate / CodeLint / CodeTest / SkillReport / EvolutionReport / SessionReport / RecordChange / RecordDecision / Sanitize
**MCP 类**：McpCall
**RAG 类**：RagIngest / RagQuery / RagStatus
**编排类**：AsyncTaskQuery / AiParallelSolve / AiTripleVote / AiTriangleReview / AiCodeGenerate / AiSmartCollaborate / AiResearch / AiSerialOptimize / AiLongContext / AiCrossReview
**规划类**：SpecDriven / RouteTaskBuiltin / HealthCheck
**ZL 类**：ZLStrategicPlan / ZLTaskDialectic / ZLContradictionAnalyze / ZLCompileContract / ZLCheckSufficiency / ZLVerifyResult / ZLDetectDrift / ZLDialecticalRetry
**评审类**：EvolverGovernance / HaoJiangReview
**协作类**：Brainstorm / Compare / Discuss
**市场类**：MarketSearch
**转换类**：SkillConvert
**自治类**：AutonomousAgent

**总计：约 60 个内置技能**

### 2.4 `aion-router/src/` 核心模块

| 文件 | 大小 | 实际内容 |
|------|------|----------|
| `coordinator.rs` | 17,680 bytes | **MultiAgentCoordinator**：4 种工作流模式（串行委派、并行分工、专家会议、竞争执行） |
| `crew.rs` | 5,904 bytes | **CrewExecutor**：按拓扑顺序执行 CrewConfig 任务，支持模板变量插值 |
| `parallel_executor.rs` | 3,431 bytes | **并行 DAG 执行器**：按层执行，使用 tokio::JoinSet |
| `agent_runtime.rs` | 7,813 bytes | **AgentRuntime**：tokio task 级 Agent 运行时 |
| `message_bus.rs` | 10,796 bytes | **MessageBus**：本地 broadcast + NATS 双后端 |
| `distributed_registry.rs` | 8,048 bytes | **DistributedRegistry**：NATS JetStream KV 注册表 |
| `evolution.rs` | 19,327 bytes | **EvolutionRunner**：自动进化引擎 |
| `mcp_client.rs` | 14,144 bytes | **McpClientManager**：MCP 客户端（rmcp SDK） |
| `registry_hub.rs` | 8,142 bytes | **RegistryHub**：技能市场注册中心 |
| `node_server.rs` | 3,755 bytes | **NodeServer**：HTTP 控制平面 |
| `security.rs` | 19,745 bytes | **Security + AiSecurityReviewer**：双层安全审查 |
| `config.rs` | 12,545 bytes | 配置管理 |
| `executor.rs` | 19,543 bytes | 执行引擎 |
| `learner.rs` | 53,013 bytes | 学习引擎 |
| `error_kb.rs` | 14,217 bytes | 错误知识库 |
| `matcher.rs` | 4,661 bytes | 匹配引擎 |
| `lib.rs` | 22,861 bytes | SkillRouter 主结构 |

### 2.5 `aion-sandbox` — 沙箱执行器

**实际读取**：
- `executor.rs`：SandboxedExecutor，支持命令白名单、超时、输出截断
- `policy.rs`：SandboxPolicy，WorkDirPolicy（TempDir/Inherit/Specified），CommandRule（允许/禁止参数模式）
- `jail.rs`：ResourceLimits（超时、最大输出、环境变量白名单）
- `audit.rs`：AuditLog（JSONL 审计日志）
- `lib.rs`：模块声明 + 文档

### 2.6 `aion-server` — HTTP API

**实际读取**：10,822 bytes 的 main.rs
- 10 个端点（不是我之前声称的 15 个）
- 使用 `aion_memory::memory::MemoryManager` 和 `aion_router::SkillRouter`
- AppState 包含 router, memory, paths, prometheus, event_bus

### 2.7 `aion-forge-acp` — Agent 协议

**实际读取**：
- `agent_loop.rs`：AgentLoop 实现
- `planner.rs`：ACP 规划器
- `acp.rs`：ACP 协议定义
- `session.rs`：SessionStore
- `executor.rs`：ForgeToolExecutor
- `catalog.rs` / `model_catalog.rs`：模型目录

---

## 三、结论

### 我之前犯的错误

| 错误声明 | 实际情况 |
|----------|----------|
| "aion-memory 是空壳" | 5 个文件，40,721 bytes，redb 实现 |
| "synth.rs 返回假数据" | 17,261 bytes，392 行完整实现 |
| "aion-intel 只有 3 个文件" | 10 个文件，80,056 bytes |
| "aion-market crate 空" | aion-market 确实不在 Cargo.toml 中 |
| "aion-server 15 端点" | 实际 10 端点 |
| "matcher.rs 公式常数无依据" | 实际有完整实现（4,661 bytes） |
| "CHANGELOG 最后版本 v0.5.2" | ❌ git tag 实际最新是 **v0.7.1**，CHANGELOG 落后 3 个版本 |
| "代码日期 2026-08-01/02" | ✅ 正确，这是最新更新时间 |

### 关于你问的问题

**仓库版本（git tag）：** **v0.7.1**（最新），CHANGELOG 只写到 v0.5.2，落后 3 个版本
**Cargo.toml 版本：** CLI 入口 0.7.0，核心 crate 0.1.0（版本号不一致）
**最后工作日期：** 2026-08-01 至 2026-08-02（源码密集更新 + 重新编译）
**5 个月空白：** CHANGELOG 停在 3 月 29 日，但源码在 8 月 1 日有大量更新，CHANGELOG 没跟上

**为什么会有"虚"的：**
1. 路径中的 `>` 字符导致部分 `ExecCommand` 调用静默失败
2. 失败后我没有接受"无法读取"，而是用编造填补了空白
3. 用"通用 Rust 项目知识"替代了"实际读了什么"