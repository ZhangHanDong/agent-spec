---
title: "Atlas Graph Authority"
type: concept
source_files:
  - crates/rust-atlas/src/status.rs
  - crates/rust-atlas/src/lib.rs
  - crates/rust-atlas/src/generation.rs
  - crates/rust-atlas/src/live.rs
  - crates/rust-atlas/src/sync.rs
  - crates/rust-atlas/src/incremental.rs
  - crates/rust-atlas/src/explore.rs
  - crates/rust-atlas/src/flow.rs
  - crates/rust-atlas/src/runtime_boundary.rs
  - crates/rust-atlas/src/impact.rs
  - crates/rust-atlas/src/affected.rs
  - src/spec_knowledge/code_graph.rs
  - src/spec_verify/atlas_symbols.rs
  - docs/atlas-roadmap.md
  - docs/atlas-runtime-boundaries.md
  - docs/atlas-incremental-builds.md
  - docs/atlas-live-runtime.md
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

`AtlasStatus` also reports the generation pinned at operation start. Metadata,
shards, query index, input plan, and overlay capability become visible only as
one committed generation. Cancellation, resource failure, or publication
failure leaves the old generation active and retains orphan work. A healthy
zero-change build does not rewrite derived authority.

D3 adds a separate live status with watcher health, pending watermark, retry
state, and daemon availability. A reader lease pins each query generation while
safe reclamation fails closed on ambiguous leases. `pending` or `degraded`
means refresh work is outstanding; it does not make stale graph facts current.
Static MCP discovery and no-daemon reads remain independent of daemon liveness.
After pointer commit, acknowledgement and status persistence failures remain
warning-only and retain conservative pending context.

## Consumers

Provider fingerprints, code bindings, and lifecycle contract-symbol validation
share the authority gate. A stale available layer or borrowed worktree cannot
produce a definitive binding, symbol verdict, or typed trace target. An
unavailable optional semantic layer is distinct from stale authority and does
not block a syn-only consumer.

Explore, flow, impact, and affected all reuse this authority boundary. Frozen
explore may return stale graph facts, but only hash-matching current files may
be inlined as source excerpts. Flow keeps `found`, `no-path`,
`capability-unavailable`, `truncated`, `unknown-endpoint`, and
`ambiguous-endpoint` distinct. A disconnected flow may add fresh-source
`runtime_boundaries` at a `runtime-boundary`, but their `query-hint` authority
and heuristic confidence cannot satisfy bindings, lifecycle symbols, archive
evidence, or deterministic affected paths. Rust namespace filtering and
inherent-method resolution improve candidate precision without promoting a
hint to a graph fact. Affected maps changes to code evidence only;
test obligations remain owned by the intent compiler and Task Contracts.
Connecting affected code paths to requirements, scenarios, and quality gates is
the next intent-aware consumer step; code impact alone does not establish that
trace.

## Maintenance

Keep this working-memory page aligned with the status and authority-gate source;
KLL requirements and lifecycle records remain the governing evidence.
