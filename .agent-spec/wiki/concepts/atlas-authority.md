---
title: "Atlas Graph Authority"
type: concept
source_files:
  - crates/rust-atlas/src/status.rs
  - crates/rust-atlas/src/lib.rs
  - src/spec_knowledge/code_graph.rs
  - src/spec_verify/atlas_symbols.rs
  - docs/atlas-roadmap.md
tags:
  - atlas
  - freshness
  - authority
  - binding
status: active
---

# Atlas Graph Authority

## Model

`GraphIdentity` binds a graph to its repository, worktree, graph root, and
toolchain. `AtlasStatus` carries that recorded/current identity and independent
syn, SCIP, and MIR layer state. A worktree mismatch blocks definitive reads; a
fresh syn baseline does not make a stale SCIP overlay authoritative.

## Consumers

Provider fingerprints, code bindings, and lifecycle contract-symbol validation
share the authority gate. A stale available layer or borrowed worktree cannot
produce a definitive binding, symbol verdict, or typed trace target. An
unavailable optional semantic layer is distinct from stale authority and does
not block a syn-only consumer.

## Maintenance

Keep this working-memory page aligned with the status and authority-gate source;
KLL requirements and lifecycle records remain the governing evidence.
