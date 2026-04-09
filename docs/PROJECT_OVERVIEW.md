# aion-forge v0.5.0 — 项目总览

## 一、项目定位

aion-forge 是一个 **Rust 实现的 MCP (Model Context Protocol) server**，为 Claude Code / Codex / Gemini 等 AI 编码助手提供 49 个工具能力，包括多引擎 AI 编排、任务路由、RAG 检索增强、规格驱动开发等。

## 二、运行环境

- 语言：**纯 Rust**（禁止其他语言）
- 操作系统：Windows 10
- 启动方式：`aion-cli.exe mcp-server`（通过 .mcp.json 配置）
- 用户地区：中国大陆（部分 Google 服务受限）
- AI 调用方式：**账号登录（非 API key）**，通过本地 CLI 工具调用三家引擎

## 三、三家 AI 引擎

| 引擎 | CLI 工具 | 认证方式 | 默认模型 |
|------|---------|---------|---------|
| Claude | `claude.cmd` | 账号登录 | claude-opus-4-5 |
| OpenAI | `codex.cmd` | 账号登录 | gpt-5.4 |
| Gemini | `gemini.cmd` | 账号登录 | gemini-2.5-pro |

调用方式：通过 stdin 管道传递 prompt，不经过 shell，无命令注入风险。

## 四、crate 结构

```
aion-types   — 数据结构与协议定义（无 IO，无 HTTP）
aion-memory  — 记忆存储管理
aion-intel   — AI 推断、规划、搜索（关键词匹配 + AI 分类 + 能力发现）
aion-router  — 技能路由、执行、协调核心（49 个 builtin）
aion-cli     — 命令行入口 + MCP server
aion-server  — HTTP REST API（axum）
```

## 五、49 个 MCP 工具一览

### 解析类（5）
yaml_parse, json_parse, toml_parse, csv_parse, pdf_parse

### 文本类（3）
text_diff, text_embed, markdown_render

### 网络类（3）
web_search, http_fetch, discovery_search

### 记忆类（4）
memory_remember, memory_recall, memory_distill, memory_team_share

### AI 类（1）
ai_task

### Agent 类（4）
agent_delegate, agent_broadcast, agent_gather, agent_status

### 管道类（2）
task_pipeline, task_race

### 新技能类（6）
echo, json_query, regex_match, code_lint, code_test, skill_report

### MCP 调用（1）
mcp_call

### RAG 检索增强（3）
rag_ingest, rag_query, rag_status

### 多模型编排（10）
ai_parallel_solve, ai_triple_vote, ai_triangle_review, ai_code_generate, ai_smart_collaborate, ai_research, ai_serial_optimize, ai_long_context, ai_cross_review, async_task_query

### 规格驱动开发（1）
spec_driven (analyze/decompose/plan/execute/status)

### AI 任务路由器（1）★ 新增
route_task

## 六、route_task — AI 任务路由器

### 功能
根据任务描述，自动选择最合适的 AI 引擎，返回可直接执行的 aion-forge 工具调用参数。

### 输入
```json
{
  "task": "原子任务自然语言描述",
  "hints": {
    "doc_size_pages": 150,
    "has_code": true
  }
}
```

### 输出（RouteDecision）
```json
{
  "rule_id": "code-agent-complex",
  "engine": "anthropic",
  "model": "claude-opus-4-5",
  "requires_external": false,
  "aion_tool": "ai_code_generate",
  "aion_params": {
    "primary": "claude",
    "reviewer": "openai",
    "language": "auto",
    "task": "重构认证模块"
  },
  "external_hint": null,
  "fallback_chain": ["openai", "gemini"],
  "access_ok": true,
  "conflict_note": null
}
```

### 三层触发机制

**第一层：结构特征快筛**
- 正则检测代码块（``` / .rs / .py / .js 等）
- 正则检测 URL（https://）
- hints.doc_size_pages > 80 → giant_doc

**第二层：关键词 × weight 匹配**
- 遍历 router.json 的 17 条规则
- 关键词命中后取规则的 weight 值（0-100）
- 结构特征加权：has_code+CODE 类 +5, giant_doc+giant 规则 +10, search_likely+SEARCH 类 +5
- 多条命中取最高 weight，同时记录 conflict_note
- 单字关键词（<2字符）自动忽略

**第三层：Passthrough 兜底**
- 前两层都不命中时，返回 passthrough 指令
- 包含所有 17 条规则的 rule_id + category + note + keywords_hint
- 宿主 LLM 从中选择 rule_id，再次调用 route_task(resolve_rule_id=xxx) 获取完整决策
- 宿主也选不出 → default-fallback (claude-sonnet-4-5 + ai_parallel_solve)

### 17 条路由规则

#### CODE 类（6 条）
| rule_id | 关键词示例 | 引擎 | 模型 | aion_tool |
|---------|-----------|------|------|-----------|
| code-agent-complex | 重构, 多文件, codebase | anthropic | claude-opus-4-5 | ai_code_generate |
| code-math | 算法题, 数学, AIME, leetcode | openai | o3 | ai_parallel_solve |
| code-daily | debug, fix, bug修复 | anthropic | claude-sonnet-4-5 | ai_code_generate |
| code-data | 数据分析, pandas, sql | openai | gpt-4.1 | ai_parallel_solve |
| code-optimize | 优化, 性能, benchmark | anthropic | claude-opus-4-5 | ai_serial_optimize |
| code-review | 代码审查, PR review | anthropic | claude-opus-4-5 | ai_triangle_review |

#### CREATIVE 类（4 条）
| rule_id | 关键词示例 | 引擎 | 模型 | 外部？ |
|---------|-----------|------|------|--------|
| creative-image-text | 图像生成, 海报, logo | openai | gpt-image-1.5 | 是 |
| creative-video-audio | 视频, 配音, 短片 | google | veo-3.1 | 是（中国受限）|
| creative-music | 音乐, 配乐, 作曲 | google | lyria-3 | 是（中国受限）|
| creative-long-text | 长文章, 深度报告, 万字 | anthropic | claude-opus-4-5 | 否 |

#### ANALYSIS 类（4 条）
| rule_id | 关键词示例 | 引擎 | 模型 | aion_tool |
|---------|-----------|------|------|-----------|
| analysis-giant-doc | 大文档, 合同, 整本书 | google | gemini-2.5-pro | ai_research(comprehensive) |
| analysis-multi-doc | 多文档, 知识库, 文献综合 | google | gemini-2.5-pro | ai_research(deep) |
| analysis-logic | 复杂推理, 逻辑分析, 合规 | anthropic | claude-opus-4-5 | ai_smart_collaborate |
| analysis-workspace | Google Docs, Gmail | google | gemini-2.5-pro | 外部 |

#### SEARCH 类（2 条）
| rule_id | 关键词示例 | 引擎 | 模型 | aion_tool |
|---------|-----------|------|------|-----------|
| search-realtime | 最新, 今天, 实时, 新闻 | google | gemini-2.5-flash | ai_research(quick) |
| search-internal | 内部文档, notion, slack | anthropic | claude-sonnet-4-5 | ai_parallel_solve |

#### VOICE 类（1 条）
| rule_id | 关键词示例 | 引擎 | 模型 | 外部？ |
|---------|-----------|------|------|--------|
| voice-realtime | 语音对话, voice chat | openai | gpt-4o-realtime | 是 |

### access_restricted 配置
```json
{
  "access_restricted": true,
  "restricted_services": {
    "google_external": true,
    "openai_realtime": false
  }
}
```
当 access_restricted=true 且规则的 engine 是受限服务，access_ok=false。

## 七、多模型编排 — 动态等待机制

### 改进前
```
spawn 后台任务 → 立刻返回 task_id → 客户端无法直接拿到结果
```

### 改进后
```
spawn 后台任务 → 等待 N 秒 → 完成则直接返回结果 / 超时则返回 task_id
```

### 动态超时策略
| 任务类型 | 默认等待 | 涉及工具 |
|---------|---------|---------|
| 单引擎 | 50 秒 | code_generate, long_context |
| 双引擎 | 55 秒 | cross_review, serial_optimize |
| 三引擎 | 55 秒 | parallel_solve, triple_vote, triangle_review, smart_collaborate, research |

注意：MCP 协议默认超时 60 秒，等待时间必须 < 60 秒。

可通过环境变量 `AION_ORCH_WAIT_SECS` 全局覆盖。

### 返回格式
等到了结果：
```json
{"type": "completed", "task_id": "orch_xxx", "status": "done", "result": {...}}
```

超时（任务仍在后台跑）：
```json
{"type": "async", "task_id": "orch_xxx", "status": "running", "waited_secs": 55, "hint": "使用 async_task_query 工具查询结果"}
```

## 八、关键文件清单

### 新增文件（v0.5.0）
| 文件 | 用途 |
|------|------|
| `router.json` | 17 条路由规则 + 全局 access 配置 |
| `aion-types/src/route_types.rs` | RouteRule, RouteDecision, Category 等数据结构 |
| `aion-router/src/builtins/task_router.rs` | route_task builtin（三层触发 + 模板渲染 + passthrough）|

### 修改文件（v0.5.0）
| 文件 | 改动 |
|------|------|
| `aion-types/src/lib.rs` | 添加 `pub mod route_types` |
| `aion-router/src/builtins/mod.rs` | 注册 RouteTaskBuiltin |
| `aion-types/src/capability_registry.rs` | 添加 route_task 能力定义 |
| `aion-intel/src/planner.rs` | 关键词表添加 route_task 入口 |
| `aion-router/src/builtins/orchestrator.rs` | 新增 `spawn_orchestration_with_wait()` 动态等待 |

## 九、.mcp.json 配置说明

```json
{
  "mcpServers": {
    "aion-forge": {
      "command": "D:/test/aionui/config/skills/aion-forge/bin/aion-cli.exe",
      "args": ["mcp-server"],
      "env": {
        "AI_PASSTHROUGH": "false",
        "CLAUDE_CLI": "C:/Users/Administrator/AppData/Roaming/npm/claude.cmd",
        "CODEX_CLI": "C:/Users/Administrator/AppData/Roaming/npm/codex.cmd",
        "GEMINI_CLI": "C:/Users/Administrator/AppData/Roaming/npm/gemini.cmd",
        "OPENAI_MODEL": "gpt-5.4",
        "REQUEST_TIMEOUT": "120"
      }
    }
  }
}
```

| 环境变量 | 说明 | 默认值 |
|---------|------|--------|
| AI_PASSTHROUGH | true=宿主处理, false=真实三引擎 | false |
| CLAUDE_CLI | Claude CLI 路径 | claude |
| CODEX_CLI | Codex CLI 路径 | codex |
| GEMINI_CLI | Gemini CLI 路径 | gemini |
| OPENAI_MODEL | OpenAI 默认模型 | gpt-5.4 |
| REQUEST_TIMEOUT | CLI 调用超时（秒） | 180 |
| AION_ORCH_WAIT_SECS | 动态等待覆盖（秒） | 按任务类型 |

## 十、编译与部署

```bash
# 编译
cargo build --release -p aion-cli

# 部署
cp target/release/aion-cli.exe /path/to/deploy/bin/
cp router.json /path/to/deploy/bin/

# 测试
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"route_task","arguments":{"task":"重构认证模块"}}}' | aion-cli.exe mcp-server
```

## 十一、版本历史

| 版本 | 日期 | 主要变更 |
|------|------|---------|
| v0.4.0 | 2026-03-20 | 初版：29 工具, Multi-Agent, HTTP API, MCP |
| v0.4.5 | 2026-03-28 | 安全修复 + 三引擎编排 + spec_driven + 48 工具 |
| v0.5.0 | 2026-03-28 | route_task 路由器 + 动态等待 + 49 工具 |
