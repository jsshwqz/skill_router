# AUTOMATION_BACKLOG_POST_PHASE1X - 自动化内核后续演进清单

以下项为“已知但不阻塞”进入 Phase 2 的长尾任务，由于其旨在提升工程美学或极高性能，不作为当前交付的硬性门槛。

### 1. 审计事件结构化 (Event Migration)
- **当前状态**：`error_history` 记录的是强类型 Error 而非解耦的 Event。
- **不阻塞理由**：现有 Error 结构已足够支撑当前的技术诊断与回滚决策。
- **后续建议**：等 Phase 2 跨 Agent 状态接力需求明确后，统一升级为 `EventStream`。优先级：中。

### 2. 回滚协议进一步对齐 (Protocol Alignment)
- **当前状态**：`rollback_contract` 是逻辑开关，执行器的参数协议尚未完全归一化。
- **不阻塞理由**：已有真执行回滚作为保底，物理验证已通过。
- **后续建议**：结合 Phase 2 的 Remote 执行器进行接口归一化。优先级：中。

### 3. 高风险确认门与 UI 集成 (Human-in-the-loop UX)
- **当前状态**：目前基于 Token 预校验，需人工手动填充。
- **不阻塞理由**：物理阻断逻辑已验证，安全闭环已成立。
- **后续建议**：在 AionUI 层面建立自动唤起确认弹窗的通道。优先级：高。

### 4. 更多真实 Verifier 开发 (Verifier Ecosystem)
- **当前状态**：目前主攻 `cargo check`。
- **不阻塞理由**：单点真实验证已证明 CPEVR 体系在真实命令行下的有效性。
- **后续建议**：增加单元测试验证器、静态分析验证器等。优先级：低（随业务需求增加）。

### 5. 自适应初始规划 (Dynamic Initial Planning)
- **当前状态**：目前的 Initial Planning 相对简单。
- **不阻塞理由**：复杂逻辑已通过 Replan 与 Patch 机制处理，启动路径已通。
- **后续建议**：引入更深层的 LLM 预分析能力。优先级：低。
