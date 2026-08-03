# Forge 升级遗留问题清单（含质量审计）

> 2026-08-03 15:40
> 基于 orchestrator.rs (3077行)、ai.rs、rag.rs、parsing.rs、memory.rs、Cargo.toml 逐项读取验证
> 声明完成度 60%，实际约 40%

---

## 一、真正完成 ✅

### P3 — 依赖升级

| 项目 | 状态 | 证据 |
|------|------|------|
| reqwest 0.13 | ✅ | Cargo.toml + 实际使用 Client::builder() |
| axum 0.8 | ✅ | Cargo.toml + 实际使用 Router/routing::get/Json |
| edition 2024 | ✅ | 各 crate 声明 |
| rmcp 3.1 | ✅ | use rmcp + SSETransport |
| aion-server 认证 | ✅ | AuthMiddleware Basic/Bearer |
| aion-zl 测试 | ✅ | 各模块 #[cfg(test)] |

### P3 — parsing.rs 换库

| 技能 | 新库 | 行号 | 状态 |
|------|------|------|------|
| yaml_parse | yaml_rust2::YamlLoader | 31 | ✅ |
| toml_parse | toml::from_str | 98 | ✅ |
| csv_parse | csv::ReaderBuilder | 120 | ✅ |
| pdf_parse | pdf_extract | 181 | ✅ |

### P1 — RAG

| 项目 | 位置 | 状态 |
|------|------|------|
| BM25 关键词搜索 | rag.rs 208-216 | ✅ |
| 语义向量搜索 | rag.rs 96-106 | ✅ |
| 双通道融合 | rag.rs 72-105 | ✅ |

### P2 — 编排基础

| 项目 | 位置 | 状态 |
|------|------|------|
| 熔断器 | orchestrator.rs 595-783 | ✅ |
| Engine 中立化 | 全文件 | ✅ |
| 多引擎 fallback | orchestrator.rs 2276-2315 | ✅ |

---

## 二、质量问题（需修复）

### 🔴 P0-1：AiParallelSolve 的 Smart 模式没有四阶段 pipeline

**orchestrator.rs 2524-2594**

AiParallelSolve 通过 `mode` 参数调用 `run_collaboration_workflow`，传 workflow 名 "smart_collaborate"。但 `run_collaboration_workflow` 不区分 workflow 名，两个模式跑同一个通用 loop。

升级计划要求 Smart 模式实现：讨论 → 共识 → 合并 → 投票四阶段 pipeline。

**修复**：在 `run_collaboration_workflow` 中增加 workflow 分支，Smart 模式走四阶段 pipeline，Parallel 模式走现有循环。

---

### 🔴 P1-1：AiTaskBuilder 是死代码

**ai.rs 56-148**

完整的 builder（8 字段 + build()），但 ai_task / ai_task_with_schema 都用硬编码 format!() prompt，builder 零调用。

**修复**：将 AiTaskBuilder 接入 ai_task 和 ai_task_with_schema 的 prompt 构造路径。

---

### 🟠 P2-1：ReviewMerger::merge_reviews() 评分是玩具

**orchestrator.rs 2637-2671**

```
if review.contains("good") -> 0.8
if review.contains("bad")  -> 0.2
else                       -> 0.5
consensus = true (硬编码)
```

真实评审是几百字分析，不含 "good"/"bad"，全部走 else 0.5。

**修复**：用情感关键词统计 + 结构标记（"优点/缺点"、"pros/cons"）+ 评分方差计算共识。

---

### 🟠 P3-1：WorkflowConfig::load_from_yaml() 是空实现

**orchestrator.rs 2515-2518**

`let _content = std::fs::read_to_string(path)?; Ok(Self::default())`

读文件后直接丢弃内容。根 Cargo.toml 已有 yaml-rust2 = "0.12"。

**修复**：用 yaml-rust2 解析 YAML 填充 WorkflowConfig 字段。

---

### 🟠 P3-2：WorkflowOutcome 20 个字段中 ~12 个未填充

**orchestrator.rs 1913-1932**

pending_engines / arbiter_reason / degraded / final_solution / solutions / review_feedback / engines_used 等字段初始化后不修改。

**修复**：清理死字段或接入填充逻辑。

---

### 🟡 P3-3：5 个 builtin 代码不存在

**parsing.rs 中搜索不到**：json_query / markdown_render / text_diff / skill_convert / ini_parse

根 Cargo.toml 已声明 pulldown-cmark=0.13、similar=3.1、jsonpath-rust=1.0。

**修复**：用已声明库实现。

---

### 🟡 P3-4：aion-memory 未换 redb

根 Cargo.toml 声明 redb=4.1，memory.rs 中搜索不到 redb，仍用 std::fs。

**修复**：迁移到 redb KV 数据库。

---

### 🟡 P2-2：PreferenceMode 枚举未接入

**ai.rs 68-73**

声明 Default/Fast/Balanced/Precise/Long 但无 match 分支。

**修复**：接入 prompt 构造或 API 参数映射。

---

## 三、总结

| 阶段 | 声称 | 实际 | 差距 |
|------|------|------|------|
| P0 编排合并 | ✅ | 🔴 Smart 模式 pipeline 缺失 | 功能退化 |
| P1 Prompt 框架 | ✅ | 🔴 Builder 是死代码 | 未接入 |
| P1 RAG | ✅ | ✅ 可信 | — |
| P2 加权投票 | ✅ | 🟠 评分玩具级 | 需重写 |
| P2 熔断/Engine | ✅ | ✅ 可信 | — |
| P3 依赖升级 | ✅ | ✅ 可信 | — |
| P3 parsing 换库 | ✅ 4/9 | 🟡 5 个缺失 | 需补全 |
| P3 memory redb | ✅ 声明 | 🔴 未接入 | 需迁移 |

**真实完成度：约 40%（20/50 验收点）**
**剩余工作量：8-12 人天**

---

## 四、本轮修复任务分配

| Agent | 负责 |
|-------|------|
| Agent-A | P0-1: Smart pipeline + P3-1: load_from_yaml |
| Agent-B | P1-1: AiTaskBuilder 接入 |
| Agent-C | P2-1: merge_reviews + P2-2: PreferenceMode + P3-2: WorkflowOutcome |
| Agent-D | P3-3: 5 builtin 实现 |
| Agent-E | P3-4: memory → redb |
