---
title: "External Code Graph Providers"
type: concept
source_files:
  - crates/code-graph-provider/src/lib.rs
  - crates/code-graph-provider/Cargo.toml
  - crates/code-graph-provider/README.md
  - src/main.rs
  - fixtures/code-graph-provider/basic/manifest.json
  - fixtures/code-graph-provider/basic/conformance.json
  - fixtures/code-graph-provider/basic/provider.sh
  - docs/code-graph-provider-kit.md
  - knowledge/requirements/req-code-graph-provider-kit.md
  - specs/task-code-graph-provider-kit.spec.md
tags:
  - atlas
  - provider
  - conformance
status: active
---

# External Code Graph Providers

## Role

The F1 adapter kit is the producer boundary for Code Graph tools outside Rust
Atlas. Its standalone Rust crate owns strict manifests, project registration,
extractor and enricher payloads, canonical projection, bounded local process
execution, atomic publication, and conformance receipts.

## Boundaries

Extractors can publish provider-scoped nodes, containment, and basic references.
Semantic enrichers have a separate schema with no node or KLL field and can add
only evidence-bearing edges or query hints against a base graph fingerprint.
Every invocation is explicit and no-daemon. Stale, partial, wrong-worktree,
malformed, oversized, timed-out, or cancelled evidence cannot replace the last
valid artifact.

## Conformance

The checked-in local fixture covers stable ids, deterministic repeat, partial
parse, stale/worktree behavior, unknown schema, bounded output, cancellation,
and atomic publish. This receipt proves the F1 transport and validation contract,
not production language support. Concrete non-Rust providers remain F2.

The 1.2 workspace release publishes this SDK independently as
`agent-spec-code-graph-provider` 0.1.0. Cargo resolution does not replace the
manifest IR-range and strict wire-schema gates.
