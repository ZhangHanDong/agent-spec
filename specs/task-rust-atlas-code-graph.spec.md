spec: task
name: "Rust Atlas Code Graph (Phase 1)"
tags: [atlas, code-graph, static-analysis, mcp]
satisfies: [REQ-RUST-ATLAS]
estimate: 1w
---

## Intent

Add a standalone workspace library crate `rust-atlas` that statically analyzes a
Rust project into a persistent, incrementally invalidated project graph —
module tree, symbol nodes, impl relations, reference edges — so agents query
structure instead of grepping text and re-reading whole files. Expose the graph
through the library API, a new `agent-spec atlas` CLI subcommand, and read-only
MCP tools on the existing `agent-spec mcp` server. This is Phase 1 of the atlas
roadmap; MIR enhancement and KLL integration are later phases.

## Decisions

- Convert the repository to a Cargo workspace: root `agent-spec` package stays the binary; new library lives at `crates/rust-atlas` with no dependency on `agent-spec`.
- Extraction baseline is `syn` (full parse, stable toolchain only); optional SCIP enhancement ingests a `rust-analyzer` SCIP index passed via `--scip <index>` to add resolved cross-file `references` edges.
- One unified schema for all phases: every edge carries `provenance` (`syn` | `scip` | `mir`); persisted graph metadata carries `schema_version` and a `capability` record (e.g. `scip` present or absent). Phase 1 emits no `mir` edges.
- Node kinds: crate, module, struct, enum, trait, fn, impl, type alias, const, macro — each with canonical symbol-path id, file, span, visibility, and signature summary. Edge kinds: `contains`, `impls-trait`, `impl-for`, `references`, `calls`, `uses-type`.
- Storage layout is `.agent-spec/graph/`: `meta.json` plus per-source-file JSON shards; every analyzed file records a `blake3` content hash for invalidation.
- Commands rebuild only dirty shards by default; `--frozen` disables rebuild and instead reports a `stale` file list alongside results; `atlas check` returns a non-zero exit code when any shard is stale.
- CLI surface: `atlas build [--full] [--scip <index>]`, `atlas tree`, `atlas query <symbol>`, `atlas refs <symbol>`, `atlas impls <name>`, `atlas check`; all support `--format json`. The subcommand is named `atlas` because `agent-spec graph` already renders the spec dependency DAG.
- MCP tools added to the existing server: `atlas_tree`, `atlas_query`, `atlas_refs`, `atlas_impls`, `atlas_status` — thin wrappers over the library query API returning structured JSON errors when the graph is missing.
- Files that fail to parse are recorded as `unparsed` diagnostics; the rest of the graph still builds and the command exits 0.
- New dependencies (`syn`, `blake3`, `ignore`) are confined to `crates/rust-atlas`; the root package gains only the `rust-atlas` path dependency.
- `rust-atlas` performs no network calls, no LLM calls, and never writes to analyzed source files.

## Boundaries

### Allowed Changes
- Cargo.toml
- Cargo.lock
- crates/rust-atlas/**
- src/main.rs
- src/spec_mcp/tools.rs
- fixtures/atlas/**
- specs/task-rust-atlas-code-graph.spec.md
- knowledge/requirements/req-rust-atlas.md
- docs/superpowers/plans/2026-07-11-rust-atlas-code-graph.md
- README.md
- AGENTS.md
- CHANGELOG.md
- skills/agent-spec-tool-first/**

### Forbidden
- Do not require a nightly toolchain anywhere in this phase.
- Do not modify `src/spec_knowledge/**`; KLL integration is a later phase.
- Do not rename, remove, or change the behavior of the existing `agent-spec graph` command.
- Do not add network calls or LLM calls to `rust-atlas`.
- Do not add new dependencies to the root `agent-spec` package other than the `rust-atlas` path dependency.
- Do not introduce `.unwrap()` or `.expect()` in non-test code.

## Out of Scope

- MIR extraction (Charon / rustc_public) — Phase 2 (`specs/roadmap/task-atlas-mir-layer.spec.md`)
- KLL integration (spec boundaries/trace referencing graph nodes) — Phase 3 (`specs/roadmap/task-atlas-kll-integration.spec.md`)
- Precise `calls` edges beyond SCIP resolution; call-graph completeness
- Patch/write operations over analyzed code — atlas is read-only analysis
- Non-Rust languages and cross-project graphs

## Questions

- [x] rust-analyzer SCIP 输出的最低兼容版本策略如何声明？（已解决：构建时探测 `rust-analyzer --version` 并写入 meta `capability.scip_version`；低于 crates/rust-atlas README 声明的最低版本或缺席时降级 syn-only，不作为硬依赖。）
- [x] 超大 workspace（>2000 源文件）是否需要目录级分片聚合以控制分片数量？（已解决：Phase 1 保持每文件分片——失效精度优先，2k 级 JSON 文件在现代文件系统无压力；目录级聚合列为非目标，仅当 `atlas build` 实测出现墙钟或 inode 压力时再立新任务。）

## Completion Criteria

<!-- lint-ack: output-mode-coverage — file side effects are asserted directly by the incremental-invalidation scenarios (shard rewrite and meta.json hash assertions) -->

### Rule: graph-extraction — syn 层构建符号图

Scenario: Build produces module tree and symbol nodes
  Test:
    Filter: test_atlas_builds_module_tree_and_symbol_nodes
    Level: integration
  Given the fixture crate `fixtures/atlas/basic` with nested modules, structs, enums, traits, and functions
  When `atlas build` runs against the fixture
  Then `.agent-spec/graph/meta.json` records `schema_version` and a `blake3` hash per source file
  And the graph contains crate, module, struct, trait, and fn nodes with file, span, and visibility
  And `contains` edges connect the module tree with provenance `syn`

Scenario: Build produces impl relation edges
  Test:
    Filter: test_atlas_builds_impl_edges
    Level: integration
  Given the fixture crate defines `trait Store` and `impl Store for MemStore`
  When `atlas build` runs
  Then the graph contains `impls-trait` and `impl-for` edges with provenance `syn`

Scenario: A file that fails to parse degrades to a diagnostic
  Test:
    Filter: test_atlas_records_unparsed_file_diagnostic
    Level: integration
  Given the fixture crate contains one file with a syntax error
  When `atlas build` runs
  Then the command exits 0
  And the broken file is recorded as an `unparsed` diagnostic
  And nodes from all other files are present in the graph

### Rule: incremental-invalidation — 哈希增量失效

Scenario: Rebuild touches only dirty shards
  Test:
    Filter: test_atlas_incremental_rebuild_only_dirty_shards
    Level: integration
  Given a built graph for the fixture crate
  When one source file changes and `atlas build` runs again
  Then only the shard for the changed file is rewritten
  And the untouched shards remain byte-identical

Scenario: Check reports stale files with a non-zero exit code
  Test:
    Filter: test_atlas_check_reports_stale_files
    Level: integration
  Given a built graph and one modified source file
  When `atlas check` runs
  Then the exit code is non-zero
  And the JSON output lists the modified file as stale

Scenario: Frozen query reports staleness instead of rebuilding
  Test:
    Filter: test_atlas_frozen_query_reports_stale_warning
    Level: integration
  Given a built graph and one modified source file
  When `atlas query` runs with `--frozen`
  Then the result includes a `stale` field listing the modified file
  And no shard is rewritten

### Rule: query-surface — 查询命令

Scenario: Query returns symbol facts and adjacent edges
  Test:
    Filter: test_atlas_query_returns_symbol_facts
    Level: integration
  Given a built graph for the fixture crate
  When `atlas query` runs for a struct's canonical symbol path
  Then the JSON output contains the node's kind, file, span, signature, and its adjacent edges

Scenario: Tree renders a module outline
  Test:
    Filter: test_atlas_tree_renders_module_outline
    Level: integration
  Given a built graph for the fixture crate
  When `atlas tree` runs
  Then the JSON output nests symbols under their modules in deterministic order

Scenario: Unknown symbol returns a structured error
  Test:
    Filter: test_atlas_query_unknown_symbol_errors
    Level: integration
  Given a built graph
  When `atlas query` runs for a symbol path that does not exist
  Then the exit code is non-zero
  And the error output contains diagnostic code `atlas-unknown-symbol`

Scenario: Query without a built graph instructs to build first
  Test:
    Filter: test_atlas_query_without_graph_errors
    Level: integration
  Given a project with no `.agent-spec/graph/` directory
  When `atlas query` runs
  Then the exit code is non-zero
  And the error output names `atlas build` as the required first step

### Rule: scip-enhancement — SCIP 可选增强与降级

Scenario: SCIP index adds resolved reference edges
  Test:
    Filter: test_atlas_ingests_scip_index_reference_edges
    Level: integration
  Given a pre-generated SCIP index fixture for the fixture crate
  When `atlas build` runs with `--scip <index>`
  Then cross-file `references` edges with provenance `scip` are present
  And graph metadata records the `scip` capability as present

Scenario: Build degrades gracefully without SCIP
  Test:
    Filter: test_atlas_build_degrades_without_scip
    Level: integration
  Given no SCIP index is supplied
  When `atlas build` runs
  Then the build succeeds with only provenance `syn` edges
  And graph metadata records the `scip` capability as absent

### Rule: mcp-tools — MCP 只读工具

Scenario: MCP atlas_query returns symbol JSON
  Test:
    Filter: test_mcp_atlas_query_returns_symbol_json
    Level: integration
  Given a built graph and a running MCP tool registry
  When the `atlas_query` tool is invoked with a known symbol path
  Then the tool result contains the same node facts as the CLI `atlas query` output

Scenario: MCP atlas tools report a missing graph as a structured error
  Test:
    Filter: test_mcp_atlas_tools_report_missing_graph
    Level: integration
  Given a project with no built graph
  When the `atlas_status` tool is invoked
  Then the tool returns a structured error payload naming `atlas build`
  And the server does not panic
