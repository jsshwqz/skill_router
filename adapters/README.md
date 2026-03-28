# Aion Forge — AI Platform Adapters / AI 平台适配器

This directory contains ready-to-use configuration files for integrating Aion Forge with various AI platforms.

本目录包含各 AI 平台的即用适配器配置文件。

---

## aionui

Copy `adapters/aionui/skill.json` into your aionui skills directory, or point aionui to this project root.

将 `adapters/aionui/skill.json` 复制到 aionui 的 skills 目录，或将 aionui 指向本项目根目录。

```
# Option A: Copy skill.json
cp adapters/aionui/skill.json ~/aionui/skills/aion-forge/skill.json

# Option B: Point to project directory (aionui auto-detects skill.json)
```

---

## Claude (MCP Protocol)

Aion Forge supports Claude's Model Context Protocol (MCP) via the `aion-cli mcp-server` command.

Aion Forge 通过 `aion-cli mcp-server` 命令支持 Claude 的 MCP 协议。

### Setup / 配置

1. Install Aion Forge (`install.sh` or `install.ps1`)
2. Add the following to your Claude Desktop config:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
**Linux**: `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "aion-forge": {
      "command": "aion-cli",
      "args": ["mcp-server"],
      "env": {
        "AI_BASE_URL": "http://localhost:11434/v1",
        "AI_MODEL": "qwen2.5:7b"
      }
    }
  }
}
```

3. Restart Claude Desktop
4. You should see Aion Forge tools (yaml_parse, web_search, code_generate, etc.) in Claude's tool list

---

## ChatGPT / OpenAI

Two integration methods:

两种集成方式：

### Method A: Custom GPT Actions (Recommended)

1. Start `aion-server` (locally or on a server with public URL)
2. In ChatGPT, create a Custom GPT
3. Go to "Configure" → "Actions" → "Import from URL"
4. Enter: `http://your-server:3000/v1/openapi.json` (or upload `adapters/http/openapi.json`)
5. ChatGPT will auto-discover all 29+ capabilities

### Method B: Function Calling (API users)

Use `adapters/openai/functions.json` as the `tools` array in your API calls:

```python
import json
tools = json.load(open("adapters/openai/functions.json"))
response = client.chat.completions.create(
    model="gpt-4",
    messages=[...],
    tools=tools
)
# When function_call received, forward to:
# POST http://localhost:3000/v1/route
# body: {"task": "yaml_parse: ...", "context": {...}}
```

---

## Generic HTTP / 通用 HTTP

Any tool or agent that supports HTTP can call Aion Forge directly.

任何支持 HTTP 的工具或 Agent 都可以直接调用 Aion Forge。

### Quick Start

```bash
# Start the server
aion-server

# List capabilities
curl http://localhost:3000/v1/capabilities

# Execute a task (natural language)
curl -X POST http://localhost:3000/v1/route \
  -H "Content-Type: application/json" \
  -d '{"task": "parse this yaml: name: test\nversion: 1.0"}'

# Execute a task (structured, Agent-to-Agent)
curl -X POST http://localhost:3000/v1/route/native \
  -H "Content-Type: application/json" \
  -d '{
    "intent": "yaml_parse",
    "capability": "yaml_parse",
    "parameters": {"text": "name: test\nversion: 1.0"}
  }'

# Health check
curl http://localhost:3000/v1/health
```

### OpenAPI Spec

Import `adapters/http/openapi.json` into Postman, Swagger UI, or any OpenAPI-compatible tool.

将 `adapters/http/openapi.json` 导入 Postman、Swagger UI 或任何支持 OpenAPI 的工具。

---

## Auto-generate / 自动生成

You can regenerate all adapter configs from the current capability registry:

```bash
aion-cli adapter generate --output ./adapters/

# Or generate specific format:
aion-cli adapter generate --format mcp --output ./adapters/
aion-cli adapter generate --format openai --output ./adapters/
aion-cli adapter generate --format openapi --output ./adapters/
aion-cli adapter generate --format aionui --output ./adapters/
```

This is useful when you add new capabilities — the adapter configs will be updated automatically.

当你新增能力后，运行此命令即可自动更新所有适配器配置。
