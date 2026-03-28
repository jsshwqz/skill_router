# Changelog

## v0.4.0 (2026-03-20)

### New Features
- **Multi-Agent System**: 4 workflow modes (serial, parallel, expert panel, competitive)
- **Distributed Architecture**: NATS message bus + JetStream KV capability registry
- **HTTP REST API**: aion-server with 10+ endpoints (Axum 0.7)
- **MCP Protocol**: `aion-cli mcp-server` for Claude Desktop integration
- **Adapter Generator**: `aion-cli adapter generate` for Claude/OpenAI/HTTP configs
- **WebSocket Events**: Real-time task lifecycle push via `/v1/stream/{session_id}`
- **Prometheus Metrics**: `skill_executions_total` + `skill_execution_duration_seconds`
- **CLI Progress**: spinner, progress bar, `--json`/`--quiet` output modes
- **Plugin System**: `BuiltinSkill` trait + 8 builtin modules
- **Security**: Fail-closed policy, IPv6 SSRF protection, delegation depth limit
- **Memory Namespaces**: global / team / private isolation
- **Docker**: Multi-stage Dockerfile + docker-compose (Ollama + NATS + aion-server)
- **Cross-platform Install**: `install.sh` (Mac/Linux) + `install.ps1` (Windows)
- **Safety Manifest**: `safety-manifest.json` for AI-assisted security review during install

### Built-in Capabilities (29+)
code_generate, code_lint, code_test, yaml_parse, json_parse, toml_parse, csv_parse,
pdf_parse, markdown_render, text_summarize, text_translate, text_classify, text_extract,
text_diff, text_embed, web_search, http_fetch, discovery_search, memory_remember,
memory_recall, memory_distill, memory_team_share, agent_delegate, agent_broadcast,
agent_gather, agent_status, task_pipeline, task_race, json_query, regex_match, echo

### Testing
- 53 unit tests across 4 crates (aion-types, aion-memory, aion-intel, aion-router)

---

## v0.3.0

- Initial skill router with basic capability registry
- Planner + Executor + Security Reviewer
- Memory management (remember, recall, distill)
- CLI interface
