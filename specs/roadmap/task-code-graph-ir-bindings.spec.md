spec: task
name: "Provider-Neutral Code Graph IR and Code Bindings"
tags: [intent-compiler, code-graph, bindings]
satisfies: [REQ-CODE-GRAPH-IR]
depends: [task-rust-atlas-code-graph]
estimate: 2d
---

## Intent

Deliver target-architecture boundary 2: a provider-neutral Code Graph IR
consumer contract plus the typed `.agent-spec/code-bindings.json` artifact
binding work units to code targets, with Rust Atlas as the first provider.
Bindings are derived working data — never KLL truth — and a stale graph blocks
definitive binding.

## Decisions

- Define the consumer contract as Rust traits/types in `src/spec_knowledge` (node, edge, provenance, capability, staleness facts) with an adapter for `rust-atlas`.
- Binding artifact `.agent-spec/code-bindings.json`: entries of requirement id, work unit id, provider, targets (node id, kind, file, provenance, graph fingerprint); schema at `docs/intent-compiler/schemas/code-bindings-v1.schema.json` with an `agent-spec/intent-compiler/` `$id`.
- `requirements bind` generates bindings for ready work units by matching declared Contract `### Symbols` entries against the provider graph; a stale graph fails with `atlas-stale` semantics instead of binding.
- Unknown providers and provider/capability mismatches are diagnostics.
- Requirement IR stays byte-identical through binding generation.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- crates/rust-atlas/**
- fixtures/atlas/**
- docs/intent-compiler/**
- knowledge/requirements/req-code-graph-ir.md
- specs/roadmap/task-code-graph-ir-bindings.spec.md
- CHANGELOG.md

### Forbidden
- Do not make bindings durable KLL truth.
- Do not bind against a stale graph.
- Do not add dependencies.

## Out of Scope

- The Intent-Code Linker's lifecycle/trace integration (boundary 3 contract)
- Non-Rust providers (contract only; implementations later)

## Questions

- [x] binding 生成的触发点在 plan 之前还是之后？（已解决：`requirements bind` 独立命令，在 work-units ready 之后、Contract 定稿之前运行——与架构文档"code grounding belongs after work-unit lowering"一致。）

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when promoted -->
<!-- lint-ack: output-mode-coverage — binding artifact emission is asserted by the generation scenario -->

Scenario: Bindings generate for ready work units from a fresh graph
  Test:
    Filter: test_code_bindings_generate_for_ready_units
    Level: integration
  Given a ready work unit whose contract declares atlas symbols and a fresh graph
  When `requirements bind` runs
  Then `.agent-spec/code-bindings.json` binds the requirement, work unit, provider, and targets with graph fingerprint

Scenario: Stale graphs block definitive binding
  Test:
    Filter: test_code_bindings_block_on_stale_graph
    Level: integration
  Given a built graph and one modified source file
  When `requirements bind` runs
  Then the command fails naming the stale files and writes no bindings

Scenario: Unknown providers are rejected
  Test:
    Filter: test_code_bindings_reject_unknown_provider
    Level: integration
  Given a contract symbol declares a provider with no adapter
  When `requirements bind` runs
  Then the diagnostic names the unknown provider

Scenario: Requirement IR is untouched by binding
  Test:
    Filter: test_code_bindings_keep_requirement_ir_byte_identical
    Level: integration
  Given a knowledge tree snapshot
  When `requirements bind` runs
  Then every knowledge document is byte-identical afterwards
