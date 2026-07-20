---
title: "Rust Atlas Architecture"
type: architecture
source_files:
  - crates/rust-atlas/src/lib.rs
  - crates/rust-atlas/src/index.rs
  - crates/rust-atlas/src/status.rs
  - src/main.rs
  - src/spec_mcp/tools.rs
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
reports status, and checks syn staleness; query-index diagnostics require a
rebuild instead of a shard-scan fallback.

## Boundaries

The graph and index are derived working data, not KLL truth. MCP Atlas reads
are frozen and read-only; `atlas_search` is listed only with
`AGENT_SPEC_MCP_ATLAS_SEARCH=1`. The status model compares graph identity and
reports syn, SCIP, and MIR separately, so read consumers do not infer semantic
freshness from syn refresh.

## Maintenance

Update this page when graph schema, read APIs, query-index validation, or MCP
tool exposure changes.
