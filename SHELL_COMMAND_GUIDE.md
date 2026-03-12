# ⚠️ 重要：PowerShell 与 Bash 语法区别指南

> **创建日期**: 2026-03-12  
> **适用范围**: Gemini CLI 所有用户  
> **优先级**: P0 - 必须知晓

---

## 🌍 shell 环境差异

### 操作系统默认 Shell

| 操作系统 | 默认 Shell | 语法类型 |
|---------|-----------|---------|
| Windows | PowerShell | `;` 分隔 |
| Linux/macOS | Bash/Zsh | `;` 或 `&&` 分隔 |

---

## 🆚 PowerShell vs Bash 命令对比

### 1. 命令连接符

| 操作 | PowerShell | Bash/Linux |
|------|-----------|-----------|
| 顺序执行 | `cmd1 ; cmd2` | `cmd1 ; cmd2` |
| 条件执行 | `cmd1 ; cmd2` | `cmd1 && cmd2` |
| 当前目录 | `pwd` | `pwd` |
| 切换目录 | `cd "path"` | `cd "path"` |

### 2. 路径分隔符

| 操作 | PowerShell | Bash/Linux |
|------|-----------|-----------|
| Windows 路径 | `C:\path	o\file` | `C:/path/to/file` |
| 相对路径 | `./file` | `./file` |
| 上级目录 | `..` | `../` |

### 3. 变量语法

| 操作 | PowerShell | Bash/Linux |
|------|-----------|-----------|
| 定义变量 | `$var = "value"` | `var="value"` |
| 引用变量 | `$var` | `$var` |
| 环境变量 | `$env:PATH` | `$PATH` |

### 4. 条件判断

| 操作 | PowerShell | Bash/Linux |
|------|-----------|-----------|
| AND | `cond1 -and cond2` | `cond1 && cond2` |
| OR | `cond1 -or cond2` | `cond1 || cond2` |
| NOT | `-not cond` | `! cond` |

### 5. 文件操作

| 操作 | PowerShell | Bash/Linux |
|------|-----------|-----------|
| 列出文件 | `Get-ChildItem` 或 `ls` | `ls` 或 `ls -la` |
| 查看文件 | `Get-Content` 或 `cat` | `cat` |
| 删除文件 | `Remove-Item` 或 `rm` | `rm` |
| 复制文件 | `Copy-Item` 或 `cp` | `cp` |

---

## 🚨 常见错误场景

### 错误 1: 使用 && 在 PowerShell 中

```powershell
# ❌ 错误
cd "C:\path" && cargo build

# ✅ 正确
cd "C:\path" ; cargo build
```

### 错误 2: 混合路径分隔符

```powershell
# ❌ 错误
cd C:/path	o\file

# ✅ 正确
cd "C:\path	o\file"
# 或
cd "C:/path/to/file"
```

### 错误 3: 引号使用不当

```powershell
# ❌ 错误
cargo build --release -- --json "task with spaces

# ✅ 正确
cargo build --release -- --json "task with spaces"
```

---

## ✅ Gemini CLI 使用规范

### 必须遵守的规则

1. **始终使用相对路径**
   ```powershell
   # ✅ 正确
   cd "C:\project"
   cargo build --release
   
   # ❌ 避免
   cargo build --release --directory "C:\project"
   ```

2. **PowerShell 中使用分号**
   ```powershell
   # ✅ 正确
   cargo check ; cargo build --release
   ```

3. **优先使用 Cargo 命令**
   ```powershell
   # ✅ 正确
   cargo build --release
   cargo run --release -- --json "task"
   ```

4. **验证命令格式**
   ```powershell
   # 1. 先 check
   cargo check
   
   # 2. 再 build
   cargo build --release
   
   # 3. 最后 run
   cargo run --release -- --json "task"
   ```

---

## 🧪 快速测试命令

```powershell
# 测试 PowerShell 环境
Write-Host "PowerShell 环境测试" ; rustc --version ; cargo --version

# 测试项目构建
cargo check ; cargo build --release ; cargo run --release -- --json "test"
```

---

## 📚 学习资源

### PowerShell 官方文档
- [PowerShell 文档](https://learn.microsoft.com/zh-cn/powershell/)
- [PowerShell 速查表](https://learn.microsoft.com/zh-cn/powershell/module/microsoft.powershell.core/about/about_command_precedence)

### Bash 官方文档
- [Bash 手册](https://www.gnu.org/software/bash/manual/)
- [Bash 速查表](https://devhints.io/bash)

---

## 🔔 关键记忆点

| 场景 | PowerShell | Bash |
|------|-----------|------|
| 多命令执行 | `cmd1 ; cmd2` | `cmd1 ; cmd2` 或 `cmd1 && cmd2` |
| 切换目录 | `cd "path"` | `cd "path"` |
| 查看文件 | `cat file` 或 `Get-Content file` | `cat file` |
| 列出文件 | `ls` 或 `Get-ChildItem` | `ls` |

**最重要**: 在 Gemini CLI 中统一使用 PowerShell 语法！

---

**最后更新**: 2026-03-12  
**维护者**: AionUi Team
