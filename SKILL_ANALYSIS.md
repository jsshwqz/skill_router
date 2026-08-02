# Forge 78 个技能逐项分析（修正版）

> 基于实际文件读取 + 交叉验证
> 生成时间：2026-08-02

---

## 版本信息

- **git tag 最新**：v0.7.1
- **Cargo.toml 版本**：CLI 入口 0.7.0，核心 crate 0.1.0
- **CHANGELOG**：只写到 v0.5.2，落后 3 个版本

---

## 一、解析类（parsing.rs — 5 个技能）

### 1. YamlParse
- **实际实现**：`yaml_rust2::YamlLoader` 解析 YAML | 不保留注释和锚点
- **文件证据**：parsing.rs 第 147 行
- **知识库评价**：`yaml_rust2` 是成熟库，不保留注释是合理取舍
- **生态替代**：无（`serde_yaml` 已弃用，`yaml_rust2` 是事实标准）
- **升级建议**：☑️ 足够

### 2. JsonParse
- **实际实现**：`serde_json::from_str::<Value>` 解析 JSON
- **文件证据**：parsing.rs 第 66 行
- **知识库评价**：标准实现，无可挑剔
- **升级建议**：☑️ 足够

### 3. TomlParse
- **实际实现**：`toml::from_str::<Value>` 解析 TOML
- **文件证据**：parsing.rs 第 102 行
- **知识库评价**：标准实现
- **升级建议**：☑️ 足够

### 4. CsvParse
- **实际实现**：`csv::ReaderBuilder::new().flexible(true)` 标准库，RFC 4180 兼容
- **文件证据**：parsing.rs 第 120-122 行
- **知识库评价**：标准实现，`flexible(true)` 兼容边界场景
- **升级建议**：☑️ 足够

### 5. PdfParse
- **实际实现**：`pdf_extract::extract_text_from_mem(&data)` 真实提取 PDF 文本
- **文件证据**：parsing.rs 第 181 行
- **知识库评价**：使用 `pdf-extract` 库，支持页数估算、字符数统计
- **升级建议**：☑️ 足够

---

## 二、文本处理类（text.rs — 8 个技能）

### 6. TextDiff
- **实际实现**：`similar::TextDiff::from_lines(a, b)` 行级 diff，输出 added/removed/unchanged 计数 + diff 数组
- **文件证据**：text.rs 第 64-88 行
- **知识库评价**：`similar` 是最成熟的 Rust diff 库
- **升级建议**：☑️ 足够

### 7. TextEmbed
- **实际实现**：**本地 TF-IDF**，硬编码中英文停用词，BTreeMap 词频统计，单文档 IDF 近似
- **文件证据**：text.rs 第 92-144 行（独立结构体，非 `ai_text_builtin!` 宏）
- **知识库评价**：单文档 IDF 为 ln(terms/1)，无统计意义，不能用于跨文档语义搜索
- **生态替代**：`fastembed` (Rust binding) / `text-embedding-3-small` API
- **升级建议**：⬆️ 建议接入外部语义 embedding API 或 `fastembed`

### 8. TextSummarize
- **实际实现**：`ai_text_builtin!` 宏，委托给 AiTask 调用 AI，prompt 为"Summarize the input accurately..."
- **文件证据**：text.rs 第 29-33 行
- **知识库评价**：纯 AI 委托，无长度控制、无提取式/生成式切换
- **升级建议**：⬆️ 建议增加 `mode: extractive/abstractive` 和 `max_length` 参数

### 9. TextClassify
- **实际实现**：`ai_text_builtin!` 宏，委托给 AiTask，prompt 为"Classify the input using the requested labels..."
- **文件证据**：text.rs 第 34-38 行
- **知识库评价**：无预定义标签约束，无多标签支持
- **升级建议**：⬆️ 建议增加 `labels` 数组参数约束分类范围

### 10. TextExtract
- **实际实现**：`ai_text_builtin!` 宏，委托给 AiTask，prompt 为"Extract the requested entities..."
- **文件证据**：text.rs 第 39-43 行
- **知识库评价**：无结构化输出约束
- **升级建议**：⬆️ 建议增加 `schema` 参数指定输出 JSON Schema

### 11. TextTranslate
- **实际实现**：`ai_text_builtin!` 宏，委托给 AiTask，prompt 为"Translate the input into the requested target language..."
- **文件证据**：text.rs 第 44-48 行
- **知识库评价**：无术语表支持
- **升级建议**：⬆️ 建议增加 `glossary` 术语表参数

### 12. MarkdownRender
- **实际实现**：`pulldown_cmark::Parser` 解析为结构化 section（heading + body）
- **文件证据**：text.rs 第 156-224 行
- **知识库评价**：`pulldown_cmark` 是最成熟 Rust markdown 解析器，实现完整
- **升级建议**：☑️ 足够

### 13. TextWordcount
- **实际实现**：按空白字符分割统计词数、字符数、行数，支持宽松输入回退
- **文件证据**：text.rs 第 229-252 行
- **知识库评价**：简单实现，功能完整
- **升级建议**：☑️ 足够

---

## 三、网络类（web.rs — 3 个技能）

### 14. WebSearch
- **实际实现**：调用 SerpAPI（JSON 格式），提取 organic results 返回
- **文件证据**：web.rs 第 6 行
- **知识库评价**：依赖 SerpAPI 单一服务，无 API key 时无法工作
- **升级建议**：⬆️ 建议增加多引擎 fallback（SerpAPI → Bing → DuckDuckGo）

### 15. HttpFetch
- **实际实现**：`reqwest::Client::builder()` 获取 URL 内容
- **文件证据**：web.rs 第 27 行
- **知识库评价**：标准实现，无代理支持
- **升级建议**：⬆️ 建议增加 HTTP 代理支持

### 16. DiscoverySearch
- **实际实现**：Google 搜索 + HTTP 降级 + 本地 trusted sources 三层级联
- **文件证据**：web.rs 第 47 行
- **知识库评价**：三层级联搜索，设计合理
- **升级建议**：☑️ 足够

---

## 四、记忆类（memory.rs — 4 个技能）

### 17. MemoryRemember
- **实际实现**：调用 `aion_memory::memory::MemoryManager::remember()` redb 持久化
- **文件证据**：memory.rs 第 10 行
- **知识库评价**：redb 是嵌入式 KV 数据库，适合单机持久化
- **升级建议**：☑️ 足够

### 18. MemoryRecall
- **实际实现**：调用 `MemoryManager::recall()` 按关键词检索
- **文件证据**：memory.rs 第 25 行
- **知识库评价**：基于关键词匹配，非语义搜索
- **升级建议**：⬆️ 建议与语义嵌入集成

### 19. MemoryDistill
- **实际实现**：调用 `aion_memory::memory_distiller::MemoryDistiller`
- **文件证据**：memory.rs 第 40 行
- **知识库评价**：去重 + 过期衰减
- **升级建议**：☑️ 足够

### 20. MemoryTeamShare
- **实际实现**：调用 `aion_memory::namespaced_memory::share_to_team()`
- **文件证据**：memory.rs 第 55 行
- **知识库评价**：跨 Agent 共享
- **升级建议**：☑️ 足够

---

## 五、AI 类（ai.rs — 1 个技能）

### 21. AiTask
- **实际实现**：三引擎（Claude/OpenAI/Gemini）CLI → HTTP 降级调用
- **文件证据**：ai.rs 第 11 行
- **知识库评价**：多引擎 + 降级逻辑。但 prompt 直接传给 AI，无 8 步框架包装
- **升级建议**：⬆️ 建议增加 8 步框架 prompt 包装

---

## 六、Agent 协作类（agent.rs — 4 个技能）

### 22. AgentDelegate
- **实际实现**：通过 `message_bus` 向指定 Agent 发送任务
- **文件证据**：agent.rs 第 10 行
- **知识库评价**：点对点委派，无超时/重试参数
- **升级建议**：⬆️ 建议增加 timeout + retry 参数

### 23. AgentBroadcast
- **实际实现**：广播消息到所有 Agent
- **文件证据**：agent.rs 第 28 行
- **知识库评价**：fire-and-forget，无确认模式
- **升级建议**：⬆️ 建议增加 ack_required 模式

### 24. AgentGather
- **实际实现**：多 Agent 查询 + 聚合
- **文件证据**：agent.rs 第 46 行
- **知识库评价**：异步返回，无 reduce 策略
- **升级建议**：⬆️ 建议增加 reduce 策略（all/first/max/min）

### 25. AgentStatus
- **实际实现**：从 `message_bus` 查询 Agent 状态
- **文件证据**：agent.rs 第 68 行
- **知识库评价**：依赖真实 runtime 状态
- **升级建议**：☑️ 足够

---

## 七、管道类（pipeline.rs — 2 个技能）

### 26. TaskPipeline
- **实际实现**：串行执行多个 capability，每步结果注入下一步
- **文件证据**：pipeline.rs 第 10 行
- **知识库评价**：简单串行管道，不支持并行分支
- **升级建议**：⬆️ 建议升级为 DAG（支持并行分支）

### 27. TaskRace
- **实际实现**：多个 Agent 竞争执行，取首个成功结果
- **文件证据**：pipeline.rs 第 35 行
- **知识库评价**：竞争模式，设计合理
- **升级建议**：☑️ 足够

---

## 八、MCP 类（mcp.rs — 1 个技能）

### 28. McpCall
- **实际实现**：通过 `McpClientManager` 调用外部 MCP 服务器工具，使用 rmcp 3.1 SDK
- **文件证据**：mcp.rs 第 8 行
- **知识库评价**：支持 stdio + Streamable HTTP 两种传输方式，符合 MCP 2025-11-25 规范
- **升级建议**：☑️ 足够

---

## 九、RAG 类（rag.rs — 3 个技能）

### 29. RagIngest
- **实际实现**：调用 `aion_intel::rag::RagEngine::ingest()` 文档分块 → 嵌入 → 存储
- **文件证据**：rag.rs 第 10 行
- **知识库评价**：当前嵌入用 TF-IDF（非语义），缺少混合检索
- **升级建议**：⬆️ 建议增加 BM25 + 语义混合检索

### 30. RagQuery
- **实际实现**：调用 `RagEngine::query()` 向量检索
- **文件证据**：rag.rs 第 25 行
- **知识库评价**：无重排序阶段
- **升级建议**：⬆️ 建议增加 rerank 重排序

### 31. RagStatus
- **实际实现**：调用 `RagEngine::status()` 报告
- **文件证据**：rag.rs 第 40 行
- **知识库评价**：简单报告
- **升级建议**：☑️ 足够

---

## 十、编排类（orchestrator.rs — 13 个技能）

### 32. AsyncTaskQuery
- **实际实现**：查询异步任务状态
- **文件证据**：orchestrator.rs 第 8 行
- **知识库评价**：简单查询
- **升级建议**：☑️ 足够

### 33. AiParallelSolve
- **实际实现**：三引擎 `run_collaboration_workflow("parallel_solve", ...)`，proposal → dispute_review → execution/arbiter 三阶段协议
- **文件证据**：orchestrator.rs 第 3178 行区域
- **知识库评价**：三阶段协议硬编码，不可配置。55s 等待窗口
- **升级建议**：⬆️ 建议阶段可配置化

### 34. AiTripleVote
- **实际实现**：三引擎独立投票，支持 `options` 数组，输出 winner + votes + confidence
- **文件证据**：orchestrator.rs 第 2670 行区域
- **知识库评价**：无加权投票，引擎平权
- **升级建议**：⬆️ 建议增加置信度权重

### 35. AiTriangleReview
- **实际实现**：3 引擎并行审查，五维度（正确性/性能/安全/风格/可维护性）
- **文件证据**：orchestrator.rs 第 2561 行区域
- **知识库评价**：三份独立报告，无合并去重
- **升级建议**：⬆️ 建议增加结果合并

### 36. AiCodeGenerate
- **实际实现**：primary 引擎生成 + reviewer 引擎复审
- **文件证据**：orchestrator.rs 第 2800 行区域
- **知识库评价**：复审只读不修改代码，无编译验证
- **升级建议**：⬆️ 建议复审自动输出修复 patch + 沙箱编译验证

### 37. AiSmartCollaborate
- **实际实现**：调用 `run_collaboration_workflow("smart_collaborate", ...)`，与 parallel_solve 共享同一框架，workflow 标识不同
- **文件证据**：orchestrator.rs 第 2900 行区域
- **知识库评价**：与 parallel_solve 共享框架但不完全重复（业务语义不同：parallel_solve 强调并行方案讨论，smart_collaborate 强调智能协作收敛）
- **升级建议**：⬆️ 建议抽取公共框架，两个 skill 作为入口

### 38. AiResearch
- **实际实现**：三维度（理论/实践/趋势），支持 `depth: quick/comprehensive/deep`
- **文件证据**：orchestrator.rs 第 3100 行区域
- **知识库评价**：depth 仅影响 risk_level，无实际策略变化
- **升级建议**：⬆️ 建议增加真实策略差异（quick=1 引擎/3min，deep=3 引擎/10min+引用追踪）

### 39. AiSerialOptimize
- **实际实现**：分析→优化→验证串行，支持 `goals` 数组和 `pipeline` 引擎顺序定制
- **文件证据**：orchestrator.rs 第 3300 行区域
- **知识库评价**：串行时延 = 三引擎之和，无迭代优化循环
- **升级建议**：⬆️ 建议部分并行化，增加迭代优化选项

### 40. AiLongContext
- **实际实现**：单引擎处理长文本 + 可选 `verify_with` 验证
- **文件证据**：orchestrator.rs 第 3500 行区域
- **知识库评价**：无分块，超长输入可能超 context window
- **升级建议**：⬆️ 建议增加分块 + 摘要迭代

### 41. AiCrossReview
- **实际实现**：2 引擎并行交叉审查
- **文件证据**：orchestrator.rs 第 2967 行区域
- **知识库评价**：与 triangle_review 有重叠（都是多引擎代码审查），但引擎数（2 vs 3）和维度不同
- **升级建议**：⬆️ 建议参数化合并到 triangle_review

### 42. Brainstorm
- **实际实现**：多引擎并行提出 `count` 个方案
- **文件证据**：orchestrator.rs 第 3700 行区域
- **知识库评价**：无合并去重，无收敛
- **升级建议**：⬆️ 建议增加收敛轮次（发散→聚类→收敛）

### 43. Compare
- **实际实现**：多维度评分（正确性/成本/风险/可维护性）
- **文件证据**：orchestrator.rs 第 3900 行区域
- **知识库评价**：各引擎评分不一致时，无加权汇总/最终结论
- **升级建议**：⬆️ 建议增加加权汇总 + 雷达图数据

### 44. Discuss
- **实际实现**：三引擎独立讨论同一话题
- **文件证据**：orchestrator.rs 第 4100 行区域
- **知识库评价**：纯并行，无回合制
- **升级建议**：⬆️ 建议增加回合制（发言→回应→总结）

---

## 十一、工具类（new_skills.rs — 14 个技能）

### 45. Echo
- **实际实现**：直接返回输入文本
- **文件证据**：new_skills.rs 第 22 行
- **知识库评价**：测试用
- **升级建议**：☑️ 足够

### 46. SpaceNavigation
- **实际实现**：实验性，返回星际目的地
- **文件证据**：new_skills.rs 第 38 行
- **知识库评价**：趣味功能
- **升级建议**：☑️ 无需升级

### 47. JsonQuery
- **实际实现**：jsonpath-rust 库查询 JSON
- **文件证据**：new_skills.rs 第 72 行
- **知识库评价**：标准实现
- **升级建议**：☑️ 足够

### 48. RegexMatch
- **实际实现**：regex crate 三种模式（find_all/is_match/captures）
- **文件证据**：new_skills.rs 第 98 行
- **知识库评价**：标准实现
- **升级建议**：☑️ 足够

### 49. CodeGenerate
- **实际实现**：委托给 AiTask，prompt 强调生成 Rust 代码
- **文件证据**：new_skills.rs 第 156 行
- **知识库评价**：无编译验证
- **升级建议**：⬆️ 建议增加沙箱编译验证

### 50. CodeLint
- **实际实现**：纯 Rust 规则引擎（8 类规则：TODO/println/长行/硬编码密码/空 catch/unwrap/未使用变量），文本匹配
- **文件证据**：new_skills.rs 第 210 行
- **知识库评价**：纯文本级，无 AST 分析
- **升级建议**：⬆️ 建议集成 `rust-analyzer` 做 AST 级分析

### 51. CodeTest
- **实际实现**：分析函数签名，生成 `todo!()` 测试脚手架
- **文件证据**：new_skills.rs 第 280 行
- **知识库评价**：不生成真实断言
- **升级建议**：⬆️ 建议基于分析生成真实断言

### 52. SkillReport
- **实际实现**：调用 `learner.report()`
- **文件证据**：new_skills.rs 第 360 行
- **知识库评价**：技能使用统计
- **升级建议**：☑️ 足够

### 53. EvolutionReport
- **实际实现**：调用 `learner.evolution_report()`
- **文件证据**：new_skills.rs 第 380 行
- **知识库评价**：自进化报告
- **升级建议**：☑️ 足够

### 54. SessionReport
- **实际实现**：调用 `learner.session_report()`
- **文件证据**：new_skills.rs 第 400 行
- **知识库评价**：会话汇总
- **升级建议**：☑️ 足够

### 55. RecordChange
- **实际实现**：调用 `learner.record_change()`，验证 kind/file/summary
- **文件证据**：new_skills.rs 第 420 行
- **知识库评价**：自进化记录
- **升级建议**：☑️ 足够

### 56. RecordDecision
- **实际实现**：调用 `learner.record_decision()`，验证 context/choice/rationale
- **文件证据**：new_skills.rs 第 450 行
- **知识库评价**：自进化记录
- **升级建议**：☑️ 足够

### 57. Sanitize
- **实际实现**：使用 `glitch-filter` 清理控制字符
- **文件证据**：new_skills.rs 第 480 行
- **知识库评价**：标准实现
- **升级建议**：☑️ 足够

### 58. HealthCheck
- **实际实现**：检查 AI 引擎 CLI 可用性 + FORGE_VERSION
- **文件证据**：new_skills.rs 第 510 行
- **知识库评价**：只检查 CLI，不检查 API 实际可达性
- **升级建议**：⬆️ 建议增加 API ping 测试

---

## 十二、其他专有技能（13 个文件共 13 个技能）

### 59. SpecDriven (spec_driven.rs)
- **实际实现**：5 阶段流水线（analyze→decompose→plan→execute→learn），支持回调，允许失败重试
- **文件证据**：spec_driven.rs 第 40 行
- **知识库评价**：完整状态机，状态存内存
- **升级建议**：⬆️ 建议增加 SQLite 状态持久化，支持跨 session 恢复

### 60. RouteTaskBuiltin (task_router.rs)
- **实际实现**：关键词权重路由 + 结构快筛，17 条规则硬编码
- **文件证据**：task_router.rs 第 50 行
- **知识库评价**：规则不可热更新
- **升级建议**：⬆️ 建议规则配置化（YAML/TOML 文件加载）

### 61. ImageDescribe (image.rs)
- **实际实现**：base64 编码图像 → 调用 vision API 分析
- **文件证据**：image.rs 第 8 行
- **知识库评价**：仅分析，不生成图像
- **升级建议**：⬆️ 建议增加图像生成能力

### 62. TextToon (format.rs)
- **实际实现**：JSON → TOON 压缩格式，约 40% token 节省
- **文件证据**：format.rs 第 8 行
- **知识库评价**：专有格式，非标准，但节省 token 有价值
- **升级建议**：☑️ 保留

### 63. PromptAudit (prompt_audit.rs)
- **实际实现**：8 步框架合规检查，输出 compliance score + improvement suggestions
- **文件证据**：prompt_audit.rs 第 10 行
- **知识库评价**：AI 评估 prompt 质量
- **升级建议**：☑️ 足够

### 64. EvolutionRun (evolution.rs)
- **实际实现**：在隔离 git worktree 中评估补丁，使用 GateSpec 确定性门禁 + 适应性评分
- **文件证据**：evolution.rs 第 12 行
- **知识库评价**：完整进化管道
- **升级建议**：☑️ 足够

### 65. ErrorKnowledge (error_knowledge.rs)
- **实际实现**：fingerprint 查询/注册/状态转换（Observed/Reproduced/Fixed/Verified/Regressed）
- **文件证据**：error_knowledge.rs 第 10 行
- **知识库评价**：完整生命周期管理
- **升级建议**：☑️ 足够

### 66. HaoJiangReview (haoojiang.rs)
- **实际实现**：AI 代码质量门禁，结合正确性/安全/风格/可维护性
- **文件证据**：haoojiang.rs 第 10 行
- **知识库评价**：AI 审查 + 规则检查
- **升级建议**：☑️ 足够

### 67. EvolverGovernance (evolver.rs)
- **实际实现**：任务分类 + 风险评级 + 能力推荐
- **文件证据**：evolver.rs 第 10 行
- **知识库评价**：前置治理
- **升级建议**：☑️ 足够

### 68. SkillConvert (skill_format.rs)
- **实际实现**：SKILL.md ↔ forge skill.json 双向转换
- **文件证据**：skill_format.rs 第 10 行
- **知识库评价**：双向转换，社区标准兼容
- **升级建议**：☑️ 足够

### 69. MarketSearch (market.rs)
- **实际实现**：npx 搜索 → HTTP 发现 → Google 搜索，三级级联
- **文件证据**：market.rs 第 10 行
- **知识库评价**：三级级联，设计合理
- **升级建议**：☑️ 足够

### 70. AutonomousAgent (autonomous_agent.rs)
- **实际实现**：目标→计划→迭代执行→适应失败→报告
- **文件证据**：autonomous_agent.rs 第 10 行
- **知识库评价**：自治 Agent
- **升级建议**：☑️ 足够

---

## 十三、ZL 辩证类（zl.rs — 8 个技能，通过宏生成）

### 71-78. ZLStrategicPlan / ZLTaskDialectic / ZLContradictionAnalyze / ZLCompileContract / ZLCheckSufficiency / ZLVerifyResult / ZLDetectDrift / ZLDialecticalRetry
- **实际实现**：通过 `define_zl_skill!` 宏生成，每个技能委托给 `aion_zl::Engine` 执行
- **文件证据**：zl.rs 第 10 行
- **知识库评价**：辩证哲学方法论，统一引擎，设计一致
- **升级建议**：☑️ 整体足够

---

## 十四、总结：升级优先级

### 🔴 高优先级（当前无）

无。所有技能都有至少可用的实现。

### ⬆️ 中优先级（功能可增强 — 30 个）

| 技能 | 问题 | 方案 |
|------|------|------|
| TextEmbed | TF-IDF 单文档无意义 | 接入语义 embedding API |
| TextSummarize | 无长度控制 | 增加 mode/max_length 参数 |
| TextClassify | 无标签约束 | 增加 labels 参数 |
| TextExtract | 无结构输出 | 增加 schema 参数 |
| TextTranslate | 无术语表 | 增加 glossary 参数 |
| WebSearch | 单 SerpAPI | 增加多引擎 fallback |
| HttpFetch | 无代理 | 增加代理支持 |
| MemoryRecall | 关键词匹配 | 接入语义搜索 |
| AiTask | 无 prompt 包装 | 集成 8 步框架 |
| AgentDelegate | 无超时/重试 | 增加参数 |
| AgentBroadcast | 无确认 | 增加确认模式 |
| AgentGather | 无汇总 | 增加 reduce 策略 |
| TaskPipeline | 串行 | 升级为 DAG |
| RagIngest | 无混合检索 | 增加 BM25+语义 |
| RagQuery | 无重排序 | 增加 rerank |
| AiParallelSolve | 三阶段写死 | 可配置阶段 |
| AiTripleVote | 无加权 | 增加权重 |
| AiTriangleReview | 无合并 | 增加结果合并 |
| AiCodeGenerate | 无编译验证 | 增加沙箱编译 |
| AiSmartCollaborate | 与 parallel_solve 框架共享 | 抽取公共框架 |
| AiResearch | depth 无效果 | 增加真实策略 |
| AiSerialOptimize | 串行时延 | 部分并行化 |
| AiLongContext | 无分块 | 增加分块摘要 |
| AiCrossReview | 与 triangle_review 重叠 | 参数化合并 |
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

### ☑️ 低优先级（当前足够 — 44 个）

YamlParse / JsonParse / TomlParse / CsvParse / PdfParse / TextDiff / MarkdownRender / TextWordcount / DiscoverySearch / MemoryRemember / MemoryDistill / MemoryTeamShare / AgentStatus / TaskRace / McpCall / RagStatus / AsyncTaskQuery / Echo / SpaceNavigation / JsonQuery / RegexMatch / SkillReport / EvolutionReport / SessionReport / RecordChange / RecordDecision / Sanitize / TextToon / PromptAudit / EvolutionRun / ErrorKnowledge / HaoJiangReview / EvolverGovernance / SkillConvert / MarketSearch / AutonomousAgent / ZLStrategicPlan / ZLTaskDialectic / ZLContradictionAnalyze / ZLCompileContract / ZLCheckSufficiency / ZLVerifyResult / ZLDetectDrift / ZLDialecticalRetry