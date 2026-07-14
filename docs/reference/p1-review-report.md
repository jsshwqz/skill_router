# P1 代码审查报告

> **审查人**: 御史（Aion Forge）
> **审查日期**: 2026-07-03
> **审查范围**: `glitch-filter` crate + `aion-forge-cli` 集成

---

## 1. 总体评估

| 维度 | 状态 | 得分 |
|------|------|------|
| Sanitizer 结构体设计 | ⚠️ 部分偏差 | 3/5 |
| 关键防御点覆盖 | ❌ 不完整 | 2/5 |
| 单元测试覆盖 | ✅ 充分 | 5/5 |
| CLI 集成 | ❌ 未完成 | 0/5 |
| Panic 风险 | ✅ 安全 | 5/5 |

**综合**: ⛔ **不及格 — CLI 集成未实现，防御面不足，不可合入。**

---

## 2. Sanitizer 结构体设计评估

### 2.1 命名偏差
- **期望**: `Sanitizer` 结构体（任务规格）
- **实际**: `GlitchFilter` 结构体

评注：`GlitchFilter` 这一名称暗示其职责仅为"过滤 glitch token"，但 P1 的安全需求远超出 glitch token 范畴（NULL 截断、ANSI escape 注入、零宽字符混淆、双向文本攻击）。建议保留 `GlitchFilter` 专用于 glitch token 检测，而后单独创建 `InputSanitizer`（或 `Sanitizer`）作为综合防御层，组合内部各检测模块。

### 2.2 结构体设计

```rust
pub struct GlitchFilter {
    tokens: HashMap<String, GlitchToken>,
}
```

**优点**:
- 纯数据驱动，无外部依赖，编译快
- `HashMap` 查询 O(1)，扫描性能取决于输入长度
- `GlitchToken` 包含 category 和 behavior，元数据驱动，可扩展

**不足**:
- 仅做子串匹配（`contains`），无法处理多字节边界、零宽字符、Unicode 规范化绕过
- 未实现 `trait Sanitize` / `trait Filter` — 缺乏标准化接口
- 无 Builder 模式，不可在运行时动态添加/移除规则

### 2.3 建议

```rust
/// 综合输入安全消毒器
pub struct InputSanitizer {
    filter: GlitchFilter,
    strip_null: bool,
    strip_ansi: bool,
    strip_zero_width: bool,
    strip_bidi: bool,
    // ...
}
```

---

## 3. 关键防御点覆盖分析

| 防御点 | 是否覆盖 | 详情 |
|--------|----------|------|
| NULL (`\x00`) | ✅ 已覆盖 | `control_characters` 中包含 `\x00` |
| `\r` 洪水 | ✅ 已覆盖 | `control_characters` 中包含 `\x0d`；另有独立 `carriage_return_detection` 测试 |
| ANSI escape (`\x1b[`) | ⚠️ 部分 | 仅检测 `\x1b` 单字节，未匹配完整逃逸序列（`\x1b[...m` 等）。对于截断场景有效，但对注入型攻击可能不足 |
| **零宽字符** | ❌ 缺失 | 未覆盖以下 Unicode 零宽字符：`U+200B` (ZWSP), `U+200C` (ZWNJ), `U+200D` (ZWJ), `U+FEFF` (BOM/ZW-NBSP), `U+00AD` (Soft Hyphen), `U+2060` (Word Joiner) |
| **双向文本** | ❌ 缺失 | 未覆盖以下 Bidi 控制字符：`U+202A` (LRE), `U+202B` (RLE), `U+202D` (LRO), `U+202E` (RLO), `U+202C` (PDF), `U+2066`-`U+2069` (Bidi Isolation) |

### 零宽字符攻击向量（严重性：高）
攻击者可通过在安全提示词中插入零宽字符来绕过关键词过滤。例如 `"definitely"` 写成 `"defi\u200bnitely"` 对 LLM 可能解析为完全不同的 token。

### 双向文本攻击向量（严重性：高）
Bidi 字符可改变代码渲染顺序，例如 `"exit(); // \u202E}"` 在编辑器中可能显示为 `"exit(); // }"` 但实际逻辑被反转。

**建议**: 继承 `unicode-security` 或 `unicode-bidi` crate，或手动添加零宽字符与 Bidi 字符黑名单。

---

## 4. 单元测试评估

### 4.1 测试清单

| # | 测试名 | 覆盖点 |
|---|--------|--------|
| 1 | `filter_has_enough_tokens` | 数据库基线数量 ≥ 400 |
| 2 | `detect_centroid_cluster` | 特定 glitch token 检测 |
| 3 | `detect_control_character_null` | NULL 字节检测 |
| 4 | `behavior_filter_loop_inducer` | 行为：过滤循环诱导器 token |
| 5 | `behavior_filter_fragment` | 行为：过滤片段 token |
| 6 | `no_false_positive_on_normal_text` | 误报防护 |
| 7 | `detect_polysemantic` | 多语义 token 检测 |
| 8 | `unsolved_mystery_tokens` | 未解之谜 token 检测 |
| 9 | `deepseek_fragment_tokens` | DeepSeek 特化 token |
| 10 | `carriage_return_detection` | \r 洪水检测 |
| 11 | `category_labels_preserved` | 所有 token 有 category |
| 12 | `each_behavior_has_tokens` | 每个 behavior 至少 1 个 token |

**统计**: 12 个测试，远超出最低要求（5 个）。

### 4.2 缺口
- ❌ 无 ANSI escape 序列测试（仅测 `\x1b` 单字节）
- ❌ 无零宽字符测试
- ❌ 无双向文本测试
- ❌ 无性能/压力测试（如 10MB 输入）
- ❌ 无 Unicode 规范化绕过测试（NFC vs NFD）

### 4.3 测试质量
- 使用 `#[test]` 原生框架，无外部依赖，可直接 `cargo test`
- 每个测试有明确的 `assert!` 断言
- `no_false_positive_on_normal_text` 覆盖正常输入不误报

---

## 5. CLI 集成评估

### 5.1 当前状态

**`glitch-filter` 已加入工作区 `forge/Cargo.toml`**:
```toml
members = [
    ...
    "crates/glitch-filter",
    ...
]
```

**但 CLI 侧完全未集成**:
- `aion-forge-cli/Cargo.toml` 中 **无 `glitch-filter` 依赖**
- `aion-forge-cli/src/main.rs` 中 **无任何 `glitch_filter` 或 `sanitize` 引用**
- **无 CLI 子命令、无 flag、无 filter 调用**

### 5.2 缺失项

1. 未在 `aion-forge-cli/Cargo.toml` 中声明 `glitch-filter = { path = "../crates/glitch-filter" }`
2. 未在 CLI 中添加 `filter`/`sanitize` 子命令
3. 未在 session 流程中集成输入过滤
4. 未定义 CLI 输出格式（JSON / 纯文本 / markdown table）

---

## 6. Panic 风险评估

### 6.1 检查结果

逐行检查 `lib.rs` 和 `main.rs`：

| 风险点 | 是否存在 | 详情 |
|--------|----------|------|
| `unwrap()` | ❌ 无 | 代码中未使用任何 unwrap |
| `expect()` | ❌ 无 | 同上 |
| 数组索引 `[i]` | ❌ 无 | 只用迭代器和 HashMap |
| 整数溢出 | ❌ 无 | 仅有 `+= 1` 累加（安全） |
| `panic!()` | ❌ 无 | 未显式调用 |
| FFI / unsafe | ❌ 无 | 纯 safe Rust |

**结论**: ✅ 无 panic 风险。代码风格安全。

---

## 7. 现有逻辑破坏检查

由于 CLI 未集成 `glitch-filter`，现有 `main.rs` 中的子命令（`run`、`github`、`hook`、`evolve`、`MCP`）均未受影响。

但需注意：一旦接入，若 filter 扫描耗时过长（如 10MB 输入），可能阻塞 CLI 响应。建议：
- 设置扫描大小上限（如 64KB）
- 异步或在另一个 spawn 线程中执行

---

## 8. 整改清单

| # | 项目 | 优先级 | 说明 |
|---|------|--------|------|
| 1 | **CLI 集成** | 🔴 P0 | 在 `aion-forge-cli` 中添加 `glitch-filter` 依赖和 `sanitize` 子命令 |
| 2 | **零宽字符防御** | 🔴 P1 | 添加 U+200B/U+200C/U+200D/U+FEFF/U+2060 等检测 |
| 3 | **双向文本防御** | 🔴 P1 | 添加 U+202A-202E/U+2066-2069 等检测 |
| 4 | **ANSI escape 完整匹配** | 🟡 P2 | 从单字节 `\x1b` 扩展到完整逃逸序列模式 |
| 5 | **零宽/Bidi 测试** | 🟡 P2 | 为新增防御点添加对应单元测试 |
| 6 | **性能测试** | 🟢 P3 | 添加大数据量压力测试 |
| 7 | **Sanitizer 命名/结构** | 🟢 P3 | 考虑重命名为 `InputSanitizer` 或保持 `GlitchFilter` 并独立新建 |
| 8 | **Builder / 可配置** | 🟢 P3 | 支持运行时动态添加规则 |

---

## 9. 结论

**工部 P1 代码当前状态**: 核心库 `glitch-filter` 已完成基础框架、glitch token 数据库和 12 个单元测试，代码质量良好且无 panic 风险。但存在以下关键缺口：

1. **CLI 集成完全缺失** — 用户无法在命令行使用 sanitizer
2. **零宽字符与双向文本攻击面未覆盖** — 两个高严重性安全缺口
3. **ANSI escape 检测不完整** — 仅匹配单字节而非完整序列

**裁决**: ⛔ **不通过**。工部必须完成 CLI 集成和零宽/Bidi 防御后，方可重新送审。

---

*报告结束 — 已归档至 `forge/docs/reference/p1-review-report.md`*
