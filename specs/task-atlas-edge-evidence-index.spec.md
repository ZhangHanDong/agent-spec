spec: task
name: "Atlas Edge Evidence and Query Index"
tags: [atlas, code-graph, evidence, search, index]
satisfies: [REQ-ATLAS-EDGE-EVIDENCE-INDEX]
depends: [task-atlas-scip-semantic]
estimate: 1w
---

## Intent

把 Atlas edge 升级为可解释事实，并增加可重建查询索引与确定性 symbol search。SCIP
occurrence 的 site、分析器证据、dispatch 与 confidence 必须保留下来；query/search 不再
为每次邻接查询扫描所有 shard，同时 JSON shard 继续作为正典图存储。

<!-- lint-ack: bdd-rule-grouping — edge evidence、query index 与 search 按同一交付链线性验收 -->

## Decisions

- `SCHEMA_VERSION` 从 5 增至 6；旧版本沿用 `SchemaMismatch` 响亮拒绝并提示 rebuild。
- `Edge` 新增带 `#[serde(default)]` 的 `site: Option<EdgeSite>`、
  `extractor: Option<ExtractorIdentity>`、`dispatch: Option<DispatchKind>`、
  `confidence: Option<EdgeConfidence>`、`candidates: Vec<String>`、
  `evidence: Option<String>`；三个枚举/结构使用 kebab-case JSON。
- `EdgeSite` 使用 repository-relative file 与 1-based line/column；SCIP range 的
  0-based line/column 在边界处转换一次。
- `validate_graph` 拒绝 `candidates.len() > 1 && confidence == exact`；edge 的派生 Ord/去重
  包含 site，保留同 caller/target 的不同 occurrence。
- build 在 graph invariant 通过后原子写入 `.agent-spec/graph/query-index.json`；index 包含
  graph fingerprint、node table、edge table、id/symbol/file 与 incoming/outgoing locator。
- `load_query_index` 要求 schema 与 graph fingerprint 同时匹配；missing/mismatch 返回
  `atlas-query-index-*` rebuild diagnostic，不静默回退为全 shard scan。
- search ranking 固定为 exact id、exact symbol、case-insensitive exact、qualified suffix、
  segmented identifier、normalized substring；每层再按 symbol、file、line、id 排序。
- 新增 library `search`、CLI `atlas search`、MCP `atlas_search`；`limit` 取值 1..=200，默认
  20，JSON 结果包含 match kind、score、node、graph fingerprint 与 stale 列表。

## Boundaries

### Allowed Changes
- crates/rust-atlas/**
- src/main.rs
- src/spec_mcp/**
- fixtures/atlas/**
- knowledge/requirements/req-atlas-edge-evidence-index.md
- specs/task-atlas-edge-evidence-index.spec.md
- docs/atlas-roadmap.md
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::index

### Forbidden
- 不增加第四种 provenance
- 不把 SQLite 或其他新存储依赖作为未经 benchmark 的正典存储
- 不删除或就地改写既有 syn edge
- 不在 query 中调用 LLM 或网络
- 不用 stale/mismatched index 返回搜索结果

## Out of Scope

- `atlas explore`、flow、impact、affected
- MIR extraction
- daemon/watch
- 非 Rust provider

## Completion Criteria

场景: SCIP edge 保留 occurrence site 与 evidence
  测试: test_scip_calls_preserve_occurrence_site_and_evidence
  假设 SCIP call occurrence 位于 fixture 的已知 range
  当 overlay 生成 `calls` edge
  那么 site 使用 repository-relative file 与转换后的 1-based range
  并且 extractor、confidence 与 evidence 可从 JSON 查询结果读取

场景: build 原子生成当前 query index
  测试: test_atlas_build_writes_current_query_index
  假设 一个可解析 Rust fixture
  当 Atlas build 完成
  那么 `query-index.json` 的 schema 与 graph fingerprint 匹配 meta
  并且 id、symbol、file、incoming、outgoing lookup 都含 fixture 事实

场景: search 排序与 JSON 输出稳定
  测试: test_atlas_search_orders_exact_suffix_segment_and_fuzzy_matches
  假设 图中存在 exact、qualified suffix、segmented 与 normalized substring candidate
  当 相同 search 运行两次
  那么 两次序列化 JSON byte-identical
  并且 candidate 按固定 match strength 和 tie-break 顺序返回

场景: 多 candidate edge 不能标 exact
  测试: test_atlas_rejects_exact_confidence_with_multiple_candidates
  假设 一条 edge 含两个 candidate 且 confidence 为 exact
  当 `validate_graph` 检查该 shard
  那么 返回 `atlas-invariant` error
  并且 error 命名 confidence 与 candidate 数量

场景: stale query index 被拒绝
  测试: test_atlas_search_propagates_index_errors_without_shard_fallback
  假设 `query-index.json` 的 graph fingerprint 被修改
  当 search 加载 index
  那么 返回 `atlas-query-index-stale` diagnostic
  并且 不返回 candidate

场景: schema v5 图不能被 schema v6 查询半读
  测试: test_atlas_rejects_mismatched_schema_version
  假设 meta schema version 为 5
  当 query 或 search 加载图
  那么 返回 `atlas-schema-mismatch` 并提示 rebuild
  并且 不读取旧 shard 或 index

场景: 无效 limit 被 CLI 与 MCP 拒绝
  测试: test_atlas_search_rejects_limit_outside_range
  假设 search limit 为 0 或 201
  当 library search 被 CLI 或 MCP 调用
  那么 返回 `atlas-search-limit` diagnostic
  并且 不执行 index traversal
