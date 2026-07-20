---
title: "Main CLI"
type: module
source_files:
  - src/main.rs
  - src/atlas_daemon.rs
  - crates/rust-atlas/src/context.rs
tags:
  - cli
  - commands
status: draft
---

# Main CLI

## Role

Primary command dispatch and text/json formatting entrypoint. Atlas `flow`
always emits JSON and accepts no `--format` flag; disconnected results may
include bounded `runtime_boundaries` while preserving the existing flow state.

`atlas context <query> --profile symbol|flow|architecture|impact` is an
additive CLI surface over the B5 library pipeline. It supports graph-pinned
`--after` continuation, explicit byte/relevance limits, failure evidence,
frozen reads, and finalized JSON byte receipts. It does not alter `explore` or
the default MCP tool list.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 adds deterministic build flags for input identity, frontier size,
batch size, and working bytes; command dispatch ownership remains here.

Atlas D3 adds `atlas daemon start|serve|status|sync|stop`. The optional watcher
persists a pending watermark and typed `degraded` state; queries use a reader
lease and preserve no-daemon behavior. Live runtime state is derived and never
replaces graph freshness, KLL, or lifecycle authority.

Atlas B5 was reviewed here; command dispatch exposes context compilation only
through CLI/library until E1 evaluates any default-surface promotion.
