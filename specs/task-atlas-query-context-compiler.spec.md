spec: task
name: "Atlas Query Context Compiler"
tags: [atlas, query, context, projection, dogfood]
satisfies: [REQ-ATLAS-QUERY-CONTEXT-COMPILER]
depends: [task-atlas-explore-flow-impact, task-atlas-query-quality-regression, task-atlas-runtime-boundary-hints]
estimate: 4d
---

## Intent

把 Atlas 图检索和 Agent 上下文投影拆成两个确定性阶段，使 symbol、flow、architecture 与
impact 查询在明确预算内保留关键证据，并能解释遗漏来自 retrieval 还是 projection。

<!-- lint-ack: bdd-rule-grouping - parser、retrieval、projection 和 receipt 构成同一个 context compiler pipeline -->

## Decisions

- 新增 library API `parse_query_intent`、`retrieve_context`、`project_context` 和
  `compile_context`；`atlas context <query> --profile <profile>` 是加性 CLI 入口。
- profile 固定为 `symbol|flow|architecture|impact`，默认仅限新命令的 `symbol`；不改变
  `atlas explore` 的 `compact|deep` 语义，也不新增默认 MCP tool。
- retrieval 扫描当前 immutable `QueryIndex`，用固定 hard cap 防御资源耗尽；每个 candidate
  保存 stable id、class、score、scoring reasons 和 required 标志。
- relevance threshold 在 byte projection 之前。required evidence 不参与可选裁剪；无法容纳时
  返回 `atlas-context-required-budget`。
- continuation 由 `--after <evidence-id>` 与 `--expect-graph <fingerprint>` 构成，排序只依赖
  score、evidence class、canonical symbol/path/site 和 evidence id。
- receipt 分为 retrieval 与 projection 两段，并携带可供 D4 使用的 deterministic load profile。
- source body 只能来自 graph hash 验证后的 symbol/edge-site line slice；stale source 只返回
  typed diagnostic，不回退到未验证全文。
- test、generated 和 vendored source 仅在 query 点名 repository path/symbol 或位于 primary
  flow、impact 或 runtime-boundary spine 时进入正文；其他情况仅保留带 provenance 的
  signature skeleton。

## Boundaries

### Allowed Changes
- crates/rust-atlas/src/**
- src/main.rs
- src/atlas_eval.rs
- atlas-eval/query-corpus-v1/**
- fixtures/atlas/context-compiler/**
- docs/atlas-query-context.md
- docs/atlas-roadmap.md
- docs/atlas-evaluation.md
- docs/superpowers/specs/2026-07-21-atlas-query-context-compiler-design.md
- docs/superpowers/plans/2026-07-21-atlas-query-context-compiler.md
- knowledge/requirements/req-atlas-query-context-compiler.md
- specs/task-atlas-query-context-compiler.spec.md
- .agent-spec/wiki/**
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::context::compile_context

### Forbidden
- 不调用 LLM，不从自由文本猜测或写回 graph/KLL intent
- 不把 retrieval truncation、projection truncation、stale 或 capability unavailable 表示为空成功
- 不在 continuation 中使用隐藏游标、进程内状态或变化中的 graph fingerprint
- 不裁剪 named symbol、primary spine、boundary site、failure evidence 或唯一实现
- 不改动现有 explore 输出 schema、默认 MCP tool list 或 MCP 默认行为

## Out of Scope

- E1 真实 Agent A/B 与默认 MCP surface 晋升
- D4 worker pool、transport isolation 或 backpressure
- 新 extraction provider、framework pack 或非 Rust 语言
- 自然语言问答、LLM reranker 或自动 prompt generation

## Completion Criteria

Scenario: intent parser 只接受确定性结构
  Test:
    Filter: test_atlas_context_intent_parses_identifiers_paths_relations_and_profiles
    Level: unit
  Given identifier、repository path、known relation 和显式 profile 混合 token
  When parser 构造 QueryIntent
  Then 字段按 first-seen order 去重且未知 token 只作为未解释 token diagnostic

Scenario: retrieval 保留候选和评分理由
  Test:
    Filter: test_atlas_context_retrieval_returns_scored_candidate_supergraph
    Level: integration
  Given exact symbol、caller、callee、implementation、alternative path 与 sibling candidates
  When retrieval 扫描 QueryIndex
  Then 每个 candidate 有 stable id、evidence class、score、reason 且 retrieval 尚未应用 byte cap

Scenario: 四种 profile 的 priority 与预算确定
  Test:
    Filter: test_atlas_context_profiles_have_deterministic_priority_and_limits
    Level: unit
  Given symbol、flow、architecture 与 impact profile
  When 生成 EvidencePriorityPlan
  Then threshold、limits、class rank 和 tie-break 在重复运行间逐字节相同

Scenario: relevance gate 先于 byte ceiling
  Test:
    Filter: test_atlas_context_relevance_gate_precedes_byte_budget
    Level: unit
  Given 高低分 candidates 且高分投影未填满 ceiling
  When 执行 projection
  Then 低分 candidate 仍按 below-relevance omission 排除

Scenario: source projection 围绕 hash-verified span
  Test:
    Filter: test_atlas_context_projects_verified_symbol_and_edge_site_spans
    Level: integration
  Given named symbol span、edge site 与匹配 graph hash
  When source evidence 进入正文
  Then slice 覆盖目标行、保持 file/hash/line provenance 且不读取整文件正文

Scenario: stale source 不回退未验证正文
  Test:
    Filter: test_atlas_context_stale_source_is_typed_and_never_projected
    Level: integration
  Given graph 后 source bytes 已变化
  When compiler 请求该 span
  Then source body 被拒绝、stale diagnostic 可机读且 graph node skeleton 仍标明 provenance

Scenario: restricted source body 需要点名或 spine 准入
  Test:
    Filter: test_atlas_context_restricted_source_requires_name_or_spine
    Level: integration
  Given 生产 symbol 的相邻 test、generated 和 vendored source candidates
  When 查询未点名这些 repository path 且 candidate 不在证据 spine
  Then source body 不进入正文、signature skeleton 保留 provenance 且 receipt 记录 policy skeleton count

Scenario: required evidence 不被 byte pruning 删除
  Test:
    Filter: test_atlas_context_byte_pruning_preserves_required_evidence
    Level: unit
  Given named symbol、primary spine、boundary site 和大量 optional siblings
  When serialized output 接近 byte ceiling
  Then required evidence 全部保留且只裁剪 optional sibling bodies

Scenario: required evidence 超预算明确失败
  Test:
    Filter: test_atlas_context_required_evidence_overflow_is_typed
    Level: unit
  Given required evidence 自身超过 profile ceiling
  When compiler 无法生成完整 required projection
  Then 返回 atlas-context-required-budget 且不输出 partial success

Scenario: omission manifest 完整且 continuation 可执行
  Test:
    Filter: test_atlas_context_omission_manifest_has_stable_continuations
    Level: integration
  Given relevance 与 byte cap 都产生 omissions
  When result serialization 完成
  Then 每类 omission 有 count、reason、highest candidate 和包含 after/expect-graph 的 argv

Scenario: continuation 拒绝不同 graph generation
  Test:
    Filter: test_atlas_context_continuation_rejects_graph_fingerprint_change
    Level: integration
  Given 旧 result 的 graph fingerprint 和 evidence id
  When continuation 在新 fingerprint 上执行
  Then 返回 typed graph mismatch 而不是从不同排序位置继续

Scenario: receipt 区分 retrieval 与 projection loss
  Test:
    Filter: test_atlas_context_receipt_separates_retrieval_and_projection_loss
    Level: unit
  Given retrieval hard cap、relevance omission 与 byte omission
  When receipt 生成
  Then coverage、retention、bytes、truncated classes、read-back、follow-up 与 load profile 分栏记录

Scenario: CLI 输出与 library finalized bytes 一致
  Test:
    Filter: test_atlas_context_cli_emits_finalized_json_and_continuation_contract
    Level: integration
  Given fixture graph 和显式 flow profile
  When atlas context 输出 JSON
  Then stdout 与 library result 加换行逐字节一致且 continuation argv 可被 clap 重新解析

Scenario: 旧 explore 与 MCP discovery 保持不变
  Test:
    Filter: test_atlas_context_is_additive_to_explore_and_mcp_discovery
    Level: integration
  Given B5 implementation
  When 运行原 explore golden 和 MCP tools/list
  Then 原 schema、bytes 与默认 tool names 不变且 context 未出现在默认 MCP surface

Scenario: B5 固定 corpus regression 通过
  Test:
    Filter: test_atlas_context_compiler_checked_in_regression_receipt_is_passing
    Level: integration
  Given parser、symbol、flow、architecture、impact、stale 与 truncation cases
  When E3 scorer 重放 checked-in observation
  Then expected/forbidden evidence、diagnostic、path 和 projection receipt 全部通过
