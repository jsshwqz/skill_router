# aion-forge AI 协作规范

## 角色分工

| 角色 | AI | 职责 | 权限 |
|------|-----|------|------|
| 编译部署 | Claude Opus 4.6 (CC) | 编译 Rust 源码、部署 exe、打 tag | 唯一有权覆盖 bin/aion-cli.exe |
| 代码审查 + 功能设计 | GPT-5.4 (Codex) | 审查代码、设计方案、编写文档 | 可改源码，不可直接部署 |
| 测试验证 | Gemini 3.1 | 出测试报告、验证功能 | 只读 |

## 源码规范

### 正确的源码目录
```
D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1
```

### 禁止使用的目录
- `skill_router` — 旧版项目，没有 mcp-server 子命令
- 任何其他未经确认的目录

### 编译命令
```bash
cd "D:\test\aionui\skill\新建文件夹\aion_forge_handoff_v2.1"
cargo build --release -p aion-cli
```

## 部署规范

### 部署路径
```
D:\test\aionui\config\skills\aion-forge\bin\aion-cli.exe
```

### 部署流程（只有 CC 执行）
1. `cargo build --release -p aion-cli`
2. 杀掉旧进程：`Stop-Process -Name 'aion-cli' -Force`
3. 覆盖 exe 到三个位置：bin/ + versions/ + 项目根目录
4. 更新 `bin/VERSION.json`（版本号 + SHA256）
5. 复制 router.json 到 bin/
6. 重启 Claude Code

### GPT 修改源码后的交接流程
1. GPT 修改源码文件
2. GPT 运行 `rustfmt --check` 验证格式
3. GPT 告诉 CC：改了哪些文件、改了什么
4. CC 检查改动、编译、部署、测试
5. CC 打 tag

### 禁止操作
- **GPT 禁止直接覆盖 bin/aion-cli.exe**
- **GPT 禁止从非 aion_forge_handoff_v2.1 目录编译**
- **任何 AI 禁止使用 API key 做直接 HTTP 调用**

## 配置规范

### .mcp.json 基础配置（不可修改的部分）
```json
{
  "command": "D:/test/aionui/config/skills/aion-forge/bin/aion-cli.exe",
  "args": ["mcp-server"]
}
```

### 可调整的环境变量
| 变量 | 当前值 | 说明 | 谁可以改 |
|------|--------|------|----------|
| AI_PASSTHROUGH | true | 稳定模式 | 用户决定 |
| AI_BASE_URL | 智谱 | AI 后端 | 用户决定 |
| AI_MODEL | glm-4.7-flash | 模型 | 用户决定 |
| CLAUDE_CLI | scripts/claude_aion.cmd | Claude 包装脚本 | CC |
| CODEX_CLI | codex.cmd | OpenAI CLI | CC |
| GEMINI_CLI | scripts/gemini_aion.cmd | Gemini 包装脚本 | CC |
| AION_ORCH_WAIT_SECS | 不设 | 动态等待覆盖 | CC |

### 不可添加的变量
- 任何 `*_API_KEY` 类变量用于直接 HTTP 调用
- `AION_MCP_MODE`（代码自动设置，不需要手动配）

## 版本管理

### 当前版本链
```
v0.4.5-pre-router → v0.5.0-route-task → v0.5.1-final → v0.5.2-tri-engine → v0.5.2 (正式)
```

### 版本验证
部署目录有 `VERSION.json`，包含版本号和 SHA256。
验证命令：
```bash
sha256sum D:/test/aionui/config/skills/aion-forge/bin/aion-cli.exe
cat D:/test/aionui/config/skills/aion-forge/bin/VERSION.json
```
两个 SHA256 必须一致。

## 已知限制

1. **三引擎真实调用受 MCP 60s 超时限制** — 已决定拆为独立技能包，当前用 passthrough 模式
2. **智谱 API 偶发限流** — AI 文本工具（summarize/translate/classify/extract）会临时失败，等限流解除即恢复
3. **hermes scan_project 在 Windows 上超时** — hermes 自身问题，不影响 aion-forge
