# Rust 版本技能迁移计划

## 项目要求
**统一使用 Rust 语言，所有技能必须用 Rust 实现**

## 现有 Python 技能列表

### skill-router/skills/
1. **autonomous_orchestrator** - 任务拆解与协调器
2. **google_search** - 网络搜索技能
3. **yaml_parser** - YAML 解析技能

### skills/
1. **autonomous_orchestrator** - 任务拆解与协调器
2. **google_search** - 网络搜索技能
3. **yaml_parser** - YAML 解析技能
4. **error_logger** - 错误日志记录器 (✅ 已用 Rust 实现)
5. **synth_jsonparse** - JSON 解析合成技能
6. **synth_skillsynthesize** - 技能合成技能
7. **synth_textsummarize** - 文本摘要技能

## 迁移优先级

### Phase 1: 核心技能 (高优先级)
1. ✅ **error_logger** - 错误日志记录器 (已完成)
2. **yaml_parser** - YAML 解析 (基础技能)
3. **synth_jsonparse** - JSON 解析 (基础技能)

### Phase 2: 功能技能 (中优先级)
4. **google_search** - 网络搜索
5. **synth_skillsynthesize** - 技能合成
6. **synth_textsummarize** - 文本摘要

### Phase 3: 协调技能 (低优先级)
7. **autonomous_orchestrator** - 任务协调器

## 技术方案

### 1. 基础技能 (解析类)
- **依赖**: serde, serde_json, serde_yaml
- **功能**: JSON/YAML 解析与处理
- **示例**: yaml_parser, synth_jsonparse

### 2. 网络技能
- **依赖**: reqwest, tokio, serde_json
- **功能**: HTTP 请求，网络搜索
- **示例**: google_search

### 3. 复合技能
- **依赖**: serde_json, std::process::Command
- **功能**: 调用其他技能，任务拆解
- **示例**: autonomous_orchestrator

### 4. 合成技能
- **依赖**: serde_json, tempfile
- **功能**: 动态代码生成
- **示例**: synth_skillsynthesize, synth_textsummarize

## 预期成果

### 最终状态
```
skills/
├── error_logger/              ✅ Rust
├── yaml_parser/               → Rust
├── google_search/             → Rust
├── autonomous_orchestrator/   → Rust
├── synth_jsonparse/           → Rust
├── synth_skillsynthesize/     → Rust
└── synth_textsummarize/       → Rust

skill-router/skills/
├── autonomous_orchestrator/   → Rust
├── google_search/             → Rust
└── yaml_parser/               → Rust
```

### 编译产物
```
target/release/
├── error_logger.exe
├── yaml_parser.exe
├── google_search.exe
├── autonomous_orchestrator.exe
├── synth_jsonparse.exe
├── synth_skillsynthesize.exe
└── synth_textsummarize.exe
```

## 验证标准
- ✅ 所有技能编译通过
- ✅ 功能与原 Python 版本一致
- ✅ 支持 JSON 输入/输出
- ✅ 错误处理完善
