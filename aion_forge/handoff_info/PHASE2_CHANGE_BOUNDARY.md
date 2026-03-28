# PHASE2_CHANGE_BOUNDARY - Phase 2 开发边界说明

为了在引入 Phase 2 新能力的同时，确保 Phase 1.x 自动化内核的稳定性，所有开发活动必须遵循以下边界：

## 1. 允许新增的内容 (Active Zones)
- **新模块**：可以在 `src/automation/` 下新增模块文件（如已新增的 `discovery.rs`）。
- **新 Executor/Verifier**：鼓励通过实现 Trait 的方式增加新的物理能力描述。
- **扩展 Payload**：在 `AiNativePayload` 中增加非破坏性的新字段。

## 2. 核心源码的修改准则 (Core Modification)
- **增量扩展**：对于 `loop_engine.rs` 等核心文件，应以“增加 Match 分支”或“注入新 Hook”为主，避免重构既有的状态机流向。
- **协议兼容 (包含 Event Stream)**：对 `AutomationState` 的修改必须维持严格的向下兼容（例如新增的 `event_stream` 使用了 `#[serde(default)]`）。任何清理废弃属性或修改解析树的行为，都不应破坏过去存档（含无 `event_stream` 的旧版本）的反序列化能力。

## 3. 底层协议变更流程 (Protocol Update)
若必须修改 Phase 1.x 已验收的底层协议（如修改 `SideEffectClass` 枚举值）：
1.  **同步更新基线**：必须在 `AUTOMATION_BASELINE_PHASE1X.md` 中记录变更原因。
2.  **强制回归**：必须完整运行 `cargo test --test cpevr_test`，确保既有安全逻辑未失效。
3.  **显式说明**：在变更说明中将其列为“破坏性变更”。

## 4. 强制回归要求
任何影响到以下逻辑的改动，必须通过全套回归测试：
- 执行器与验证器的物理接线。
- 高风险操作的 Token 校验路径。
- 回滚决策的触发算法。

---
**目标**：Phase 2 是盖楼，Phase 1 是地基。盖楼可以加柱子、铺地板，但不能拆地基的钢筋。
