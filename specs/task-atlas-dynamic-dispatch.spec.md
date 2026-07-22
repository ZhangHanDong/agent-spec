spec: task
name: "Atlas Bounded Dynamic Dispatch"
tags: [atlas, code-graph, dynamic-dispatch, static-analysis]
satisfies: [REQ-ATLAS-DYNAMIC-DISPATCH]
depends: [task-atlas-mir-layer, task-atlas-explore-flow-impact]
risk: B
---

## Intent

在不把启发式推理伪装成 compiler fact 的前提下，为 Rust trait method call 增加有界、
可解释、可逆的 implementation candidates，使 flow 和 impact 能跨越动态分派边界。

## Decisions

- v1 只处理已有 resolved SCIP call 指向 trait method 的 whole-graph 形状；不从函数名猜调用。
- enricher 位于独立 `dynamic_dispatch` 模块，由 `atlas build --dynamic-dispatch` 显式启用。
- 保留原始 exact SCIP declaration edge，另加 unresolved bounded-candidate edge。
- candidate 来自 resolved `ImplsTrait` 和 impl containment edge，按 node id 排序去重。
- 每个 call site 最多 64 个候选；超限时不截断成看似完整的边，而是报告 typed diagnostic。
- 同一 concrete target 同时有 exact edge 与 bounded candidate 时，派生邻接选择 exact，
  canonical shard 仍保留两份证据。

## Boundaries

### Allowed Changes
- crates/rust-atlas/**
- src/main.rs
- knowledge/requirements/req-atlas-dynamic-dispatch.md
- specs/task-atlas-dynamic-dispatch.spec.md
- docs/atlas-roadmap.md
- docs/atlas-dynamic-dispatch.md
- README.md
- AGENTS.md

### Symbols
- rust-atlas: rust_atlas::build
- rust-atlas: rust_atlas::flow
- rust-atlas: rust_atlas::impact

### Forbidden
- 不在 syn extraction visitor 内加入 whole-graph 分派推理
- 不把 inferred candidate 标为 exact、resolved 或 MIR provenance
- 不按函数名全局连接无 call anchor 的 method
- 不在 fan-out 超限时静默截断候选
- 不改变默认构建的 graph 行为

## Out of Scope

- closure/function pointer、async task、channel 和 callback registry 推理
- framework-specific route/DI plug-in
- runtime profile 或 observed call trace
- MIR producer

## Completion Criteria

场景: Trait call 产生有界候选
  测试: test_atlas_dynamic_dispatch_enriches_trait_call_and_flow
  假设 resolved SCIP call 指向有两个实现的 trait method
  当启用 dynamic-dispatch enricher
  那么原始 exact edge 保留、inferred edge 含排序后的两个候选且 flow 能到达实现

场景: 无 call anchor 时保持 inert
  测试: test_atlas_dynamic_dispatch_is_inert_without_trait_call
  假设 graph 含 trait 和 impl 但没有指向 trait method 的 resolved SCIP call
  当启用 enricher
  那么 graph 不含 dynamic-dispatch extractor edge

场景: Fan-out 超限失败关闭
  测试: test_atlas_dynamic_dispatch_fanout_fails_closed
  假设一个 trait call 对应 65 个 implementation method
  当运行 enricher
  那么 diagnostics 含 dynamic-dispatch-truncated 且 graph 不含该 inferred edge

场景: 默认 rebuild 清理 inferred edge
  测试: test_atlas_default_build_removes_dynamic_dispatch_edges
  假设 graph 已含 dynamic-dispatch inferred edge
  当不启用 enricher 重新 build
  那么 inferred edge 被删除且 syn edge 仍存在

场景: CLI 显式启用 enrichment
  测试: test_atlas_dynamic_dispatch_cli_flag
  假设 atlas build invocation 包含非默认 dynamic dispatch flag
  当解析 atlas build --dynamic-dispatch
  那么 BuildOptions.dynamic_dispatch 为 true

场景: Exact relation 优先于 candidate alternative
  测试: test_query_index_adjacent_projection_prefers_exact_edge_over_candidate_target
  假设同一 caller 到 implementation method 同时有 exact MIR edge 和 dynamic candidate edge
  当构建派生 query index 的 incoming 与 outgoing adjacency
  那么两种 adjacency 都选择 exact MIR edge 且不改变 canonical shard evidence
