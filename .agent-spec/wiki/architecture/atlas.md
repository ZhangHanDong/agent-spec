---
title: "Rust Atlas Architecture"
type: architecture
source_files:
  - crates/rust-atlas/src/lib.rs
  - crates/rust-atlas/src/index.rs
  - crates/rust-atlas/src/generation.rs
  - crates/rust-atlas/src/live.rs
  - crates/rust-atlas/src/locking.rs
  - crates/rust-atlas/src/scope.rs
  - crates/rust-atlas/src/sync.rs
  - crates/rust-atlas/src/watch.rs
  - crates/rust-atlas/src/input_plan.rs
  - crates/rust-atlas/src/incremental.rs
  - crates/rust-atlas/src/status.rs
  - crates/rust-atlas/src/traversal.rs
  - crates/rust-atlas/src/flow.rs
  - crates/rust-atlas/src/runtime_boundary.rs
  - crates/rust-atlas/src/impact.rs
  - crates/rust-atlas/src/affected.rs
  - crates/rust-atlas/src/explore.rs
  - src/main.rs
  - src/atlas_daemon.rs
  - src/spec_mcp/tools.rs
  - docs/atlas-live-runtime.md
  - specs/task-atlas-explore-flow-impact.spec.md
  - specs/task-atlas-runtime-boundary-hints.spec.md
  - specs/task-atlas-incremental-hardening.spec.md
  - specs/task-atlas-live-runtime.spec.md
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
For disconnected flow, `runtime_boundary.rs` scans fresh Rust AST function
bodies in source-first static-reachability order and returns bounded heuristic
continuation hints for async, channel, callback, reflection, and route sites.
Receiver roles come only from the AST access chain that produces the receiver,
while qualified-self candidates retain their self type, trait path, generic
arguments, and member during index lookup. Stale source or semantic layers do
not expand the scan frontier; default trait methods derive their declaration
module from graph containment before resolving lowercase relative paths or
qualified `Self` trait members. Framework-route hints preserve handlers from
both `route(path, handler)` and `service(handler)` forms. Generic reflection
text remains visible while candidate lookup resolves its indexed type
declaration and rejects same-symbol value functions. Callable hints retain only
function nodes; `crate::Type::method` and related source-relative paths expand
through the indexed type declaration to the canonical inherent-impl method.
Namespace filtering runs before candidate fan-out. Bare candidates prefer an
exact symbol in the source module before global suffix fallback. Frontier
construction and AST scanning share one hash-validated per-file cache, applying
node and byte budgets before another source read.

Build authority is published as one immutable committed generation. The
content-addressed Cargo input plan caches target metadata but source module
ownership is reconstructed each build; automatic stale refresh retains the
committed feature, target, and cfg inputs. Dirty declarations produce a bounded
reverse-dependent frontier; overflow and capability changes become explicit
full passes. Frontier planning, resolution, and validation stream shard batches
under source, serialized-graph, and overlay byte admission. Resolution work is
persisted as an orphan queue before processing, and the queue is cleared only
after pointer commit. A failed post-commit clear rebases the queue for the next
recovery build. Healthy zero-change builds validate artifact digests and
perform no staging, resolution, validation, or authority rewrite. Every read
surface pins and reports one generation id.
Only current-transaction staging is cleaned in D2. D3 adds an optional bounded
watcher and local daemon over the same `AtlasScope`. A persisted pending
watermark survives failed sync; separate retry budgets expose `degraded`
instead of retrying forever. Query results scope pending paths to represented
files while global status retains the complete journal.

Every query owns a reader lease for its pinned generation. Under the
single-writer lock, reclamation removes an old generation only after an
exclusive reader scan proves it inactive; malformed or unreadable lease state
fails closed. The daemon registry is accepted only after a loopback identity
handshake. Static MCP discovery and no-daemon reads do not require daemon
liveness.

## Boundaries

The graph and index are derived working data, not KLL truth. MCP Atlas reads
are frozen and read-only; `atlas_search` is listed only with
`AGENT_SPEC_MCP_ATLAS_SEARCH=1`, while `atlas_explore` is unavailable unless
`AGENT_SPEC_MCP_ATLAS_EXPLORE=1`. Explore excerpts require current source hashes
and fixed output budgets. Affected results expose code nodes and paths but do
not infer test coverage. The status model compares graph identity and reports
syn, SCIP, and MIR separately, so consumers do not infer semantic freshness
from syn refresh.
`runtime-boundary` results carry `query-hint` authority: they never become graph
edges or deterministic impact, binding, lifecycle, or archive evidence.
Live runtime state is also derived: watcher health, watermark, daemon identity,
reader lease, and `degraded` status never replace graph freshness, KLL, or
lifecycle authority. Stopping the daemon preserves no-daemon access to the last
committed generation.

## Maintenance

Update this page when graph schema, read APIs, query-index validation, or MCP
tool exposure changes.
