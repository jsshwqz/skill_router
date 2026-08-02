# Forge 升级对比与执行计划（2026-08-01）

> 状态：**全部完成**。P1-P5 两轮执行完毕，全量测试通过。

## 一、功能 vs 现成库对比结论

### A. 解析/文本类 —— 手写实现 → 替换为成熟库

| 功能 | 当前实现 | 替换库（最新版） | 工作量 |
|------|----------|------------------|--------|
| `yaml_parse` | 手写 naive 行解析 `parsing.rs` L29-68 | `yaml-rust2` 0.11（serde_yaml 已 deprecated） | S |
| `toml_parse` | 手写 naive 行解析 `parsing.rs` L99-126 | `toml` 1.1（官方规范实现） | S |
| `csv_parse` | 手写 RFC4180 状态机 `parsing.rs` L133-161 | `csv` 1.4 | S |
| `pdf_parse` | 手写 stream 扫描 + flate2 `parsing.rs` L243-343 | `pdf-extract` 0.12（纯 Rust） | M |
| `markdown_render` | 手写 `#` 分节 `text.rs` L156-179 | `pulldown-cmark` 0.13.4（GFM 事件流） | S |
| `text_diff` | 手写 LCS O(n×m) `text.rs` L244-272 | `similar` 3.1 | S |
| `json_query` | 手写简易 JSONPath `new_skills.rs` L102-133 | `jsonpath-rust` 1.0.8 | S |

### B. 协议层

| 功能 | 当前实现 | 替换库 | 备注 |
|------|----------|--------|------|
| MCP 客户端/服务端 | 手写 JSON-RPC（`mcp_client.rs`、`mcp.rs`） | `rmcp` 3.1 | 一次解决协议 2025-11-25 + SSE；**重构量大，单独排期** |
| `code_lint` | 手写 ~6 条规则 | 增强：`cargo clippy` 子进程 | 排 P5 |

### C. 存储/检索层

| 功能 | 当前实现 | 替换库 | 备注 |
|------|----------|--------|------|
| `memory_*` | JSON 文件 + 关键词匹配 `memory.rs` | `redb` 4.1 嵌入式 KV | 单文件零运维 |
| `rag_*` | 手写暴力余弦 `rag.rs` | `usearch` 2.26（HNSW 嵌入式） | 万块级性能 |

### D. 保持手写（有理由）

- `text_embed`：离线词袋是特性（快、零依赖、无语义检索需求）
- `ai_*` 编排家族 / `orchestrator.rs`：业务核心，无现成等价物
- `agent_*` MessageBus：进程内总线，跨节点由 NATS 承担

## 二、执行分工（并行，按文件域不重叠）

| Agent | write_paths | 任务 |
|-------|-------------|------|
| Agent-A | `aion-router/` | parsing.rs（yaml/toml/csv/pdf）+ text.rs（diff/markdown）+ new_skills.rs（json_query）换库 + Cargo.toml 依赖 |
| Agent-B | `aion-memory/` | memory.rs 换 redb 存储（保留 JSON 兼容加载） |
| Agent-C | `aion-intel/` | rag.rs 换 usearch 检索（保留降级路径） |

### 硬约束

1. **只改上述域内文件**，不跨域修改（Cargo.toml 只动各自 crate 的依赖声明）
2. 每次替换后 `cargo check -p <crate>` 验证（若 cargo 不可用则**只写不验证**并标注）
3. 保留原输入/输出 JSON 结构（serde 兼容，`#[serde(default)]`）
4. 手写实现的降级路径保留（如 embedding API 失败时词袋降级）
5. 不加新功能、不重构无关代码、不加注释

## 三、分阶段排期（含本轮执行范围）

| 阶段 | 内容 | 本轮是否执行 |
|------|------|--------------|
| P1 速赢 | toml/csv 替换、thiserror 统一、aion-zl 补测试 | ✅（Agent-A 覆盖前两项） |
| P2 核心 | yaml/jsonpath/similar/pulldown-cmark/pdf-extract 替换 | ✅（Agent-A） |
| P3 协议 | rmcp 替换手写 MCP + SSE + NATS AgentFailover | ⏸ 单独排期 |
| P4 存储 | redb 记忆 + usearch RAG | ✅（Agent-B/C） |
| P5 安全 | fail-closed、clippy 增强、限流 | ⏸ 下一轮 |

## 四、自进化收口

执行完成后：`record_change`（每个改动文件）→ `record_decision` → `session_report`（被安全拦截时用 `memory_remember` 替代）→ 失败项列表留档。
