# AUTOMATION_BASELINE_PHASE1X - 自动化内核验收基线说明

## 1. 版本范围
本基线覆盖了自 Phase 1 启动至 Phase 1.96 终极验收的所有核心代码与设计。包括：
- **Phase 1/1.5**: 建立了基本的 CPEVR (Plan-Execute-Verify-Recover) 循环与工程硬化。
- **Phase 1.8/1.9**: 引入了真实的 `cargo check` 验证、副作用治理 (`dirty_state`) 与文件级回滚。
- **Phase 1.95/1.96**: 落实了协议驱动的回滚 (`rollback_contract`)、全量决策审计 (`error_history`) 以及物理级的安全阻断门控（Token 校验前移）。

## 2. 已验收通过的能力 (Frozen Capabilities)
- **CPEVR 闭环集成**：系统能自主完成从初始规划到验证失败、再到 Replan 修复的全流程，并具备 `max_replan_count` 断路保护。
- **物理安全阻断**：高风险动作（`HighRiskHumanConfirm`）在无 Token 时会在执行副作用前被预防性拦截（由 `test_high_risk_physical_block` 证明）。
- **真实验证链路**：已接入基于 `std::process::Command` 的 `cargo_check` 真实验证，不再依赖 Mock 结果。
- **自适应恢复决策**：`RecoveryEngine` 能根据不同的 `SideEffectClass` 自动分流为 Retry, Replan, Rollback 或 Abort。

## 3. 已知 Backlog 尾项与演进 (Phase 2.1 更新)
- **结构化事件流（Phase 2.1 已引入）**：审计追踪已从单薄的 `error_history` 升级到具备生命周期的结构化 `event_stream`，为向后兼容，这两种机制当前系统内存续。
- **协议深度对齐**：`rollback_contract` 目前作为准入门槛，未来可进一步与执行器的参数协议深度缝合。
- **发现深度扩展**：`DiscoveryRadar` 目前在 Central 层为 Mock 协议，未来将接入真实的 HTTP 嗅探链路。

## 4. 刻意设计 (Forbidden to Simplify)
- **Gate Pre-Execution**：Token 校验必须位于副作用调用之前，这是系统安全的最后一道物理围栏。
- **Audited Failure**：所有失败（包括 Verifier 未找到）必须显式写入 `error_history`，禁止静默失败。
- **Class-Based Recovery**：恢复策略必须严格区分副作用等级（Reversible/Irreversible/External），禁止一刀切。

---
**基线状态判定**：Phase 1.x 已形成强可用底座，已足够支撑 Phase 2 开工。后续仅做增量硬化，不再反复返工底盘。
