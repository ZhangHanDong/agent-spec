spec: task
name: "Atlas KLL Integration (Phase 3)"
tags: [atlas, code-graph, kll, knowledge]
depends: [task-rust-atlas-code-graph]
estimate: 1w
---

## Intent

Let the Knowledge Liveness Layer consume atlas facts: spec boundaries and trace
entries may reference atlas node ids, and knowledge lint validates that
referenced symbols actually exist in the current graph — turning "the spec
mentions a symbol that no longer exists" into a mechanical diagnostic instead
of silent drift.

## Decisions

- KLL references atlas nodes by canonical symbol path; the graph remains derived working memory, never durable KLL truth.
- `lint-knowledge` gains an `atlas-symbol-missing` diagnostic for spec/wiki references to symbols absent from a fresh graph.
- Lint runs `atlas check` first: a stale graph yields an `atlas-stale` diagnostic rather than false symbol-missing errors.
- No atlas write path from KLL; the integration is read-only in both directions.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- crates/rust-atlas/**
- fixtures/atlas/**
- specs/roadmap/task-atlas-kll-integration.spec.md
- CHANGELOG.md

### Forbidden
- Do not make atlas facts durable KLL truth.
- Do not let KLL mutate the graph store.

## Out of Scope

- Wiki article auto-generation from atlas facts
- Cross-project atlas graphs

## Questions

- boundary 路径 glob 与 atlas 符号引用的语法如何区分（前缀标记 vs 独立字段）？

## Completion Criteria

<!-- lint-ack: error-path — the atlas-symbol-missing and atlas-stale scenarios are the failure paths of this read-only lint feature -->
<!-- lint-ack: output-mode-coverage — lint-knowledge output modes are owned by the existing lint-knowledge contract; this task only adds diagnostics -->
<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when the spec is promoted to specs/ -->

Scenario: Knowledge lint validates atlas symbol references
  Test:
    Filter: test_lint_knowledge_reports_missing_atlas_symbol
    Level: integration
  Given a spec references an atlas symbol path that is absent from the built graph
  When `lint-knowledge` runs
  Then diagnostics include `atlas-symbol-missing` for that reference

Scenario: Stale graph blocks symbol validation with a distinct diagnostic
  Test:
    Filter: test_lint_knowledge_reports_stale_atlas_graph
    Level: integration
  Given a built graph with one modified source file
  When `lint-knowledge` runs
  Then diagnostics include `atlas-stale` and no `atlas-symbol-missing` is emitted

Scenario: Valid references pass without diagnostics
  Test:
    Filter: test_lint_knowledge_accepts_valid_atlas_references
    Level: integration
  Given a spec references symbols present in a fresh graph
  When `lint-knowledge` runs
  Then no atlas diagnostics are emitted
