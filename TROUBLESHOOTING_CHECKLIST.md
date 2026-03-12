# 故障排查检查清单

> **创建日期**: 2026-03-12  
> **版本**: v0.1.0

---

## 🚨 紧急检查（命令无响应时）

### 第一步：确认环境

```powershell
# 1. 检查当前目录
pwd

# 2. 检查 Rust 是否安装
rustc --version

# 3. 检查 Cargo 是否可用
cargo --version
```

### 第二步：验证编译

```powershell
# 1. 快速检查
cargo check

# 2. 完整构建
cargo build --release
```

### 第三步：运行测试

```powershell
# JSON 模式测试
cargo run --release -- --json "yaml parse test"

# 期望输出
# {"duration_ms":XXX,"lifecycle":null,"skill":"xxx","status":"success"}
```

---

## 📋 日常开发检查清单

### 代码修改后

- [ ] 运行 `cargo check` 检查语法
- [ ] 运行 `cargo fmt` 格式化代码
- [ ] 运行 `cargo clippy` 检查警告
- [ ] 运行 `cargo test` 运行测试
- [ ] 运行 `cargo build --release` 构建发布版本

### 提交代码前

- [ ] README 已更新
- [ ] CHANGELOG 已更新
- [ ] 错误日志已记录
- [ ] 文档已同步

### 部署前

- [ ] 清理构建缓存：`cargo clean`
- [ ] 构建发布版本：`cargo build --release`
- [ ] 测试运行：`cargo run --release -- --json "test"`
- [ ] 检查输出格式正确

---

## 🔧 常见错误解决方案

### 错误 1: PowerShell 命令连接符错误

**❌ 错误**:
```powershell
cd "path" && command
```

**✅ 正确**:
```powershell
cd "path" ; command
```

### 错误 2: 参数引号问题

**❌ 错误**:
```powershell
cargo run --release -- --json "task with spaces
```

**✅ 正确**:
```powershell
cargo run --release -- --json "task with spaces"
```

### 错误 3: 相对路径 vs 绝对路径

**❌ 错误**:
```powershell
# 使用绝对路径可能导致问题
cargo run --release -- --json "task"
# (当前目录不正确)
```

**✅ 正确**:
```powershell
# 使用相对路径
cargo run --release -- --json "task"
```

---

## 📊 故障排查流程图

```
用户报告"没反应"
    ↓
检查 Exit Code 是否为 0
    ↓
   是 → 检查输出格式
    ↓                  ↓
   否              正常
    ↓
检查命令格式
    ↓
PowerShell 用 && ?
    ↓
   是 → 改用 ; 分隔
    ↓
重新运行测试
```

---

## 🎯 三层验证机制

### 第一层：快速验证（< 10秒）
```powershell
cargo check
```

### 第二层：完整验证（< 60秒）
```powershell
cargo build --release
cargo run --release -- --json "test"
```

### 第三层：深度验证（< 5分钟）
```powershell
cargo clean
cargo build --release
cargo test
cargo run --release -- --json "full test"
```

---

## 📞 需要帮助时

如果以上检查都不能解决问题：

1. **记录完整输出**
   ```powershell
   cargo run --release -- --json "task" | Out-File output.txt
   ```

2. **检查 ERROR_LOG.md**
   - 查找相似问题
   - 参考解决方案

3. **准备信息**
   - 操作系统
   - Rust 版本
   - 仓库版本
   - 完整命令
   - 完整输出

---

## 🔔 关键记忆点

| 场景 | 正确做法 | 错误做法 |
|------|---------|---------|
| PowerShell 多命令 | `cmd1 ; cmd2` | `cmd1 && cmd2` |
| 项目路径 | 相对路径为主 | 硬编码绝对路径 |
| 命令验证 | 先 check 再 build | 直接 build |
| 输出检查 | 看完整输出 | 只看错误信息 |
| 错误处理 | 记录日志 | 忽略错误 |

---

**最后更新**: 2026-03-12  
**维护者**: AionUi Team

## 最近更新 (2026-03-12)

- [ ] 测试项1
- [ ] 测试项2

