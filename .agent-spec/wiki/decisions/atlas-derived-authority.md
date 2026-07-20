---
title: "Atlas Derived Authority"
type: decision
source_files:
  - crates/rust-atlas/src/lib.rs
  - crates/rust-atlas/src/index.rs
  - crates/rust-atlas/src/status.rs
  - crates/rust-atlas/src/flow.rs
  - crates/rust-atlas/src/runtime_boundary.rs
  - src/spec_mcp/tools.rs
  - docs/atlas-roadmap.md
  - docs/atlas-runtime-boundaries.md
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
`runtime-boundary` candidates remain a `query-hint` even when they resolve to
canonical nodes. Candidate lookup follows Rust type/function namespaces and
expands inherent associated paths through indexed type declarations, but this
extra precision does not increase authority. The hints do not gain edge provenance or enter impact,
binding, lifecycle, trace, or archive authority without a later mechanism-
specific promotion gate.
The delivered impact graph therefore remains derived input until a later
intent-aware consumer joins it to explicit bindings and Contract selectors.

## Maintenance

Revise this page when the graph authority boundary, rebuild diagnostics, or MCP
exposure policy changes.
