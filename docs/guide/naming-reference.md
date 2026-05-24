# Aion Forge Naming Reference

## 目的

这份文档用于统一 `战略`、`自进化`、`郝匠`、`朝堂` 等历史高频名词在不同层级中的标准叫法，避免把角色名、技能名、代码组件名、能力概念名混为一谈。

## 类型定义

- `角色名`：用于分析、协作、分工的角色称呼
- `技能名`：本地安装或分发的技能目录名
- `代码组件名`：仓库内正式 crate、二进制或服务标识
- `MCP / 工具服务`：以 MCP 工具集合形式暴露的一组能力
- `能力概念名`：代码或文档中表达的一类能力，不一定对应独立安装项
- `历史别名`：曾出现过但不再建议继续使用的称呼

## 统一命名表

| 原始称呼 | 建议中文名 | 英文标准名 | 类型 | 主技能 / 主工具名 | 核心功能定义 | 首次可证实时间 | 证据来源 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 郝匠 | 工程匠审 | Master Craftsman | 角色名 | 未发现独立技能 | 聚焦代码质量、工程实践、Rust 实现细节、技术债治理 | 2026-04-13 | `C:/Users/Administrator/.claude/projects/D--test-aionui-forge/370a24be-4152-4886-8bc8-de6693a08717.jsonl` 中同轮映射与职责描述 |
| 战略 | 辩证战略引擎 | Aion ZhanLue / aion-zl | 代码组件名 | `strategic_plan` / `task_dialectic` | 为 AI Agent 编排提供战略分析、辩证拆解与 MCP 服务入口 | 代码层已存在 | `D:/test/aionui/forge/aion-zl/src/main.rs`、`D:/test/aionui/forge/aion-zl/src/mcp.rs`、`D:/test/aionui/forge/aion-zl/Cargo.toml` |
| 自进化 | 闭环演进顾问 | Evolution Advisor | 角色名 | 未发现独立技能 | 聚焦技能路由、记忆系统、Agent 编排是否形成反馈闭环，并识别演进瓶颈 | 2026-04-13 | `C:/Users/Administrator/.claude/projects/D--test-aionui-forge/370a24be-4152-4886-8bc8-de6693a08717.jsonl` |
| 自进化 | 技能演化能力 | self-evolution / evolve | 能力概念名 | `evolve` | 在代码层生成或注册 evolved skill 的能力链路 | 代码层已存在 | `D:/test/aionui/forge/aion-intel/src/synth.rs`、`D:/test/aionui/forge/aion-intel/src/tests.rs`、`D:/test/aionui/forge/docs/archive/测试报告.md` |
| forge-evolver | 前置治理路由器 | forge-evolver | 技能名 | `forge-evolver` | 在调用 aion-forge 或其他技能前，负责任务澄清、风险收束、路由决策与验证约束 | 2026-04-15 23:25:21 | `C:/Users/Administrator/.codex/skills/forge-evolver/SKILL.md`、`C:/Users/Administrator/.agents/skills/forge-evolver/SKILL.md` |
| forge-evolver-baseline | 轻量前置治理基线 | forge-evolver-baseline | 技能名 | `forge-evolver-baseline` | 提供更轻量的前置治理与最小验证约束，用作增强版 forge-evolver 的基线对照 | 2026-04-16 00:14:08 | `C:/Users/Administrator/.codex/skills/forge-evolver-baseline/SKILL.md`、`C:/Users/Administrator/.agents/skills/forge-evolver-baseline/SKILL.md` |
| 朝堂 | 群体协作议事器 | chaotang | MCP / 工具服务 | `chaotang_solve` | 提供 brainstorm、compare、discuss、review、solve、status 等群体协作动作 | 会话中已存在 | `mcp__chaotang__chaotang_brainstorm`、`mcp__chaotang__chaotang_compare`、`mcp__chaotang__chaotang_discuss`、`mcp__chaotang__chaotang_review`、`mcp__chaotang__chaotang_solve`、`mcp__chaotang__chaotang_status` |
| 赫匠 | 不再使用 | 不再使用 | 历史别名 | 不适用 | 视为 `郝匠` 的误写，不再单列 | 后续回溯确认 | 会话回溯结论 |

说明：
- `战略` 目前只保留 `Aion ZhanLue / aion-zl` 这一条英文标准名，原先 `Strategist` 不再作为该条目的标准英文名。
- 2026-04-13 可证实的是 `闭环演进顾问 / Evolution Advisor` 这类角色定义，不是落盘安装的 `forge-evolver` 技能。
- `forge-evolver` 与 `forge-evolver-baseline` 的中文名按其功能重命名，不再直接按英文表面字义归到“自进化”。

## 禁用与替代

| 不建议继续使用 | 统一替代为 | 适用场景 |
| --- | --- | --- |
| `自进化`（指角色） | `闭环演进顾问` | 指 2026-04-13 角色层 `Evolution Advisor` 时 |
| `自进化`（指 2026-04-15 技能） | `前置治理路由器` | 指 `forge-evolver` 时 |
| `自进化 baseline` / `自进化基线` | `轻量前置治理基线` | 指 `forge-evolver-baseline` 时 |
| `自进化`（指代码能力） | `技能演化能力` | 指 `self-evolution / evolve` 能力链路时 |
| `Strategist`（用于战略正式名） | `Aion ZhanLue / aion-zl` | 指战略正式组件名时 |

## 角色功能定义

### 1. 战略 / Aion ZhanLue

- 建议中文名：`辩证战略引擎`
- 标准英文名：`Aion ZhanLue / aion-zl`
- 核心职责：
  - 为任务提供战略分析与辩证拆解
  - 判断架构或方案是否支撑目标场景
  - 输出更高层的战略规划与路径建议
  - 作为 MCP 服务暴露战略类工具

### 2. 自进化角色 / Evolution Advisor

- 建议中文名：`闭环演进顾问`
- 标准英文名：`Evolution Advisor`
- 核心职责：
  - 评估技能路由是否形成闭环
  - 评估记忆系统是否支持持续改进
  - 评估 Agent 编排是否形成反馈回路
  - 识别演进瓶颈与自我提升断点

### 3. forge-evolver / 前置治理路由器

- 建议中文名：`前置治理路由器`
- 标准英文名：`forge-evolver`
- 核心职责：
  - 在执行前澄清任务与关键假设
  - 先做风险收束，再决定路由
  - 选择直接处理、分流给专门技能，或升级到 aion-forge
  - 在完成前强制要求验证边界

### 4. forge-evolver-baseline / 轻量前置治理基线

- 建议中文名：`轻量前置治理基线`
- 标准英文名：`forge-evolver-baseline`
- 核心职责：
  - 提供轻量前置治理
  - 保留最小验证约束
  - 作为增强版 forge-evolver 的对照基线

### 5. 郝匠 / Master Craftsman

- 标准中文角色功能名：`郝匠`
- 标准英文角色功能名：`Master Craftsman`
- 核心职责：
  - 深入代码质量与工程实践
  - 审查 Rust 实现细节
  - 识别技术债、脆弱点与维护成本
  - 给出可落地的工程修整建议

## 使用规范

1. `战略` 的英文标准名只保留 `Aion ZhanLue / aion-zl`。
2. 涉及团队分工或分析视角时，优先使用 `角色名`。
3. 涉及本地安装项或技能目录时，优先使用 `技能名`。
4. 涉及 Rust crate、CLI、MCP server 或代码标识时，优先使用 `代码组件名`。
5. 涉及一组 MCP 工具名时，优先使用 `MCP / 工具服务`。
6. `自进化` 一词必须根据上下文显式区分是 `闭环演进顾问`、`技能演化能力`，还是 `前置治理路由器 / 轻量前置治理基线`。
7. `赫匠` 不再作为正式名称使用，统一归并到 `郝匠 / Master Craftsman`。
8. 从本规则生效后，文档、沟通、命名表中应优先使用功能型中文名，不再把多个不同对象统称为 `自进化`。

## 当前结论

- `战略` 的英文标准名只保留 `Aion ZhanLue / aion-zl`
- 2026-04-13 的“自进化”对应角色层 `Evolution Advisor`，不等于 2026-04-15 安装的 `forge-evolver`
- `forge-evolver` 应按功能理解为 `前置治理路由器`
- `forge-evolver-baseline` 应按功能理解为 `轻量前置治理基线`
- `朝堂` 当前确认是 `MCP / 工具服务`
- `郝匠` 当前确认是 `角色名`
- `赫匠` 视为误写，不纳入正式命名体系
