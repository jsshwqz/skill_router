# Memory Manager 技能 - 全局调用指南

## 🎯 核心理念

**其他项目不需要复制任何文件！**

只需要通过**语言描述**即可调用 `memory_manager` 技能。

## 🚀 最简方案（推荐）

### 方案 1：设置全局 PATH 环境变量

将 `memory_manager.exe` 所在目录添加到系统 PATH：

```powershell
# 临时添加（当前会话）
$env:PATH += ";C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\memory_manager	argetelease"

# 永久添加（需要管理员权限）
[Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\memory_manager	argetelease", "User")
```

然后在任何项目中都可以直接调用：

```bash
memory_manager save "今天学习了 Rust" #learning
memory_manager load
memory_manager search "Rust"
```

---

## 🛠️ 通过语言调用（推荐给 AI）

### 方案 2：修改 registry.json 配置

在其他项目的 `registry.json` 中添加：

```json
{
  "skills": {
    "memory_manager": {
      "name": "memory_manager",
      "version": "0.1.0",
      "capabilities": ["memory_management", "context_storage", "knowledge_retention", "project_history"],
      "source": "C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\memory_manager",
      "permissions": {
        "network": false,
        "filesystem_read": true,
        "filesystem_write": true,
        "process_exec": false
      },
      "usage": null,
      "lifecycle": null,
      "description": "Memory manager skill - Manages project memory, saves and retrieves context, maintains project history",
      "entrypoint": "targetelease\memory_manager.exe"
    }
  }
}
```

关键修改：
- `source`: 指向原始项目路径
- `entrypoint`: 指向编译好的 `.exe` 文件

---

## 📋 完整的调用命令

| 命令 | 说明 | 示例 |
|------|------|------|
| `memory_manager save <内容> [标签]` | 保存记忆 | `memory_manager save "今天学习了 Rust" #learning` |
| `memory_manager load` | 加载记忆 | `memory_manager load` |
| `memory_manager search <关键词>` | 搜索记忆 | `memory_manager search "Rust"` |
| `memory_manager search-tag <标签>` | 按标签搜索 | `memory_manager search-tag "learning"` |
| `memory_manager summary` | 查看摘要 | `memory_manager summary` |
| `memory_manager clear` | 清空记忆 | `memory_manager clear` |

---

## 🎁 AI 调用示例

在任何项目中，直接说：

> "保存记忆：今天讨论了项目架构，使用了微服务设计"

或者

> "加载最近的记忆，看看我们之前讨论了什么"

系统会自动匹配到 `memory_manager` 技能并执行！

---

## ✅ 优势

- ✅ **零复制** - 不需要在每个项目中复制文件
- ✅ **统一存储** - 所有项目的记忆可以集中管理
- ✅ **语言驱动** - 通过自然语言调用
- ✅ **节省空间** - 只需要一份 `.exe` 文件
