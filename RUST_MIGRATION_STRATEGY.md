# Skill Router - Rust 版本迁移策略

## 📋 当前状态分析

### Skill-Router 中的技能状态

| 技能目录 | main.py | main.rs | skill.json (entrypoint) | 状态 |
|---------|---------|---------|------------------------|------|
| autonomous_orchestrator | ✅ | ✅ | main.rs | ✅ 已迁移 |
| google_search | ✅ | ✅ | main.rs | ✅ 已迁移 |
| yaml_parser | ✅ | ✅ | main.rs | ✅ 已迁移 |
| synth_jsonparse | ✅ | ❌ | main.jsonparse | ⚠️ 待迁移 |
| synth_skillsynthesize | ✅ | ❌ | main.jsonparse | ⚠️ 待迁移 |
| synth_textsummarize | ✅ | ❌ | main.jsonparse | ⚠️ 待迁移 |

**注意**: synth_jsonparse, synth_skillsynthesize, synth_textsummarize 的 skill-router 目录中只有 Python 版本，Rust 版本在 skills/ 目录中。

---

## 🎯 迁移策略

### 方案 A: 保持向后兼容 (推荐)
**做法**: 保留 old/ 目录存放 Python 版本，新版本使用 Rust

```
skill-router/
├── skills/
│   ├── yaml_parser/          # Rust 版本
│   │   ├── main.rs
│   │   └── skill.json (entrypoint: "main.rs")
│   ├── google_search/        # Rust 版本
│   │   ├── main.rs
│   │   └── skill.json (entrypoint: "main.rs")
│   └── autonomous_orchestrator/  # Rust 版本
│       ├── main.rs
│       └── skill.json (entrypoint: "main.rs")
└── old/                      # Python 版本（备份）
    └── skills/
        ├── yaml_parser/      # 旧版 Python
        ├── google_search/    # 旧版 Python
        └── autonomous_orchestrator/  # 旧版 Python
```

**优点**:
- ✅ 完全向后兼容
- ✅ 可以平滑过渡
- ✅ 方便回滚
- ✅ 有完整的历史记录

**缺点**:
- ⚠️ 增加维护成本
- ⚠️ 项目体积增大

---

### 方案 B: 全面升级 (当前采用)
**做法**: 直接替换所有 Python 版本为 Rust 版本

```
skill-router/
└── skills/
    ├── yaml_parser/          # Rust 版本
    ├── google_search/        # Rust 版本
    ├── autonomous_orchestrator/  # Rust 版本
    ├── synth_jsonparse/      # Rust 版本 (待完成)
    ├── synth_skillsynthesize/  # Rust 版本 (待完成)
    └── synth_textsummarize/  # Rust 版本 (待完成)
```

**优点**:
- ✅ 纯 Rust 项目
- ✅ 维护简单
- ✅ 性能最优

**缺点**:
- ⚠️ 无法回滚到 Python 版本
- ⚠️ 需要重新测试所有功能

---

### 方案 C: 逐步迁移 (折中方案)
**做法**: 先完成 synth_ 系列技能的 Rust 版本，然后清理 Python 版本

**步骤**:
1. 完成 synth_jsonparse Rust 版本
2. 完成 synth_skillsynthesize Rust 版本  
3. 完成 synth_textsummarize Rust 版本
4. 测试验证所有技能
5. 删除 Python 版本 (main.py)
6. 更新 CHANGELOG.md 记录版本变更

---

## 📊 版本号管理

### 当前版本
- **v0.1.0**: Python 原版

### 迁移版本
- **v0.2.0**: Rust 版本迁移完成
  - 所有技能改为 Rust 实现
  - 入口文件改为 main.rs
  - 移除 main.py

### 未来版本
- **v0.3.0**: 功能增强
  - 添加真实网络搜索
  - 优化摘要算法
  - 添加单元测试

---

## 🔄 迁移操作步骤

### 第一步: 完成剩余技能
```bash
# synth_jsonparse
cd skill-router/skills/synth_jsonparse
# 创建 main.rs 和 Cargo.toml

# synth_skillsynthesize
cd skill-router/skills/synth_skillsynthesize
# 创建 main.rs 和 Cargo.toml

# synth_textsummarize
cd skill-router/skills/synth_textsummarize
# 创建 main.rs 和 Cargo.toml
```

### 第二步: 更新 skill.json
确保所有技能的 skill.json 中:
```json
{
  "entrypoint": "main.rs",
  "metadata": {
    "lang": "rust"
  }
}
```

### 第三步: 编译测试
```bash
# 编译所有技能
cargo build --release

# 测试技能
cargo run --release -- --json "test task"
```

### 第四步: 清理 Python 文件
```bash
# 删除所有 main.py
rm skill-router/skills/*/main.py
```

### 第五步: 更新文档
```markdown
# CHANGELOG.md

## [0.2.0] - 2026-03-12

### Breaking Changes
- 所有技能改为 Rust 实现
- skill.json 中 entrypoint 改为 "main.rs"

### Migration Guide
- 旧版 Python 技能请备份后删除
- 新版 Rust 技能请参考新的 skill.json 格式
```

---

## 📝 建议

### 推荐方案: 方案 C (逐步迁移)

**原因**:
1. ✅ 确保所有技能都经过充分测试
2. ✅ 可以在迁移过程中发现并修复问题
3. ✅ 最终实现纯 Rust 项目

**实施计划**:
1. ✅ 完成 synth_jsonparse Rust 版本
2. ✅ 完成 synth_skillsynthesize Rust 版本
3. ✅ 完成 synth_textsummarize Rust 版本
4. ⏳ 编译测试所有技能
5. ⏳ 清理 Python 文件
6. ⏳ 发布 v0.2.0

---

## ❓ 关于已发布的老版本

### 问题: 已发布的 v0.1.0 怎么处理？

**答案**: 保持不变

- GitHub releases 中的 v0.1.0 标签保留
- 代码历史保留 (使用 git tag v0.1.0)
- 作为 Python 版本的历史快照
- v0.2.0 作为 Rust 版本的起始点

### 命令:
```bash
# 查看历史版本
git tag -l v*
git show v0.1.0:skill-router/skills/yaml_parser/main.py
```

---

## ✅ 当前进度

| 技能 | skill-router | skills/ | 状态 |
|------|-------------|---------|------|
| yaml_parser | ✅ | ✅ | 已完成 |
| google_search | ✅ | ✅ | 已完成 |
| autonomous_orchestrator | ✅ | ✅ | 已完成 |
| synth_jsonparse | ⏳ | ✅ | 待完成 |
| synth_skillsynthesize | ⏳ | ✅ | 待完成 |
| synth_textsummarize | ⏳ | ✅ | 待完成 |

---

## 📞 下一步行动

1. 完成 synth_ 系列技能的 Rust 版本
2. 编译测试所有技能
3. 更新 CHANGELOG.md
4. 发布 v0.2.0
5. 在 README.md 中注明 v0.2.0 为 Rust 版本
