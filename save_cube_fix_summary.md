# save-cube 命令修复总结

## 问题描述
`save-cube` 命令无法正确保存 tags 参数到 cube 文件中。

## 根本原因
1. **PowerShell 注释解释**：`#tag1` 在 PowerShell 中被解释为注释，导致参数丢失
2. **content 解析错误**：`content = args[3..].join(" ")` 将所有参数都当作 content
3. **生命周期问题**：`parse_args` 返回 `Vec<&str>`，但 `MemoryEntry::new` 需要 `Vec<String>`

## 修复内容

### 1. 修复 content 解析 (main.rs)
```rust
// 修复前
let content = args[3..].join(" ");

// 修复后
let content = &args[3];
```

### 2. 修改 parse_args 返回类型 (main.rs)
```rust
// 修复前
fn parse_args(args: &[String]) -> (Vec<&str>, HashMap<String, String>)

// 修复后
fn parse_args(args: &[String]) -> (Vec<String>, HashMap<String, String>)
```

### 3. 修改 MemoryEntry::new (memcube.rs)
```rust
// 修复前
pub fn new(id: &str, content: &str, tags: &[&str]) -> Self

// 修复后
pub fn new(id: &str, content: &str, tags: Vec<String>) -> Self
```

### 4. 修复 legacy add_entry 调用 (main.rs)
```rust
// 修复前
storage.add_entry(&content, &tags, metadata);

// 修复后
storage.add_entry(&content, &tags, metadata);
// (tags 类型已从 Vec<&str> 改为 Vec<String>)
```

## 使用说明

### PowerShell 中使用
在 PowerShell 中必须用引号包裹标签，因为 `#` 是注释符号：

```powershell
# 创建 cube
.\skills\memory_manager	argetelease\memory_manager.exe create-cube project "Project Context"

# 保存带标签的记忆（必须用引号）
.\skills\memory_manager	argetelease\memory_manager.exe save-cube project "测试内容" "#tag1" "#tag2"
```

### 验证结果
```json
{
  "memories": [
    {
      "content": "测试内容",
      "tags": ["tag1", "tag2"],
      ...
    }
  ]
}
```

## 验证命令
```powershell
# 测试 save-cube 带标签
.\skills\memory_manager	argetelease\memory_manager.exe create-cube project "Project Context"
.\skills\memory_manager	argetelease\memory_manager.exe save-cube project "测试内容" "#tag1" "#tag2"
cat MEMORY\cubes\project.json
```

## 关键文件
- `skills/memory_manager/src/main.rs`
- `skills/memory_manager/src/memcube.rs`

## 编译命令
```bash
cd skills/memory_manager
cargo build --release
```
