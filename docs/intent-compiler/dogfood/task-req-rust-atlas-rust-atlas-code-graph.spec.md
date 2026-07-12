spec: task
name: "Rust Atlas Code Graph"
tags: [requirements, generated-draft, imported-yaml]
satisfies: [REQ-RUST-ATLAS]
---

## Intent

Agents working on Rust projects rediscover structure by grepping text, reading whole files, and recompiling to find errors. That loop wastes tokens and guesses at facts the toolchain can produce deterministically. Agents need a queryable, incrementally invalidated project graph — symbols, module tree, impl relations, references — built from stable-toolchain static analysis and exposed through CLI and MCP surfaces.

## Decisions

- Generated draft from KLL requirement artifact; human review must confirm boundaries and test selectors before implementation.
- The repository MUST become a Cargo workspace with a standalone library crate at `crates/rust-atlas`.
- The `rust-atlas` crate MUST NOT depend on the `agent-spec` binary crate.
- The graph schema MUST define node kinds crate, module, struct, enum, trait, fn, impl, type alias, const, and macro.
- Every edge MUST carry a `provenance` value of `syn`, `scip`, or `mir` on the edge kinds contains, impls-trait, impl-for, references, calls, and uses-type.
- The persisted graph MUST carry a `schema_version` field.
- The syn extraction layer MUST build the module tree, symbol nodes with file, span, visibility, and signature, and declaration-level edges.
- The syn extraction layer MUST compile and run on the stable toolchain without nightly features.
- The build MUST support optional ingestion of a SCIP index that adds resolved cross-file `references` edges with provenance `scip`.
- When no SCIP index is available, the build MUST complete syn-only and record the absent capability in graph metadata.
- The graph MUST persist under `.agent-spec/graph/` as per-source-file JSON shards plus metadata recording a blake3 content hash for every analyzed source file.
- Commands MUST detect stale shards by comparing stored content hashes against current file hashes.
- Incremental rebuild MUST rewrite only the shards of changed files.
- The `atlas check` command MUST exit non-zero when any shard is stale.
- When rebuild is disabled, query commands MUST attach the stale file list to every result served from an outdated graph.
- The `agent-spec atlas` subcommand MUST provide `build`, `tree`, `query`, `refs`, `impls`, and `check` with machine-readable JSON output.
- The atlas surface MUST NOT reuse, rename, or alter the existing `agent-spec graph` spec-dependency command.
- The existing `agent-spec mcp` server MUST expose read-only atlas tools (`atlas_tree`, `atlas_query`, `atlas_refs`, `atlas_impls`, `atlas_status`) as thin wrappers over the library API.
- When the graph is missing or stale, atlas MCP tools MUST return a structured error payload instead of panicking.
- A file that fails to parse MUST be recorded as an `unparsed` diagnostic while the rest of the graph still builds.
- Extraction MUST treat analyzed source code as read-only.
- The `rust-atlas` crate MUST NOT perform network or LLM calls.
- Satisfying specs MUST include negative scenarios covering unknown symbol lookups, queries before any build, stale graphs with rebuild disabled, missing SCIP indexes, and unparsable source files.

## Boundaries

### Allowed Changes
- src/**
- tests/**

### Forbidden
- Do not weaken or remove the source requirement clauses.
- Do not mark this generated draft complete until each `Test:` selector names a real test.

## Completion Criteria

Scenario: Build without rust-analyzer present
  Test: pending_req_rust_atlas_build_without_rust_analyzer_present
  Given no SCIP index is supplied and rust-analyzer is absent
  When `atlas build` runs
  Then the build succeeds with only provenance `syn` edges and metadata records the `scip` capability as absent

Scenario: Frozen query on a stale graph
  Test: pending_req_rust_atlas_frozen_query_on_a_stale_graph
  Given a built graph and one modified source file
  When `atlas query` runs with `--frozen`
  Then the result includes a `stale` field listing the modified file and no shard is rewritten

Scenario: MCP tool without a built graph
  Test: pending_req_rust_atlas_mcp_tool_without_a_built_graph
  Given a project with no built graph
  When the `atlas_status` tool is invoked
  Then the tool returns a structured error payload naming `atlas build` and the server does not panic

Scenario: Syntax error in one source file
  Test: pending_req_rust_atlas_syntax_error_in_one_source_file
  Given the analyzed crate contains one file with a syntax error
  When `atlas build` runs
  Then the command exits 0, the broken file is recorded as an `unparsed` diagnostic, and nodes from all other files are present

## Questions

- Source trace: imported-yaml: docs/rust-atlas-requirements.yaml
- Replace pending test selectors with real test names before lifecycle verification.
