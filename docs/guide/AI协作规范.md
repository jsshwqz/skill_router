# Aion Forge AI 协作规范

## 角色分工

| 角色 | 职责 | 权限 |
|------|------|------|
| 开发者 (当前助手) | 修改源码、审查代码、编写文档、运行测试 | 可改源码，不可部署 |
| 部署者 (用户决策) | 编译、部署、打 tag、重启服务 | 唯一有权覆盖 bin/aion-cli.exe |

> 当前项目不再使用多 AI 引擎分工模式（GPT 写代码 + Claude 部署）。所有工作由当前 AI 助手完成，部署由用户确认。

## 源码规范

### 工作目录

```
D:\test\aionui\forge
```

### 编译命令

```bash
cargo build --release -p aion-cli
cargo build --release -p aion-server
```

## 部署规范

### 部署路径

```
D:\test\aionui\config\skills\aion-forge\bin\aion-cli.exe
```

### 部署流程

1. `cargo build --release -p aion-cli`
2. 杀掉旧进程：`Stop-Process -Name 'aion-cli' -Force`
3. 覆盖 exe 到目标路径
4. 更新 VERSION.json（版本号 + SHA256）
5. 重启 Claude Code

### 禁止操作

- **禁止在非本项目目录编译**
- **禁止使用 API key 做直接 HTTP 调用**
- **禁止覆盖未确认的部署路径**

## 配置规范

### 核心环境变量

| 变量 | 说明 |
|------|------|
| AI_PASSTHROUGH | true=passthrough 模式，false=直连 AI 后端 |
| AI_BASE_URL | OpenAI-compatible AI 后端地址 |
| AI_MODEL | 模型名称 |
| ANTHROPIC_BASE_URL | 宿主 Anthropic 代理地址（优先） |
| CLAUDE_CLI | Claude 包装脚本路径 |
| CODEX_CLI | Codex CLI 路径 |
| GEMINI_CLI | Gemini 包装脚本路径 |
| AI_PROVIDERS_DISABLED | 禁用的 provider 列表（逗号分隔） |
| AI_SECURITY_FAIL_POLICY | open（放行）/ closed（拒绝） |

### 不可添加的变量

- 任何 `*_API_KEY` 类变量用于直接 HTTP 调用
- `AION_MCP_MODE`（代码自动设置，不需要手动配）

## 版本管理

当前主分支：`fix/cleanup-and-passthrough-fix` | 最新版本见 CHANGELOG.md

### 版本验证

```bash
sha256sum D:/test/aionui/config/skills/aion-forge/bin/aion-cli.exe
cat D:/test/aionui/config/skills/aion-forge/bin/VERSION.json
```

两个 SHA256 必须一致。

## 已知限制

1. **三引擎真实调用受 MCP 60s 超时限制** — 当前用 passthrough 模式
2. **AI 文本工具（summarize/translate/classify/extract）依赖后端可用性**
3. **Windows 上大规模文件扫描可能超时**

---

## AionUI 扩展维护规范（2026-05-27 新增）

### 扩展目录
```
D:\test\aionui\forge\aionext-forge\  ← 扩展源文件
```

### 相关文件
| 文件 | 用途 |
|------|------|
| `aion-extension.json` | 扩展清单（引擎版本、贡献点引用） |
| `contributes/acp-adapters.json` | ACP 适配器定义（id: aion-forge） |
| `contributes/assistants.json` | 助手定义（presetAgentType: aion-forge） |
| `contributes/assistants/forge-context.md` | 助手上下文提示 |
| `assets/forge-icon.svg` | 助手图标 |

### ACP 服务器规范
- 可执行文件：`aion-forge-cli.exe`
- 启动命令：`aion-forge-cli.exe acp`
- 监听端口：52001
- API Key 配置：同级的 `.env` 文件
- 编译命令：`cargo build --release -p aion-forge-cli`

### 扩展部署流程
1. 编译 `aion-forge-cli.exe`
2. 确认 `aionext-forge/` 目录结构完整
3. 将 `aionext-forge/` 复制到 AionUI 用户数据目录的 `extensions/` 下
4. 重启 AionUI

### 扩展排错清单
- [ ] `aion-extension.json` 中 `engines.aionui` 版本范围正确（`^2.0.0`）
- [ ] ACP 适配器 `cliCommand` 含 `.exe` 后缀
- [ ] ACP 适配器 `acpArgs` 包含 `["acp"]`
- [ ] assistant 的 `presetAgentType` 与 adapter 的 `id` 匹配
- [ ] AionUI 扩展目录存在且文件完整
- [ ] API Key 已配置

### 已知问题
- **扩展自动识别失败** — 配置正确但仍不识别，可能原因：
  1. 扩展未拷贝到正确的 userData 目录
  2. aioncore 内部对扩展的 `engines` 版本校验有额外逻辑
  3. AionUI 需要完全重启（包括 aioncore 进程）
- **自定义智能体"无大模型可用"** — API Key 未配置到 AionUI 设置中
