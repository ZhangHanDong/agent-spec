spec: task
name: "Atlas Contract And Trace Integration (Phase 3)"
tags: [atlas, code-graph, linker, lifecycle, trace]
satisfies: [REQ-INTENT-CODE-LINKER]
depends: [task-rust-atlas-code-graph, task-code-graph-ir-bindings]
estimate: 1w
---

## Intent

Deliver the first Rust-specific slice of the planned Intent-Code Linker: Task
Contract symbol boundaries and trace entries may reference Atlas node ids, and
spec lifecycle validates those references against a fresh graph. This turns
"the Contract mentions a symbol that no longer exists" into a mechanical
diagnostic while keeping Requirement IR independent of derived program facts.

## Decisions

- Task Contracts reference Atlas nodes by canonical symbol path in a distinct `### Symbols` subsection under `## Boundaries`; path globs remain unchanged.
- Spec lint and lifecycle, not `lint-knowledge`, own Contract symbol validation and emit `atlas-symbol-missing` for a symbol absent from a fresh graph.
- Symbol validation runs Atlas freshness checking first; a stale graph yields `atlas-stale` instead of false symbol-missing errors.
- Trace evidence records provider, canonical node id, node kind, file, provenance, and graph fingerprint instead of an untyped code-target string when Atlas facts are available.
- Requirement KLL remains connected through `satisfies` and trace evidence; it does not embed mutable Atlas node facts as durable truth.
- Atlas access is read-only. Neither KLL nor spec validation mutates the graph store.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/spec_core/**
- src/spec_parser/**
- src/spec_gateway/**
- src/spec_verify/**
- src/main.rs
- crates/rust-atlas/**
- fixtures/atlas/**
- knowledge/requirements/req-intent-code-linker.md
- docs/intent-compiler/schemas/requirement-trace-ledger-v1.schema.json
- specs/task-atlas-kll-integration.spec.md
- specs/roadmap/task-atlas-kll-integration.spec.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Forbidden
- Do not make atlas facts durable KLL truth.
- Do not let KLL mutate the graph store.
- Do not make Rust Atlas mandatory for non-Rust projects.

## Out of Scope

- Wiki article auto-generation from atlas facts
- Cross-project atlas graphs
- Automatic planning-time symbol discovery and provider-neutral code-binding generation
- Quality Planning, Clippy, rustfmt, cargo-deny, Miri, and skill resolution

## Questions

- [x] boundary 路径 glob 与 atlas 符号引用的语法如何区分（前缀标记 vs 独立字段）？（已解决：独立字段——Boundaries 下新增 `### Symbols` 子节承载 atlas 符号路径，与路径 glob 完全分离，BoundariesVerifier 保持不动。）
- [x] Allowed Changes 是否覆盖 lifecycle 接线位置？（已解决：合同起草于 workspace 重构前；验证管线现居 `src/spec_verify/**` + `src/spec_gateway/**`，trace 记录调用点在 `src/main.rs`——三者补入边界，并补 `satisfies` 治理链接。）

## Completion Criteria

<!-- lint-ack: error-path — the atlas-symbol-missing and atlas-stale scenarios are the failure paths of this read-only lint feature -->
<!-- lint-ack: output-mode-coverage — lifecycle output modes are owned by the existing lifecycle contracts; this task only adds Atlas diagnostics -->
<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when the spec is promoted to specs/ -->

Scenario: Lifecycle validates Atlas symbol references in Task Contracts
  Test:
    Filter: test_lifecycle_reports_missing_atlas_contract_symbol
    Level: integration
  Given a spec references an atlas symbol path that is absent from the built graph
  When lifecycle validates the Task Contract
  Then diagnostics include `atlas-symbol-missing` for that reference

Scenario: Stale graph blocks symbol validation with a distinct diagnostic
  Test:
    Filter: test_lifecycle_reports_stale_atlas_graph
    Level: integration
  Given a built graph with one modified source file
  When lifecycle validates a Task Contract symbol reference
  Then diagnostics include `atlas-stale` and no `atlas-symbol-missing` is emitted

Scenario: Valid references pass without diagnostics
  Test:
    Filter: test_lifecycle_accepts_valid_atlas_contract_symbols
    Level: integration
  Given a spec references symbols present in a fresh graph
  When lifecycle validates the Task Contract
  Then no atlas diagnostics are emitted

Scenario: Atlas trace targets remain typed and stale-aware
  Test:
    Filter: test_atlas_trace_target_records_provider_node_and_fingerprint
    Level: integration
  Given a passing lifecycle run for a Task Contract with a valid Atlas symbol reference
  When requirement trace evidence is persisted
  Then the code target records provider, node id, node kind, file, provenance, and graph fingerprint
  And Requirement IR remains byte-identical

Scenario: Non-Rust projects do not require Atlas
  Test:
    Filter: test_non_rust_lifecycle_without_atlas_symbols_does_not_require_graph
    Level: integration
  Given a non-Rust project whose Task Contract declares no Atlas symbols
  When lifecycle runs without an Atlas graph
  Then no Atlas diagnostic is emitted
