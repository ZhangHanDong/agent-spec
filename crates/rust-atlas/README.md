# rust-atlas

`rust-atlas` is an evidence-aware Rust code graph for AI agents. It extracts a
stable-toolchain `syn` baseline, overlays optional compiler and language-server
evidence, publishes immutable graph generations, and exposes deterministic,
bounded queries. It performs no network or LLM calls.

## Graph Construction

- Cargo-aware workspace, crate, module, and source ownership discovery.
- Symbols with canonical ids, file spans, visibility, signatures, and docs.
- `contains`, `calls`, `references`, `uses-type`, `impls-trait`, and `impl-for`
  edges with provenance, resolution, dispatch, confidence, extractor, site,
  candidates, and evidence.
- Optional rust-analyzer SCIP overlay for resolved cross-file semantics.
- Feature-gated, versioned MIR artifact/driver consumer for call and CFG facts;
  the crate does not ship an official `rustc_public` producer.
- Opt-in bounded trait-dispatch candidates and query-time runtime-boundary
  hints that remain separate from authoritative graph facts.

Unparsable files produce diagnostics instead of aborting the whole graph. A
schema mismatch fails closed and requires rebuilding the derived graph.

## Storage And Freshness

Source-owned JSON shards, metadata, the derived query index, input plan, and
overlay capabilities commit as one immutable generation. BLAKE3 fingerprints
track source and graph identity. Queries distinguish repository, Git worktree,
generation, and independent `syn`, SCIP, and MIR freshness.

Incremental builds use a bounded reverse-dependent frontier, recoverable orphan
work, cancellation checkpoints, resource admission, and atomic generation
publication. The optional watcher/daemon persists pending watermarks and keeps
reader leases on pinned generations. Concurrent query workers are opt-in and
use bounded queues, deadlines, memory admission, cancellation, and typed
backpressure outcomes.

## Query Surface

The library provides low-level `tree`, `query`, `search`, `refs`, `impls`,
`status`, and `check` operations plus:

- `explore`: compact or deep ranked graph context with fresh source excerpts;
- `flow`: bounded, explainable paths with explicit ambiguity and truncation;
- `impact`: reverse dependency paths from a symbol;
- `affected`: changed files mapped to affected nodes without guessing tests;
- `context`: deterministic `symbol`, `flow`, `architecture`, or `impact`
  evidence projections with omission manifests and query receipts.

The `agent-spec` binary exposes these through `agent-spec atlas ...` and through
read-only, frozen MCP tools. Search, explore, context, and concurrent MCP modes
remain opt-in until real Agent evaluation supports changing the defaults.

## Version 0.3

Version 0.3 uses graph schema v6. Graphs created by 0.2 are derived cache data
and must be rebuilt. The public Rust crate remains independently versioned from
the `agent-spec` CLI.

See the repository's
[Atlas roadmap](https://github.com/ZhangHanDong/agent-spec/blob/main/docs/atlas-roadmap.md),
[incremental build guide](https://github.com/ZhangHanDong/agent-spec/blob/main/docs/atlas-incremental-builds.md),
and
[query context guide](https://github.com/ZhangHanDong/agent-spec/blob/main/docs/atlas-query-context.md).
