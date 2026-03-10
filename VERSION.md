# Skill Router 版本变更日志

## [0.0.1] - 2026-03-10 - 初始发布

### ✨ 核心功能
- **Planner**: 任务意图解析与 Capability 推断
- **Loader**: 技能元数据动态加载
- **Registry**: 技能状态持久化管理
- **Matcher**: 基于能力的最佳技能匹配
- **SkillsFinder**: 智能技能发现模块
- **OnlineSearch**: 纯 Rust GitHub API 搜索
- **Synth**: 自动技能代码合成（Rust 优先）
- **Executor**: 安全敏感型进程执行器
- **Security**: 严格的权限校验模型
- **Lifecycle**: 自动技能生命周期管理

### 🏗️ 架构特性
- Rust-first 纯实现架构
- reqwest HTTP 客户端替代 Python 搜索
- 四阶段 Pipeline: 本地匹配 → SkillsFinder → GitHub搜索 → Synth
- 集成安全审计和权限验证

### 📦 已实现技能
- yaml_parser (v0.0.1)
- google_search (v0.0.1)
- synth_jsonparse (v0.0.1)
- synth_textsummarize (v0.0.1)
- synth_skillsynthesize (v0.0.1)
- autonomous_orchestrator (v0.0.1)

### 📚 文档
- README.md - 完整项目文档
- CHANGELOG.md - 遵循 Keep a Changelog 规范
- CONTRIBUTING.md - 贡献指南
- SECURITY.md - 安全策略
- LICENSE - MIT 开源协议