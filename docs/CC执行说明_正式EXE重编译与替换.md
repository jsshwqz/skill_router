# CC 执行说明：正式 `aion-cli.exe` 重编译与替换

## 1. 背景

当前仓库里的源码修复已经完成，但这台机器缺少 Windows MSVC 链接环境，导致我**无法把源码重新编译进正式 `aion-cli.exe`**。

真实报错如下：

```text
error: linker `link.exe` not found
note: the msvc targets depend on the msvc linker but `link.exe` was not found
```

所以现在的状态是：

- 源码已经修好
- 当前项目工作区已经通过运行时兜底恢复可用
- 但**正式发布 exe** 还没有重新构建

这一步需要 CC 在具备 `link.exe` 的环境里执行。

---

## 2. 这次已经完成的源码修复

### 2.1 自进化 `space_navigation` 源码补口

已经修改：

- [aion-router/src/builtins/new_skills.rs](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/aion-router/src/builtins/new_skills.rs)
- [aion-router/src/builtins/mod.rs](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/aion-router/src/builtins/mod.rs)

目的：

- 修复 `CapabilityRegistry` 暴露了 `space_navigation`
- 但 `BuiltinRegistry` 没注册，导致真实 CLI 黑盒报：

```text
unknown builtin: 'space_navigation'
```

### 2.2 当前运行时兜底

已经新增本地兜底 skill：

- [skills/space_navigation/skill.json](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/skills/space_navigation/skill.json)
- [skills/space_navigation/README.md](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/skills/space_navigation/README.md)

这个兜底只用于**当前无法重编译正式 exe 时**，保证项目默认工作区可用。

---

## 3. CC 需要完成的最终目标

CC 接手后，需要完成下面 4 件事：

1. 在带 MSVC 的环境里重新构建正式 `aion-cli.exe`
2. 替换这两个交付 exe
3. 做真实黑盒回归
4. 如回归通过，可移除运行时兜底 skill，或保留为额外保险

---

## 4. 需要替换的两个 exe

### 目标 1：仓库根目录 exe

```text
D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1\aion-cli.exe
```

### 目标 2：主测交付 exe

```text
D:\test\aionui\config\skills\aion-forge\bin\aion-cli.exe
```

建议 CC 先备份：

```text
D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1\aion-cli.exe.<date>.bak
D:\test\aionui\config\skills\aion-forge\bin\aion-cli.exe.<date>.bak
```

---

## 5. CC 的执行步骤

### 步骤 1：进入具备 MSVC 的终端

CC 需要使用下面任意一种环境：

1. `x64 Native Tools Command Prompt for VS`
2. 先运行 `vcvars64.bat`
3. 或确认 `link.exe` 在 `PATH` 中可用

验证命令：

```powershell
where.exe link
cl
```

如果这两条都能返回有效结果，再继续。

---

### 步骤 2：在仓库根目录重新构建

工作目录：

```text
D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1
```

建议命令：

```powershell
cargo build -p aion-cli --bin aion-cli --release
```

如需连带验证：

```powershell
cargo check -p aion-router
cargo check -p aion-cli
```

产物通常在：

```text
target\release\aion-cli.exe
```

---

### 步骤 3：替换两个正式 exe

#### 3.1 替换仓库根目录 exe

```powershell
Copy-Item `
  'D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1\target\release\aion-cli.exe' `
  'D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1\aion-cli.exe' `
  -Force
```

#### 3.2 替换主测交付 exe

```powershell
Copy-Item `
  'D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1\target\release\aion-cli.exe' `
  'D:\test\aionui\config\skills\aion-forge\bin\aion-cli.exe' `
  -Force
```

---

## 6. CC 完成后必须跑的真实回归

### 6.1 直接工具调用 `space_navigation`

在仓库根目录执行：

```powershell
@'
{"jsonrpc":"2.0","id":101,"method":"tools/call","params":{"name":"space_navigation","arguments":{"destination":"Andromeda galaxy"}}}
'@ | .\aion-cli.exe mcp-server
```

预期：

```json
{
  "isError": false
}
```

不应再出现：

```text
unknown builtin: 'space_navigation'
```

---

### 6.2 自然语言自进化入口

```powershell
.\aion-cli.exe --json "navigate to the Andromeda galaxy"
```

预期：

```json
{
  "status": "ok",
  "skill": "space_navigation_placeholder" 或 "space_navigation",
  "result": {
    "capability": "space_navigation"
  }
}
```

关键是：

- `status = ok`
- 不再报 `unknown builtin`

---

### 6.3 新工作区黑盒

```powershell
$wd='D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1\work_dir\qa_self_evolution_final'
Remove-Item -Recurse -Force $wd -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $wd | Out-Null
.\aion-cli.exe --json --workdir $wd "navigate to the Andromeda galaxy"
```

预期：

- 直接通过
- **不依赖**再拷贝 `skills/space_navigation/skill.json`

这一条是最关键的“正式内建闭环”验证。

---

## 7. 如果 CC 重编译成功后，建议做的清理

如果 6.1 / 6.2 / 6.3 全部通过，说明正式 exe 已经内建了 `space_navigation` 的修复。

这时可以考虑：

### 方案 A：保留运行时兜底 skill

保留这两个文件作为额外保险：

- [skills/space_navigation/skill.json](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/skills/space_navigation/skill.json)
- [skills/space_navigation/README.md](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/skills/space_navigation/README.md)

优点：

- 更稳
- 即使用户切换旧 exe，也不容易再次黑盒失败

### 方案 B：移除运行时兜底 skill

如果希望保持系统更干净，且确认正式 exe 已经完全内建：

可以删除：

```text
skills/space_navigation/
```

但删除前必须先跑完 6.3。

---

## 8. 这次我已经实际改动过的文件

CC 可直接复核这些文件，不需要重新设计方案：

- [aion-router/src/builtins/new_skills.rs](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/aion-router/src/builtins/new_skills.rs)
- [aion-router/src/builtins/mod.rs](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/aion-router/src/builtins/mod.rs)
- [skills/space_navigation/skill.json](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/skills/space_navigation/skill.json)
- [skills/space_navigation/README.md](D:/test/aionui/skill/新建文件夹/aion_forge_handoff_v2.1/skills/space_navigation/README.md)

---

## 9. CC 需要知道的真实当前状态

### 已经真实通过的

- 三引擎并发编排
- 错误学习
- 长期记忆
- 上下文自动整理
- 长上下文处理
- 当前项目默认工作区下的 `space_navigation` 运行时兜底

### 还没彻底完成的唯一点

- **正式 exe 内建修复**还没完成
- 原因不是代码没写，而是当前机器缺：

```text
link.exe
```

---

## 10. 给 CC 的一句话任务描述

请在具备 MSVC `link.exe` 的 Windows 环境中：

1. 基于当前仓库源码重新构建 `aion-cli.exe`
2. 替换这两个正式 exe：
   - 仓库根目录 exe
   - `D:\test\aionui\config\skills\aion-forge\bin\aion-cli.exe`
3. 跑完第 6 节的三条真实回归
4. 将结果补回测试报告

