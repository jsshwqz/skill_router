# memory_manager 技能文档

## 📚 概述

`memory_manager` 技能提供长效记忆管理功能，用于保存、检索和组织项目上下文与历史记录。

## 🎯 能力标签

- `memory_management` - 记忆管理
- `context_storage` - 上下文存储
- `knowledge_retention` - 知识保留
- `project_history` - 项目历史

## 📦 安装位置

```
skills/memory_manager/
├── src/
│   └── main.rs           # Rust 源码
├── skill.json            # 技能定义
├── SKILL.md             # 本文档
└── target/
    └── release/
        └── memory_manager.exe  # 编译后的可执行文件
```

## 💾 存储位置

记忆数据保存在：
```
MEMORY/memory.json
```

## 🚀 使用方法

### 1. 保存记忆

```bash
memory_manager save "记忆内容" #标签1 #标签2 key=value
```

**示例**：
```bash
memory_manager save "已完成用户认证模块开发" #feature #auth #completed
memory_manager save "修复了 YAML 解析的编码问题" #bug #fix #yaml
```

**输出**：
```json
{"status":"success","message":"记忆已保存","id":"mem_1234567890_0"}
```

---

### 2. 加载所有记忆

```bash
memory_manager load
```

**输出**：
```json
{"status":"success","count":5,"summary":"共 5 条记忆，最近 3 条：..."}
{"id":"mem_1234567890_0","timestamp":"2026-03-13T12:30:00+00:00","content":"已完成用户认证模块开发","tags":["feature", "auth", "completed"]}
{"id":"mem_1234567890_1","timestamp":"2026-03-13T12:25:00+00:00","content":"修复了 YAML 解析的编码问题","tags":["bug", "fix", "yaml"]}
```

---

### 3. 搜索记忆

```bash
memory_manager search "关键词"
```

**示例**：
```bash
memory_manager search "认证"
```

**输出**：
```json
{"status":"success","found":2,"keyword":"认证"}
{"id":"mem_1234567890_0","content":"已完成用户认证模块开发","score":100}
```

---

### 4. 按标签搜索

```bash
memory_manager search-tag "标签名"
```

**示例**：
```bash
memory_manager search-tag "bug"
```

**输出**：
```json
{"status":"success","found":1,"tag":"bug"}
{"id":"mem_1234567890_1","content":"修复了 YAML 解析的编码问题"}
```

---

### 5. 查看摘要

```bash
memory_manager summary
```

**输出**：
```json
{"status":"success","count":5,"summary":"共 5 条记忆，最近 3 条：..."}
```

---

### 6. 清空所有记忆

```bash
memory_manager clear
```

**输出**：
```json
{"status":"success","cleared":5,"message":"所有记忆已清空"}
```

---

### 7. 启用长记忆模式

```bash
memory_manager enable-long-memory
```

**输出**：
```json
{"status":"success","enabled":true,"message":"Long memory enabled"}
```

---

### 8. 禁用长记忆模式

```bash
memory_manager disable-long-memory
```

**输出**：
```json
{"status":"success","enabled":false,"message":"Long memory disabled"}
```

---

### 9. 查看长记忆状态

```bash
memory_manager long-memory-status
```

**输出**：
```json
{"status":"success","enabled":true,"message":"Long memory is enabled"}
```

---

## 🔧 编译方法

```bash
cd skills/memory_manager
cargo build --release
```

**编译后位置**：
```
skills/memory_manager/target/release/memory_manager.exe
```

## 📋 典型使用场景

### 场景 1: 项目进度记录

```bash
# 每次会话结束时保存进度
memory_manager save "会话日期: 2026-03-13, 完成任务: A, 进行中: B" #session #progress
```

### 场景 2: 错误和解决方案记录

```bash
memory_manager save "错误: XXX, 原因: YYY, 解决方案: ZZZ" #error #solution
```

### 场景 3: 技术决策记录

```bash
memory_manager save "技术选型: Rust, 原因: 性能要求高" #decision #tech-stack
```

### 场景 4: 长期上下文维护

```bash
# 每次重要变更时保存
memory_manager save "项目架构更新: 添加了新模块 X" #architecture #update
```

## 🔄 与 Gemini CLI 集成

在 Gemini CLI 中调用：

```bash
# 保存会话摘要
& "skills\memory_manager	argetelease\memory_manager.exe" save "会话摘要: ..." #session

# 查询历史记录
& "skills\memory_manager	argetelease\memory_manager.exe" search-tag "feature"

# 获取最新摘要
& "skills\memory_manager	argetelease\memory_manager.exe" summary
```

## 📊 数据格式

记忆条目包含以下字段：

```json
{
  "id": "mem_timestamp_index",
  "timestamp": "2026-03-13T12:30:00+00:00",
  "content": "记忆内容",
  "tags": ["tag1", "tag2"],
  "metadata": {
    "key": "value"
  }
}
```

## ⚙️ 技术细节

- **语言**: Rust
- **依赖**: `serde`, `serde_json`, `chrono`, `anyhow`
- **文件格式**: JSON
- **编码**: UTF-8

## 🛠️ 开发计划

- [ ] 支持记忆版本控制
- [ ] 支持记忆导出/导入
- [ ] 支持记忆压缩/归档
- [ ] 支持记忆关联/引用
- [ ] 支持记忆搜索排序

## 📝 注意事项

1. **存储路径**: 确保 `MEMORY/` 目录存在且有写入权限
2. **编码**: 所有内容使用 UTF-8 编码
3. **备份**: 定期备份 `MEMORY/memory.json`
4. **清空**: `clear` 命令不可恢复，请谨慎使用
