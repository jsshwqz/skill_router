# Forge 重构实施报告

## 文档元信息

- **分析范围**：按 `docs/refactor/04-final-design.md` 执行阶段 0A 至 0C，并按用户授权从 `d19866a` 恢复 `aion-intel/src/rag.rs` 后以 TDD 重建 BM25/KeywordExtractor/Reranker；阶段 0C 在严格 clippy 失败后停止。
- **已读取的代码和文档**：`docs/refactor/01-current-state.md`、`02-refactor-proposal.md`、`03-adversarial-review.md`、`04-final-design.md`；`aion-intel/src/rag.rs` 当前损坏版本、`rag.rs.bak`、提交 `d19866a` 的可信版本及提交 `9ba54ed` 的差异；当前 Git 状态与相关构建输出。
- **未读取或无法确认的内容**：未逐项读取或执行其余 77 个能力；未连接真实 AionUI、OmniRoute、NATS 或第三方 provider；未执行全 workspace 测试、clippy 或构建；未确认共享工作目录中其他并发提交的正确性；`session_report` 输出被安全审查拦截。
- **结论的证据**：损坏文件与备份 hash 均为 `d7a80ce640a62359dd6d6954042cc6e2a840a9bc`；恢复基线 hash 为 `c2823f7f2923f03823b4ed5b75c0cc286d38886b`；恢复前 `cargo check -p aion-intel` 报非法 UTF-8，恢复后检查成功；新增测试先因缺少 `KeywordExtractor`/`LinearReranker` 编译失败，实施后 `aion-intel` 27 项库测试通过。
- **当前置信度**：阶段 0A 编码阻断已解除为高（0.98）；新增关键词与重排 API 的单元行为为高（0.93）；其生产集成收益为低（0.35），因为当前没有将 reranker 接入 `RagEngine::search`；整个重构完成度为低（0.20）。

## 已完成阶段

### 阶段 0A：UTF-8 阻断恢复

- 从提交 `d19866a` 恢复 `aion-intel/src/rag.rs` 的可信 UTF-8 基线。
- 恢复中文文档、字符串、字符内容和合法的 `RagEngine` 初始化结构。
- 未修改持久数据、外部服务、配置或其他源码文件。

回滚方式：将 `aion-intel/src/rag.rs` 恢复为实施前 blob `d7a80ce`；该 blob 可复现非法 UTF-8 阻断，仅用于精确回滚证据，不建议作为可构建状态。

### 用户授权追加阶段：BM25/KeywordExtractor TDD 重建

- 新增 `KeywordExtractor`：规范化、停用词过滤、频次统计和确定性排序。
- 新增 BM25-like 词法相关度评分。
- 新增 `Reranker` trait 与 `LinearReranker`，支持语义分数和关键词分数线性组合、权重钳制、稳定排序与 `top_k` 截断。
- 保留现有 `RagEngine::search` 行为；未擅自把 reranker 接入生产检索路径。

回滚方式：恢复 `aion-intel/src/rag.rs` 到 `d19866a` 内容。

## 修改文件和核心变化

| 文件 | 变化 |
|---|---|
| `aion-intel/src/rag.rs` | 恢复可信 UTF-8 基线；重建关键词提取、BM25-like 评分、线性 reranker；新增 3 项单元测试 |
| `docs/refactor/05-implementation-report.md` | 记录阶段结果、证据、偏差、风险与后续行动 |

## 测试命令及结果

| 命令 | 结果 |
|---|---|
| `cargo check -p aion-intel`（恢复前） | 失败：`rag.rs` 非法 UTF-8，字节 `[229, 133]` |
| `rustc --crate-name rag_backup_utf8_check --crate-type lib aion-intel/src/rag.rs.bak --emit metadata` | 失败：备份同样为非法 UTF-8 |
| `cargo check -p aion-intel`（恢复后） | 通过，约 16.44 秒 |
| `cargo fmt --all -- --check` | 失败：已能解析 `rag.rs`，但 workspace 存在大量既有格式差异；未自动格式化并发修改 |
| `cargo test -p aion-intel keyword_extractor_counts_terms_and_filters_stop_words --no-fail-fast`（首次 RED） | 69 秒超时，无诊断；不计为有效 RED |
| 同一命令重跑（RED） | 预期失败：缺少 `KeywordExtractor` 与 `LinearReranker` |
| 3 项新增定向测试 | 全部通过，各 1 项通过、26 项过滤 |
| `cargo test -p aion-intel --lib` | 27 项通过，0 项失败 |
| `rustfmt --edition 2021 --check aion-intel/src/rag.rs` | 通过 |
| 最终 `cargo check -p aion-intel` | 通过，约 3.94 秒 |
| 最终 `cargo test -p aion-intel --lib` | 27 项通过，0 项失败，约 0.04 秒测试时间 |
| `git diff --check` | 通过 |

## 与设计的偏差

- `04-final-design.md` 的阶段 0A 原要求仅恢复编码、不改变可执行内容；损坏已覆盖字符串、字符字面量和结构初始化，无法满足该限定。用户明确选择基于 `d19866a` 恢复并重新实现 BM25/KeywordExtractor 后才继续。
- 原提交 `9ba54ed` 尝试把关键词提取器和 reranker 字段加入 `RagEngine`，但对应结构字段不完整且生产检索路径未使用它们。本次仅提供独立、可测试 API，不推断新的生产集成语义。
- 未按阶段 0B 全量运行 rustfmt，因为计划列出的文件与共享工作区并发修改重叠，自动格式化会扩大范围。

## 未完成项

- 阶段 0C：零 warning 与 clippy 门禁（已执行，未通过）。
- 阶段 0D：CI 结果可信化。
- 阶段 1：四入口 characterization tests。
- 阶段 2：E1-E6 独立实验。
- 阶段 3：关闭式决策门。
- `LinearReranker` 尚未接入 `RagEngine::search` 或 `query`，其生产效果未验证。
- 未执行 workspace 全量构建、测试或 clippy。

## 已知风险

- 共享工作目录的 HEAD 在实施期间多次由外部流程推进；本报告只对当前 `rag.rs` diff 和本轮命令负责。
- BM25 实现是轻量近似评分，没有语料级 IDF 和全库平均文档长度，不应描述为完整标准 BM25。
- 中文文本没有专用分词，仅按空白切词；中文连续文本的关键词召回未验证。
- reranker 返回按组合分数排序的原始结果，但保留原始语义 `score` 字段，调用方不能把该字段误认为组合分数。

## 建议的后续行动

1. 固定共享工作区基线，确认没有并发写入后再实施阶段 0B。
2. 为中文分词、空查询、`top_k = 0`、非有限语义分数和 alpha 边界补充测试，再决定是否接入生产检索。
3. 若要接入 `RagEngine`，先明确输出 `score` 的兼容语义并新增 characterization test，不直接改变当前搜索结果外形。
4. 完成阶段 0B 后依次执行 workspace check、clippy 和离线测试，并把真实耗时与失败项追加到本报告。

## 阶段 0B 与并发协作说明

- `cargo fmt --all` 与随后的 `cargo fmt --all -- --check` 执行成功。
- 用户确认另一 AI 对 Q6/Q7 和共享工作区的并发修改属于授权协作，因此不要求拆分或回滚 `2e8e3da`。
- `2e8e3da` 同时包含 UTF-8/BM25、workspace 格式化和 Q6/Q7，审查与回滚粒度较粗；这是已接受的协作边界，不再作为阶段 0C 的阻断项。
- **建议修复**：后续提交信息应覆盖实际变化，或在实施报告中保留各 Agent 的独立证据链。
- **可接受风险**：轻量 BM25 不含语料级 IDF、中文连续文本无专用分词、reranker 尚未接入生产检索；这些均已限制在独立 API，当前不改变 `RagEngine::search` 行为。
- **阶段结论**：阶段 0A 与 BM25 单元行为有通过证据；阶段 0B 格式门通过；继续进入阶段 0C 审查。

## 阶段 0C：workspace check 与严格 clippy

- `cargo check --workspace` 通过，但 `AiSmartCollaborate` 产生 1 个 deprecated warning。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 失败：14 个错误。
- 错误分布：`discovery_radar.rs` 1 项、`embedding.rs` 4 项、`planner.rs` 5 项、`rag.rs` 3 项、`synth.rs` 1 项；规则为 11 项 collapsible-if、2 项 needless borrow、1 项 new-without-default。
- 新鲜诊断超出最终设计为阶段 0C 批准的 `orchestrator.rs` 单文件范围，因此按停止条件未自动扩大修改。
- **必须修复**：决定是否把上述 5 个 `aion-intel` 文件纳入阶段 0C；未授权前严格 clippy 门禁不通过。
- **建议修复**：若批准扩大范围，先为涉及控制流的折叠修改运行现有单元测试，再重新执行 workspace check 与严格 clippy。
- **可接受风险**：这些诊断当前均为静态质量项，没有证据表明运行时功能已经回归；但 `-D warnings` 门禁仍然失败。
- **最终结论**：阶段 0C 未通过，阶段 0D 及后续阶段保持暂停，不能声明重构完成。
