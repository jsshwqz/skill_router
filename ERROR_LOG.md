# 错误日志与教训总结

> **记录日期**: 2026-03-12  
> **问题类型**: 命令执行失败  
> **影响范围**: 所有使用 Gemini CLI 的用户  
> **严重程度**: ⚠️ 中等（影响操作判断）

---

## 🐛 问题现象

**用户反馈**: "又没反应了吗"

### 表现症状
- 用户执行命令后未看到预期输出
- 可能误判为系统卡死或无响应
- 影响后续操作判断

---

## 🔍 根本原因分析

### 原因 1: PowerShell 语法混淆

**错误命令示例**:
```powershell
# ❌ 错误 - PowerShell 不支持 && 连接符
cd "C:\path" && cargo build
```

**错误信息**:
```
所在位置 行:1 字符: 85
+ ... " && cargo b ...
+                                                                ~~
Error: (none)
Exit Code: 1
```

**原因**: 
- PowerShell 中 `&&` 不是有效的命令连接符
- `&&` 是 Bash/Linux Shell 的语法
- PowerShell 应使用分号 `;` 或换行分隔命令

---

### 原因 2: 工具调用参数顺序错误

**错误示例**:
```json
{
  "command": "cargo run --release -- --json "yaml parse this test"",
  "directory": "C:\Users\..."
}
```

**问题**: 
- 某些工具调用可能参数缺失或格式不正确
- 缺少必要的参数校验

---

## ✅ 正确解决方案

### 方案 1: 使用分号分隔命令（PowerShell）

```powershell
# ✅ 正确 - 使用分号
cd "C:\path" ; cargo build
```

或直接使用相对路径：

```powershell
# ✅ 最佳 - 直接运行
cargo build --release
```

### 方案 2: 使用 `run_shell_command` 正确格式

```json
{
  "command": "cargo build --release",
  "directory": "C:\path	o\project"
}
```

---

## 📋 必须遵守的规则

### ✅ 必须做的

1. **在项目根目录工作**
   ```
   使用相对路径，不要依赖绝对路径
   ```

2. **使用正确的命令格式**
   ```
   cargo build --release
   cargo run --release -- --json "task"
   ```

3. **先验证编译再运行**
   ```
   1. cargo check (快速检查)
   2. cargo build --release (完整构建)
   3. cargo run --release (运行测试)
   ```

4. **观察完整输出**
   ```
   不要只看错误信息，要看完整的构建/运行输出
   ```

### ❌ 绝对禁止的

1. **不要在 PowerShell 中使用 `&&`**
   ```powershell
   # 禁止
   cd "path" && command
   
   # 应该
   cd "path" ; command
   ```

2. **不要假设命令格式**
   ```
   每次都要确认参数顺序和格式
   ```

3. **不要跳过编译验证**
   ```
   修改代码后必须先 cargo check
   ```

---

## 🛠️ 标准操作流程 (SOP)

### 场景 1: 编译项目

```powershell
# 1. 进入项目目录
cd "C:\path	o\project"

# 2. 快速检查
cargo check

# 3. 完整构建
cargo build --release
```

### 场景 2: 运行程序

```powershell
# 直接运行（自动编译）
cargo run --release -- --json "your task"

# 或运行已构建的二进制
targetelease\skill-router.exe --json "your task"
```

### 场景 3: 调试问题

```powershell
# 查看帮助
cargo run --release -- --help

# 查看版本
cargo run --release -- --version

# 运行测试
cargo test
```

---

## 📝 本次问题记录

| 项目 | 内容 |
|------|------|
| **问题日期** | 2026-03-12 |
| **问题类型** | 命令执行失败 |
| **影响人数** | 1 (用户) |
| **解决方案** | 使用正确的 PowerShell 语法 |
| **验证状态** | ✅ 已验证通过 |

### 修复验证

```powershell
✅ cargo check       # 通过
✅ cargo build --release  # 通过 (0 warnings)
✅ cargo run --release -- --json "test"  # 通过
```

**输出结果**:
```json
{"duration_ms":426.0,"lifecycle":null,"skill":"yaml_parser","status":"success"}
```

---

## 🔔 教训总结

### 1. 环境感知
> **教训**: 不能假设用户的 shell 环境  
> **对策**: 
- 明确区分 PowerShell 和 Bash 语法
- 在文档中注明使用的 shell 类型
- 测试不同环境的兼容性

### 2. 命令验证
> **教训**: 命令执行失败时没有及时发现  
> **对策**:
- 每次命令执行后检查 `Exit Code`
- 观察完整输出，不只是错误信息
- 添加适当的错误处理和提示

### 3. 用户沟通
> **教训**: 用户说"没反应"时没有快速定位问题  
> **对策**:
- 建立标准诊断流程
- 快速验证关键功能
- 提供清晰的反馈信息

---

## 📌 预防措施

### 代码层面
- [ ] 添加命令格式校验
- [ ] 添加环境检测
- [ ] 添加错误处理

### 文档层面
- [ ] 在 README 中添加故障排查章节
- [ ] 添加常见错误示例
- [ ] 添加调试技巧

### 工具层面
- [ ] 添加命令预检查
- [ ] 添加输出超时检测
- [ ] 添加自动重试机制

---

## 🎯 下一步行动

1. **更新文档**
   ```markdown
   - 在 README 添加 "故障排查" 章节
   - 添加 PowerShell vs Bash 对比表
   - 添加典型错误案例
   ```

2. **创建检查清单**
   ```
   - [ ] 确认当前目录正确
   - [ ] 确认命令格式正确
   - [ ] 确认输出格式预期
   ```

3. **添加自动化验证**
   ```bash
   # 创建验证脚本
   cargo run --release -- --json "test" | Out-File test_output.json
   ```

---

**最后更新**: 2026-03-12  
**版本**: v0.1.0  
**作者**: Gemini CLI

## 错误 ERR-1773293436: Rust版本技能测试

> **记录日期**: 2026-03-12 05:30:36
> **错误类型**: 功能测试
> **影响范围**: 所有用户
> **严重程度**: 
> **错误ID**: ERR-1773293436

### 现象
测试Rust版本是否正常

### 根本原因
测试

### 解决方案
验证通过

### 验证结果
测试成功
