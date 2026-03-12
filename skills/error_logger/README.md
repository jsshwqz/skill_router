# 错误日志技能 (Error Logger Skill)

> **版本**: v0.1.0  
> **创建日期**: 2026-03-12  
> **状态**: ✅ 已安装

---

## 📖 功能介绍

错误日志技能是一个自动化的错误记录和通知系统。当发生错误时，它会：

1. **自动捕获**错误信息
2. **记录到** `ERROR_LOG.md`
3. **更新** `TROUBLESHOOTING_CHECKLIST.md`
4. **发送通知**（控制台/日志）

---

## 🚀 使用方式

### 基本用法

```powershell
# JSON 模式调用
call_yaml_skill.py error_logger "{"title":"错误标题","type":"错误类型","symptom":"现象描述","root_cause":"根本原因","solution":"解决方案"}"

# 或使用技能路由
cargo run --release -- --json "error logger: 记录错误"
```

### 完整参数

```json
{
  "title": "错误标题",
  "type": "错误类型",
  "affected": "影响范围",
  "severity": "严重程度",
  "symptom": "现象描述",
  "root_cause": "根本原因",
  "solution": "解决方案",
  "verification": "验证结果",
  "checklist": ["检查项1", "检查项2"],
  "notify": true
}
```

---

## 📊 输出格式

### 成功输出
```json
{
  "status": "success",
  "skill": "error_logger",
  "error_id": "ERR-202603121323",
  "duration_ms": 100
}
```

---

## 🔍 技能配置

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `log_file` | string | `ERROR_LOG.md` | 错误日志文件路径 |
| `checklist_file` | string | `TROUBLESHOOTING_CHECKLIST.md` | 检查清单文件路径 |
| `enabled` | boolean | `true` | 是否启用技能 |
| `notify` | boolean | `true` | 是否发送通知 |

---

## 📁 相关文件

| 文件 | 说明 |
|------|------|
| `skills/error_logger/skill.json` | 技能配置 |
| `skills/error_logger/main.py` | 主程序 |
| `ERROR_LOG.md` | 错误日志 |
| `TROUBLESHOOTING_CHECKLIST.md` | 检查清单 |
| `SHELL_COMMAND_GUIDE.md` | Shell 命令指南 |
| `README.md` | 故障排查章节 |

---

## ✅ 验证状态

```powershell
# 运行测试
cargo run --release -- --json "parse yaml test"

# 期望输出
{"duration_ms":69.0,"lifecycle":"keep","skill":"yaml_parser","status":"success"}
```

---

## 📝 错误记录示例

### 示例 1: PowerShell 命令连接符错误

```json
{
  "title": "PowerShell 命令连接符错误",
  "type": "语法错误",
  "affected": "所有 Windows 用户",
  "severity": "中等",
  "symptom": "用户执行命令后未看到预期输出",
  "root_cause": "在 PowerShell 中使用了 && 连接符（Bash 语法）",
  "solution": "改用分号 ; 分隔命令",
  "verification": "cargo build --release 验证通过",
  "checklist": [
    "确认在项目根目录",
    "确认 Rust 已安装"
  ],
  "notify": true
}
```

---

## 🔧 维护说明

### 更新技能
```powershell
# 修改 main.py 后重新测试
python test_error_logger.py
```

### 查看错误日志
```powershell
# 查看最新错误
Get-Content ERROR_LOG.md -Tail 50
```

### 清理错误日志
```powershell
# 备份后清空
Copy-Item ERROR_LOG.md ERROR_LOG_BACKUP.md
Set-Content ERROR_LOG.md ""
```

---

**维护者**: Gemini CLI  
**最后更新**: 2026-03-12
