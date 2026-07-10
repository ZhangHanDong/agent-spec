---
kind: requirement
id: REQ-RUST-ATLAS
title: "Rust Atlas Code Graph"
status: accepted
liveness: auto
tags: [atlas, code-graph, static-analysis, mcp]
---

# Rust Atlas Code Graph

## Problem

Agents working on Rust projects rediscover structure by grepping text, reading
whole files, and recompiling to find errors. That loop wastes tokens and guesses
at facts the toolchain can produce deterministically. Agents need a queryable,
incrementally invalidated project graph — symbols, module tree, impl relations,
references — built from stable-toolchain static analysis and exposed through
CLI and MCP surfaces.

## Requirements

[REQ-RUST-ATLAS-CRATE] The repository MUST become a Cargo workspace with a standalone library crate `crates/rust-atlas` that has no dependency on the `agent-spec` binary crate and can be published independently.

[REQ-RUST-ATLAS-SCHEMA] The graph schema MUST define node kinds (crate, module, struct, enum, trait, fn, impl, type alias, const, macro) and edge kinds (contains, impls-trait, impl-for, references, calls, uses-type), where every edge carries a `provenance` value of `syn`, `scip`, or `mir`, and the persisted graph carries a `schema_version`.

[REQ-RUST-ATLAS-EXTRACT] The syn extraction layer MUST build the module tree, symbol nodes with file, span, visibility, and signature, and declaration-level edges using only the stable toolchain.

[REQ-RUST-ATLAS-SCIP] The build MUST optionally ingest a SCIP index to add resolved cross-file `references` edges with provenance `scip`, and MUST degrade to syn-only extraction with a capability marker in graph metadata when no SCIP index is available.

[REQ-RUST-ATLAS-STORE] The graph MUST persist under `.agent-spec/graph/` as per-source-file JSON shards plus metadata recording a blake3 content hash for every analyzed source file.

[REQ-RUST-ATLAS-STALENESS] Commands MUST detect stale shards by hash comparison, rebuild only dirty shards incrementally, expose an `atlas check` command whose exit code reflects staleness, and report staleness instead of silently answering from an outdated graph when rebuild is disabled.

[REQ-RUST-ATLAS-CLI] The `agent-spec atlas` subcommand MUST provide `build`, `tree`, `query`, `refs`, `impls`, and `check` with machine-readable JSON output, and MUST NOT reuse or alter the existing `agent-spec graph` spec-dependency command.

[REQ-RUST-ATLAS-MCP] The existing `agent-spec mcp` server MUST expose read-only atlas tools (`atlas_tree`, `atlas_query`, `atlas_refs`, `atlas_impls`, `atlas_status`) as thin wrappers over the library API that return structured errors instead of panicking when the graph is missing.

[REQ-RUST-ATLAS-DEGRADE] Files that fail to parse MUST be recorded as `unparsed` diagnostics while the rest of the graph still builds; extraction MUST be read-only over analyzed code and MUST NOT perform network or LLM calls.
