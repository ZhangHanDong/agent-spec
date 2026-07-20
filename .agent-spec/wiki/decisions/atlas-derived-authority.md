---
title: "Atlas Derived Authority"
type: decision
source_files:
  - crates/rust-atlas/src/lib.rs
  - crates/rust-atlas/src/index.rs
  - crates/rust-atlas/src/status.rs
  - src/spec_mcp/tools.rs
  - docs/atlas-roadmap.md
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

## Maintenance

Revise this page when the graph authority boundary, rebuild diagnostics, or MCP
exposure policy changes.
