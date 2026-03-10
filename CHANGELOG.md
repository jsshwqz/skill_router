# 更新日志

本文件记录项目的所有重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

## [0.1.0] - 2026-03-10

### 新增

- **Planner**: 任务意图解析与能力推断
- **Loader**: 动态技能元数据加载
- **Registry**: 技能状态持久化管理
- **Matcher**: 基于能力的技能匹配算法
- **SkillsFinder**: 智能技能发现与评分
- **OnlineSearch**: 纯 Rust 实现 GitHub API 搜索，带安全审计
- **Synth**: 自动技能代码合成（Rust 优先）
- **Executor**: 安全进程执行
- **Security**: 严格权限验证模型
- **Lifecycle**: 自动技能生命周期管理
- yaml_parser 技能
- google_search 技能
- synth_jsonparse 技能
- synth_textsummarize 技能
- synth_skillsynthesize 技能
- autonomous_orchestrator 技能

### 变更

- 将 Python 实现在线搜索替换为纯 Rust reqwest 实现
- 实现四阶段流水线：本地匹配 → 技能发现 → GitHub 搜索 → 代码合成

### 安全

- 集成安全审计与权限验证
- 默认拒绝权限模型
- 每次执行前进行运行时验证

[未发布]: https://github.com/jsshwqz/skill_router/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jsshwqz/skill_router/releases/tag/v0.1.0