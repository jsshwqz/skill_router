# aion-forge 可观测性 + 推理质量增强 — 6 项功能规划

> 本规划文档综合参考 spec_driven_develop (S.U.P.E.R / Adaptive Control) 和 1flowbase (Token Tracking / Model Cascade / Output Verification)
> 适用执行者：aion-forge 编排器（ai_parallel_solve / ai_smart_collaborate）或人工 PR
> 每次改动前先读 `CLAUDE.md` 第五节的 8 步框架
> 每个功能以独立 PR 方式实现，PR 标题格式：`feat/feat-aion-forge: <功能名> — <简要描述>`

---

## PR 实施顺序

```
PR-1 Token Tracking (Token 追踪)
    └── 基础层，后续功能依赖
PR-6 S.U.P.E.R. (架构规范)
    └── 哲学层，可并行
PR-4 Output Verification (输出校验)
    └── 基于已有 Verifier trait
PR-2 Model Cascade (模型级联)
    └── 依赖 Token Tracking
PR-3 Adaptive Control (自适应控制)
    └── 依赖 Output Verification + Model Cascade
PR-5 Deep Discuss (深度讨论)
    └── 独立功能，可与上述并行
```

---

## PR-1: Token Tracking — AI 调用 Token 追踪与统计

> 提取自 1flowbase 的 Token 追踪能力
> 当前状态：aion-forge 完全无 token 统计，AI API 返回的 usage 字段被丢弃
> 依赖：无（纯新增）

### 涉及文件

| 文件 | 操作 | 改动量 |
|------|------|--------|
| `aion-types/src/types.rs` | 新增 `TokenUsage` struct | +10 行 |
| `aion-types/src/ai_native.rs` | PayloadMeta 加 TokenUsage 字段 | +2 行 |
| `aion-router/src/builtins/ai.rs` | 解析 OpenAI/Anthropic 响应的 usage 字段 | +20 行 |
| `aion-router/src/metrics.rs` | 新增 token 计数器指标 | +20 行 |
| `aion-router/src/learner.rs` | SkillStats 聚合 token 数据 | +10 行 |
| `aion-router/src/executor.rs` | 传递 TokenUsage 到 metrics/learner | +5 行 |
| `aion-router/src/lib.rs` | route() 返回带 token 数据 | +3 行 |

### 具体改动

**1. `aion-types/src/types.rs` — 新增 TokenUsage**

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cached_tokens: u32,  // Anthropic 独有
}

// 现有 ExecutionResponse 加字段
pub struct ExecutionResponse {
    pub skill_name: String,
    pub capability: String,
    // ... existing fields ...
    pub token_usage: Option<TokenUsage>,  // 新增
}
```

**2. `aion-router/src/builtins/ai.rs` — 解析 usage 字段**

在 `parse_openai_response()` 中：
```rust
// 当前: 只解析 content
// 改为:
let usage: Option<TokenUsage> = response.get("usage").and_then(|u| {
    Some(TokenUsage {
        prompt_tokens: u.get("prompt_tokens")?.as_u32()?,
        completion_tokens: u.get("completion_tokens")?.as_u32()?,
        total_tokens: u.get("total_tokens")?.as_u32()?,
        cached_tokens: u.get("cached_tokens").and_then(|c| c.as_u32()).unwrap_or(0),
    })
}).or_else(|| {
    // 降级: 用估计算法
    let content_len = content.len();
    Some(TokenUsage {
        prompt_tokens: estimate_tokens(prompt.as_ref().unwrap_or(&String::new())),
        completion_tokens: estimate_tokens(&content),
        total_tokens: 0,
        cached_tokens: 0,
    })
});
```

同理在 `parse_anthropic_response()` 中解析 `usage.input_tokens` 和 `usage.output_tokens`。

**3. `aion-router/src/metrics.rs` — 新增 token 计数器**

```rust
pub fn record_token_usage(skill_name: &str, capability: &str, provider: &str, tokens: &TokenUsage) {
    metrics::counter!(
        "skill_ai_prompt_tokens_total",
        "skill" => skill_name.to_string(),
        "capability" => capability.to_string(),
        "provider" => provider.to_string()
    ).increment(tokens.prompt_tokens as u64);

    metrics::counter!(
        "skill_ai_completion_tokens_total",
        "skill" => skill_name.to_string(),
        "capability" => capability.to_string(),
        "provider" => provider.to_string()
    ).increment(tokens.completion_tokens as u64);
}
```

**4. `aion-router/src/learner.rs` — 聚合 token 数据**

```rust
pub struct SkillStats {
    // existing fields ...
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub avg_tokens_per_call: f64,
}

impl SkillLearner {
    pub fn record_execution(&mut self, ... , token_usage: Option<&TokenUsage>) {
        // ... existing logic ...
        if let Some(tokens) = token_usage {
            stats.total_prompt_tokens += tokens.prompt_tokens as u64;
            stats.total_completion_tokens += tokens.completion_tokens as u64;
            stats.avg_tokens_per_call = (stats.total_prompt_tokens + stats.total_completion_tokens) as f64 / stats.execution_count as f64;
        }
    }
}
```

### 验证方法

1. `cargo build` — 编译通过
2. `cargo test` — 全部通过
3. 运行 `aion-cli text_summarize "Rust is a language"` 后检查 metrics 是否记录 token
4. 运行 `aion-server` 后检查 Prometheus `/v1/metrics` 端点

---

## PR-6: S.U.P.E.R. 架构规范 — 代码质量自检体系

> 提取自 spec_driven_develop 的 S.U.P.E.R 架构哲学
> 当前状态：aion-forge 无明确的架构质量评估体系
> 依赖：无（纯文档 + 轻量代码）

### 涉及文件

| 文件 | 操作 | 改动量 |
|------|------|--------|
| `CLAUDE.md` | 新增 S.U.P.E.R 章节 | +30 行 |
| `aion-router/src/builtins/prompt_audit.rs` | 新增 SuperComplianceCheck | +80 行 |
| `aion-router/src/builtins/mod.rs` | 注册 super_audit builtin | +3 行 |
| `aion-types/src/types.rs` | 新增 SuperPhase enum | +10 行 |

### 具体改动

**1. `CLAUDE.md` — 新增 S.U.P.E.R 章节**

```markdown
### 2.8 架构规范 — S.U.P.E.R

写代码像搭积木：每个模块一个职责、标准接口、单向数据流、环境无关、随时可替换。

每个 AI Agent 在新增模块或重构现有模块时，必须逐项自检：

| 原则 | 英文 | 自检问题 |
|------|------|---------|
| **S** 单一职责 | Single Purpose | 这个模块能用一句话说明它的职责吗？ |
| **U** 单向数据流 | Unidirectional Flow | 数据是否只向一个方向流动？是否存在循环依赖？ |
| **P** 接口优先 | Ports over Implementation | 模块的输入输出是否有明确的 schema 定义？能否通过 JSON 序列化？ |
| **E** 环境无关 | Environment-Agnostic | 配置是否从环境变量或配置文件注入？有无硬编码路径或密钥？ |
| **R** 可替换性 | Replaceable Parts | 替换这个模块会不会引发其他模块的连锁修改？ |

**判分规则：** 5 项全 Yes → 通过。1-2 项 No → 必须修复后再提交。3+ 项 No → 禁止提交，先重构。

**常见反模式（禁止）：**
- 一个文件既做数据获取、又做计算、又做渲染、又做通知（违反 S）
- 内层模块反向依赖外层模块，出现循环引用（违反 U）
- 模块间通过"猜格式"通信，没有 schema 定义（违反 P）
- API Key、URL、文件路径写死在代码里（违反 E）
- 改一个模块需要同时修改其他 3 个模块才能编译通过（违反 R）
```

**2. `aion-router/src/builtins/prompt_audit.rs` — 新增 SuperComplianceCheck**

```rust
// 在 prompt_audit.rs 中新增:

/// S.U.P.E.R. 合规性检查
pub struct SuperComplianceCheck;

impl BuiltinSkill for SuperComplianceCheck {
    fn name(&self) -> &str { "super_compliance_check" }
    fn capabilities(&self) -> Vec<&str> { vec!["super_audit"] }

    async fn execute(&self, ctx: &ExecutionContext) -> SkillExecutionResult {
        let code = ctx.input.get("code").as_str().unwrap_or("");
        let files = ctx.input.get("files").as_array().unwrap_or(&vec![]);

        let mut scores = HashMap::new();
        // S: 检查单一职责
        scores.insert("S", self.check_single_purpose(code, files));
        // U: 检查单向数据流
        scores.insert("U", self.check_unidirectional_flow(code, files));
        // P: 检查接口优先
        scores.insert("P", self.check_ports_over_impl(code, files));
        // E: 检查环境无关
        scores.insert("E", self.check_env_agnostic(code, files));
        // R: 检查可替换性
        scores.insert("R", self.check_replaceable(code, files));

        let total = scores.values().sum::<f64>();
        let avg = total / scores.len() as f64;

        SkillExecutionResult {
            output: json!({
                "super_scores": scores,
                "average": avg,
                "pass": avg >= 0.8,
                "suggestions": self.generate_suggestions(&scores),
            }),
            // ... other fields ...
        }
    }
}
```

**3. `aion-router/src/builtins/mod.rs`**

```rust
reg.register(Box::new(super_compliance_check::SuperComplianceCheck));
```

### 验证方法

1. `cargo build` — 编译通过
2. 运行 `aion-cli super_compliance_check --code <rust_code>` 检查输出格式
3. 用 S.U.P.E.R 反模式代码测试是否能检测出违规

---

## PR-4: Output Verification — JSON Schema 输出校验

> 提取自 1flowbase 的 Output Verification 能力
> 当前状态：已有 `automation/verifier.rs` 的 `CargoCheckVerifier`，无 JSON schema 校验
> 依赖：无（基于已有 Verifier trait）

### 涉及文件

| 文件 | 操作 | 改动量 |
|------|------|--------|
| `aion-router/src/automation/verifier.rs` | 新增 `JsonSchemaVerifier` | +60 行 |
| `aion-types/src/verification.rs` | 新建类型定义 | +30 行 |
| `aion-types/src/lib.rs` | 加 `pub mod verification;` | +1 行 |
| `aion-router/src/builtins/format.rs` | 加 schema 校验调用 | +15 行 |
| `aion-router/Cargo.toml` | 加 `jsonschema = "0.18"` | +1 行 |
| `Cargo.toml` | 可选：加 workspace dep | +1 行 |

### 具体改动

**1. `aion-types/src/verification.rs` — 新建**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVerificationConfig {
    pub schema: serde_json::Value,
    pub strict: bool,
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDetail {
    pub field_path: String,
    pub expected: String,
    pub actual: String,
    pub error: String,
}
```

**2. `aion-router/src/automation/verifier.rs` — 新增 JsonSchemaVerifier**

```rust
pub struct JsonSchemaVerifier {
    config: SchemaVerificationConfig,
}

impl JsonSchemaVerifier {
    pub fn new(config: SchemaVerificationConfig) -> Self {
        Self { config }
    }
}

impl Verifier for JsonSchemaVerifier {
    fn name(&self) -> &str { "json_schema" }

    async fn verify(&self, output: &str) -> VerificationReport {
        let data: serde_json::Value = match serde_json::from_str(output) {
            Ok(v) => v,
            Err(e) => return VerificationReport {
                success: false,
                details: vec![VerificationDetail {
                    field_path: "$".to_string(),
                    expected: "valid JSON".to_string(),
                    actual: format!("parse error: {e}"),
                    error: "JSON_PARSE_ERROR".to_string(),
                }],
            },
        };

        let schema = jsonschema::JSONSchema::from_value(self.config.schema.clone()).unwrap();
        let result = schema.validate(&data);

        if result.is_ok() {
            // 可选: strict 模式检查额外字段
            if self.config.strict {
                // 检查是否有 schema 未定义的字段
            }
            VerificationReport { success: true, details: vec![] }
        } else {
            let details: Vec<VerificationDetail> = result.unwrap_err()
                .into_iter()
                .map(|e| VerificationDetail {
                    field_path: e.instance_path.to_string(),
                    expected: e.expected.unwrap_or_default(),
                    actual: e.got.clone(),
                    error: e.message.clone(),
                })
                .collect();
            VerificationReport { success: false, details }
        }
    }
}

// 更新 resolution 函数
pub fn resolve_verifier(kind: &str, config: &Value) -> Option<Box<dyn Verifier>> {
    match kind {
        "cargo_check" => Some(Box::new(CargoCheckVerifier::new())),
        "json_schema" | "output_format" => {
            if let Ok(cfg) = serde_json::from_value(config.clone()) {
                Some(Box::new(JsonSchemaVerifier::new(cfg)))
            } else {
                None
            }
        }
        _ => None,
    }
}
```

### 验证方法

1. `cargo build` — 编译通过
2. `cargo test` — 全部通过
3. 用 `json_schema` verifier 类型运行 automation step，验证 schema 校验结果

---

## PR-2: Model Cascade — 模型级联策略

> 提取自 1flowbase 的 Model Cascading 能力
> 当前状态：所有 AI 调用走固定的 endpoint fallback 链，无模型分层
> 依赖：PR-1 (Token Tracking) — 用于测量级联后的成本节省

### 涉及文件

| 文件 | 操作 | 改动量 |
|------|------|--------|
| `aion-router/src/config.rs` | 新增 `ModelTier` 分类 | +20 行 |
| `aion-router/src/builtins/task_router.rs` | 新增复杂度评估 + tier 推荐 | +40 行 |
| `aion-router/src/builtins/ai.rs` | 新增 tier 过滤 + 级联升级 | +30 行 |
| `aion-types/src/route_types.rs` | 加 `model_tier` 字段 | +3 行 |
| `router.json` | 加 tier 配置 | +5 行 |
| `aion-router/src/cascade.rs` | 新建级联编排模块（可选） | +50 行 |

### 具体改动

**1. `aion-types/src/route_types.rs`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelTier {
    FastCheap,      // 简单任务：分类、提取、格式化
    Balanced,       // 默认：通用推理
    Reasoning,      // 复杂任务：代码生成、深度分析、多步推理
}

pub struct RouteDecision {
    // existing fields ...
    pub model_tier: Option<ModelTier>,  // 新增
}
```

**2. `aion-router/src/config.rs`**

```rust
// AiEndpoint 加字段
pub struct AiEndpoint {
    // existing fields ...
    pub tier: ModelTier,  // 新增
}

// 现有 endpoint 分类建议:
// FastCheap:   opencode-zen, ollama-local (若配置)
// Balanced:    primary, openai-compatible
// Reasoning:   host-anthropic-proxy, openrouter
```

**3. `aion-router/src/builtins/task_router.rs` — 复杂度评估**

```rust
impl RouteTaskBuiltin {
    fn estimate_complexity(&self, task: &str, capability: &str) -> ModelTier {
        let lower = task.to_lowercase();
        let is_simple = matches!(capability, "classify" | "extract" | "format" | "summarize")
            || lower.contains("简单")
            || lower.contains("简短");
        let is_reasoning = matches!(capability, "code_generate" | "code_lint" | "analyze")
            || lower.contains("分析")
            || lower.contains("为什么")
            || lower.contains("详细");

        if is_simple { ModelTier::FastCheap }
        else if is_reasoning { ModelTier::Reasoning }
        else { ModelTier::Balanced }
    }
}
```

**4. `aion-router/src/builtins/ai.rs` — tier 过滤 + 级联**

```rust
// execute() 中加 tier 过滤逻辑
fn filter_endpoints_by_tier(endpoints: &[AiEndpoint], requested_tier: ModelTier) -> Vec<&AiEndpoint> {
    // 先找匹配 tier 的
    let matching: Vec<&AiEndpoint> = endpoints.iter()
        .filter(|e| e.tier == requested_tier)
        .collect();
    if !matching.is_empty() { return matching; }
    // 降级: 找下一个可用的
    match requested_tier {
        ModelTier::FastCheap => {
            let fallback: Vec<&AiEndpoint> = endpoints.iter()
                .filter(|e| e.tier == ModelTier::Balanced)
                .collect();
            if !fallback.is_empty() { return fallback; }
        }
        ModelTier::Balanced => {
            let fallback: Vec<&AiEndpoint> = endpoints.iter()
                .filter(|e| e.tier == ModelTier::Reasoning)
                .collect();
            if !fallback.is_empty() { return fallback; }
        }
        ModelTier::Reasoning => {}  // 已是最高级
    }
    // 全部 fallback
    endpoints.iter().collect()
}
```

### 验证方法

1. `cargo build` — 编译通过
2. 运行 `aion-cli code_generate "写一个 hello world"` → 应该走 Reasoning 模型
3. 运行 `aion-cli text_classify "这是一段文字"` → 应该走 FastCheap 模型
4. 检查 metrics 中是否记录了每个 tier 的调用量

---

## PR-3: Adaptive Control — 自适应控制与漂移检测

> 提取自 spec_driven_develop 的 Adaptive Control 反馈环
> 当前状态：SkillLearner 有 success/failure 统计，无 drift 检测
> 依赖：PR-1 (Token Tracking), PR-4 (Output Verification)

### 涉及文件

| 文件 | 操作 | 改动量 |
|------|------|--------|
| `aion-router/src/learner.rs` | 新增 `DriftRecord` + `detect_drift()` | +50 行 |
| `aion-router/src/executor.rs` | 执行后调用 drift check | +10 行 |
| `aion-types/src/types.rs` | 加 `drift_score` 字段 | +5 行 |
| `aion-router/src/metrics.rs` | 加 `skill_drift_score` gauge | +5 行 |
| `aion-router/src/builtins/zl.rs` | 增强 ZLDetectDrift | +20 行 |

### 具体改动

**1. `aion-router/src/learner.rs` — 漂移检测**

```rust
#[derive(Debug, Clone, Default)]
pub struct DriftRecord {
    pub skill_name: String,
    pub baseline_latency_ms: f64,
    pub baseline_tokens: f64,
    pub actual_latency_ms: f64,
    pub actual_tokens: f64,
    pub actual_quality: f64,  // 0.0-1.0
    pub drift_score: f64,
    pub timestamp: u128,
}

impl SkillLearner {
    /// 检测技能执行漂移
    pub fn detect_drift(&mut self, skill_name: &str, actual: &DriftRecord) {
        let stats = self.stats.get_mut(skill_name).unwrap();
        
        let latency_drift = if stats.execution_count > 10 {
            let baseline = stats.avg_latency_ms;
            (actual.actual_latency_ms - baseline).abs() / baseline.max(1.0)
        } else { 0.0 };

        let quality_drift = if stats.execution_count > 10 {
            let baseline = stats.avg_quality_score;
            (baseline - actual.actual_quality).abs()
        } else { 0.0 };

        let token_drift = if stats.execution_count > 10 {
            let baseline = stats.avg_tokens_per_call;
            (actual.actual_tokens - baseline).abs() / baseline.max(1.0)
        } else { 0.0 };

        actual.drift_score = (latency_drift + quality_drift + token_drift) / 3.0;

        // 超过阈值触发
        if actual.drift_score > 0.3 {
            tracing::warn!(
                "Drift detected for {}: score={:.2} (latency={:.2}, quality={:.2}, tokens={:.2})",
                skill_name, actual.drift_score, latency_drift, quality_drift, token_drift
            );
            // 可选: 触发 auto-evolve 或 alert
        }

        self.drift_records.push(actual.clone());
    }
}
```

**2. `aion-router/src/executor.rs` — 执行后漂移检测**

```rust
// execute_builtin() 结束后:
let drift_record = DriftRecord {
    skill_name: skill_name.clone(),
    baseline_latency_ms: 0.0,  // 由 learner 填充
    baseline_tokens: 0.0,
    actual_latency_ms: execution_duration.as_millis() as f64,
    actual_tokens: token_usage.map(|t| t.total_tokens as f64).unwrap_or(0.0),
    actual_quality: quality_score.unwrap_or(0.5),
    drift_score: 0.0,
    timestamp: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis(),
};
learner.detect_drift(&skill_name, &drift_record);
```

### 验证方法

1. `cargo build` — 编译通过
2. 运行同一技能 10+ 次，检查 drift score 是否正确计算
3. 人工制造 drift（如用不同模型），检查是否触发 warning

---

## PR-5: Deep Discuss — 深度讨论独立 Skill

> 提取自 spec_driven_develop 的 Deep Discuss 7 阶段流程
> 当前状态：`orchestrator.rs` 已有 `Discuss` builtin，但简单（无 turn 管理、无共识检测）
> 依赖：无（独立功能）

### 涉及文件

| 文件 | 操作 | 改动量 |
|------|------|--------|
| `aion-router/src/builtins/orchestrator.rs` | 增强 `Discuss` 结构 | +100 行 |
| `aion-router/src/builtins/deep_discuss.rs` | 新建独立模块（可选，如果增强逻辑太大） | +80 行 |
| `aion-router/src/builtins/mod.rs` | 注册 deep_discuss | +3 行 |
| `router.json` | 加 deep_discuss 路由规则 | +3 行 |

### 具体改动

**增强 `Discuss` 的 execute() 方法：**

```rust
// 配置
struct DiscussionConfig {
    max_turns: u32,              // 默认 3
    moderator_model: String,     // 默认 host-anthropic-proxy
    participant_models: Vec<String>,  // 分支模型列表
    consensus_threshold: f64,    // 0.0-1.0, 默认 0.67
    format: DiscussionFormat,    // structured / freeform / debate
}

enum DiscussionFormat {
    Structured,   // 7 阶段: Receive → Audit → Analysis → Design → Review → Final → Execute
    Freeform,
    Debate,       // pro/con 角色分配
}

// TurnManager: 管理讨论轮次
struct TurnManager {
    current_turn: u32,
    max_turns: u32,
    participants: Vec<(String, String)>,  // (engine, response)
}

// ConsensusDetector: 分析共识度
struct ConsensusDetector;
impl ConsensusDetector {
    fn detect(&self, responses: &[String], threshold: f64) -> ConsensusResult {
        let keyword_overlap = self.compute_keyword_overlap(responses);
        let semantic_similarity = self.compute_semantic_similarity(responses);
        let consensus_score = (keyword_overlap + semantic_similarity) / 2.0;

        ConsensusResult {
            score: consensus_score,
            consensus_reached: consensus_score >= threshold,
            final_summary: if consensus_score >= threshold {
                self.synthesize_conclusion(responses)
            } else {
                String::from("未达到共识，继续讨论")
            },
        }
    }
}
```

### 验证方法

1. `cargo build` — 编译通过
2. 运行 `aion-cli deep_discuss --topic "讨论 Rust vs Go 的适用场景"` 
3. 检查输出是否包含 7 阶段讨论过程 + 共识度 + 最终结论

---

## 综合实施建议

### 优先级与难度

| PR | 功能 | 难度 | 预计耗时 | 风险 |
|----|------|------|---------|------|
| PR-1 | Token Tracking | ⭐⭐ | 30min | 低（纯新增） |
| PR-6 | S.U.P.E.R. | ⭐ | 20min | 极低（文档+轻代码） |
| PR-4 | Output Verification | ⭐⭐⭐ | 1h | 中（新 crate 依赖） |
| PR-2 | Model Cascade | ⭐⭐⭐ | 1.5h | 中（配置联动） |
| PR-3 | Adaptive Control | ⭐⭐⭐ | 1h | 中（依赖 PR-1/PR-4） |
| PR-5 | Deep Discuss | ⭐⭐⭐ | 1.5h | 低（独立功能） |

### 建议 PR 执行顺序

1. **PR-6 (S.U.P.E.R.)** → 最低成本，建立架构规范
2. **PR-1 (Token Tracking)** → 基础设施，后续依赖
3. **PR-4 (Output Verification)** → 基于已有 trait，可独立
4. **PR-2 (Model Cascade)** → 依赖 Token Tracking
5. **PR-3 (Adaptive Control)** → 依赖 Token Tracking + Output Verification
6. **PR-5 (Deep Discuss)** → 与上述并行，互不阻塞

### 每个 PR 的 PR 模板

```markdown
## feat/feat-aion-forge: Token Tracking

### 改动范围
- 新增 TokenUsage struct
- 解析 OpenAI/Anthropic 响应的 usage 字段
- 新增 Prometheus token 指标
- learner.rs 聚合 token 数据

### 影响范围
- aion-types: +TokenUsage struct
- aion-router: ai.rs, metrics.rs, learner.rs, executor.rs, lib.rs
- 无 breaking change

### 测试
- cargo build ✓
- cargo test ✓
- 运行 skills 后检查 /v1/metrics

### S.U.P.E.R 自检
- [ ] S: TokenUsage 单一职责
- [ ] U: 数据流单向（AI → TokenUsage → metrics → learner）
- [ ] P: TokenUsage 有明确的 JSON 序列化定义
- [ ] E: 无硬编码配置
- [ ] R: TokenUsage 可独立替换（不影响其他模块）
```

---

## 实施注意事项

1. **新增依赖**（PR-4 需要 `jsonschema = "0.18"`）需要在 `Cargo.toml` 中评估版本兼容性
2. **向后兼容**：所有新增字段加 `#[serde(default)]`，避免破坏已有消费者
3. **CLAUDE.md 同步**：PR-6 完成后更新 CLAUDE.md，后续 PR 在 Step 5 验证中加入 S.U.P.E.R 自检
4. **每个 PR 自包含**：每个 PR 的 `cargo build` 和 `cargo test` 必须独立通过