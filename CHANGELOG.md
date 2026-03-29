# Changelog

## v0.5.2 (2026-03-29)

### New Features
- **route_task AI 任务路由器**: 三层触发（结构快筛→关键词×weight→passthrough兜底），17 条路由规则，5 类分类（CODE/CREATIVE/ANALYSIS/SEARCH/VOICE）
- **三模型协作框架**: proposal→dispute_review→execution_or_arbiter 三阶段协议
- **动态等待机制**: 编排工具启动后等待 50-55 秒拿结果，超时返回 task_id；MCP 模式自动截断 ≤55s
- **CLI 健康追踪**: .skill-router/cli_health.json 持久化引擎健康状态（成功率/延迟/连续失败/冷却）
- **编排审计轨迹**: .skill-router/orchestration_trace.jsonl 记录每次编排调用
- **统一错误分类**: auth_error/timeout/empty_output/model_not_found/process_error/safety_block
- **CLI 包装脚本**: claude_aion.cmd（清理 ANTHROPIC_* 污染变量+代理）、gemini_aion.cmd（代理+重试+清理模型变量）
- **SpaceNavigation builtin**: 实验性星际导航占位实现
- **Planner 关键词补全**: 从 25 条扩展到 49 条，覆盖全部 capability
- **49 个 MCP 工具**: 从 48 个增至 49 个（+route_task）

### Configuration
- `AI_PASSTHROUGH=false`: 真实三引擎模式（Claude+OpenAI+Gemini CLI）
- `AION_ORCH_WAIT_SECS`: 动态等待覆盖（MCP 模式自动截断 ≤55s）
- `AION_MCP_MODE`: 自动检测，MCP 启动时设为 1
- `CLAUDE_MODEL`: Claude CLI 模型配置（默认 sonnet）
- `HTTP_PROXY`/`HTTPS_PROXY`: MCP server 进程代理

### Bug Fixes
- 修复 EngineCallReport 缺失 output 字段的编译错误
- 修复单字关键词（"页""画"）误匹配，添加最小 2 字符过滤
- 修复 AI 兜底 rule_id 匹配不精确，改为精确匹配优先+最长包含
- 修复 MCP 超时：AION_ORCH_WAIT_SECS=130 在 MCP 模式下被截断到 55s

### Contributors
- Claude Opus 4.6: route_task 设计实现、动态等待、MCP 截断、关键词补全
- GPT-5.4: 三模型协作框架、健康追踪、审计轨迹、CLI 包装脚本、错误分类

## v0.4.5 (2026-03-28)

### Security Fixes (GPT-5.4 审计 + Claude Opus 4.6 验证修复)
- **mcp_call 命令注入**: server_name 白名单校验 + 直接 Command::new 不经 shell
- **orchestrator shell 注入**: prompt 全部通过 stdin 管道传入，不再拼入 cmd /c 命令行
- **builtin 路径沙箱**: 文件操作类 builtin（rag_ingest/pdf_parse 等）限制在 workspace 内
- **HTTP server 安全**: 默认绑定 127.0.0.1 + Bearer token 认证中间件 + CORS 受控
- **MCP isError 修复**: executor 返回 "ok" 但 MCP 判断 != "success" 导致永远报错
- **AI 失败状态修复**: 全 provider 失败时正确返回 Err 而非 Ok(json)
- **MessageBus 全局化**: agent builtin 共享全局总线，消息不再发给空气

### New Features
- **三引擎 Rust 编排器**: 9 个 AI 协作模式（parallel_solve/triple_vote/triangle_review/code_generate/smart_collaborate/research/serial_optimize/long_context/cross_review）
- **spec_driven 规格驱动开发**: 大型代码改造五阶段流水线（analyze→decompose→plan→execute→learn）
- **AI Passthrough 模式**: AI 类工具返回 instruction+input，宿主 LLM 直接处理
- **Echo builtin**: 真正实现的连通性测试工具
- **48 个 MCP 工具**: 从 29 个扩展到 48 个

### Bug Fixes
- MCP tracing 日志写 stderr 不污染 stdout JSON-RPC 通道
- UTF-8 安全截断（safe_truncate），中文字符不再 panic
- 子进程超时后 taskkill /T 终止进程树
- Codex CLI 参数修正（exec 而非 -p）
- Windows .cmd 后缀自动补全

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
