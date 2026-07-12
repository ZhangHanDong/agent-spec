# Rust Atlas Code Graph Roadmap & Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give agents a queryable, incrementally invalidated static-analysis graph of any Rust project — symbols, module tree, impl relations, references — via a standalone `rust-atlas` library crate, an `agent-spec atlas` CLI, and read-only MCP tools, so agents query structure instead of grepping text.

**Architecture:** `crates/rust-atlas` is a workspace library with zero `agent-spec` dependency (independently publishable). One schema serves all phases: edges carry `provenance` (`syn` | `scip` | `mir`), so later MIR facts overlay without schema changes. The graph persists under `.agent-spec/graph/` as per-source-file JSON shards with blake3 hashes; staleness is detected by hash comparison and repaired by incremental rebuild — the same problem zerolang's "stale hash interception" solves, done here with stable Rust tooling.

**Tech Stack:** Rust 2024, stable toolchain only for Phase 1. New deps confined to `crates/rust-atlas`: `syn` (full), `blake3`, `ignore`, plus existing `serde`/`serde_json`/`thiserror`. Root package gains only the `rust-atlas` path dependency. No network calls, no LLM calls.

## Global Constraints

- The CLI subcommand is `atlas` — `agent-spec graph` (spec dependency DAG) must keep its current name and behavior.
- `rust-atlas` is read-only over analyzed code; it never edits user source files.
- Parse failures degrade to `unparsed` diagnostics; they never abort a build.
- SCIP is an optional enhancer: absent rust-analyzer output, builds succeed syn-only and metadata records the capability as absent.
- `src/spec_knowledge/**` is untouched until Phase 3.
- Contract: `specs/task-rust-atlas-code-graph.spec.md` (satisfies `REQ-RUST-ATLAS`); verify with the agent-spec lifecycle before stamping.

---

## Roadmap Overview

| Phase | Scope | Toolchain | Spec |
|-------|-------|-----------|------|
| 1 (this plan) | workspace conversion, syn extraction, sharded store + hashes, CLI, optional SCIP, MCP tools | stable | `specs/task-rust-atlas-code-graph.spec.md` |
| 2 | MIR layer: evaluate Charon first, else `rustc_public` driver; precise `calls` edges + CFG summaries as a feature-gated overlay | nightly (opt-in) | `specs/roadmap/task-atlas-mir-layer.spec.md` |
| 3 | KLL integration: spec boundaries/trace reference atlas nodes; lint validates declared symbols exist and detects drift | stable | `specs/roadmap/task-atlas-kll-integration.spec.md` |

Phase 2 and 3 specs live in `specs/roadmap/` and are promoted to `specs/` when activated (self-hosting convention).

---

## Target File Structure (Phase 1)

- Modify: `Cargo.toml` — add `[workspace]` with `members = ["crates/rust-atlas"]`; add `rust-atlas` path dependency.
- Create: `crates/rust-atlas/Cargo.toml` — lib crate, edition 2024, deny unsafe/unwrap/expect lints matching root policy.
- Create: `crates/rust-atlas/src/model.rs` — node/edge/provenance/meta types, `schema_version`, serde derives.
- Create: `crates/rust-atlas/src/extract/syn_layer.rs` — file → nodes + declaration-level edges.
- Create: `crates/rust-atlas/src/extract/scip_layer.rs` — SCIP index ingestion → resolved `references` edges.
- Create: `crates/rust-atlas/src/store.rs` — shard read/write under `.agent-spec/graph/`, blake3 hashing, dirty detection.
- Create: `crates/rust-atlas/src/query.rs` — tree/query/refs/impls, staleness reporting, `--frozen` semantics.
- Create: `crates/rust-atlas/src/lib.rs` — public API facade.
- Modify: `src/main.rs` — `Atlas` subcommand (build/tree/query/refs/impls/check).
- Modify: `src/spec_mcp/tools.rs` — `atlas_tree`, `atlas_query`, `atlas_refs`, `atlas_impls`, `atlas_status`.
- Create: `fixtures/atlas/basic/**` — small crate: nested modules, trait + impl, cross-file references, one syntax-error file variant.
- Create: `fixtures/atlas/scip/**` — pre-generated SCIP index fixture.
- Modify: `README.md`, `AGENTS.md`, `CHANGELOG.md`, `skills/agent-spec-tool-first/**` — document the atlas commands.

---

## Phase 1 Tasks

### Task 1: Workspace conversion
- [ ] Add `[workspace]` to root `Cargo.toml`; scaffold `crates/rust-atlas` with matching lint policy
- [ ] Root package depends on `rust-atlas` by path; `cargo build` and existing tests stay green

### Task 2: Graph model + store
- [ ] Implement `model.rs` (nodes, edges, provenance, meta with `schema_version` + capability record)
- [ ] Implement `store.rs`: per-file shards, blake3 hashes, dirty-file detection
- [ ] Tests: shard round-trip; incremental rebuild rewrites only dirty shards (`test_atlas_incremental_rebuild_only_dirty_shards`)

### Task 3: syn extraction layer
- [ ] Walk project with `ignore`, parse each file with `syn`, emit nodes + `contains`/`impls-trait`/`impl-for`/`uses-type` edges
- [ ] Parse failures become `unparsed` diagnostics (`test_atlas_records_unparsed_file_diagnostic`)
- [ ] Tests: `test_atlas_builds_module_tree_and_symbol_nodes`, `test_atlas_builds_impl_edges`

### Task 4: Query layer
- [ ] Implement tree/query/refs/impls with deterministic ordering; staleness check + auto-rebuild + `--frozen`
- [ ] Tests: `test_atlas_query_returns_symbol_facts`, `test_atlas_tree_renders_module_outline`, `test_atlas_query_unknown_symbol_errors`, `test_atlas_query_without_graph_errors`, `test_atlas_frozen_query_reports_stale_warning`

### Task 5: CLI wiring
- [ ] Add `atlas` subcommand family to `main.rs` with `--format json`
- [ ] Tests: `test_atlas_check_reports_stale_files` (exit codes), CLI JSON snapshots against fixture

### Task 6: SCIP enhancement
- [ ] Ingest SCIP index (`--scip <index>`) → `references` edges with provenance `scip`; capability marker in meta
- [ ] Tests: `test_atlas_ingests_scip_index_reference_edges`, `test_atlas_build_degrades_without_scip`

### Task 7: MCP tools
- [ ] Register five read-only atlas tools in `spec_mcp/tools.rs`; structured errors on missing graph
- [ ] Tests: `test_mcp_atlas_query_returns_symbol_json`, `test_mcp_atlas_tools_report_missing_graph`

### Task 8: Docs + lifecycle
- [ ] Update README/AGENTS/CHANGELOG/skill docs
- [ ] Run `agent-spec lifecycle specs/task-rust-atlas-code-graph.spec.md --code .` to green, then stamp

---

## Phase 2 Sketch (MIR layer — not in this plan's scope)

- Evaluate Charon as the extractor (serialized ULLBC/LLBC) before writing a custom `rustc_public` driver.
- MIR facts overlay as higher-provenance `calls` edges and per-fn CFG summaries; schema unchanged.
- Feature-gated (`--features mir`), nightly required only when enabled; degrade to Phase 1 behavior otherwise.

## Phase 3 Sketch (KLL integration — not in this plan's scope)

- Spec boundaries/trace may reference atlas node ids; `lint-knowledge` validates referenced symbols exist.
- Wiki/architecture inventory can cite atlas facts for staleness detection of documented symbols.
