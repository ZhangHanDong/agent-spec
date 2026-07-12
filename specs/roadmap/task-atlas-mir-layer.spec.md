spec: task
name: "Atlas MIR Layer (Phase 2)"
tags: [atlas, code-graph, mir, static-analysis]
depends: [task-rust-atlas-code-graph]
estimate: 2w
---

## Intent

Overlay precise MIR-derived facts onto the Phase 1 atlas graph: resolved
`calls` edges and per-function CFG summaries with provenance `mir`, so agents
get call-graph precision the syn/SCIP layers cannot provide. The MIR extractor
is an optional, feature-gated enhancer — without it, atlas behaves exactly as
Phase 1.

## Decisions

- Evaluate Charon (serialized ULLBC/LLBC output) as the extractor before writing a custom `rustc_public` driver; record the choice in this spec before implementation starts.
- MIR facts are an overlay: same schema, edges carry provenance `mir`; queries prefer the highest-provenance edge when duplicates exist.
- The MIR layer is gated behind `--features mir` and is the only place a nightly toolchain may be required; the default build stays stable-only.
- Extraction failures (crate does not compile, extractor version mismatch) degrade to the Phase 1 graph with a diagnostic, never a hard failure of `atlas build`.

## Boundaries

### Allowed Changes
- crates/rust-atlas/**
- src/main.rs
- fixtures/atlas/**
- specs/roadmap/task-atlas-mir-layer.spec.md
- CHANGELOG.md

### Forbidden
- Do not make nightly a requirement of the default feature set.
- Do not change `schema_version` semantics for non-MIR consumers.
- Do not modify `src/spec_knowledge/**`.

## Out of Scope

- Borrow-check/ownership fact extraction (future exploration)
- KLL integration (Phase 3)

## Questions

- [x] Charon 对当前 rustc 版本的跟进节奏是否满足 CI 稳定性要求？（已解决：MIR 层 feature 门控、永不进默认 CI，Charon 滞后不可能破坏 CI；Phase 2 开工时设评估门——Charon 支持仓库钉住的 stable rustc 且滞后不超过 2 个 minor 版本，否则改用 rustc_public 自写 driver。）
- [x] 单态化前的泛型 `calls` 边如何标注（保留泛型形参 vs 展开实例）？（已解决：保留泛型形参形式，边上加 `generic: true` 属性；实例级展开有组合爆炸风险，列为非目标，等出现真实消费方再评估。）

## Completion Criteria

<!-- lint-ack: output-mode-coverage — MIR overlay writes the same shard files as Phase 1; file side effects are covered by the Phase 1 contract -->
<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when the spec is promoted to specs/ -->
<!-- lint-ack: precedence-fallback-coverage — the provenance precedence chain is verified by test_atlas_query_prefers_highest_provenance_edge -->

Scenario: Query prefers the highest-provenance edge
  Test:
    Filter: test_atlas_query_prefers_highest_provenance_edge
    Level: integration
  Given a caller-callee pair has both a `syn` references edge and a `mir` calls edge
  When `atlas query` runs for the caller
  Then the reported call relation carries provenance `mir`
  And the lower-provenance edge remains stored in the shard

Scenario: MIR overlay adds precise call edges
  Test:
    Filter: test_atlas_mir_overlay_adds_calls_edges
    Level: integration
  Given a fixture crate built with the `mir` feature enabled
  When `atlas build` runs with MIR extraction
  Then `calls` edges with provenance `mir` connect caller and callee functions
  And Phase 1 `syn` edges remain present

Scenario: MIR extraction failure degrades to the Phase 1 graph
  Test:
    Filter: test_atlas_mir_failure_degrades_to_syn_graph
    Level: integration
  Given a fixture crate that fails MIR extraction
  When `atlas build` runs with MIR extraction
  Then the command exits 0 with a `mir-extraction-failed` diagnostic
  And the graph contains only `syn` and `scip` provenance edges

Scenario: Default build has no MIR requirement
  Test:
    Filter: test_atlas_default_build_excludes_mir
    Level: integration
  Given the default feature set on a stable toolchain
  When `atlas build` runs
  Then the build succeeds without any MIR extractor invocation
