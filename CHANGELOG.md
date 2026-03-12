# 更新日志

本文件记录项目的所有重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.2.0] - 2026-03-12

### 🚀 新增

- **Rust 版本迁移**: 所有技能从 Python 迁移到 Rust
- **synth_jsonparse**: JSON 解析合成技能 (Rust)
- **synth_skillsynthesize**: 技能合成技能 (Rust)
- **synth_textsummarize**: 文本摘要技能 (Rust)

### 🔄 变更

- **Breaking**: 所有技能改为 Rust 实现
- **Breaking**: skill.json 中 entrypoint 改为 "main.rs"
- **Breaking**: 移除所有 main.py 文件

### 📦 技能列表

| 技能 | 语言 | 状态 |
|------|------|------|
| yaml_parser | Rust | ✅ |
| google_search | Rust | ✅ |
| autonomous_orchestrator | Rust | ✅ |
| synth_jsonparse | Rust | ✅ |
| synth_skillsynthesize | Rust | ✅ |
| synth_textsummarize | Rust | ✅ |

### 📝 迁移说明

#### 从 v0.1.0 迁移到 v0.2.0

1. **备份旧版**: 如需保留 Python 版本，请备份后删除
2. **更新配置**: 确保 skill.json 中 entrypoint 为 "main.rs"
3. **重新编译**: 运行 `cargo build --release` 编译所有技能

#### 删除 Python 版本

```bash
# 如果需要保留 Python 版本，请先备份
# 然后删除所有 main.py
Remove-Item -Path "skill-router/skills/*/main.py" -Force
```

### 🔒 安全

- 所有权限验证已集成
- 默认拒绝权限模型

### 🐛 修复

- 无

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
[0.2.0]: https://github.com/jsshwqz/skill_router/releases/tag/v0.2.0
[0.1.0]: https://github.com/jsshwqz/skill_router/releases/tag/v0.1.0
