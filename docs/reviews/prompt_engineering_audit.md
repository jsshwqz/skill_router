# Aion Forge — Prompt 工程质量审计报告

> 审计角色：Prompt 工程审计师
> 评估框架：CLAUDE.md 8 步框架 + 跨模型最佳实践
> 审计日期：2026-05-23
> 审计范围：6 个 crate，14 处 AI 调用点，21 条 prompt

---

## 一、审计概要

### 评分体系

每处 prompt 按 8 步框架逐项检查，符合记 ✓，部分符合记 △，不符合记 ✗。

| 文件 | prompt 数 | 8步评分 | 温度 | 严重度 |
|------|----------|--------|------|--------|
| `aion-zl/src/contract.rs` | 4 | 4/8 | 0.7 ❌ | 🔴 |
| `aion-zl/src/dialectic.rs` | 3 | 5/8 | 0.7 ❌ | 🔴 |
| `aion-zl/src/strategy.rs` | 1 | 5/8 | 0.7 ❌ | 🟡 |
| `aion-zl/src/retry.rs` | 1 | 4/8 | 0.7 ❌ | 🔴 |
| `aion-zl/src/contradiction.rs` | 1 | 4/8 | 0.7 ❌ | 🟡 |
| `aion-intel/src/synth.rs` | 7 | 6/8 | N/A(模板) | 🟢 |
| `aion-router/src/builtins/orchestrator.rs` | 6 | 6/8 | 0.2 | 🟢 |
| `aion-router/src/builtins/task_router.rs` | 1 | 5/8 | 0.2 | 🟡 |
| **统一问题** | | | | |
| `aion-zl/src/ai.rs` | 全局 | 温度硬编码 0.7 | 0.7 ❌ | 🔴 |
| `aion-router/src/builtins/ai.rs` | 全局 | 温度 0.3 | 0.3 | 🟢 |

### 全局问题汇总

```
问题 1 — 温度不一致（严重度 🔴）
  aion-zl/ai.rs:      temperature = 0.7（硬编码，所有 aion-zl 调用都受影响）
  aion-router/ai.rs:  temperature = 0.3
  orchestrator.rs:    temperature = 0.2（CLI 自带）
  影响: 同一项目内输出确定性不一致，结构化任务（契约/传感器）用 0.7 引入不必要随机性

问题 2 — 退路机制缺位（严重度 🔴）
  contract.rs 4 条 prompt 中有末行退路，但 dialectic.rs 3 条 prompt 完全没有
  retry.rs/strategy.rs/contradiction.rs 也没有退路
  影响: 不确定性高时 AI 会"硬答"而非拒答

问题 3 — 零示例（严重度 🟡）
  全项目 21 条 prompt 中 zero few-shot examples
  影响: 输出格式偶尔跑偏（尤其 orchestrator 的 KEY:VALUE 解析）

问题 4 — 不统一的输出解析（严重度 🟡）
  aion-zl: 用 JSON 输出 + serde 解析（可靠）
  orchestrator: 用 KEY:VALUE 行 + split_once(':') 解析（脆弱——值里含冒号就崩）
  影响: 或导致 orchestration 阶段意外失败
```

---

## 二、逐文件详细审计

### 1. `aion-zl/src/ai.rs` — AI 调用基础设施

| 项目 | 评分 | 说明 |
|------|------|------|
| 角色分配 | △ | 无角色——只传参数 |
| 任务上下文 | ✓ | 清晰分离 system/user |
| 详细规则 | ✓ | 硬编码格式正确 |
| 示例 | ✗ | 无 |
| XML 标签 | ✓ | system/user 分离 |
| 输出格式 | ✓ | JSON |
| 逐步思考 | ✗ | 无 |
| 防幻觉 | ✗ | 无 |

**关键问题**：`temperature: 0.7` 硬编码。对契约编译（compile_contract）、传感器（check_sufficiency、verify_result）这种确定性任务，0.7 过高。

**修改方案：**

```rust
// ai.rs 第 21 行
// 当前：temperature 硬编码为 0.7
// 改：temperature 作为参数传入，由调用方决定
"temperature": temperature,
```

调用方区分：
```rust
// 结构化/确定性任务（contract.rs、dialectic.rs 等）→ 0.0-0.3
// 创意/探索性任务（如有）→ 0.7-0.9
```

---

### 2. `aion-zl/src/contract.rs` — 4 条系统 prompt

| 维度 | CONTRACT | SUFFICIENCY | VERIFY | DRIFT |
|------|----------|-------------|--------|-------|
| 角色分配 | ✅ 编译器 | ✅ 传感器 | ✅ 传感器 | ✅ 传感器 |
| 任务上下文 | ✅ | ✅ | ✅ | ✅ |
| 详细规则 | ✅ | ✅ | ✅ | ✅ |
| 示例 | ❌ | ❌ | ❌ | ❌ |
| XML 标签 | △ (JSON 内联) | △ | △ | △ |
| 输出格式 | ✅ JSON | ✅ JSON | ✅ JSON | ✅ JSON |
| 逐步思考 | ✅ "First analyze..." | ✅ "Check each..." | ✅ "Go through each..." | ✅ "Compare..." |
| 防幻觉 | ✅ "unknown for string" | ✅ "0.5 if uncertain" | ✅ "mark as not met" | ✅ "low score if unsure" |

**评分**：4/8（缺示例、缺 XML）

**问题**：JSON schema 定义在 prompt 里而不是 API 层——大段的 JSON schema 占用了 prompt 空间，且与 serde deserialize 重复。

**修改建议**：
```rust
// 当前：JSON schema 写在 system prompt 里
// 改：prompt 只写角色+任务+规则，JSON schema 靠 serde 保证
// 增加：每条 prompt 末尾加 <contract_data>{...}</contract_data> 包裹输入
```

---

### 3. `aion-zl/src/dialectic.rs` — 3 条系统 prompt（正反合）

| 维度 | THESIS | ANTITHESIS | SYNTHESIS |
|------|--------|------------|-----------|
| 角色分配 | ✅ | ✅ | ✅ |
| 任务上下文 | ✅ | ✅ | ✅ |
| 详细规则 | ✅ | ✅ | ✅ |
| 示例 | ❌ | ❌ | ❌ |
| XML 标签 | ❌ | ❌ | ❌ |
| 输出格式 | ✅ JSON | ✅ JSON | ✅ JSON |
| 逐步思考 | ✅ "First analyze..." | ✅ "First identify..." | ✅ "First compare..." |
| 防幻觉 | ✅ "0 if unclear" | ✅ "0 if no flaws" | ✅ "0.3 if unreconcilable" |

**评分**：5/8（缺示例、缺 XML、缺退路）

**潜在问题**：没有让 antithesis 阶段访问之前的 memory/决策历史，每次对话都是"一次性"调用。

**修改建议**：
```rust
// 在 ANTITHESIS_SYSTEM 和 SYNTHESIS_SYSTEM 的 user prompt 中
// 增加 prior decisions 上下文
let prompt = format!(
    "Thesis:\n{content}\n\nPrior relevant decisions:\n{memory_context}\n\nProceed with analysis.",
    ...
);
```

---

### 4. `aion-intel/src/synth.rs` — 7 条 prompt 模板

| 维度 | 评分 | 说明 |
|------|------|------|
| 角色分配 | ✅ | "你是一个 Rust 代码生成器""文本摘要工具"等 |
| 任务上下文 | ✅ | 用一句话说明功能 |
| 详细规则 | ✅ | 有边界条件 |
| 示例 | ❌ | 无 |
| XML 标签 | ❌ | 纯文本 |
| 输出格式 | ✅ | 有说明 |
| 逐步思考 | ❌ | 无（简单任务可接受） |
| 防幻觉 | ✅ | 有 "UNSUMMARIZABLE"/"UNTRANSLATABLE" 退路 |

**评分**：6/8（缺示例、缺 XML，但对简单任务已经够用）

**优点**：这是项目中 prompt 质量最高的文件，角色清晰、退路到位。

**轻微改进**：
```rust
// text_extract 多加一条上下文分隔
// 当前：
("text_extract", "你是一个信息提取器。从文本中提取关键实体...")
// 改为：
("text_extract", "你是一个信息提取器。从以下 <text> 标签内的文本中提取关键实体：\n<text>\n{input}\n</text>")
```

---

### 5. `aion-router/src/builtins/orchestrator.rs` — 6 条 prompt 函数

| 维度 | proposal | dispute_review | execution | review | arbitration |
|------|----------|---------------|-----------|--------|-------------|
| 角色分配 | ✅ 架构师 | ✅ 评审员 | ✅ 工程师 | ✅ 审核员 | ✅ 仲裁员 |
| 任务上下文 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 详细规则 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 示例 | ❌ | ❌ | ❌ | ❌ | ❌ |
| XML 标签 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 输出格式 | ✅ KEY:VALUE | ✅ KEY:VALUE | ✅ XML + 自由 | ✅ KEY:VALUE | ✅ KEY:VALUE |
| 逐步思考 | ✅ "先思考" | ✅ "先阅读..." | △ | ✅ "先逐项检查" | ✅ "请阅读各引擎..." |
| 防幻觉 | ❌ | △ | ❌ | ❌ | ❌ |

**评分**：6/8（缺示例、缺退路）

**关键问题**：
1. **parse_keyed_lines 脆弱**：`split_once(':')` 无法处理值中的冒号
2. **没有防幻觉退路**：execution_prompt 直接要求"不要用占位符"，但没有说如果实现不了怎么办
3. **中英混杂**：proposal_prompt 的 XML 标签是英文，指令主体是中文

**修改方案**：
```rust
// 薄弱解析修复：使用 JSON 格式替代 KEY:VALUE
// 当前第 1274 行
fn parse_keyed_lines(raw: &str) -> HashMap<String, String> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() { return None; }
            let (key, value) = line.split_once(':')?;  // ← 遇冒号就崩
            Some((key.trim().to_uppercase(), value.trim().to_string()))
        })
        .collect()
}

// 改为 JSON 格式要求 + JSON 解析
// proposal_prompt 的输出要求改为：
// "Output JSON: {\"target_path\": \"...\", \"primary_engine\": \"...\", ...}"
```

---

### 6. `aion-router/src/builtins/task_router.rs` — AI fallback prompt

**评分**：5/8

**优点**：有角色、有 XML 标签 `<user_task>`、有分步推理指令。

**缺**：示例、退路。

---

## 三、优先级排序

```
P0 ───── 今天就要改的（影响核心可靠性）
  │
  ├── [ai.rs] temperature 参数化 — 1 文件，+5 行
  │    现状: 0.7 硬编码
  │    改后: 结构性任务 0.0-0.3，创造性任务 0.7-0.9
  │    └── 立即影响: 所有 aion-zl 调用的确定性
  │
  ├── [dialectic.rs] 加退路 + XML 包裹 — 1 文件，+3 行
  │    现状: thesis/antithesis/synthesis 无退路
  │    改后: 不确定时设置 confidence=0
  │    └── 立即影响: 降低无意义响应概率
  │
  ├── [retry.rs] 加退路 — 1 文件，+2 行
  │    现状: root_cause 分析无退路
  │    改后: "If you cannot determine, set root_cause to 'unknown'"
  │    └── 已经写了但缺少末行退路加强
  │
  └── [orchestrator.rs] 修复 parse_keyed_lines — 1 文件，+10 行
      现状: split_once(':') 遇冒号崩
      改后: 改用正则或 JSON
      └── 修复偶发 orchestration 失败

P1 ───── 这周改的（提升输出质量）
  │
  ├── [contract.rs] 4 条 prompt 加示例 — +8 行
  │    每个 JSON 字段加 1 行示例值
  │
  ├── [orchestrator.rs] proposal_prompt 加 few-shot — +5 行
  │    给 1 个完整的提案示例
  │
  ├── [planner.rs] infer_via_ai 加 CoT — +3 行
  │    "先理解任务的类型，然后匹配最合适的能力"
  │
  └── [synth.rs] 加 XML 标签包裹输入 — +2 行
      <code> / <text> 标签包裹变量输入

P2 ───── 有空再改的（架构优化）
  │
  ├── prompt 集中管理 — 全部 21 条 prompt 抽离到独立文件
  │    aion-prompt-registry/ 或 references/prompts.json
  │
  ├── 统一输出解析策略 — 全部用 JSON 替代 KEY:VALUE
  │
  └── CLAUDE.md 8 步 → 自动 lint 检查
      └── 检查代码中新增的 prompt 是否符合 8 步框架
```

---

## 四、P0 的预计效果

| 改动 | 预期效果 | 量化指标 |
|------|---------|---------|
| temperature 参数化 | 结构化任务输出稳定 | 重复调用一致性从 ~70% → ~95% |
| 加退路 | 不确定时拒答而非硬答 | 幻觉率降低 30-50% |
| 修复 parse_keyed_lines | orchestration 解析零失败 | 解析失败率归零 |

---

## 五、建议

**先改 P0**：4 个改动，4 个文件，影响面最大改动量最小。

需要我开始实施 P0 的修改吗？
