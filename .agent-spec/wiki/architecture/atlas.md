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
  - crates/rust-atlas/src/context.rs
  - src/main.rs
  - src/atlas_daemon.rs
  - src/atlas_query_service.rs
  - src/atlas_eval.rs
  - src/atlas_agent_eval.rs
  - crates/code-graph-provider/src/lib.rs
  - crates/code-graph-provider/Cargo.toml
  - crates/code-graph-provider/README.md
  - src/spec_mcp/mod.rs
  - src/spec_mcp/tools.rs
  - docs/atlas-live-runtime.md
  - docs/atlas-query-context.md
  - docs/atlas-concurrent-query-serving.md
  - docs/atlas-agent-ab-gate.md
  - docs/code-graph-provider-kit.md
  - specs/task-atlas-explore-flow-impact.spec.md
  - specs/task-atlas-runtime-boundary-hints.spec.md
  - specs/task-atlas-incremental-hardening.spec.md
  - specs/task-atlas-live-runtime.spec.md
  - specs/task-atlas-query-context-compiler.spec.md
  - specs/task-atlas-concurrent-query-serving.spec.md
  - specs/task-atlas-agent-ab-gate.spec.md
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
`context.rs` adds a two-stage compiler: retrieval emits stable scored evidence
before profile relevance and byte projection produce verified source slices,
omission continuations, dual-loss receipts, and a deterministic load profile.
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
Post-commit acknowledgement failure remains a maintenance warning and falls
back to the pre-build snapshot rather than converting the committed build into
failure.

Every query owns a reader lease for its pinned generation. Under the
single-writer lock, reclamation removes an old generation only after an
exclusive reader scan proves it inactive; malformed or unreadable lease state
fails closed. The daemon registry is accepted only after a loopback identity
handshake. Static MCP discovery and no-daemon reads do not require daemon
liveness.

D4 adds a fixed, bounded query service over those leases. Admission reserves
query index, request, and response memory before enqueueing. Queue timeout,
execution deadline, cancellation, one panic retry, circuit state, and daemon
unavailability remain distinct outcomes. Explicit sync runs in one maintenance
lane while listener capacity is reserved for control traffic. CLI direct mode
remains the default; worker and fallback modes are explicit. The hidden MCP
`atlas_context` route uses a fixed client lane and one stdout writer so ping and
discovery remain responsive during slow context work.

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

Context projections are derived too. Their stable evidence ids and receipts
explain selection and loss but do not become graph facts, code bindings, or KLL
truth. Required evidence fails explicitly; continuation must retain the graph
fingerprint. Test, generated, and vendored bodies require an explicit path or
symbol, or a primary evidence-spine role; incidental candidates remain
provenance-bearing skeletons. The command remains outside default MCP until E1.

`atlas_context` is listed only with `AGENT_SPEC_MCP_ATLAS_CONTEXT=1`; concurrent
daemon routing additionally requires `AGENT_SPEC_MCP_ATLAS_QUERY_MODE=worker`.
The strict D4 fixture receipt gates semantic parity, snapshot identity, typed
outcomes, worktree isolation, and cleanup. Timing and resource observations do
not become correctness thresholds or graph authority.

E1 adds a separate adoption layer in `src/atlas_agent_eval.rs`. Its strict
Agent A/B/C and serving plans, receipts, and gates consume Atlas behavior but
do not become graph or KLL authority. Checked-in plans prove only deterministic
scheduling; real receipt evidence and human acceptance are still required.

F1 adds a sibling producer contract in `crates/code-graph-provider`, not a
module inside Rust Atlas. External extractors and semantic enrichers project to
strict, separate payloads and pass a bounded process/conformance gate before
their artifacts can be published. The fixture proves protocol behavior only;
Rust Atlas remains the only production language provider in this repository.

The 1.2 workspace release publishes `rust-atlas` 0.3.0 with schema v6 and the
provider SDK 0.1.0. Existing 0.2 graphs are derived artifacts and require a full
rebuild; the version change does not promote graph data to KLL authority.

## Maintenance

Update this page when graph schema, read APIs, query-index validation, or MCP
tool exposure changes.
