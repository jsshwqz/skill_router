# InnoForge（创研台）可用等级评估

> 评估对象：`D:\test\patent-hub-backup`
> 评估目的：判断哪些模块可以直接复用、哪些需要改造、哪些应该跳过
> 评估日期：2026-07-04

---

## 总览

| 模块 | 可用等级 | 说明 |
|------|---------|------|
| Pipeline 状态机引擎 | ⭐⭐⭐⭐⭐ | 核心架构，直接复用 |
| Orchestrator（跳转/分支/回退） | ⭐⭐⭐⭐⭐ | 编排能力极强 |
| PipelineContext 状态传递 | ⭐⭐⭐⭐⭐ | 断点续跑 + 步骤间数据流 |
| 多 AI 提供商适配层 | ⭐⭐⭐⭐ | 需去掉 Google OAuth 部分 |
| 新颖性评分系统 | ⭐⭐⭐⭐ | 评分公式可直接搬 |
| MCP Server 模式 | ⭐⭐⭐⭐⭐ | 让 InnoForge 成为 Aion Forge 的工具 |
| 前端 Next.js | ⭐⭐⭐ | 重做 UI 即可，逻辑在后端 |
| 专利搜索（SerpAPI） | ⭐⭐ | 仅适合专利场景，需替换搜索源 |
| 专利数据结构 | ⭐⭐ | 太垂直，通用研发场景需改写 |
| 权利要求树生成 | ⭐ | 纯专利领域，研发助手用不上 |
| OA 答复生成 | ⭐ | 纯专利审查场景 |
| 实验沙箱 | ⭐⭐⭐ | 概念好，需扩展支持范围 |
| Feature Cards | ⭐⭐⭐ | AI 生成方案卡片，可改造 |
| 数据库 Schema | ⭐⭐⭐⭐ | SQLite + 状态机设计值得借鉴 |
| 提示词模板 | ⭐⭐⭐⭐ | 大部分可保留，去掉专利专属 |
| 版本快照/Pipeline 版本 | ⭐⭐⭐⭐⭐ | 断点续跑的核心 |

---

## 详细评估

### ⭐⭐⭐⭐⭐ 直接可用（零改造或微调整合）

#### 1. Pipeline 状态机引擎 (`orchestrator/engine.rs`)

**为什么高：** 这是整个项目的精华。15 步 Pipeline + Continue/Jump/Branch/Retry/Abort 状态机，天然适合做"研发任务编排"。

**怎么用：**
- ��� `PipelineStep` 枚举中的专利相关步骤替换为你的研发步骤
- 保留状态机执行循环、分支并行、回退机制
- `retry_count` + `diversity_gate_runs` 防无限循环的设计可以直接搬

**改造量：** 小。只需改 `PipelineStep` 定义和相关 prompt，执行引擎本身不动。

**关键代码片段可直接复用：**
```rust
// 状态机跳转逻辑
OrchestratorCommand::Jump { step, branch_id } => { ... }
// 分支合并
fn merge_branches(state: &mut ResearchState, ...) -> Result<(), Error>
// 回退保护
let max_diversity_retries = 2;
```

#### 2. PipelineContext (`pipeline/context.rs`)

**为什么高：** 步骤间数据传递的载体，包含所有中间状态。断点续跑全靠它序列化。

**怎么用：** 保留结构，把 `search_results`、`patent_data` 等专利字段替换为你的研发字段（如 `code_analysis`、`test_results`、`design_docs` 等）。

**改造量：** 中。需要增减字段，但模式照搬。

#### 3. MCP Server 模式 (`bin/mcp-server.rs`)

**为什么高：** 让 InnoForge 变成 Aion Forge 可调用的工具。这是你"放飞 AI 能力"的关键入口。

**怎么用：** 直接编译 `cargo run -- mcp-server`，在 Aion Forge 中注册为 MCP 工具源。

**改造量：** 极小。可能需要注册新的 tool name（如 `research_pipeline.run`）。

#### 4. 版本快照系统 (`db/version.rs` + `db/pipeline_versions`)

**为什么高：** 每次 Pipeline 执行自动存快照，支持回滚到任意步骤。这对长周期研发任务至关重要。

**怎么用：** 直接复用，不需要改。

---

### ⭐⭐⭐⭐ 高价值，需少量改造

#### 5. 多 AI 提供商适配层 (`ai/client.rs`)

**现状：** 支持 OpenAI 兼容、Anthropic、Google Gemini、小米、商汤、智谱。

**需要改造：**
- 去掉 Google OAuth 相关逻辑（`google_access_token`、`refresh_token`、OAuth ���新）
- 去掉 Gemini CLI 子进程模式（除非你真的需要）
- 保留 `ProviderMode::Http` + `ProviderMode::Anthropic` + 通用 OpenAI 兼容

**改造量：** 中。删代码比写代码容易。

**关键可复用：**
- `call_chat` / `call_chat_streaming` 统一接口
- `add_fallback` 模型降级机制
- `MAX_INPUT_CHARS = 500_000` 大上下文支持

#### 6. 新颖性评分系统 (`pipeline/steps/scoring.rs`)

**现状：** 专利新颖性评分，综合相似度、矛盾信号、技术覆盖缺口。

**改造方向：** 把"新颖性"改为"创新性评分"或"方案质量评分"。评分公式本身是通用的：

```
score = 1.0 - similarity_penalty + contradiction_bonus + gap_bonus + diversity_bonus
```

**改造量：** 小。改 prompt + 调整权重参数。

#### 7. 提示词模板 (`data/prompts/`)

**现状：** 大量精心设计的 prompt，涵盖解析、扩展、分析、评分、行动规划。

**需要改造：** 去掉专利专属术语（"权利要求"、"审查意见"、"现有技术"），替换为研发术语（"方案设计"、"技术评审"、"竞品分析"）。

**改造量：** 中。主要是文本替换，结构不变。

---

### ⭐⭐⭐ 中等价值，视需求而定

#### 8. 实验沙箱 (`experiment/`)

**现状：** 支持代码执行沙箱，用于验证创意可行性。

**可用场景：** 如果你的研发涉及代码生成/验证，这个很有用。如果只是文档/分析类研发，价值有限。

**改造方向：** 扩展支持更多执行环境（Python、Rust 编译执行等）。

#### 9. Feature Cards (`routes/feature_cards.rs`)

**现状：** AI 生成的创新方案卡片，结构化展示。

**可用场景：** 适合做"研发成果可视化"——把 AI 的分析结果打包成卡片。

**改造方向：** 卡片字段从专利导向改为研发导向（技术栈、复杂度、预期效果等）。

#### 10. 数据库 Schema 设计

**现状：** SQLite + sqlx，12 张表，状态机设计成熟。

**可用价值：** 数据库设计模式值得学习，但不一定直接复用。如果你的研发助手不需要专利相关数据，大部分表是冗余的。

**建议：** 保留 `ideas`、`chat_messages`、`settings`、`pipeline_versions`，删掉专利专属表。

---

### ⭐⭐ 低价值，建议跳过

#### 11. 专利搜索（SerpAPI）

**现状：** 专门搜索 Google Patents / EPO / USPTO。

**为什么不复用：** 你的研发助手需要的是通用搜索（GitHub、arXiv、StackOverflow、技术博客），不是专利数据库。

**替代方案：** 保留搜索的**架构模式**（expand_query → search_web → rank → filter），但替换搜索后端。

#### 12. 专利数据结构 (`patent.rs`)

**现状：** PatentInfo、PatentResult、SearchType（applicant/inventor/patent_number）等。

**为什么不复用：** 太垂直。研发场景需要的是 CodeSnippet、DesignDoc、TestResult 等结构。

---

### ⭐ 不适用，直接删除

#### 13. 权利要求树生成 (`pipeline/steps/claim_tree.rs`)

**原因：** 纯专利法律文档，研发助手完全用不上。

#### 14. OA 答复生成 (`pipeline/steps/oa_response.rs`)

**原因：** 专利审查意见答复，纯专利场景。

#### 15. IPC 分类 (`routes/ipc.rs`)

**原因：** 国际专利分类，研发不需要。

---

## 改造优先级建议

如果你要把 InnoForge 改成通用研发助手，建议按以下顺序改造：

### Phase 1：核心引擎（1-2 天）
1. ✅ 复制 `orchestrator/` 和 `pipeline/` 目录
2. ✅ 重写 `PipelineStep` 枚举（专利 → 研发）
3. ✅ 重写 `PipelineContext` 字段
4. ✅ 保留状态机执行循环不变

### Phase 2：AI 适配（1 天）
5. ✅ 清理 `ai/client.rs`（去 Google OAuth）
6. ✅ 改造提示词模板（去��利术语）
7. ✅ 保留多提供商支持

### Phase 3：数据层（1 天）
8. ✅ 精简数据库 schema（保留核心表）
9. ✅ 重写 `db/idea.rs` → `db/research_item.rs`
10. ✅ 保留版本快照和断点续跑

### Phase 4：MCP 集成（半天）
11. ✅ 编译 MCP Server
12. ✅ 注册新 tool name
13. ✅ 在 Aion Forge 中测试调用

### Phase 5：可选增强（按需）
14. 🔄 实验沙箱扩展
15. 🔄 Feature Cards 改造
16. 🔄 前端 UI 重写

---

## 总结

| 等级 | 模块 | 建议 |
|------|------|------|
| ⭐⭐⭐⭐⭐ 直接复用 | Pipeline 状态机、Orchestrator、MCP Server、版本快照 | 零改造，搬过来就用 |
| ⭐⭐⭐⭐ 少量改造 | AI 适配层、评分系统、提示词模板 | 删专利术语，改字段名 |
| ⭐⭐⭐ 视需求 | 实验沙箱、Feature Cards、DB 设计 | 有代码验证需求才做 |
| ⭐⭐ 低价值 | 专利搜索、专利数据结构 | 保留架构模式，换搜索源 |
| ⭐ 不适用 | 权利要求树、OA 答复、IPC 分类 | 直接删除 |

**核心价值判断：** InnoForge 的真正价值不在"专利"本身，而在**Pipeline 状态机编排 + 多 AI 适配 + 断点续跑 + MCP 集成**这套架构。专利只是应用场景，换了场景这套架构依然成立。
