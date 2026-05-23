# Prompt 工程质量提升 — P1/P2 实施规划

> 本规划文档由 aion-prompt 技能审计后产出
> 适用执行者：aion-forge 编排器（ai_parallel_solve / ai_smart_collaborate）
> 每次改动前先读 `CLAUDE.md` 第五节的 8 步框架

---

## P1 实施项（提升输出质量）

### P1-1: contract.rs 4 条 prompt 加少样本示例

**文件**: `aion-zl/src/contract.rs`
**改动量**: ~+8 行
**风险**: 低（纯 prompt 文本，不影响编译）

#### 具体改动

在 `CONTRACT_SYSTEM` 的 Output JSON 部分后，增加：

```rust
Example:
Input: "写一个 Python 函数计算斐波那契数列"
Output: {
  "task_summary": "实现斐波那契数列函数",
  "acceptance_criteria": ["函数能正确计算第 n 项", "处理 n=0 边界情况"],
  "expected_outputs": [{"type": "code", "description": "Python 函数源码"}],
  "required_context": ["编程语言: Python"],
  "verification_method": "运行测试用例验证结果",
  "complexity": "low",
  "estimated_steps": 1
}
```

同样的格式在 `SUFFICIENCY_SYSTEM`、`VERIFY_SYSTEM`、`DRIFT_SYSTEM` 各自的 JSON schema 描述后各加一个示例。

**注意**：示例值要用实际有意义的字段值，不要用"..."占位符。示例放 JSON schema 下方，用空行隔开。

---

### P1-2: orchestrator.rs proposal_prompt 加少样本示例

**文件**: `aion-router/src/builtins/orchestrator.rs`
**改动量**: ~+8 行
**风险**: 低

#### 具体改动

在 `proposal_prompt()` 函数的 output field 说明之后，加一个完整示例：

```rust
Example:
TARGET_PATH: code_refactor
PRIMARY_ENGINE: claude
REVIEW_ENGINES: openai | gemini
EXECUTION_MODE: primary_plus_review
KEY_RISKS: regression | edge_cases
EXECUTION_ORDER: analyze > execute > review
VERIFY: run tests and compare output
SUMMARY: refactor with multi-engine review
```

---

### P1-3: planner.rs infer_via_ai 加 CoT

**文件**: `aion-intel/src/planner.rs`
**改动量**: ~+3 行
**风险**: 低

#### 具体改动

在 `infer_via_ai()` 的 system prompt 中，capabilities 列表之后加：

```rust
"First, understand the user's core intent. Then match it to the most relevant capability. Return ONLY the capability name."
```

---

### P1-4: synth.rs 加 XML 标签包裹输入

**文件**: `aion-intel/src/synth.rs`
**改动量**: ~+2 行
**风险**: 低

#### 具体改动

在 `ai_instruction_for()` 调用处或每个模板的输入说明中：

```rust
// 当前: "你是一个 Rust 代码生成器..."
// 改为: "你是一个 Rust 代码生成器。输入放在 <code> 标签内。根据需求生成..."

// 当前: code_generate 的 instruction:
("code_generate", "你是一个 Rust 代码生成器。根据需求生成完整可编译的 Rust 代码。...")
// 改为:
("code_generate", "你是一个 Rust 代码生成器。根据 <requirements> 标签内的需求生成完整可编译的 Rust 代码。只返回代码本身，不要额外说明。...")
```

---

## P2 实施项（架构优化）

### P2-1: prompt 集中管理

**文件**: 新建 `aion-prompt-registry/`
**改动量**: 新建目录 + 提取所有 prompt 到独立文件
**风险**: 中（涉及多处 import 改动）

#### 方案

```rust
// 当前: prompt 字符串硬编码在 Rust 源码中
// 改: 移到独立 JSON/YAML 文件，运行时加载

// aion-prompt-registry/contracts.json
{
  "compile_contract": {
    "role": "task contract compiler",
    "system": "...",
    "temperature": 0.2,
    "examples": [...]
  }
}
```

### P2-2: 统一输出解析 — KEY:VALUE → JSON

**文件**: `orchestrator.rs`（proposal_prompt / dispute_review_prompt / review_execution_prompt / arbitration_prompt）
**改动量**: 修改 4 处 prompt 的输出格式要求 + 输出解析逻辑
**风险**: 中（需要同步修改解析函数 `parse_proposal_output` 和 `parse_review_output`）

#### 方案

```
// 当前 prompt 要求:
"只输出以下字段，每行一项：TARGET_PATH: xxx"

// 改为:
"Output valid JSON only:
{
  \"target_path\": \"code_refactor\",
  \"primary_engine\": \"claude\",
  ...
}"
```

然后直接用 `serde_json::from_str` 解析，替代 `parse_keyed_lines`。

---

## 实施建议

| 项 | 难度 | 推荐执行者 | 预计耗时 |
|----|------|-----------|---------|
| P1-1 | ⭐ | 编排器 | 15min |
| P1-2 | ⭐ | 编排器 | 10min |
| P1-3 | ⭐ | 编排器 | 5min |
| P1-4 | ⭐ | 编排器 | 5min |
| P2-1 | ⭐⭐⭐ | 人工 | 1h |
| P2-2 | ⭐⭐⭐ | 人工 | 30min |

**建议顺序**：P1-3 → P1-4 → P1-2 → P1-1 → P2-2 → P2-1

---

## 验证方法

每个改动后执行：

1. `cargo build` — 必须编译通过
2. `cargo test` — 必须全部通过
3. 检查 prompt 内容是否按 8 步框架执行
