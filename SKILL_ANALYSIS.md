# Forge 78 个技能逐项分析

> 基于实际文件读取，不做任何推断

---

## 一、解析类（parsing.rs — 5 个技能）

### 1. YamlParse
- **实现**：调用 `serde_yaml::from_str::<Value>` 解析 YAML 文本
- **知识库评价**：简单 JSON 化的 YAML 解析，不保留 YAML 注释、锚点
- **生态替代**：`yaml-rust2` 支持保留注释，但非必需
- **升级建议**：☑️ 足够，无需升级

### 2. JsonParse
- **实现**：调用 `serde_json::from_str::<Value>` 解析 JSON
- **知识库评价**：标准实现，无可挑剔
- **生态替代**：无
- **升级建议**：☑️ 足够

### 3. TomlParse
- **实现**：调用 `toml::from_str::<Value>` 解析 TOML
- **知识库评价**：标准实现
- **生态替代**：无
- **升级建议**：☑️ 足够

### 4. CsvParse
- **实现**：使用 `csv::ReaderBuilder::new().flexible(true)` 标准库，RFC 4180 兼容
  - 从 `parsing.rs` 第 120-122 行实际读取确认
- **知识库评价**：标准实现，`flexible(true)` 兼容边界场景
- **生态替代**：无（已用标准库）
- **升级建议**：☑️ 足够

### 5. PdfParse
- **实现**：读取文件路径 → `pdf_extract::extract_text_from_mem(&data)` 真实提取 PDF 文本
  - 从 `parsing.rs` 第 181 行实际读取确认
- **知识库评价**：使用 `pdf-extract` 库，支持页数估算、字符数统计
- **生态替代**：无
- **升级建议**：☑️ 足够

---

## 二、文本处理类（text.rs — 8 个技能）

### 6. TextDiff
- **实现**：使用 `similar` crate 的 `DiffOp` 做行级 diff
- **知识库评价**：成熟实现，支持 `text_diff` 和 `json_diff` 模式
- **生态替代**：`similar` 是最成熟的 Rust diff 库
- **升级建议**：☑️ 足够

### 7. TextEmbed
- **实现**：本地 TF-IDF 词袋向量（单文档，IDF 近似为常数）
- **知识库评价**：单文档 IDF 无统计意义，无法用于语义搜索
- **生态替代**：`text-embedding-3-small` API / `fastembed` (Rust binding)
- **升级建议**：⬆️ 建议接入外部 embedding API 或 `fastembed`

### 8. TextSummarize
- **实现**：通过 `ai_text_builtin!` 宏生成，委托给 AiTask 调用 AI
- **知识库评价**：纯 AI 委托，无长度控制、无提取式/生成式切换
- **生态替代**：`text-splitter` + AI 分段摘要可做更长的文档
- **升级建议**：⬆️ 建议增加 `mode: extractive/abstractive` 和 `max_length` 参数

### 9. TextClassify
- **实现**：同上，AI 委托
- **知识库评价**：无预定义标签约束，无多标签支持
- **生态替代**：无
- **升级建议**：⬆️ 建议增加 `labels` 数组参数约束分类范围

### 10. TextExtract
- **实现**：同上，AI 委托
- **知识库评价**：无结构化输出约束
- **生态替代**：无
- **升级建议**：⬆️ 建议增加 `schema` 参数指定输出 JSON Schema

### 11. TextTranslate
- **实现**：同上，AI 委托
- **知识库评价**：无术语表支持
- **生态替代**：无
- **升级建议**：⬆️ 建议增加 `glossary` 术语表参数

### 12. MarkdownRender
- **实现**：使用 `pulldown_cmark` 解析为结构化 section
- **知识库评价**：成熟实现
- **生态替代**：`pulldown_cmark` 是最成熟 Rust markdown 解析器
- **升级建议**：☑️ 足够

### 13. TextWordcount
- **实现**：按空白字符分割统计词、字符、行数
- **知识库评价**：简单实现，支持宽松输入回退
- **生态替代**：无
- **升级建议**：☑️ 足够

---

## 三、网络类（web.rs — 3 个技能）

### 14. WebSearch
- **实现**：调用 SerpAPI（JSON）搜索，返回 organic results
- **知识库评价**：依赖外部 SerpAPI 服务，无 API key 时无法工作
- **生态替代**：无开源替代（必应搜索 API / Google Custom Search）
- **升级建议**：⬆️ 建议增加多引擎 fallback（SerpAPI → Bing → DuckDuckGo）

### 15. HttpFetch
- **实现**：使用 `reqwest` 获取 URL 内容
- **知识库评价**：标准实现，无代理支持
- **生态替代**：无
- **升级建议**：⬆️ 建议增加代理支持

### 16. DiscoverySearch
- **实现**：Google 搜索 + HTTP 降级 + 本地 trusted sources
- **知识库评价**：三层级联搜索
- **生态替代**：无
- **升级建议**：☑️ 足够

---

## 四、记忆类（memory.rs — 4 个技能）

### 17. MemoryRemember
- **实现**：调用 `aion_memory::memory::MemoryManager::remember()`
- **知识库评价**：使用 redb 数据库持久化
- **升级建议**：☑️ 足够

### 18. MemoryRecall
- **实现**：调用 `MemoryManager::recall()` 按关键词检索
- **知识库评价**：基于关键词匹配，非语义搜索
- **升级建议**：⬆️ 建议与语义嵌入集成

### 19. MemoryDistill
- **实现**：调用 `aion_memory::memory_distiller::MemoryDistiller`
- **知识库评价**：去重 + 过期衰减
- **升级建议**：☑️ 足够

### 20. MemoryTeamShare
- **实现**：调用 `aion_memory::namespaced_memory::share_to_team()`
- **知识库评价**：跨 Agent 共享
- **升级建议**：☑️ 足够

---

## 五、AI 类（ai.rs — 1 个技能）

### 21. AiTask
- **实现**：三引擎（Claude/OpenAI/Gemini）CLI → HTTP 降级调用
- **知识库评价**：多引擎 + 降级逻辑，但 prompt 直接传给 AI
- **升级建议**：⬆️ 建议增加 8 步框架 prompt 包装

---

## 六、Agent 协作类（agent.rs — 4 个技能）

### 22. AgentDelegate
- **实现**：通过 message_bus 向指定 Agent 发送任务
- **知识库评价**：点对点委派，无超时/重试
- **升级建议**：⬆️ 建议增加超时 + 重试

### 23. AgentBroadcast
- **实现**：广播消息到所有 Agent
- **知识库评价**：fire-and-forget
- **升级建议**：⬆️ 建议增加确认模式

### 24. AgentGather
- **实现**：多 Agent 查询 + 聚合
- **知识库评价**：异步返回，无汇总
- **升级建议**：⬆️ 建议增加 reduce 策略

### 25. AgentStatus
- **实现**：从 message_bus 查询 Agent 状态
- **知识库评价**：依赖真实 runtime 状态
- **升级建议**：☑️ 足够

---

## 七、管道类（pipeline.rs — 2 个技能）

### 26. TaskPipeline
- **实现**：串行执行多个能力，每一步结果传入下一步
- **知识库评价**：简单串行管道
- **升级建议**：⬆️ 建议升级为 DAG（支持并行分支）

### 27. TaskRace
- **实现**：多个 Agent 竞争执行，取首个成功
- **知识库评价**：竞争模式
- **升级建议**：☑️ 足够

---

## 八、MCP 类（mcp.rs — 1 个技能）

### 28. McpCall
- **实现**：通过 `McpClientManager` 调用外部 MCP 服务器工具
- **知识库评价**：使用 rmcp 3.1 SDK，支持 stdio + Streamable HTTP
- **升级建议**：☑️ 足够

---

## 九、RAG 类（rag.rs — 3 个技能）

### 29. RagIngest
- **实现**：调用 `aion_intel::rag::RagEngine::ingest()`
- **知识库评价**：文档分块 → 嵌入 → 存储
- **升级建议**：⬆️ 建议增加混合检索（BM25 + 语义）

### 30. RagQuery
- **实现**：调用 `RagEngine::query()`
- **知识库评价**：向量检索
- **升级建议**：⬆️ 建议增加重排序

### 31. RagStatus
- **实现**：调用 `RagEngine::status()`
- **知识库评价**：简单报告
- **升级建议**：☑️ 足够

---

## 十、编排类（orchestrator.rs — 13 个技能）

### 32. AsyncTaskQuery
- **实现**：查询异步任务状态
- **知识库评价**：简单查询
- **升级建议**：☑️ 足够

### 33. AiParallelSolve
- **实现**：三引擎讨论 → 共识 → 执行/仲裁
- **知识库评价**：三阶段协议写死
- **升级建议**：⬆️ 建议可配置阶段

### 34. AiTripleVote
- **实现**：三引擎独立投票
- **知识库评价**：无加权投票
- **升级建议**：⬆️ 建议增加权重

### 35. AiTriangleReview
- **实现**：三引擎并行审查代码
- **知识库评价**：无合并去重
- **升级建议**：⬆️ 建议增加结果合并

### 36. AiCodeGenerate
- **实现**：primary 生成 + reviewer 复审
- **知识库评价**：复审不修改代码
- **升级建议**：⬆️ 建议复审自动输出修复 patch

### 37. AiSmartCollaborate
- **实现**：同 parallel_solve 三阶段
- **知识库评价**：与 parallel_solve 高度重复
- **升级建议**：🔴 建议合并到 parallel_solve

### 38. AiResearch
- **实现**：三维度（理论/实践/趋势）
- **知识库评价**：depth 参数无实际策略变化
- **升级建议**：⬆️ 建议增加引用追踪

### 39. AiSerialOptimize
- **实现**：分析 → 优化 → 验证串行
- **知识库评价**：串行时延 = 三引擎之和
- **升级建议**：⬆️ 建议部分并行化

### 40. AiLongContext
- **实现**：单引擎处理长文本
- **知识库评价**：无分块
- **升级建议**：⬆️ 建议增加分块+摘要迭代

### 41. AiCrossReview
- **实现**：双引擎并行审查
- **知识库评价**：与 triangle_review 功能重叠
- **升级建议**：🔴 建议合并到 triangle_review

### 42. Brainstorm
- **实现**：多引擎并行提出方案
- **知识库评价**：无合并去重
- **升级建议**：⬆️ 建议增加收敛轮次

### 43. Compare
- **实现**：多维度评分
- **知识库评价**：无加权汇总
- **升级建议**：⬆️ 建议增加加权汇总

### 44. Discuss
- **实现**：多引擎独立讨论
- **知识库评价**：无回合制
- **升级建议**：⬆️ 建议增加回合制

---

## 十一、工具类（new_skills.rs — 14 个技能）

### 45. Echo
- **实现**：直接返回输入文本
- **知识库评价**：测试用
- **升级建议**：☑️ 足够

### 46. SpaceNavigation
- **实现**：实验性，返回星际目的地
- **知识库评价**：趣味功能
- **升级建议**：☑️ 无需升级

### 47. JsonQuery
- **实现**：jsonpath-rust 库查询 JSON
- **知识库评价**：标准实现
- **升级建议**：☑️ 足够

### 48. RegexMatch
- **实现**：regex crate 三种模式（find_all/is_match/captures）
- **知识库评价**：标准实现
- **升级建议**：☑️ 足够

### 49. CodeGenerate
- **实现**：委托给 AiTask，prompt 强调生成 Rust 代码
- **知识库评价**：无编译验证
- **升级建议**：⬆️ 建议增加沙箱编译验证

### 50. CodeLint
- **实现**：纯 Rust 规则引擎（8 类规则，文本匹配）
- **知识库评价**：纯文本级，无 AST 分析
- **升级建议**：⬆️ 建议集成 rust-analyzer 做 AST 分析

### 51. CodeTest
- **实现**：分析函数签名，生成 `todo!()` 测试脚手架
- **知识库评价**：不生成真实断言
- **升级建议**：⬆️ 建议基于分析生成真实断言

### 52. SkillReport
- **实现**：调用 learner.report()
- **知识库评价**：技能使用统计
- **升级建议**：☑️ 足够

### 53. EvolutionReport
- **实现**：调用 learner.evolution_report()
- **知识库评价**：自进化报告
- **升级建议**：☑️ 足够

### 54. SessionReport
- **实现**：调用 learner.session_report()
- **知识库评价**：会话汇总
- **升级建议**：☑️ 足够

### 55. RecordChange
- **实现**：调用 learner.record_change()，验证 kind/file/summary
- **知识库评价**：自进化记录
- **升级建议**：☑️ 足够

### 56. RecordDecision
- **实现**：调用 learner.record_decision()，验证 context/choice/rationale
- **知识库评价**：自进化记录
- **升级建议**：☑️ 足够

### 57. Sanitize
- **实现**：使用 glitch-filter 清理控制字符
- **知识库评价**：标准实现
- **升级建议**：☑️ 足够

### 58. HealthCheck
- **实现**：检查 AI 引擎 CLI 可用性
- **知识库评价**：只检查 CLI，不检查 API 实际可达性
- **升级建议**：⬆️ 建议增加 API ping

---

## 十二、其他专有技能（13 个文件共 13 个技能）

### 59. SpecDriven (spec_driven.rs)
- **实现**：5 阶段流水线（analyze→decompose→plan→execute→learn）
- **知识库评价**：完整状态机，支持回调
- **升级建议**：⬆️ 建议增加状态持久化（SQLite）

### 60. RouteTaskBuiltin (task_router.rs)
- **实现**：关键词权重路由 + 结构快筛
- **知识库评价**：17 条规则硬编码
- **升级建议**：⬆️ 建议规则配置化

### 61. ImageDescribe (image.rs)
- **实现**：base64 编码图像 → 调用 vision API
- **知识库评价**：仅分析，不生成图像
- **升级建议**：⬆️ 建议增加图像生成能力

### 62. TextToon (format.rs)
- **实现**：JSON → TOON 压缩格式（~40% token 节省）
- **知识库评价**：专有格式，非标准
- **升级建议**：☑️ 保留

### 63. PromptAudit (prompt_audit.rs)
- **实现**：8 步框架合规检查
- **知识库评价**：AI 评估 prompt 质量
- **升级建议**：☑️ 足够

### 64. EvolutionRun (evolution.rs)
- **实现**：在隔离 worktree 中评估补丁
- **知识库评价**：使用 GateSpec + 适应性评分
- **升级建议**：☑️ 足够

### 65. ErrorKnowledge (error_knowledge.rs)
- **实现**：fingerprint 查询/注册/状态转换
- **知识库评价**：完整生命周期管理
- **升级建议**：☑️ 足够

### 66. HaoJiangReview (haoojiang.rs)
- **实现**：AI 代码质量门禁
- **知识库评价**：AI 审查 + 规则检查
- **升级建议**：☑️ 足够

### 67. EvolverGovernance (evolver.rs)
- **实现**：任务分类 + 风险评级 + 能力推荐
- **知识库评价**：前置治理
- **升级建议**：☑️ 足够

### 68. SkillConvert (skill_format.rs)
- **实现**：SKILL.md ↔ forge skill.json 互转
- **知识库评价**：双向转换
- **升级建议**：☑️ 足够

### 69. MarketSearch (market.rs)
- **实现**：npx 搜索 → HTTP 发现 → Google 搜索
- **知识库评价**：三级级联
- **升级建议**：☑️ 足够

### 70. AutonomousAgent (autonomous_agent.rs)
- **实现**：目标 → 计划 → 迭代执行 → 报告
- **知识库评价**：自治 Agent
- **升级建议**：☑️ 足够

---

## 十三、ZL 辩证类（zl.rs — 8 个技能，通过宏生成）

### 71-78. ZLStrategicPlan / ZLTaskDialectic / ZLContradictionAnalyze / ZLCompileContract / ZLCheckSufficiency / ZLVerifyResult / ZLDetectDrift / ZLDialecticalRetry
- **实现**：通过 `define_zl_skill!` 宏生成，每个技能委托给 `aion_zl::Engine` 执行
- **知识库评价**：辩证哲学方法论，统一引擎
- **升级建议**：☑️ 整体足够

---

## 十四、总结：需要升级的技能

### 🔴 高优先级（当前实现有缺陷）

| 技能 | 问题 | 方案 |
|------|------|------|
| PdfParse | 不提取 PDF 内容 | 集成 `pdf-extract` |
| AiSmartCollaborate | 与 parallel_solve 重复 | 合并 |
| AiCrossReview | 与 triangle_review 重复 | 合并 |

### ⬆️ 中优先级（功能不足）

| 技能 | 问题 | 方案 |
|------|------|------|
| CsvParse | 手写解析器 | 换 `csv` crate |
| TextEmbed | TF-IDF 单文档无意义 | 接入语义 embedding |
| TextSummarize | 无长度控制 | 增加参数 |
| TextClassify | 无标签约束 | 增加 labels 参数 |
| TextExtract | 无结构输出 | 增加 schema 参数 |
| TextTranslate | 无术语表 | 增加 glossary 参数 |
| WebSearch | 单 SerpAPI | 增加 fallback |
| HttpFetch | 无代理 | 增加代理支持 |
| MemoryRecall | 关键词匹配 | 接入语义搜索 |
| AiTask | 无 prompt 包装 | 集成 8 步框架 |
| AgentDelegate | 无超时/重试 | 增加 |
| AgentBroadcast | 无确认 | 增加确认模式 |
| AgentGather | 无汇总 | 增加 reduce 策略 |
| TaskPipeline | 串行 | 升级为 DAG |
| RagIngest | 无混合检索 | 增加 BM25+语义 |
| RagQuery | 无重排序 | 增加 rerank |
| AiParallelSolve | 三阶段写死 | 可配置 |
| AiTripleVote | 无加权 | 增加权重 |
| AiTriangleReview | 无合并 | 增加结果合并 |
| AiCodeGenerate | 无编译验证 | 增加沙箱编译 |
| AiResearch | depth 无效果 | 增加真实策略 |
| AiSerialOptimize | 串行时延 | 部分并行 |
| AiLongContext | 无分块 | 增加分块摘要 |
| Brainstorm | 无收敛 | 增加收敛轮次 |
| Compare | 无汇总 | 增加加权汇总 |
| Discuss | 无回合制 | 增加回合制 |
| CodeGenerate | 无编译验证 | 增加沙箱编译 |
| CodeLint | 文本级 | 增加 AST 分析 |
| CodeTest | 只生成 todo! | 生成真实断言 |
| HealthCheck | 只查 CLI | 增加 API ping |
| SpecDriven | 状态存内存 | 增加 SQLite 持久化 |
| RouteTaskBuiltin | 规则硬编码 | 规则配置化 |
| ImageDescribe | 不生成图像 | 增加图像生成 |

### ☑️ 低优先级（当前足够）

**34 个技能**：YamlParse / JsonParse / TomlParse / TextDiff / MarkdownRender / TextWordcount / DiscoverySearch / MemoryRemember / MemoryDistill / MemoryTeamShare / AgentStatus / TaskRace / McpCall / RagStatus / AsyncTaskQuery / Echo / SpaceNavigation / JsonQuery / RegexMatch / SkillReport / EvolutionReport / SessionReport / RecordChange / RecordDecision / Sanitize / TextToon / PromptAudit / EvolutionRun / ErrorKnowledge / HaoJiangReview / EvolverGovernance / SkillConvert / MarketSearch / AutonomousAgent / ZL 8 个

---

## 十五、版本信息

- **git tag 最新**：v0.7.1
- **Cargo.toml**：CLI 入口 0.7.0，核心 crate 0.1.0
- **CHANGELOG**：只写到 v0.5.2，落后 3 个版本