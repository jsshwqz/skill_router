# AionForge Harness Engineering 升级规划

> 基于三引擎协作自审结果（2026-04-05），借鉴 [dialectical-vibe-coding](https://github.com/whatwoods/dialectical-vibe-coding) 方法论

---

## 一、当前 Harness 评分：6.5~6.8 / 10

| 维度 | 评分 | 说明 |
|------|------|------|
| Model 侧 | 8/10 | 3 引擎 + 多 workflow，能力面够宽 |
| Guide 侧 | 6.5/10 | 有 route_task、risk_level，但偏一次性决策，缺动态重规划 |
| Sensor 侧 | 6/10 | 有安全、健康、分歧检测，缺前置充分性和结果验收 |
| 闭环性 | 5.5/10 | "执行前检查 + 执行后少量兜底"，非持续修正 |
| 风险控制 | 6/10 | 能防明显危险，不够防回退到默认方案/输出泛化/执行跑偏 |

**核心问题**：当前是"静态编排器"，不是完整的闭环 Harness。

---

## 二、已有 Harness 机制清单

### Guide（前馈）
| 机制 | 位置 | 状态 |
|------|------|------|
| 任务路由 route_task | aion-router | ✅ |
| 风险分级 risk_level | orchestrator | ✅ |
| 执行策略选择 | smart_collaborate/triple_vote | ✅ |

### Sensor（反馈）
| 机制 | 位置 | 状态 |
|------|------|------|
| 安全审查（pre-execution） | aion-intel/immunity.rs | ✅ |
| 引擎健康检测（cooldown） | aion-router | ✅ |
| 分歧检测（dispute_review） | orchestrator | ✅ |

---

## 三、需要新增的 Sensor（按优先级）

### P0: Context Sufficiency Sensor（上下文充分性检测）

**作用**：执行前判断"是否理解够了"，防止回退到模板化输出。

**实现方案**：
- 输入：用户任务、TARGET_PATH、已读取文件列表、未解决问题列表
- 检查规则：
  - 任务要求"基于当前架构分析"但没读到相关实现 → `insufficient_context=true`
  - 存在高影响未决项（路径不确定、接口未知）→ 输出 gap 列表
- 输出：`score: 0~1`, `missing_context: []`, `blocking: bool`
- 触发：`score < 0.72` → 阻断执行，先走 context_expand 或 repo_scan

**建议文件**：`aion-intel/src/context_sensor.rs`

### P1: Result Contract Sensor（结果契约验证）

**作用**：执行后校验"输出是否满足任务契约"。

**实现方案**：
- 由 TaskContract Guide 生成契约（目标类型、必含部分、必引实体）
- 对最终输出做规则化校验（完整性、结构、实体命中率）
- 输出：`contract_score`, `missing_requirements`, `retry_hint`
- 触发：`contract_score < 0.8` → 自动进入 review_and_revise

**建议文件**：`aion-intel/src/result_sensor.rs`

### P1: Execution Drift Sensor（执行偏航检测）

**作用**：监控执行过程是否偏离目标。

**实现方案**：
- 检查规则：
  - 阶段跑偏：用户要求执行但系统仍在讨论
  - 意图跑偏：输出与原始目标语义偏离
  - 泛化漂移：高频模板句 + 低频任务实体
- 输出：`drift_type`, `severity`, `correction_action`
- 触发：轻度 → 局部重试；重度 → 回退到 adaptive_replan

**建议文件**：`aion-intel/src/drift_sensor.rs`

---

## 四、需要新增的 Guide 进化机制

### Guide 1: Task Contract Guide（任务契约编译器）

把用户任务编译成结构化契约，作为全流程的"北极星"：
- 目标类型（analysis / code / review）
- 必须包含的结构
- 必须引用的实体
- 是否需要文件级建议
- 验证方式

**建议文件**：`aion-intel/src/task_contract.rs`

### Guide 2: Adaptive Replanning Guide（自适应重规划）

从静态 risk_level 升级为动态决策：
- 输入：context_score + contract_score + engine_health + dispute_level
- 决策：是否切换 workflow / 改变引擎权重 / 先补上下文 / 转 review_and_revise
- 核心：每个阶段根据 Sensor 再决策一次，而非开始时选一次

**建议文件**：`aion-router/src/adaptive_planner.rs`

---

## 五、代码改动路线图

### 第一阶段（基础闭环）
1. `aion-intel/src/task_contract.rs` — TaskContract 结构体 + 编译逻辑
2. `aion-intel/src/context_sensor.rs` — 上下文充分性检测
3. `aion-intel/src/result_sensor.rs` — 结果契约验证
4. orchestrator 入口加 Sensor 调用链

### 第二阶段（动态重规划）
5. `aion-router/src/adaptive_planner.rs` — 自适应重规划
6. `aion-intel/src/drift_sensor.rs` — 执行偏航检测
7. 引擎 runner 加 stage_trace / tool_trace

### 第三阶段（经验沉淀）
8. 执行历史 → 引擎偏好学习（这类任务 gemini 比 openai 好）
9. 失败模式库 → 自动规避已知陷阱

---

## 六、预期效果

| 指标 | 当前 | 改进后 |
|------|------|--------|
| Harness 评分 | 6.5~6.8 | 8.2+ |
| 回退到默认方案 | 常见 | 上下文不足时自动阻断 |
| 输出泛化/模板化 | 偶发 | 契约验证自动拦截 |
| 执行跑偏 | 人工发现 | 系统自动检测 + 纠偏 |
| 引擎选择准确率 | 静态规则 | 基于历史成功率动态优化 |

---

*由 AionForge 三引擎协作自审生成，Claude Opus 4.6 整理*
*2026-04-05*
