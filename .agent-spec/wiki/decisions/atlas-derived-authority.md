---
title: "Atlas Derived Authority"
type: decision
source_files:
  - crates/rust-atlas/src/lib.rs
  - crates/rust-atlas/src/index.rs
  - crates/rust-atlas/src/status.rs
  - crates/rust-atlas/src/live.rs
  - crates/rust-atlas/src/generation.rs
  - crates/rust-atlas/src/context.rs
  - crates/rust-atlas/src/flow.rs
  - crates/rust-atlas/src/runtime_boundary.rs
  - src/spec_mcp/tools.rs
  - src/spec_mcp/mod.rs
  - src/atlas_query_service.rs
  - src/atlas_daemon.rs
  - src/atlas_eval.rs
  - docs/atlas-roadmap.md
  - docs/atlas-runtime-boundaries.md
  - docs/atlas-live-runtime.md
  - docs/atlas-concurrent-query-serving.md
tags:
  - atlas
  - derived-data
  - fail-closed
status: active
---

# Atlas Derived Authority

## Decision

Treat Atlas shards, query indexes, provider fingerprints, and code bindings as
rebuildable derived data. Schema mismatch, query-index corruption or drift,
worktree mismatch, and stale available semantic authority fail closed rather
than yielding partial definitive answers. Rebuild with `atlas build` when a
schema or query-index diagnostic names a stale graph artifact.

## Consequences

The search index accelerates deterministic lookup but is never a second source
of truth. MCP keeps graph reads frozen, makes indexed search opt-in in its tool
list, and makes `atlas_explore` unavailable to both discovery and dispatch
unless explicitly enabled. Explore source excerpts require a current hash that
matches graph metadata. KLL artifacts own durable requirements and decisions;
lifecycle and replay establish current verification evidence.
The optional watcher/daemon, pending watermark, retry counters, reader lease,
and `degraded` state are also derived runtime data. They can require a sync or
rebuild but cannot replace graph freshness, KLL, lifecycle, or trace evidence.
No-daemon reads remain a supported authority-equivalent path for the same
committed generation.
`runtime-boundary` candidates remain a `query-hint` even when they resolve to
canonical nodes. Candidate lookup follows Rust type/function namespaces and
expands inherent associated paths through indexed type declarations, but this
extra precision does not increase authority. The hints do not gain edge provenance or enter impact,
binding, lifecycle, trace, or archive authority without a later mechanism-
specific promotion gate.
The delivered impact graph therefore remains derived input until a later
intent-aware consumer joins it to explicit bindings and Contract selectors.

B5 context evidence and receipts are also derived. Stable evidence ids pin
continuation to one graph fingerprint, while retrieval/projection counts expose
loss; neither the projected body nor its receipt can promote a graph fact,
create a code binding, or satisfy a requirement or scenario.

D4 queue state, worker counters, cancellation, panic circuit, fallback, and
measurement receipts remain derived runtime evidence. Transport isolation does
not make stale graph data current. Only success carries a complete context;
non-success typed outcomes cannot be interpreted as empty proof.

## Maintenance

Revise this page when the graph authority boundary, rebuild diagnostics, or MCP
exposure policy changes.

Atlas D2 now commits the complete derived graph as one generation. This
strengthens publication consistency without promoting graph facts to KLL truth
or changing default MCP exposure.

Atlas D3 adds bounded refresh and fail-closed reclamation without changing that
decision. Post-commit pending/status maintenance failures remain warning-only
and retain conservative pending context.

Atlas B5 adds a bounded representation layer without changing graph, KLL,
lifecycle, trace, or default MCP authority.

Atlas D4 adds bounded serving and hidden MCP context opt-ins without changing
that authority or promoting worker defaults before E1.

Atlas E1 adds adoption evidence without promoting it to graph authority. A
passing gate is a human-review candidate, not an automatic MCP, B5, or worker
configuration change.
