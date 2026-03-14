# 在其他项目使用 memory_manager 技能的三种方式

## 📋 概述

`memory_manager` 技能目前位于:
```
C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\memory_manager
```

## 方式 1: 复制技能目录 (推荐用于快速使用)

### 操作步骤

```powershell
# 源路径 (当前项目)
$source = "C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\memory_manager"

# 目标路径 (新项目)
$target = "C:\path	o\other\project\skills\memory_manager"

# 复制技能目录
cp -Recurse $source $target

# 在新项目的 registry.json 中注册 (如果需要)
```

### 优点
- ✅ 简单快速
- ✅ 完全独立，互不影响
- ✅ 可以独立修改

### 缺点
- ❌ 多个副本需要分别维护
- ❌ 更新需要手动同步

---

## 方式 2: 创建符号链接 (推荐用于跨多个项目共享)

### 操作步骤

```powershell
# 在新项目的 skills 目录中创建符号链接
cd "C:\path	o\other\project\skills"
mklink /D memory_manager "C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\memory_manager"
```

### 验证符号链接

```powershell
# 查看链接
dir

# 应该看到:
# m----         2026/3/13     12:30                memory_manager -> ...memory_manager
```

### 优点
- ✅ 多个项目共享同一个副本
- ✅ 更新只需修改一次
- ✅ 节省磁盘空间

### 缺点
- ⚠️ 需要管理员权限创建符号链接
- ⚠️ 链接断开时会导致技能不可用

---

## 方式 3: 使用 skill_router 自动部署

### 操作步骤

在新项目的 `registry.json` 中添加:

```json
{
  "skills": {
    "memory_manager": {
      "name": "memory_manager",
      "version": "0.1.0",
      "capabilities": ["memory_management", "context_storage", "knowledge_retention", "project_history"],
      "source": null,
      "permissions": {
        "network": false,
        "filesystem_read": true,
        "filesystem_write": true,
        "process_exec": false
      },
      "usage": null,
      "lifecycle": null,
      "description": "Memory manager skill - Manages project memory, saves and retrieves context, maintains project history",
      "entrypoint": "main.rs"
    }
  }
}
```

确保 `skills_dir` 指向正确路径，或者复制技能目录到该路径。

---

## 🧪 验证技能是否可用

在新项目中执行:

```powershell
# 切换到新项目目录
cd "C:\path	o\other\project"

# 执行 memory_manager
.\skills\memory_manager	argetelease\memory_manager.exe

# 应该看到帮助信息
```

---

## 📊 决策指南

| 场景 | 推荐方式 | 说明 |
|------|----------|------|
| 单次使用 / 临时项目 | 方式1 (复制) | 简单直接，用完即删 |
| 多个项目需要 | 方式2 (符号链接) | 一次维护，处处可用 |
| 自动化部署 | 方式3 (配置注册) | 集成到项目配置中 |

---

## ⚠️ 注意事项

1. **MEMORY 目录**: 记忆数据存储在 `skills/memory_manager/MEMORY/memory.json`，各项目独立

2. **编译**: 如果新项目中没有编译过，需要先运行:
   ```powershell
   cd skills\memory_manager
   cargo build --release
   ```

3. **权限**: 确保有读写权限:
   - `filesystem_read: true`
   - `filesystem_write: true`
