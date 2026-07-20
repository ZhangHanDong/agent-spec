spec: task
name: "Atlas Runtime Boundary Hints"
tags: [atlas, flow, runtime, boundary, query-hint, dogfood]
satisfies: [REQ-ATLAS-RUNTIME-BOUNDARY-HINTS]
depends: [task-atlas-explore-flow-impact, task-atlas-dynamic-dispatch, task-atlas-query-quality-regression]
estimate: 3d
---

## Intent

在 Rust Atlas 的 disconnected flow 上增加可解释的 runtime boundary：只对 fresh function
body 做 query-time `syn` AST 检测，返回 site、mechanism、key 与有界候选，但不写入 graph、
impact 或 lifecycle authority。E3 live fixture 必须证明该提示能被回归评分且不会掩盖静态路径。

<!-- lint-ack: bdd-rule-grouping — scanner、flow projection 与 E3 regression 构成同一条 query-hint 交付链 -->

## Decisions

- 新建 `crates/rust-atlas/src/runtime_boundary.rs`，使用 `syn::visit::Visit` 检测 Rust AST；
  不把 CodeGraph 的多语言 regex 表复制进 Rust Atlas。
- v1 mechanism 固定为 `async-task`、`channel`、`callback-registry`、`reflection`、
  `framework-route`。每个 hint 包含 source node、`EdgeSite`、normalized expression、静态 key、
  candidate text、resolved candidate nodes、`authority: query-hint` 与 `confidence: heuristic`。
  framework route 分别从 `route(path, handler)` 的第二参数和 `service(handler)` 的首参数提取候选。
- flow 只有在 endpoint 已解析、traversal 未 truncated 且无 complete path 时扫描；found、unknown、
  ambiguous 与 truncated flow 保持 inert。
- 扫描 source endpoint 及其静态可达 function，`--through` 只扫描 through function；顺序按
  depth ascending 再按 canonical node id，确保新鲜 SCIP helper edge 不会隐藏 caller boundary。
  上限固定为 8 nodes、200000 source bytes、4 hints。
- 每个 candidate set 最多 16 nodes。超限时清空 resolved candidates，保留 candidate text 并标记
  `candidates_truncated=true`，不得返回任意前 16 个造成伪完整集合。
- bare candidate 先查 source module exact symbol；只有不存在 contextual exact match 时才允许
  query-index suffix fallback，不能并入其他 module 的同名 symbol。
- source path 必须在 canonical code root 内，且 blake3 必须与 `Meta.files` 相等；stale、missing、
  escaped、non-UTF-8 或 parse failure 都不产生 hint。stale source 也不能扩展后继；SCIP/MIR
  edge 只有对应 layer 为 fresh 时才能扩展 scan frontier。frontier 与 AST scan 共享 per-file
  hash-validated byte cache，在读取新文件前执行 8-node/200000-byte budget。
- scanner 先用 graph node 的函数名、签名与 source span 选择唯一 function AST，再只访问该 body；
  同行 sibling 或其他不唯一选择 fail closed。candidate lookup 使用 source package、module 与 impl
  context 规范化 `crate`、`self`、`super`、`Self` 和 qualified path，并保留 qualified self type、
  trait、generic arguments 与 member。存储与解析签名使用相同的 whitespace canonicalization；
  default trait method 的 lowercase `self`/`super` 使用 trait declaration module，而 `Self`
  仍使用 trait container；`<Self as Trait>::member` 解析到 indexed trait declaration member。
  reflection 的 generic type text 保持原样，但 lookup 去除实例化参数后绑定 indexed type declaration。
  reflection 只保留 Rust type-namespace declaration；async、callback 和 route continuation 只保留
  indexed function declaration。同名 type/function 必须在 16-candidate limit 前完成过滤。
  `crate::Type::method`、`self::Type::method` 与 source-relative `Type::method` 必须先定位 type
  declaration，再展开到 Atlas 的 canonical inherent-impl method symbol。
  receiver role 只匹配生成实际 receiver 的 callable/access chain 上完整或下划线分隔的 AST
  identifier，忽略 call arguments、index values 与 string literal 内容。
- `FlowResult` 增加可选 `runtime_boundaries` 和 `runtime_boundary_truncated`；没有 hint 且没有扫描
  截断时省略字段，保持已连通结果的现有 JSON bytes。hint 存在时增加 exact diagnostic code
  `atlas-flow-runtime-boundary`。
- query 前后 `meta.json`、`query-index.json` 和 shard bytes 必须 byte-identical；runtime hint 不创建
  `Edge`，也不进入 `impact`、`affected`、binding 或 lifecycle。
- E3 corpus 新增 `fixtures/atlas/runtime-boundaries` live case，scorer 同时检查 expected continuation
  path、source evidence 与 `{kind: runtime-boundary, code: atlas-flow-runtime-boundary}`。

## Boundaries

### Allowed Changes
- crates/rust-atlas/Cargo.toml
- crates/rust-atlas/src/lib.rs
- crates/rust-atlas/src/flow.rs
- crates/rust-atlas/src/runtime_boundary.rs
- fixtures/atlas/runtime-boundaries/**
- benchmarks/atlas/query-corpus.json
- benchmarks/atlas/query-results.json
- src/atlas_eval.rs
- src/main.rs
- .agent-spec/wiki/**
- docs/atlas-runtime-boundaries.md
- docs/atlas-evaluation.md
- docs/atlas-roadmap.md
- knowledge/requirements/req-atlas-runtime-boundary-hints.md
- specs/task-atlas-runtime-boundary-hints.spec.md
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::flow

### Forbidden
- 不修改 graph schema 或持久化 runtime hint
- 不让 runtime hint 参与 impact、affected、binding、lifecycle 或 archive authority
- 不在 found、unknown、ambiguous 或 truncated flow 上扫描源码
- 不扫描 stale、root-escaped 或 hash 不匹配的 source
- 不沿 stale source 或 stale SCIP/MIR edge 扩展 runtime scan frontier
- 不把候选截断后的任意前缀伪装成完整 candidate set
- 不新增网络、LLM、daemon 或 framework dependency

## Out of Scope

- 把某个 mechanism 晋升为 A4.2 persisted bounded-candidate edge
- 自动证明某个 runtime candidate 一定执行
- 非 Rust 语言 detector
- 默认新增 MCP tool 或改变 MCP discovery
- source endpoint 之外的全仓库启发式扫描

## Completion Criteria

场景: Rust AST scanner 区分五类 runtime mechanism
  测试:
    过滤: test_runtime_boundary_scanner_detects_rust_mechanisms_and_candidates
    层级: unit
  假设 function bodies 分别包含 spawn、channel send、callback registry、downcast 与 route site
  当 `syn` visitor 扫描这些表达式
  那么 每个 hint 返回对应 mechanism、exact site、normalized form、key 与 candidate text

场景: scanner 不匹配 comments strings 与无关调用
  测试:
    过滤: test_runtime_boundary_scanner_ignores_comments_strings_and_unrelated_calls
    层级: unit
  假设 dispatch-like 文本只在 comment、string、receiver literal、computed receiver 的无关 argument、普通业务方法或 suffix lookalike 中出现
  当 AST scanner 运行
  那么 hint 集合为空且不使用 regex text fallback

场景: 单参数 service 保留 handler continuation
  测试:
    过滤: test_runtime_boundary_service_uses_its_single_handler_argument
    层级: unit
  假设 route receiver 调用 `service(get(handler))`
  当 `syn` visitor 扫描该 method call
  那么 framework-route hint 的 candidate text 精确等于 `handler`

场景: qualified function signature 仍绑定 body
  测试:
    过滤: test_runtime_boundary_scan_matches_graph_signatures_with_qualified_paths
    层级: integration
  假设 graph 存储的 free-function signature 包含 `& crate :: Registry`
  当 disconnected flow 对应 source AST 使用 `&crate::Registry`
  那么 result 包含一个 callback-registry runtime boundary

场景: scanner 不把同行 sibling runtime site 归给 source
  测试:
    过滤: test_runtime_boundary_scan_binds_sites_to_the_selected_function_ast
    层级: integration
  假设 dispatch、callback 与无关 source function 位于同一 source line
  当 disconnected flow 从无关 source 查询 callback
  那么 runtime boundary 集合为空

场景: candidate lookup 规范化 Rust 相对与 qualified-self 路径
  测试:
    过滤: test_runtime_boundary_candidate_lookup_canonicalizes_rust_relative_paths
    层级: unit
  假设 candidate text 使用 `crate`、`self`、`Self` 与 generic qualified-self path 且 index 含 competing trait implementations
  当 query index 按 source context 解析这些文本
  那么 对应 root、module、impl 与 exact qualified handler 按 canonical id 返回且未 truncated

场景: generic reflection candidate 绑定类型声明
  测试:
    过滤: test_runtime_boundary_reflection_candidate_resolves_generic_type_declaration
    层级: unit
  假设 reflection candidate text 为 `Message<u8>` 且 index 含 `Message` declaration
  当 query index 解析该 preserved candidate text
  那么 resolved candidate 精确等于 `Message` declaration 且未 truncated

场景: reflection candidate 遵守 Rust type namespace
  测试:
    过滤: test_runtime_boundary_reflection_candidate_uses_type_namespace
    层级: unit
  假设 module 合法声明同名 `Message` type 与 function
  当 reflection candidate lookup 解析 `Message`
  那么 resolved candidates 只包含 type declaration 且 function 在 fan-out 计数前被拒绝

场景: inherent associated callback 解析到 impl method
  测试:
    过滤: test_atlas_flow_resolves_inherent_runtime_candidate_from_built_graph
    层级: integration
  假设 built graph 将 `crate::Handler::callback` 索引为 canonical inherent-impl method
  当 disconnected flow 扫描 callback registration
  那么 resolved candidate 精确等于该 inherent implementation method

场景: bare candidate 优先 source module exact match
  测试:
    过滤: test_runtime_boundary_candidate_lookup_prefers_source_context
    层级: unit
  假设 source module 与 sibling module 都声明 `handler`
  当 query index 解析 bare candidate text `handler`
  那么 resolved candidates 只包含 source module handler

场景: disconnected flow 输出 query hint 且不改图
  测试:
    过滤: test_atlas_flow_reports_query_time_runtime_boundary_without_mutating_graph
    层级: integration
  假设 fresh fixture 的 callback registration 没有 complete graph path
  当 flow query 连续执行两次
  那么 两次 runtime boundary JSON byte-identical并包含 site、candidate、query-hint authority 与 heuristic confidence
  并且 meta、query index、shard bytes 和 graph fingerprint 在查询前后 byte-identical

场景: connected flow 不输出 runtime hint
  测试:
    过滤: test_atlas_flow_suppresses_runtime_hints_for_connected_paths
    层级: integration
  假设 相同 source site 已有完整 static call path
  当 flow query 返回 found
  那么 runtime boundary 字段被省略且没有 `atlas-flow-runtime-boundary` diagnostic

场景: fresh SCIP helper edge 不隐藏 caller boundary
  测试:
    过滤: test_atlas_flow_runtime_hints_survive_fresh_scip_helper_edges
    层级: integration
  假设 fresh SCIP 已解析 source 到 registration helper 的静态调用但未解析 runtime continuation
  当 disconnected flow 从 source 查询到 registered callback
  那么 source function 的 callback-registry hint 仍被返回且 SCIP authority 保持 fresh

场景: stale source 不产生 runtime hint
  测试:
    过滤: test_atlas_flow_runtime_hints_require_fresh_source
    层级: integration
  假设 fixture graph 构建后修改包含 dispatch site 的 source
  当 frozen disconnected flow 运行
  那么 result 保留 stale status 且不从当前 source 产生 runtime hint

场景: stale source 不扩展 fresh descendant
  测试:
    过滤: test_runtime_boundary_stale_source_does_not_expand_to_fresh_descendants
    层级: unit
  假设 changed source 的旧图仍含一条到 unchanged helper 的 edge
  当 runtime boundary projection 构建 source-first frontier
  那么 stale source 与 helper 都不产生 runtime hint 且结果未 truncated

场景: stale semantic edge 不扩展 scan frontier
  测试:
    过滤: test_runtime_boundary_stale_semantic_edges_do_not_expand_scan_frontier
    层级: integration
  假设 source hash 保持 fresh 但 SCIP index fingerprint 已变化
  当 frozen disconnected flow 读取旧 SCIP helper edge
  那么 status 保留 stale SCIP 且不输出 helper runtime boundary

场景: default trait candidate 使用声明 module
  测试:
    过滤: test_runtime_boundary_trait_default_candidates_use_declaring_module
    层级: integration
  假设 nested module 的 default trait method 注册 `self::handler`
  当 runtime candidate 通过 query index 解析
  那么 candidate symbol 精确等于 nested module handler 而非 trait member

场景: default trait qualified Self 解析到 trait member
  测试:
    过滤: test_runtime_boundary_trait_default_resolves_qualified_self_candidate
    层级: integration
  假设 default trait method 注册 `<Self as Runner>::handler`
  当 runtime candidate 通过 query index 解析
  那么 candidate symbol 精确等于 indexed `Runner::handler` trait member

场景: candidate 与 scan budget fail closed
  测试:
    过滤: test_runtime_boundary_limits_fail_closed_and_order_deterministically
    层级: unit
  假设 runtime site 匹配超过 16 candidates 且 frontier 超过 scan/hint 上限
  当 runtime boundary projection 重复运行
  那么 结果顺序和 truncation metadata byte-identical
  并且 超限 candidate set 为空而 candidate text 仍保留

场景: frontier 在 source read 前执行 node 与 byte limit
  测试:
    过滤: test_runtime_boundary_scan_frontier_enforces_node_and_byte_limits
    层级: unit
  假设 reachable frontier 超过 8 functions 或下一个 source 超过剩余 200000-byte budget
  当 runtime boundary 构建 source-first frontier
  那么 frontier 只含 8 functions、oversized source 不进入 byte cache 且返回 truncated

场景: runtime hint JSON 不包含 graph edge
  测试:
    过滤: test_runtime_boundary_json_marks_query_hint_not_graph_fact
    层级: unit
  假设 一个含 candidate continuation 的 runtime hint
  当 result 序列化为 JSON
  那么 authority 为 `query-hint`、confidence 为 `heuristic` 且对象不含 edge provenance 或 resolution

场景: E3 live fixture 对 runtime boundary 做完整评分
  测试:
    过滤: test_atlas_query_live_runtime_boundary_probe_scores_current_flow
    层级: integration
  假设 checked-in runtime-boundary fixture、fresh SCIP helper edge 与 query corpus case
  当 当前 flow 输出及其原始 diagnostics 投影为 E3 observation
  那么 scorer 通过 expected continuation path、source evidence 与 exact runtime-boundary diagnostic

场景: runtime hint 不能进入 deterministic impact
  测试:
    过滤: test_runtime_boundary_query_does_not_change_impact_or_bindable_edges
    层级: integration
  假设 disconnected flow 返回 callback candidate hint
  当 对 source 与 candidate 分别执行 impact 和 adjacency query
  那么 两者都不包含由 runtime hint 合成的 edge 或 dependent path

场景: tracked wiki 保留 runtime boundary authority 边界
  测试:
    过滤: test_atlas_runtime_boundary_docs_and_wiki_describe_authority
    层级: integration
  假设 reader docs 与 tracked Atlas wiki 页面
  当 文档契约测试读取这些页面
  那么 页面指明 runtime boundary 是 query hint 且不能成为 graph 或 lifecycle authority
