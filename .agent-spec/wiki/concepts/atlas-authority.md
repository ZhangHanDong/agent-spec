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
  - crates/rust-atlas/src/context.rs
  - crates/rust-atlas/src/flow.rs
  - crates/rust-atlas/src/runtime_boundary.rs
  - crates/rust-atlas/src/impact.rs
  - crates/rust-atlas/src/affected.rs
  - src/atlas_query_service.rs
  - src/atlas_daemon.rs
  - src/spec_mcp/mod.rs
  - src/atlas_eval.rs
  - src/spec_knowledge/code_graph.rs
  - src/spec_verify/atlas_symbols.rs
  - docs/atlas-roadmap.md
  - docs/atlas-runtime-boundaries.md
  - docs/atlas-incremental-builds.md
  - docs/atlas-live-runtime.md
  - docs/atlas-concurrent-query-serving.md
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

D4 worker admission pins the same immutable snapshot before queueing and keeps
its reader lease through response serialization. Worker, maintenance, control,
and MCP client lanes isolate scheduling but do not add authority. Typed busy,
timeout, cancel, degraded, failed, or unavailable responses mean no context was
proven. Explicit fallback records a separate complete direct generation rather
than combining evidence across attempts.

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

`atlas context` compiles a bounded view over one pinned graph generation. Its
evidence ids, source slices, omission continuations, and receipts are derived
query artifacts: they explain what was selected or dropped but cannot create a
binding, requirement state, scenario verdict, or lifecycle evidence. Restricted
test/generated/vendor bodies require an explicit name or evidence-spine role;
stale bytes remain graph skeletons with typed read-back diagnostics.

The checked-in D4 receipt gates direct/worker semantic parity, snapshot and
worktree identity, typed outcomes, and lease cleanup. Its latency, heartbeat,
CPU, RSS, and response-size values remain measurements and cannot establish
freshness, correctness, or requirement satisfaction. The hidden MCP context
surface remains opt-in pending E1.

## Maintenance

Keep this working-memory page aligned with the status and authority-gate source;
KLL requirements and lifecycle records remain the governing evidence.

Atlas B5 and D4 were reviewed here; context compilation and bounded serving
improve code reading without crossing graph, KLL, Contract, or lifecycle
authority boundaries.

Atlas E1 receipts evaluate adoption but do not create code-graph facts,
freshness, KLL state, or scenario verdicts. Machine pass remains subject to
human acceptance before any default changes.
