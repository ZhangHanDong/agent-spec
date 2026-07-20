# Rust Atlas Live Runtime

Atlas D3 adds an optional local watcher and daemon around the immutable
generation build model. The daemon improves refresh latency; it is not required
for queries and it does not change Atlas authority.

## Commands

Build a graph before starting the optional runtime:

```bash
agent-spec atlas build --code . --graph .agent-spec/graph
agent-spec atlas daemon start --code . --graph .agent-spec/graph
agent-spec atlas daemon status --code . --graph .agent-spec/graph
agent-spec atlas daemon sync --code . --graph .agent-spec/graph
agent-spec atlas daemon stop --code . --graph .agent-spec/graph
```

`start` launches one local process for the canonical worktree and graph pair.
`serve` runs the same service in the foreground for supervision and debugging.
`status`, `sync`, and `stop` accept only a registry entry whose loopback
handshake matches the canonical roots, tool version, Atlas schema, PID, startup
time, and random nonce. A stale registry is not process-liveness proof.
Status, stop, connect, and protocol writes use a one-second I/O timeout. A
manual sync response has a bounded ten-minute timeout because it executes a D2
build; startup still requires a verified handshake within five seconds.
The D3 server handles one control request at a time, so status can time out
while an explicit or startup sync owns that loop. D4 is responsible for
control-plane isolation and concurrent query backpressure; callers can retry
status after the sync window without losing pending work.

The no-daemon path remains supported:

```bash
agent-spec atlas query rust_atlas::build --code . \
  --graph .agent-spec/graph --frozen
```

MCP discovery is static and does not connect to or wait for the daemon. Frozen
query facts and the pinned generation are the same with and without a daemon;
the additive `live` status can differ.

## Event And Sync Model

The watcher and build use one `AtlasScope`, including Cargo workspace excludes
and ignore rules. macOS and Windows install one recursive watch. Linux and
other platforms install sorted non-recursive watches for at most 50,000
directories. The event channel holds 4,096 entries; directory or channel
overflow is reported as partial or degraded coverage instead of healthy.

Events are coalesced for 100 ms and persisted in
`.agent-spec/graph/.runtime/pending.json`. The pending journal is limited to
16 MiB, 100,000 paths, and normalized repo-relative input paths. Every event has
a monotonic sequence watermark. A sync snapshots a watermark, publishes a D2
generation, and acknowledges only events at or below that snapshot. An event
arriving mid-sync remains pending. Failed sync and writer contention never
clear pending input.

Writer-lock and ordinary extractor/I/O retry counters are independent. Each
allows five delayed retries using exponential backoff from 100 ms up to
30,000 ms. The sixth failure enters `degraded`, preserves pending work, and
requires a later successful sync or manual `atlas build` recovery. A successful
sync resets both counters.

## States

The additive live state is one of:

- `warming`: startup or initial sync has not produced a usable runtime state.
- `healthy`: the watcher is complete and no input is pending.
- `pending`: supported input has changed since the committed generation.
- `degraded`: watch coverage or a bounded retry budget failed.
- `unavailable`: no valid local daemon/runtime status is available.

Global status lists every pending path. Query results intersect pending paths
with files represented by that result, so unrelated changes do not make every
answer locally stale. `pending` and `degraded` are warnings about refresh state,
not new graph facts.

## Concurrency And Reclamation

Build and sync take the advisory single-writer lease before creating staging,
orphan, or generation work. A competing writer gets typed `atlas-writer-busy`
and can use the separate retry budget. Daemon singleton ownership uses a
different lock from graph writing.

Each public query pins one immutable generation and creates a reader lease
under `.runtime/readers/`. Cloned snapshots share that lease. A writer can
remove an old generation only while holding the writer lease and after an
exclusive reader-registry scan proves that no active lease references it. A
locked malformed lease, unreadable registry, or ambiguous reader directory
fails closed and retains all generations. Abandoned staging cleanup is
idempotent. Maintenance cleanup failures are diagnostics and do not turn an
already committed build into failure.

## Authority Boundary

The live runtime does not replace graph freshness, KLL truth, Task Contracts,
or lifecycle evidence. Graph shards, pending state, daemon registry, reader
lease files, and query indexes are derived working data. The daemon never writes
source, `knowledge/`, specs, Agent configuration, or Git state.

Use the recorded graph identity and independent syn/SCIP/MIR freshness for
deterministic binding and verification. Treat pending/degraded state as a reason
to sync or rebuild, not as proof that a requirement is satisfied or violated.
Stopping the daemon leaves the last committed generation readable through the
no-daemon CLI and MCP paths.

## Acceptance Evidence

`fixtures/atlas/live-runtime/matrix.json` maps event, retry, identity,
supervision, reader lease, reclamation, and no-daemon cases to tests governed by
`specs/task-atlas-live-runtime.spec.md`. The matrix records the receipt fields
asserted by those tests; lifecycle remains the execution evidence.
