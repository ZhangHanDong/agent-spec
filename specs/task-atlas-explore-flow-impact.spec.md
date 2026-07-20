spec: task
name: "Atlas Explore Flow Impact and Affected"
tags: [atlas, explore, flow, impact, affected, evaluation]
satisfies: [REQ-ATLAS-EXPLORE-FLOW-IMPACT]
depends: [task-atlas-agent-evaluation, task-atlas-edge-evidence-index, task-atlas-worktree-layered-freshness]
estimate: 2w
---

## Intent

在现有 schema v6 图和 derived query index 上增加确定性综合查询、flow、impact 与 affected
能力，使 Agent 一次获得有界且可解释的源码、路径和变更影响，同时保持 freshness、歧义、
truncation 与测试覆盖结论的诚实边界。

<!-- lint-ack: bdd-rule-grouping — explore、flow、impact、affected 与评测指标构成同一条 Agent 查询交付链 -->

## Decisions

- graph schema 保持 v6，不新增持久化图或数据库；`query-index.json` 仍是可重建索引，JSON
  shard 仍是正典图存储。
- `crates/rust-atlas/src/traversal.rs` 定义共享 `GraphPath`、`PathHop`、`FlowState`、
  `TraversalLimits` 和确定性 bounded traversal；`flow.rs`、`impact.rs`、`explore.rs` 与
  `affected.rs` 只组合共享入口。
- `compact` profile 固定为 8 seeds、32 nodes、48 edges、8 paths、4 excerpts、每段 20 行、
  16,000 serialized bytes；`deep` 固定为 16、96、160、20、12、40、24,000。
- 超预算时按 excerpts、alternative paths、off-spine edges、off-spine nodes 的固定顺序从尾部
  裁剪；seed、status、diagnostic 和主 spine 不得静默丢失，结果记录 limits、usage 与原因。
- 可选字段全部裁剪后，如果 seed、status、diagnostic 或主 spine 仍使结果超过 profile byte
  上限，则整个 query 返回 typed `atlas-explore-budget` error，不返回超限或缺少必需证据的 JSON。
- source excerpt 使用 repository-relative path；读取前逐文件比较当前 blake3 与 `Meta.files`，
  并拒绝 parent traversal、root escape 与 symlink escape。
- shortest path 按 hops、confidence cost、canonical path signature 排序；highest-confidence path
  按 confidence cost、hops、signature 排序。exact/implicit resolved、bounded-candidates、heuristic
  cost 分别为 0、10、100。
- from/to flow 默认 max depth 8、max expansions 2,000、max paths 8；`--through` 返回排序后的
  incoming-through-outgoing 两跳 spine，candidate target 通过 `chosen_target` 显式标注而不改写 edge。
- endpoint 在 traversal 前按 exact id/symbol、suffix candidate 的顺序解析；零个 candidate 为
  `unknown-endpoint`，多个为 `ambiguous-endpoint`。已找到 syn path 时即为 `found`；仅在 endpoint
  已解析、没有找到 path 且 SCIP 不可用时为 `capability-unavailable`，SCIP 可用且搜索完整时才为
  `no-path`；本阶段不以 MIR 可用性改变 flow verdict。
- impact depth 取值 1..=8、默认 3，默认 max nodes 200；container 成员以相同 distance 展开，
  leaf 不反向沿 `contains` 进入 parent 后再扩展 siblings。
- `atlas affected` 的 explicit paths、`--stdin`、`--staged`、`--worktree`、`--commit <range>`
  五种模式互斥；Git 使用 `Command` argv 调用并拒绝以 `-` 开头的 revision。
- affected 只返回 code nodes、distance 与 evidence path；本任务不得按 `_test.rs`、`tests/`、
  `#[test]` 名称或其他文件名模式生成 test selector。
- CLI 增加 `atlas explore|flow|impact|affected`；MCP 只增加由
  `AGENT_SPEC_MCP_ATLAS_EXPLORE=1` 开启的 frozen `atlas_explore`，默认 tools/list 不变。
- `atlas explore <QUERY>` 的 `--profile` 仅接受 `compact|deep` 且默认 compact；flow 只接受完整
  `--from/--to` 对或单独 `--through`；impact 要求一个 symbol；affected 必须且只能选择一种输入模式。
- `RunReceipt` 新增带 serde default 的 `response_bytes`、`read_back_calls`、
  `follow_up_queries`、`truncated_queries`，summary 对四项继续使用 median 与 MAD。

## Boundaries

### Allowed Changes
- crates/rust-atlas/**
- src/main.rs
- src/spec_mcp/**
- src/atlas_eval.rs
- fixtures/atlas/**
- benchmarks/atlas/**
- scripts/atlas-eval/**
- docs/atlas-roadmap.md
- docs/atlas-evaluation.md
- knowledge/requirements/req-atlas-explore-flow-impact.md
- specs/task-atlas-explore-flow-impact.spec.md
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- .agent-spec/wiki/**
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::explore
- rust-atlas: rust_atlas::traversal
- rust-atlas: rust_atlas::flow
- rust-atlas: rust_atlas::impact
- rust-atlas: rust_atlas::affected

### Forbidden
- 不调用 LLM、网络或 shell 字符串解释
- 不改变 graph schema v6 或把 derived index 提升为正典真相
- 不在 stale hash 下内联当前源码
- 不把 traversal truncation 报告为 no-path
- 不按测试文件名推断确定性 test coverage
- 不在真实 E1 A/B 结果前默认暴露 `atlas_explore`

## Out of Scope

- Intent-Code Linker 的 requirement/spec/scenario/test obligation 合并
- MIR extractor 与 dynamic-dispatch enricher
- daemon、watch 与 incremental hardening
- 非 Rust provider
- 自动选择或调用真实 Agent 产品

## Completion Criteria

场景: shared traversal types 驱动所有 query surface
  测试:
    过滤: test_atlas_query_surfaces_share_traversal_contract
    层级: unit
  假设 explore、flow、impact 与 affected 消费同一 query index
  当 四个 library result 序列化 path、edge evidence、status 与 stale mirror
  那么 含 path 的结果复用 `GraphPath` 与 `PathHop` 的同一 JSON shape
  并且 不存在 surface-local edge evidence 或 freshness 重解释

场景: explore 组合可解释的相关代码上下文
  测试:
    过滤: test_atlas_explore_composes_ranked_context_and_relationships
    层级: integration
  假设 query 同时命中 repository path、identifier、caller、callee、implementation 与 reverse dependent
  当 compact explore 在充分预算下运行
  那么 result 包含按固定 rank 排序的 seeds、source excerpts、完整 edges、主 spine、alternative paths 与 impact summary
  并且 每个 relationship 与 path hop 保留原始 evidence 且 repeated JSON byte-identical

场景: explore 输入歧义与无匹配结果保持确定性
  测试:
    过滤: test_atlas_explore_ranks_query_terms_and_reports_no_match
    层级: unit
  假设 query 含重复 identifier、repository path、标点、多个 suffix candidate 或完全未知 token
  当 tokenizer、path lookup 与 symbol lookup 生成 seeds
  那么 token first-occurrence、MatchKind 与 canonical node ordering 共同决定稳定 seed 顺序
  并且 未解析 query 返回 `atlas-explore-no-match` diagnostic 而不伪造 context

场景: compact 与 deep explore 具有稳定硬预算
  测试: test_atlas_explore_compact_and_deep_budgets_are_deterministic
  假设 relevant neighborhood 同时超过两个 profile 的部分限制
  当 相同 query 对每个 profile 各运行两次
  那么 每组 JSON byte-identical 且 serialized bytes 不超过对应上限
  并且 truncation reasons、limits 与 usage 精确一致

场景: budget pruning 遵循固定优先级
  测试:
    过滤: test_atlas_explore_prunes_optional_sections_in_fixed_order
    层级: unit
  假设 result 的 excerpts、alternative paths、off-spine edges 与 off-spine nodes 依次超过预算
  当 serializer 逐步收紧可用 bytes
  那么 四类内容严格按已定顺序从确定性尾部裁剪
  并且 seed、status、diagnostic 与主 spine 保持 byte-identical

场景: 必需 explore 证据自身超预算时整次查询失败
  测试: test_atlas_explore_rejects_unshrinkable_required_payload
  假设 seed、status、diagnostic 或主 spine 的必需序列化内容已超过 profile byte 上限
  当 所有可选 sections 已按固定优先级裁剪
  那么 query 返回 typed `atlas-explore-budget` error
  并且 不返回超限 JSON 或静默删除必需证据

场景: selected source stale 时不内联 excerpt
  测试: test_atlas_explore_omits_excerpt_when_selected_source_hash_is_stale
  假设 frozen graph 的 selected file 已被修改
  当 explore 组合 graph facts 与 source context
  那么 result 保留 stale graph node 并省略该文件 excerpt
  并且 diagnostic 命名 `atlas-excerpt-stale-source` 与 repository-relative file

场景: unsafe 或不可读 source 不内联 excerpt
  测试:
    过滤: test_atlas_explore_rejects_missing_and_escaping_excerpt_sources
    层级: integration
  假设 selected nodes 分别指向 missing file、out-of-root path 与 escaping symlink
  当 explore 尝试读取 source context
  那么 三类 source 均不出现在 excerpts
  并且 每项都返回排序稳定且包含 repository-relative file 的 typed diagnostic

场景: flow 同时返回 shortest 与 highest-confidence path
  测试:
    过滤: test_atlas_flow_returns_shortest_and_highest_confidence_paths
    层级: unit
  假设 一条两跳 bounded path 与一条三跳 exact path 同时存在
  当 from-to flow 在充分预算下运行
  那么 shortest 选择两跳路径且 highest-confidence 选择三跳 exact 路径
  并且 每一跳保留原始 edge evidence

场景: flow 区分 no-path unavailable 与 truncated
  测试:
    过滤: test_atlas_flow_distinguishes_no_path_unavailable_and_truncated
    层级: unit
  假设 三个 fixture 分别具有完整无路径图、缺失 required semantic capability 与耗尽 expansion 的图
  当 相同 endpoint query 运行
  那么 state 分别为 `no-path`、`capability-unavailable` 与 `truncated`
  并且 truncated 不得序列化为 no-path

场景: flow 在 traversal 前区分 endpoint 歧义且优先返回已找到路径
  测试:
    过滤: test_atlas_flow_handles_ambiguous_endpoints_and_syn_paths_without_scip
    层级: unit
  假设 三组 query 分别有零个 endpoint、多个 suffix candidates、SCIP unavailable 时的完整 syn path
  当 from-to flow 分别运行
  那么 三组依次为 `unknown-endpoint`、`ambiguous-endpoint` 与 `found`，歧义 candidates 保持排序
  并且 只有 endpoint 已解析、path 未找到且 SCIP unavailable 时才为 `capability-unavailable`

场景: through flow 保留 bounded candidate alternatives
  测试:
    过滤: test_atlas_flow_preserves_bounded_candidate_alternatives
    层级: unit
  假设 through symbol 的 outgoing edge 含两个可解析 candidate
  当 through flow 生成 spines
  那么 两个 candidate path 都按 canonical id 排序返回
  并且 path hop 用 chosen_target 标注选择而不把原 edge 改为 exact

场景: impact 返回最短 distance 与 evidence path
  测试:
    过滤: test_atlas_impact_returns_distance_and_explanation_paths
    层级: unit
  假设 多条 calls、references、uses-type 与 impl edge 汇入同一 dependent
  当 impact depth 为 3
  那么 每个 affected node 只出现一次并携带 minimum distance
  并且 其 evidence path 可连续回到 seed

场景: leaf impact 不产生 containment sibling explosion
  测试: test_atlas_impact_container_expansion_avoids_sibling_explosion
  假设 一个 container 含 changed leaf 与多个无关 sibling
  当 分别对 leaf 与 container 执行 impact
  那么 leaf result 不含无关 siblings
  并且 container result 以相同 dependency distance 包含其 members

场景: affected path spelling 归一化且拒绝 escape
  测试:
    过滤: test_atlas_affected_normalizes_repo_relative_dot_and_absolute_paths
    层级: integration
  假设 同一文件分别以 relative、`./` 与 in-root absolute path 输入
  当 affected 对三个输入运行
  那么 三个 provider-neutral result byte-identical
  并且 parent、out-of-root 与 symlink escape 返回 `atlas-affected-path` error

场景: affected 不从文件名推断 tests
  测试:
    过滤: test_atlas_affected_does_not_infer_tests_from_filenames
    层级: unit
  假设 impact neighborhood 包含 `tests/feature_test.rs` 与普通 source node
  当 affected 序列化结果
  那么 两者只按 graph distance 与 path 出现
  并且 result 不包含 inferred test selector 或 coverage verdict 字段

场景: affected CLI 拒绝冲突输入模式
  测试:
    过滤: test_atlas_affected_cli_rejects_conflicting_input_modes
    层级: integration
  假设 command 同时提供 explicit path 与 `--staged` 或 `--stdin` 与 `--commit`
  当 CLI 在读取 stdin 或调用 Git 前验证参数
  那么 返回 `atlas-affected-input-mode` error
  并且 不读取 stdin、不运行 Git 且 stdout 为空

场景: affected VCS 模式只执行固定 argv
  测试: test_atlas_affected_cli_covers_all_vcs_modes_and_failures
  假设 staged、worktree、commit、空 mode、option-like revision 与失败 Git fixture 分别输入
  当 CLI 解析 mode 并构造 Git request
  那么 三个合法 VCS mode 只产生已定 `git -C` argv 且不经过 shell
  并且 零 mode、以 `-` 开头的 revision 与 Git nonzero 分别返回 typed error 且 stdout 为空

场景: atlas query CLI grammar 固定 profile 与 endpoint 模式
  测试: test_atlas_explore_flow_impact_cli_parse_contract
  假设 explore、flow、impact 与 affected 的合法和非法参数组合
  当 clap parser 与 preflight validation 运行
  那么 explore profile 仅为 compact 或 deep、flow 仅为 paired from-to 或 through、impact 要求 symbol
  并且 affected 五种模式中恰好一种被选择后才允许 I/O

场景: 所有新 query surface 复用 worktree authority gate
  测试: test_atlas_query_surfaces_reject_worktree_mismatch
  假设 graph identity 来自另一个 worktree 且当前 code root 内容相同
  当 explore、flow、impact 与 affected 分别读取 query index
  那么 四个调用都返回现有 `atlas-worktree-mismatch` error
  并且 不返回 graph fact、source excerpt 或 partial result

场景: atlas_explore MCP 默认隐藏且 opt-in 同构
  测试: test_atlas_explore_mcp_is_hidden_by_default_and_opt_in
  假设 未设置或显式设置 `AGENT_SPEC_MCP_ATLAS_EXPLORE=1`
  当 tools/list 与 frozen dispatch 运行
  那么 默认列表不含该工具且 opt-in 列表含固定 schema
  并且 dispatch JSON 精确等于 library explore result

场景: evaluator 汇总查询 payload 与补查指标
  测试: test_atlas_eval_receipts_measure_explore_readback_and_response_bytes
  假设 paired receipts 包含 response bytes、read-back、follow-up 与 truncation count
  当 benchmark summarize 运行
  那么 aggregate 与 per-arm summary 为 `response_bytes`、`read_back_calls`、`follow_up_queries`、`truncated_queries` 生成 median 与 MAD
  并且 旧 receipt 缺失这些字段时按零读取而不是解析失败
