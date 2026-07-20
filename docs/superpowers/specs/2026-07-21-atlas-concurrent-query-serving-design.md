# Atlas Concurrent Query Serving Design

**Status:** Approved by the reviewed `docs/atlas-roadmap.md` D4 contract

**Requirement:** `REQ-ATLAS-CONCURRENT-QUERY-SERVING`

**Task Contract:** `specs/task-atlas-concurrent-query-serving.spec.md`

## Purpose

D4 adds an opt-in execution service for B5 context queries without making the daemon, a worker
pool or a new MCP tool part of Atlas correctness. The service isolates CPU-heavy retrieval and
source projection from daemon control and MCP stdio transport, applies bounded admission, and
keeps every accepted query on one reader-leased immutable generation.

Direct frozen library calls remain the semantic reference. Worker mode is a latency and
concurrency candidate that cannot become the default until the E1 direct-versus-worker gate has
real evidence.

## Chosen Approach

The implementation uses fixed `std::thread` workers and bounded `sync_channel` queues. This matches
the existing synchronous TCP daemon, needs no new dependency, makes admission deterministic, and
lets tests inject a runner, clock, barriers and panics. The alternatives are deliberately deferred:

- Tokio `spawn_blocking` shares a process-global blocking pool and does not make Atlas worker count,
  queue ownership or panic recovery explicit enough for the D4 contract.
- A subprocess worker gives stronger crash isolation but adds cross-platform process supervision,
  graph/session serialization and cold-start cost before E1 has shown that workers are useful.
- Per-request threads violate bounded admission and make overload depend on host scheduler limits.

The fixed pool is still only an opt-in prototype. E1 may reject it or justify a later process-worker
design.

## Architecture

```text
default CLI / default MCP
  -> direct frozen rust-atlas query

opt-in CLI context / opt-in atlas_context MCP
  -> protocol-v2 daemon client
  -> loopback listener validates identity and request
  -> pin CURRENT generation + create reader lease
  -> bounded admission (queue + estimated memory reservation)
       full/open circuit -> typed busy/degraded + retry_after_ms
  -> fixed query worker
       pinned retrieval -> cancellation/deadline checkpoint
       bounded projection -> cancellation/deadline checkpoint
       one panic retry against the same pinned snapshot
  -> completion channel
  -> listener serializes one typed response and releases the lease

watch event / manual sync
  -> one fixed maintenance lane
  -> existing D2 sync transaction and writer lock
  -> completion channel

listener main loop
  -> always accepts status, stop, cancel and service-status traffic
  -> drains query/sync completions and watcher events
```

`crates/rust-atlas` owns a pinned context session. It retains `GraphSnapshot`, `Meta`, `QueryIndex`
and `AtlasStatus` until the result has been finalized. Existing direct APIs use the same retrieval
and projection functions; worker execution adds cooperative checks between bounded phases.

`src/atlas_query_service.rs` owns scheduling, not graph semantics. A `QueryRunner` trait is the
fault-injection boundary. Production execution calls the pinned B5 compiler. Tests can block,
cancel, expire or panic without relying on OS timing.

`src/atlas_daemon.rs` owns protocol v2, pending sockets and the listener loop. The listener never
waits for query traversal, sync or blocking socket I/O. It incrementally reads a bounded set of
nonblocking connections, stores admitted connections by request id, polls completion channels, and
pre-encodes exactly one typed response. A priority egress queue serves control responses before bulk
query and sync output while reserving a minimum bulk progress budget on every poll. Pending bulk
work is capped at 256 sockets with 16 additional control slots. A context request that reaches the
listener while bulk capacity is full receives a typed `busy` receipt through a control slot before
the daemon pins a generation or admits worker work; manual sync is rejected before maintenance
admission under the same condition. No request creates a thread.

## Configuration And Admission

The checked-in prototype configuration is:

| Setting | Default | Validation |
|---|---:|---:|
| query workers | `0` | `0..=4`; zero disables serving |
| opt-in worker profile | `2` | explicit CLI/env only |
| queue capacity | `4` | `1..=64` when workers are enabled |
| queue timeout | `2000 ms` | `10..=30000 ms` |
| execution deadline | `20000 ms` | `100..=60000 ms` |
| estimated memory budget | `268435456 bytes` | `16777216..=2147483648` |
| retry delay | `100 ms` | `1..=10000 ms` |
| panic retry | `1` | fixed |
| circuit threshold | `2` consecutive panics | fixed; reset only by daemon restart |

Admission pins the generation before queueing. It estimates a reservation from the selected
generation's query-index bytes, a fixed deserialization multiplier, request bytes and the B5 output
ceiling. This is a conservative scheduling estimate, not measured allocator use. A reservation
that would exceed the configured total receives the same typed busy outcome as a full queue.
Actual RSS remains an E1 measurement and is never reported as a correctness fact.

Queue timeout and execution deadline are separate. Expired queued work is discarded before the
runner starts. Executing work checks a shared cancellation/deadline control before index loading,
after retrieval, during candidate/source projection, and before final serialization. The service
does not claim forceful preemption of arbitrary filesystem or operating-system calls.

Shutdown boundedness means finite admitted work, bounded queues and cooperative checkpoints. It is
not a hard wall-clock termination promise: Rust threads are not force-killed, and shutdown waits for
an in-progress bounded phase to reach its next checkpoint so reader leases can be released cleanly.
Watcher events are drained into the pending journal before watcher shutdown. While maintenance is
outstanding, the listener does not race the sync transaction's runtime-status writes; it resumes
runtime projection only after the maintenance lane has completed its serialized update.

## Snapshot And Failure Semantics

The listener calls `pin_context_snapshot` before admission. The queued job owns that lease. Its
worker loads the index from the pinned generation directory and never resolves `CURRENT.json`.
A writer may publish a new generation concurrently; result nodes, graph fingerprint, generation
and response digest still refer to the admitted snapshot. Safe reclamation retains the old
generation until the response is finalized or discarded.

Worker execution is wrapped in `catch_unwind`. The first panic retries once with the same job and
snapshot. A second consecutive panic completes the request as `degraded`, opens the circuit and
rejects later worker admissions until daemon restart. This protects the process from Rust panics;
it does not claim memory-corruption isolation.

Typed outcomes are `success`, `busy`, `timeout`, `cancelled`, `degraded`, `failed` and
`unavailable`. Only success carries `ContextResult`. Busy and degraded include `retry_after_ms`.
No timeout returns a friendly text payload that resembles an empty success.

Direct fallback is an explicit caller choice. It discards the worker response and starts one
complete direct query with a newly pinned snapshot. The wrapper receipt records the rejected
worker generation and the fallback generation separately. It never combines nodes, paths or
source slices from two generations.

## CLI And MCP Surfaces

`atlas context` keeps direct execution as its default. `--execution worker` calls a verified local
daemon; `--fallback-direct` is valid only with worker execution. Worker success and all non-success
outcomes use the versioned service response wrapper. Direct execution keeps the existing B5 JSON
shape.

`atlas daemon start|serve` gain explicit query worker settings. `atlas daemon service-status`
shows queue, active jobs, limits, counters and circuit state without changing the existing runtime
status shape.

The `atlas_context` MCP tool is hidden unless `AGENT_SPEC_MCP_ATLAS_CONTEXT=1`. Worker routing and
the concurrent MCP dispatcher additionally require `AGENT_SPEC_MCP_ATLAS_QUERY_MODE=worker`.
The default tool list and serial direct server remain byte-compatible. In worker mode, the stdin
reader handles initialize, ping, notifications and discovery immediately while a fixed client
lane waits for daemon query responses; one stdout writer serializes responses by JSON-RPC id and
may return them out of request order as the protocol permits.

## Evaluation

D4 adds `agent-spec/atlas-eval/concurrent-query-v1`. Deterministic fixtures cover direct/worker
semantic parity, all four B5 load profiles, queue and memory rejection, queued and executing
cancellation, panic retry/circuit, writer publish, lease cleanup, daemon stop and two worktrees.
They compare canonical context bytes, generation/fingerprint pairs, response digests and typed
outcomes.

The receipt also records queue wait, service time, heartbeat observations, response bytes, CPU and
RSS when the opt-in driver can measure them. Those values are observations, not unit-test
thresholds. E1 must retain failed trials, run direct and worker arms under symmetric conditions,
show no correctness regression, and obtain human acceptance before workers can become a default.

## Non-Goals

- Remote, multi-host or multi-writer graph serving
- Automatic worker enablement or host-core-based sizing
- A graph database or mutable server-side cursor
- General-purpose execution of arbitrary commands in workers
- Force-killing Rust threads or claiming process-level crash isolation
- Replacing graph freshness, KLL, Task Contracts or lifecycle authority
- Changing default MCP discovery before E1
