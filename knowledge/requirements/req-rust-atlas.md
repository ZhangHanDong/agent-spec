---
kind: requirement
id: REQ-RUST-ATLAS
title: "Rust Atlas Code Graph"
status: proposed
liveness: auto
tags: [atlas, code-graph, static-analysis, mcp]
---

# Rust Atlas Code Graph

## Problem

Agents working on Rust projects rediscover structure by grepping text, reading whole files, and recompiling to find errors. That loop wastes tokens and guesses at facts the toolchain can produce deterministically. Agents need a queryable, incrementally invalidated project graph — symbols, module tree, impl relations, references — built from stable-toolchain static analysis and exposed through CLI and MCP surfaces.

## Requirements

[REQ-RUST-ATLAS-CRATE] The repository MUST become a Cargo workspace with a standalone library crate at `crates/rust-atlas`.

[REQ-RUST-ATLAS-CRATE-INDEPENDENT] The `rust-atlas` crate MUST NOT depend on the `agent-spec` binary crate.

[REQ-RUST-ATLAS-SCHEMA-NODES] The graph schema MUST define node kinds crate, module, struct, enum, trait, fn, impl, type alias, const, and macro.

[REQ-RUST-ATLAS-SCHEMA-EDGES] Every edge MUST carry a `provenance` value of `syn`, `scip`, or `mir` on the edge kinds contains, impls-trait, impl-for, references, calls, and uses-type.

[REQ-RUST-ATLAS-SCHEMA-VERSION] The persisted graph MUST carry a `schema_version` field.

[REQ-RUST-ATLAS-EXTRACT] The syn extraction layer MUST build the module tree, symbol nodes with file, span, visibility, and signature, and declaration-level edges.

[REQ-RUST-ATLAS-EXTRACT-STABLE] The syn extraction layer MUST compile and run on the stable toolchain without nightly features.

[REQ-RUST-ATLAS-SCIP] The build MUST support optional ingestion of a SCIP index that adds resolved cross-file `references` edges with provenance `scip`.

[REQ-RUST-ATLAS-SCIP-DEGRADE] When no SCIP index is available, the build MUST complete syn-only and record the absent capability in graph metadata.

[REQ-RUST-ATLAS-STORE] The graph MUST persist under `.agent-spec/graph/` as per-source-file JSON shards plus metadata recording a blake3 content hash for every analyzed source file.

[REQ-RUST-ATLAS-STALE-DETECT] Commands MUST detect stale shards by comparing stored content hashes against current file hashes.

[REQ-RUST-ATLAS-STALE-REBUILD] Incremental rebuild MUST rewrite only the shards of changed files.

[REQ-RUST-ATLAS-STALE-CHECK] The `atlas check` command MUST exit non-zero when any shard is stale.

[REQ-RUST-ATLAS-STALE-FROZEN] When rebuild is disabled, query commands MUST attach the stale file list to every result served from an outdated graph.

[REQ-RUST-ATLAS-CLI] The `agent-spec atlas` subcommand MUST provide `build`, `tree`, `query`, `refs`, `impls`, and `check` with machine-readable JSON output.

[REQ-RUST-ATLAS-CLI-NAME] The atlas surface MUST NOT reuse, rename, or alter the existing `agent-spec graph` spec-dependency command.

[REQ-RUST-ATLAS-MCP] The existing `agent-spec mcp` server MUST expose read-only atlas tools (`atlas_tree`, `atlas_query`, `atlas_refs`, `atlas_impls`, `atlas_status`) as thin wrappers over the library API.

[REQ-RUST-ATLAS-MCP-ERRORS] When the graph is missing or stale, atlas MCP tools MUST return a structured error payload instead of panicking.

[REQ-RUST-ATLAS-PARSE-DEGRADE] A file that fails to parse MUST be recorded as an `unparsed` diagnostic while the rest of the graph still builds.

[REQ-RUST-ATLAS-READ-ONLY] Extraction MUST treat analyzed source code as read-only.

[REQ-RUST-ATLAS-NO-NETWORK] The `rust-atlas` crate MUST NOT perform network or LLM calls.

[REQ-RUST-ATLAS-NEGATIVE] Satisfying specs MUST include negative scenarios covering unknown symbol lookups, queries before any build, stale graphs with rebuild disabled, missing SCIP indexes, and unparsable source files.

## Scenarios

Scenario: Build without rust-analyzer present
  Given no SCIP index is supplied and rust-analyzer is absent
  When `atlas build` runs
  Then the build succeeds with only provenance `syn` edges and metadata records the `scip` capability as absent

Scenario: Frozen query on a stale graph
  Given a built graph and one modified source file
  When `atlas query` runs with `--frozen`
  Then the result includes a `stale` field listing the modified file and no shard is rewritten

Scenario: MCP tool without a built graph
  Given a project with no built graph
  When the `atlas_status` tool is invoked
  Then the tool returns a structured error payload naming `atlas build` and the server does not panic

Scenario: Syntax error in one source file
  Given the analyzed crate contains one file with a syntax error
  When `atlas build` runs
  Then the command exits 0, the broken file is recorded as an `unparsed` diagnostic, and nodes from all other files are present

## Source Trace

- requirements conversation: atlas planning session 2026-07-11, human-confirmed
- design: docs/superpowers/specs/2026-07-11-rust-atlas-design.md
- plan: docs/superpowers/plans/2026-07-11-rust-atlas-code-graph.md
- staged contract: specs/roadmap/task-rust-atlas-code-graph.spec.md
