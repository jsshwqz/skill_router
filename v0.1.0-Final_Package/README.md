# Skill Router Rust Core - 交付说明 (Handover)

## 🚀 项目状态
本系统已按照 `docs/` 中的四份规范文档完成 **Phase 1 至 Phase 5** 的所有核心功能实现与验证。系统处于可交付、可运行状态。

### ✅ 已实现核心功能
- **Planner**: 任务意图解析与 Capability 推断（识别 yaml, json, search 等）。
- **Loader**: 技能元数据（skill.json）动态加载与扫描。
- **Registry**: 技能状态持久化管理，支持使用统计（total_calls, latency 等）。
- **Matcher**: 基于能力的最佳技能匹配算法。
- **Executor**: 安全敏感型进程执行器，支持 Python 脚本和二进制调用。
- **Security**: 严格的权限校验模型（默认拒绝，需在 skill.json 明确授权）。
- **OnlineSearch & Synth**: 缺失能力时的“搜索候选-自动安装”与“代码合成”闭环。
- **Lifecycle**: 自动判定技能阶段（keep, polish, publish_candidate 等）。

## 📂 目录结构
- `bin/`: 预编译的 Windows 可执行文件 `skill-router.exe`。
- `src/`: Rust 源代码实现（模块化设计，高内聚低耦合）。
- `skills/`: 技能库（包含已验证的 `yaml_parser` 和 `google_search`）。
- `docs/`: 原始需求规范、系统架构及任务清单。
- `registry.json`: 存储所有已发现技能的状态与运行数据。
- `config.json`: 系统全局配置文件。

## 🛠️ 快速开始 (运行验证)
### 1. 直接执行二进制 (无需 Rust 环境)
```powershell
.\bin\skill-router.exe "parse this yaml"
```

### 2. 获取结构化 JSON 输出 (适配 Agent 工具调用)
```powershell
.\bin\skill-router.exe --json "parse this yaml"
```

### 3. 开发模式运行 (需安装 Rust/Cargo)
```powershell
cargo run -- "search for something"
```

## 🤖 给接手 AI 的指令
1. **环境同步**: 确保系统安装有 Python（用于运行默认技能脚本）。
2. **逻辑确认**: 核心 Pipeline 位于 `src/main.rs`，采用 任务->能力->匹配->执行->生命周期 的流式处理。
3. **后续演进建议**: 
   - 增强 `OnlineSearch`：接入真实的 GitHub 或 API 搜索。
   - 强化 `Planner`：引入 LLM 辅助进行更精准的复杂任务分解。
   - 增加 `MCP` 支持：实现与 Model Context Protocol 的对接。

---
*交付日期：2026年3月8日*
