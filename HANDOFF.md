# Aion Forge — 项目状态交接文档

> 最后更新: 2026-07-03  
> 当前状态: ✅ P0+P1 完成，AI 引擎已接通

---

## 项目概览

| 项目 | 值 |
|------|-----|
| 路径 | `D:\test\aionui\forge\` |
| 语言 | Rust (edition 2024) |
| crate 数 | 10 |
| 二进制 | 5 (aion-forge-cli, aion-forge-server, forge-mcp-gateway, aion-proxy, aion-permission-bridge) |
| 编译状态 | ✅ 通过 (零错误，8 warnings 无害) |

---

## AI 引擎

| 引擎 | 状态 |
|------|------|
| Gitcode AI (Qwen3-235B-A22B) | ✅ **已接通** (免费、国内直连) |
| DeepSeek | ❌ 401 (Key 无效) |
| OpenRouter | ❌ Cloudflare 阻断 |
| 其余 | ⏸️ 已禁用 |

### 引擎配置 (`D:\test\aionui\forge\.env`)

```
AI_BASE_URL=https://api-ai.gitcode.com/v1
AI_API_KEY=<已配置>
AI_MODEL=Qwen/Qwen3-235B-A22B
AI_PROVIDERS_DISABLED=host-anthropic-proxy,opencode-zen,openrouter,openai-compatible,deepseek,zhipu-compatible,ollama-local
```

---

## 已完成任务

### P0-1: Glitch Token 过滤层

| 项目 | 详情 |
|------|------|
| 位置 | `crates/glitch-filter/` |
| 行数 | 1195 行 (`src/lib.rs`) |
| Token 数 | 200+ (25 个分类来自 L1B3RT4S) |
| 测试 | 19 个单元测试 |
| 核心 API | `GlitchFilter::check()`, `GlitchFilter::check_behavior()`, `token_count()` |

### P0-2: 系统提示词参考文档

| 项目 | 详情 |
|------|------|
| 位置 | `docs/reference/system-prompts-analysis.md` |
| 行数 | 520 行 |
| 内容 | 7 种控制面格式对比 + 4 种推理机制 + 3 种工具调用格式 + Forge 设计建议 |

### P1: 控制字符过滤 + CLI 集成

| 项目 | 详情 |
|------|------|
| Sanitizer | `glitch-filter::Sanitizer` |
| 防御覆盖 | NULL / DEL / C1 / 零宽 / Bidi / BOM / ANSI / `\r`洪水 |
| CLI 集成 | `aion-forge-cli/src/main.rs:13` — `use glitch_filter::{GlitchFilter, Sanitizer}` |

---

## 待办事项

| 阶段 | 任务 | 状态 |
|------|------|------|
| P2 | DeepSeek 攻击面研究 | ⏸️ 暂缓 (DeepSeek Key 401) |
| P3 | 其他 jailbreak 参考 | ⏸️ 暂缓 (价值较低) |
| — | 启动 AionUi 桌面端，通过 REST API 配置 Provider | 📋 待做 |
| — | 端到端 AI 调用测试 (通过 forge 二进制) | 📋 待做 |
| — | 连接更高质量模型 (Claude/GPT 等) | 📋 待做 |

---

## 构建指南

### 前置条件

1. Rust 工具链 (edition 2024)
2. 网络可访问 `https://index.crates.io/` (sparse 协议)

### 常见问题

**tuna 镜像阻断**: 全局 `~/.cargo/config.toml` 可能使用清华 tuna 镜像，该镜像在某些网络下不可达。解决：

```powershell
# 临时重命名全局配置
Move-Item ~\.cargo\config.toml ~\.cargo\config.toml.bak
cargo build
Move-Item ~\.cargo\config.toml.bak ~\.cargo\config.toml
```

### 构建命令

```bash
cd D:\test\aionui\forge
cargo build              # 全量构建 (首次 ~7 分钟)
cargo build -p glitch-filter  # 仅构建过滤层
cargo test -p glitch-filter    # 运行过滤层测试
```

### 运行

```bash
# 列出所有工具 (73 个)
.\target\debug\aion-forge-cli.exe -l

# 健康检查
.\target\debug\aion-forge-cli.exe -t health_check -q

# 工具调用
.\target\debug\aion-forge-cli.exe -t echo -p "hello" -q
```

---

## 关键文件索引

| 文件 | 用途 |
|------|------|
| `.env` | AI 引擎 + 服务配置 |
| `Cargo.toml` | workspace 定义 |
| `crates/glitch-filter/src/lib.rs` | GlitchFilter + Sanitizer |
| `aion-forge-cli/src/main.rs` | CLI 入口 (已集成过滤) |
| `aion-router/src/config.rs` | 配置加载 + AI 端点 fallback 链 |
| `aion-router/src/builtins/ai.rs` | AI 调用实现 |
| `docs/reference/system-prompts-analysis.md` | 各厂商系统提示词分析 |
| `.cargo/config.toml` | 项目级 cargo 配置 (sparse 协议) |
