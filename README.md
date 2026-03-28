# Aion Forge — AI Universal Skill Pack / AI 万能技能包

29+ built-in capabilities. One-click install. Works with aionui, Claude, ChatGPT, and any HTTP client.

29+ 种内置能力。一键安装。支持 aionui、Claude、ChatGPT 及任意 HTTP 调用。

---

## Install / 安装

**Mac / Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/aioncore/aion-forge/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/aioncore/aion-forge/main/install.ps1 | iex
```

**Docker:**
```bash
docker compose up -d
```

**From source / 从源码构建:**
```bash
cargo build --release -p aion-cli -p aion-server
```

---

## Quick Start / 快速开始

```bash
# Run a task / 执行任务
aion-cli "parse this yaml: name: test"

# JSON output / JSON 输出
aion-cli --json "summarize: Rust is a systems language"

# Start HTTP API / 启动 HTTP API
aion-server
# -> http://localhost:3000/v1/health

# Generate adapter configs / 生成适配器配置
aion-cli adapter generate --output ./adapters/
```

---

## AI Platform Integration / AI 平台集成

| Platform | Method | Setup |
|----------|--------|-------|
| **aionui** | Skill directory | Copy `skill.json` to aionui skills folder |
| **Claude** | MCP Protocol | Add to `claude_desktop_config.json` (see below) |
| **ChatGPT** | Custom GPT Actions | Import `adapters/http/openapi.json` |
| **Any HTTP** | REST API | `POST http://localhost:3000/v1/route` |

### Claude MCP Setup

```json
{
  "mcpServers": {
    "aion-forge": {
      "command": "aion-cli",
      "args": ["mcp-server"]
    }
  }
}
```

See [adapters/README.md](adapters/README.md) for detailed instructions per platform.

---

## Built-in Capabilities / 内置能力

| Category | Skills |
|----------|--------|
| Code / 代码 | `code_generate`, `code_lint`, `code_test` |
| Parsing / 解析 | `yaml_parse`, `json_parse`, `toml_parse`, `csv_parse`, `pdf_parse`, `markdown_render` |
| Text / 文本 | `text_summarize`, `text_translate`, `text_classify`, `text_extract`, `text_diff`, `text_embed` |
| Search / 搜索 | `web_search`, `http_fetch`, `discovery_search` |
| Memory / 记忆 | `memory_remember`, `memory_recall`, `memory_distill`, `memory_team_share` |
| Multi-Agent | `agent_delegate`, `agent_broadcast`, `agent_gather`, `agent_status`, `task_pipeline`, `task_race` |
| Utility / 工具 | `json_query`, `regex_match`, `echo`, `image_describe` |

---

## Architecture / 架构

```
aion-types   -- Data structures & protocol definitions (no IO)
aion-memory  -- Memory storage with namespace isolation
aion-intel   -- AI inference, planning, web search
aion-router  -- Skill routing, execution, security, multi-agent coordination
aion-cli     -- CLI entry point + MCP server
aion-server  -- HTTP REST API (Axum) + WebSocket events
```

```
User / AI Agent
     |
     v
[aion-cli] or [aion-server:3000]
     |
     v
SkillRouter -> Planner -> Executor -> BuiltinSkill
     |                                    |
     v                                    v
CapabilityRegistry              AiSecurityReviewer
(29+ capabilities)              (pre + post review)
     |
     v
MemoryManager (namespace: global / team / private)
```

---

## HTTP API Endpoints / HTTP API 端点

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/health` | Health check |
| GET | `/v1/capabilities` | List all capabilities |
| GET | `/v1/metrics` | Prometheus metrics |
| POST | `/v1/route` | Natural language task routing |
| POST | `/v1/route/native` | Structured Agent-to-Agent |
| GET | `/v1/memory/recall?query=...` | Search memories |
| POST | `/v1/memory/remember` | Store a memory |
| GET | `/v1/memory/stats` | Memory statistics |
| GET | `/v1/agents` | Agent node info |
| POST | `/v1/agents/delegate` | Delegate to specific agent |
| GET | `/v1/stream/{session_id}` | WebSocket event stream |

---

## Security / 安全

Aion Forge includes a comprehensive security system:

- **Pre-execution review**: Blocks SSRF (private network URLs), detects sensitive fields, blocks unauthorized process execution
- **Post-execution review**: Detects API key leaks (sk-, AKIA, ghp_, glpat-), PEM private keys, .env content
- **Fail-closed policy**: When AI review is unavailable, high-risk operations are denied by default
- **Audit logging**: All security decisions logged to `security_audit.log`
- **Safety manifest**: `safety-manifest.json` lets AI agents verify the package before installation

```bash
# Set security policy
export AI_SECURITY_FAIL_POLICY=closed  # Production (default)
export AI_SECURITY_FAIL_POLICY=open    # Development
```

---

## Configuration / 配置

Edit `~/.aion/.env` (created by installer) or set environment variables:

```bash
# AI Backend
AI_BASE_URL=http://localhost:11434/v1   # Ollama (default)
AI_API_KEY=                             # API key (if needed)
AI_MODEL=qwen2.5:7b                    # Model name

# Web Search (optional)
SERPAPI_KEY=                            # serpapi.com API key

# Server
AION_HOST=0.0.0.0
AION_PORT=3000

# Security
AI_SECURITY_FAIL_POLICY=closed
```

---

## Troubleshooting / 故障排除

| Problem | Solution |
|---------|----------|
| `aion-cli: command not found` | Restart terminal or run `source ~/.bashrc` |
| Connection refused on port 3000 | Start `aion-server` first |
| AI tasks return errors | Check `AI_BASE_URL` points to a running LLM |
| Web search fails | Set `SERPAPI_KEY` in `~/.aion/.env` |
| Security blocks execution | Check `AI_SECURITY_FAIL_POLICY` setting |

---

## License

MIT
