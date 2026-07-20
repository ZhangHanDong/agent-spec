---
title: "Rust Atlas Architecture"
type: architecture
source_files:
  - crates/rust-atlas/src/lib.rs
  - crates/rust-atlas/src/index.rs
  - crates/rust-atlas/src/status.rs
  - crates/rust-atlas/src/traversal.rs
  - crates/rust-atlas/src/flow.rs
  - crates/rust-atlas/src/impact.rs
  - crates/rust-atlas/src/affected.rs
  - crates/rust-atlas/src/explore.rs
  - src/main.rs
  - src/spec_mcp/tools.rs
  - specs/task-atlas-explore-flow-impact.spec.md
tags:
  - atlas
  - code-graph
  - query-index
status: active
---

# Rust Atlas Architecture

## Role

Rust Atlas persists the Rust code graph as JSON shards and rebuilds a derived
query index for deterministic lookup. Schema v6 records edge occurrence and
evidence fields alongside graph nodes and edges. The CLI builds, searches,
reports status, checks syn staleness, composes bounded explore context, explains
flow paths, and calculates reverse impact from symbols or changed files.
`traversal.rs` owns the shared path, hop, state, limits, and ordering contract;
surface modules compose it instead of redefining evidence or freshness.
Query-index diagnostics require a rebuild instead of a shard-scan fallback.

## Boundaries

The graph and index are derived working data, not KLL truth. MCP Atlas reads
are frozen and read-only; `atlas_search` is listed only with
`AGENT_SPEC_MCP_ATLAS_SEARCH=1`, while `atlas_explore` is unavailable unless
`AGENT_SPEC_MCP_ATLAS_EXPLORE=1`. Explore excerpts require current source hashes
and fixed output budgets. Affected results expose code nodes and paths but do
not infer test coverage. The status model compares graph identity and reports
syn, SCIP, and MIR separately, so consumers do not infer semantic freshness
from syn refresh.

## Maintenance

Update this page when graph schema, read APIs, query-index validation, or MCP
tool exposure changes.
