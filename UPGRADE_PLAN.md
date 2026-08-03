# Forge 升级计划表

> 基于 SKILL_ANALYSIS.md 的 78 个技能分析，30 个升级项
> 生成时间：2026-08-02

---

## 一、总览

| 阶段 | 名称 | 技能数 | 文件数 | 估算人天 | 依赖 |
|------|------|--------|--------|----------|------|
| P0 | 即时修复 | 2 | 1 | 1 | 无 |
| P1 | 基础设施增强 | 5 | 4 | 5 | P0 |
| P2 | 编排层升级 | 10 | 1 | 8 | P1 |
| P3 | 工具链升级 | 9 | 4 | 6 | P1 |
| P4 | 生态扩展 | 4 | 2 | 4 | P2 |
| **合计** | | **30** | | **24** | |

---

## 二、P0：即时修复（1 天，0 依赖）

目标：修复重复 skill，消除冗余

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 1 | AiSmartCollaborate → 合并到 AiParallelSolve | `orchestrator.rs` | 抽取公共框架 `run_collaboration_workflow`，将 AiSmartCollaborate 作为 AiParallelSolve 的 `mode: smart_collaborate` 参数 | 0.5 | AiSmartCollaborate 功能通过 AiParallelSolve 调用可达 |
| 2 | AiCrossReview → 合并到 AiTriangleReview | `orchestrator.rs` | 将 AiCrossReview 作为 AiTriangleReview 的 `engine_count: 2` 参数，统一入口 | 0.5 | AiCrossReview 功能通过 AiTriangleReview 调用可达 |

---

## 三、P1：基础设施增强（5 天，依赖 P0）

### P1-A：语义嵌入（2 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 3 | TextEmbed 接入外部语义 API | `text.rs`、`aion-intel` | 新增 `EmbeddingProvider` trait，OpenAI `text-embedding-3-small` 实现，配置 fallback 到本地 TF-IDF | 1.5 | TextEmbed 返回 1536 维语义向量 |
| 4 | MemoryRecall 接入语义搜索 | `memory.rs`、`aion-memory` | 将 TextEmbed 语义向量用于记忆检索，替代纯关键词匹配 | 0.5 | MemoryRecall 支持语义相似度排序 |

### P1-B：RAG 增强（2 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 5 | RagIngest 增加混合检索 | `rag.rs`、`aion-intel/rag.rs` | 增加 BM25 全文索引 + 语义向量双路检索 | 1 | RagIngest 存储 BM25 + 向量双索引 |
| 6 | RagQuery 增加重排序 | `rag.rs`、`aion-intel/rag.rs` | 增加 `Reranker` trait，支持 Cohere / 本地交叉编码器重排序 | 1 | RagQuery 返回重排序后的结果 |

### P1-C：Prompt 增强（1 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 7 | AiTask 增加 8 步框架包装 | `ai.rs`、`orchestrator.rs` | 新增 `PromptBuilder`，自动包装 role/context/rules/examples/format | 1 | 所有 AI 调用自动带 8 步框架头 |

---

## 四、P2：编排层升级（8 天，依赖 P1-A）

### P2-A：编排协议可配置化（3 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 8 | AiParallelSolve 三阶段可配置 | `orchestrator.rs` | 新增 `WorkflowConfig` 结构体，支持 YAML 定义阶段数、顺序、超时 | 1.5 | 通过 YAML 配置可改变阶段顺序 |
| 9 | AiTripleVote 增加加权投票 | `orchestrator.rs` | 新增 `trust_weight` 配置，支持每引擎权重 | 0.5 | 加权投票影响最终结果 |
| 10 | AiTriangleReview 增加结果合并 | `orchestrator.rs` | 新增 `ReviewMerger`，去重 + 冲突检测 + 置信度聚合 | 1 | 三份报告合并为一份 |

### P2-B：编排功能增强（3 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 11 | AiResearch depth 增加真实策略 | `orchestrator.rs` | quick=1引擎/3min，comprehensive=2引擎/5min+引用，deep=3引擎/10min | 1 | 三种 depth 输出不同质量的结果 |
| 12 | AiSerialOptimize 部分并行化 | `orchestrator.rs` | 分析+优化可并行，仅验证依赖优化结果 | 1 | 三阶段总时延降低 30%+ |
| 13 | AiLongContext 增加分块摘要 | `orchestrator.rs` | >20K tokens 自动分块，逐块摘要，合并后处理 | 1 | 支持 100K+ token 输入 |

### P2-C：协作功能增强（2 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 14 | Brainstorm 增加收敛轮次 | `orchestrator.rs` | 发散→聚类→去重→收敛，两轮迭代 | 0.5 | 输出结果按主题聚类 |
| 15 | Compare 增加加权汇总 | `orchestrator.rs` | 各引擎评分加权均值，输出雷达图数据 | 0.5 | 输出包含加权总分 |
| 16 | Discuss 增加回合制 | `orchestrator.rs` | 发言→回应→总结，最多 N 轮可配置 | 1 | 三轮讨论后输出最终结论 |

---

## 五、P3：工具链升级（6 天，依赖 P1-A）

### P3-A：文本处理增强（1.5 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 17 | TextSummarize 增加 mode/length | `text.rs` | 新增 `mode: extractive/abstractive` 和 `max_length` 参数 | 0.5 | 支持提取式摘要 |
| 18 | TextClassify 增加 labels 约束 | `text.rs` | 新增 `labels` 数组参数，约束分类范围 | 0.5 | 输出限定在指定标签内 |
| 19 | TextExtract 增加 schema | `text.rs` | 新增 `schema` 参数，支持 JSON Schema 指定输出结构 | 0.5 | 输出严格符合 schema |

### P3-B：网络增强（1 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 20 | WebSearch 增加多引擎 fallback | `web.rs` | SerpAPI → Bing → DuckDuckGo 三级降级 | 0.5 | SerpAPI 失败时自动切换 |
| 21 | HttpFetch 增加代理支持 | `web.rs` | 新增 `proxy` 参数，支持 HTTP/HTTPS/SOCKS5 代理 | 0.5 | 通过代理访问目标 URL |

### P3-C：Agent 增强（1 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 22 | AgentDelegate 增加超时/重试 | `agent.rs` | 新增 `timeout_secs` 和 `retry_count` 参数 | 0.5 | 超时后自动重试 |
| 23 | AgentBroadcast 增加确认模式 | `agent.rs` | 新增 `ack_required` 布尔参数，等待所有 Agent 确认 | 0.5 | 广播可等待确认 |

### P3-D：代码工具增强（1.5 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 24 | CodeGenerate 增加沙箱编译 | `new_skills.rs`、`aion-sandbox` | 生成代码后自动在沙箱内 `cargo build`，编译失败则重试 | 0.5 | 生成的代码经过编译验证 |
| 25 | CodeLint 增加 AST 分析 | `new_skills.rs` | 集成 `rust-analyzer` lib 做 AST 级分析，规则从 8 提升到 50+ | 0.5 | 检出未使用变量、类型错误等 |
| 26 | CodeTest 生成真实断言 | `new_skills.rs` | 分析函数逻辑，生成 `assert_eq!` 等真实断言 | 0.5 | 测试包含至少一个真实断言 |

### P3-E：其他增强（1 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 27 | HealthCheck 增加 API ping | `new_skills.rs` | 增加对 AI_BASE_URL 的 HTTP 连接测试 | 0.5 | 返回真实 API 可达性 |
| 28 | TextTranslate 增加术语表 | `text.rs` | 新增 `glossary` 键值对参数，翻译时保留术语 | 0.5 | 术语在翻译中保持不变 |

---

## 六、P4：生态扩展（4 天，依赖 P2）

### P4-A：基础设施（2 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 29 | SpecDriven 增加 SQLite 持久化 | `spec_driven.rs`、`aion-memory` | 将 spec_driven 状态从内存迁移到 SQLite | 1 | 进程重启后恢复状态 |
| 30 | RouteTaskBuiltin 规则配置化 | `task_router.rs` | 在 `config/route-rules.yaml` 中定义规则，热加载 | 1 | 修改 YAML 后无需重启即生效 |

### P4-B：新增技能（2 天）

| # | 任务 | 涉及文件 | 操作 | 人天 | 验收标准 |
|---|------|---------|------|------|----------|
| 31 | ImageDescribe 增加图像生成 | `image.rs` | 新增 `generate` 模式，调用 DALL-E / Stable Diffusion API | 1 | 返回生成的图像 URL |
| 32 | AgentGather 增加 reduce 策略 | `agent.rs` | 新增 `reduce: all/first/max/min` 参数 | 0.5 | reduce 策略影响返回结果 |
| 33 | TaskPipeline 升级为 DAG | `pipeline.rs` | 支持并行分支，结果合并 | 1 | 支持 A→(B,C)→D 拓扑 |

---

## 七、任务依赖关系图

```
P0 (1天)
  ├─ 1. 合并 AiSmartCollaborate
  └─ 2. 合并 AiCrossReview
       │
       ▼
P1 (5天) — 基础设施
  ├─ 3. TextEmbed 语义嵌入 ───┐
  ├─ 4. MemoryRecall 语义搜索 ┘
  ├─ 5. RagIngest 混合检索
  ├─ 6. RagQuery 重排序
  └─ 7. AiTask 8步框架
       │
       ├──────────────────────┐
       ▼                      ▼
P2 (8天) — 编排层          P3 (6天) — 工具链
  ├─ 8. 三阶段可配置          ├─ 17. TextSummarize 增强
  ├─ 9. 加权投票              ├─ 18. TextClassify 增强
  ├─ 10. 结果合并              ├─ 19. TextExtract 增强
  ├─ 11. Research 深度策略     ├─ 20. WebSearch fallback
  ├─ 12. SerialOptimize 并行   ├─ 21. HttpFetch 代理
  ├─ 13. LongContext 分块      ├─ 22. AgentDelegate 超时
  ├─ 14. Brainstorm 收敛       ├─ 23. AgentBroadcast 确认
  ├─ 15. Compare 加权          ├─ 24. CodeGenerate 编译
  ├─ 16. Discuss 回合制        ├─ 25. CodeLint AST
       │                      ├─ 26. CodeTest 断言
       │                      ├─ 27. HealthCheck API
       │                      └─ 28. TextTranslate 术语表
       │
       ▼
P4 (4天) — 生态扩展
  ├─ 29. SpecDriven SQLite
  ├─ 30. RouteTask 配置化
  ├─ 31. ImageGenerate
  ├─ 32. AgentGather reduce
  └─ 33. DAG TaskPipeline
```

---

## 八、风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 语义嵌入 API 不可用 | 中 | 高 | 保留本地 TF-IDF 作为降级 |
| 外部 API 依赖增加 | 高 | 中 | 所有外部调用有超时 + 降级 |
| 编排层重构影响现有用户 | 中 | 高 | 向后兼容：旧参数仍可用 |
| 编译验证增加构建时间 | 低 | 中 | 沙箱编译有独立超时配置 |
| DAG 升级复杂度高 | 中 | 高 | 从简单 DAG（无循环）开始 |

---

## 九、里程碑

| 里程碑 | 时间 | 交付物 |
|--------|------|--------|
| M1: 技能去重 | Day 1 | AiSmartCollaborate + AiCrossReview 合并 |
| M2: 语义搜索可用 | Day 3-4 | TextEmbed 语义向量 + MemoryRecall 语义检索 |
| M3: RAG 增强 | Day 5-6 | 混合检索 + 重排序工作 |
| M4: 编排可配置 | Day 8-10 | WorkflowConfig YAML 支持 |
| M5: 代码工具链 | Day 11-13 | 沙箱编译 + AST 检查 |
| M6: 生态扩展 | Day 14-15 | SQLite 持久化 + 规则配置化 |