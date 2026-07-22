# Atlas Live Runtime Design

**Status:** Approved by the reviewed `docs/atlas-roadmap.md` D3 contract

**Requirement:** `REQ-ATLAS-LIVE-RUNTIME`

**Task Contract:** `specs/task-atlas-live-runtime.spec.md`

## Purpose

Track D3 adds an optional process that watches one Rust worktree and keeps its
Atlas graph close to current. It is a latency optimization over D2's correct
incremental transaction, not a second graph authority and not a query proxy.
CLI and MCP queries continue to read the same local immutable generation in
daemon and no-daemon operation.

## Architecture

```text
notify event
  -> shared Atlas input scope
  -> persisted pending event + monotonic sequence
  -> debounce / bounded retry scheduler
  -> D2 build under the graph writer lock
  -> atomic generation publish
  -> acknowledge only events <= sync watermark
  -> safe generation reclamation under writer lock

query
  -> acquire per-query reader lease
  -> pin CURRENT.json generation
  -> read immutable graph
  -> intersect returned source files with pending paths
  -> return graph facts plus typed live-runtime status
```

The live runtime has four focused units:

1. `scope.rs` owns the source, Cargo-input, ignore and excluded-directory
   classification shared by build and watch.
2. `live.rs` owns persisted pending events, sequence watermarks, retry state and
   the typed `warming | healthy | pending | degraded | unavailable` status.
3. `watch.rs` adapts `notify` events into the shared scope. macOS and Windows
   install one recursive root watch. Linux installs non-recursive directory
   watches up to a configured hard cap and reports partial coverage explicitly.
4. `locking.rs` owns process-bound advisory locks and reader leases. D2 build
   transactions take the short-lived writer lock. A daemon takes a separate
   singleton lock for its whole lifetime. Reader lease files hold shared locks;
   reclamation removes only unlocked stale leases, unleased old generations and
   abandoned staging while the writer lock is held.

`src/atlas_daemon.rs` owns the executable boundary: foreground serve, detached
start, status, sync and stop. It uses a localhost TCP control socket because it
is portable across supported platforms and avoids Unix-socket path limits. The
registry is discovery data only; every client request must complete a protocol
handshake containing the canonical worktree identity, agent-spec version,
schema version and random startup nonce. PID alone is never attachment proof.

## Persistent State

All live state is derived runtime data under `<graph>/.runtime/`:

```text
.runtime/
  build.lock
  daemon.lock
  daemon.json
  pending.json
  status.json
  readers/<pid>-<nonce>.json
```

Each JSON document has a schema id/version, rejects unknown fields, rejects
unsafe paths and has a fixed byte limit. Writes use the existing atomic JSON
writer. Runtime files are outside immutable generation manifests, so a pending
event does not mutate graph authority or graph fingerprint.

`pending.json` stores a monotonic `next_sequence` and one entry per normalized
project-relative path. Repeated events update the latest sequence/time while
retaining the first sequence/time. A sync snapshots the current maximum
sequence. Only a successful D2 commit may acknowledge entries whose latest
sequence is at or below that snapshot. Events recorded during the sync remain.
Paths named by `BuildReport.unparsed` also remain pending because the committed
generation did not successfully incorporate their current source semantics.
Lock contention, cancellation, extractor failure, I/O failure and degraded
transition preserve all pending entries.

## Writer And Daemon Identity

The build writer lock is an OS advisory exclusive lock held only around one
D2 build and its post-commit maintenance. Losing the process releases the OS
lock. The persistent file is not interpreted as liveness evidence.

The daemon singleton lock is distinct and held for the daemon lifetime. A
daemon registry record contains canonical worktree and graph roots, PID,
process start timestamp, random nonce, control endpoint, tool version and graph
schema version. A registry record is accepted only after a live handshake
returns every expected identity field. This rejects dead PID records, PID reuse,
stale endpoints, version mismatch and records copied from another worktree.
When the daemon lock is free, a new daemon may atomically replace stale registry
state. When it is held, an incompatible live daemon is reported and not replaced.

The daemon never edits user Agent configuration. It remains alive when a client
disconnects and exits only through explicit stop, termination signal or an
unrecoverable daemon setup error. A sync failure degrades auto-sync but does not
terminate the control endpoint or erase pending work.

## Watch And Retry Semantics

`notify = 8.2` supplies the platform event backend. Watch planning is separated
from event delivery so platform policy and caps are deterministic tests:

- macOS/Windows: exactly one recursive watch at canonical worktree root;
- Linux/other non-recursive backends: canonically sorted directories accepted
  by the shared scope, up to `max_directories`;
- a cap or backend resource error marks watch coverage partial/degraded and
  preserves manual `atlas build` as the recovery path;
- source `.rs` files plus Cargo manifests, lockfile, `.cargo/config.toml`, toolchain and config inputs
  enter the pending queue; ignored, target, graph-runtime and outside-root paths
  do not.

Lock contention and ordinary sync failures have independent consecutive retry
counters. Backoff is deterministic exponential growth from `base_delay_ms`,
capped by `max_delay_ms`, and attempts stop after `max_attempts`. A successful
sync resets both counters. Exhaustion produces a typed degraded status that
retains pending events and names the failure class and last error.

## Query Semantics

Every graph query pins one immutable generation with a reader lease. Result
facts, paths and ranking are identical whether a daemon exists or not. The
embedded `AtlasStatus.live` field reports runtime state and the intersection of
pending paths with files represented in that result. Global `atlas status`
reports the complete pending set. A missing daemon is `unavailable`, not a graph
error; a missing graph remains a missing graph for fact queries, while
`atlas daemon status` can still report `warming` or `unavailable`.

MCP `tools/list` remains a pure static function and never starts, connects to or
waits for a daemon. Existing MCP Atlas tools remain frozen graph readers. The
daemon can improve freshness before a call, but it is not required for query
correctness and does not change the default MCP tool surface.

## Safe Reclamation

After a successful publish and while holding the build writer lock, maintenance
may reclaim:

- staging directories left by a process that no longer owns the writer lock;
- reader lease files on which an exclusive lock can be acquired;
- old generation directories not named by `CURRENT.json` and not named by any
  active reader lease.

An active reader lease always wins. Unreadable or ambiguous lease/state data
fails closed: retain the generation and emit maintenance diagnostics. Repeated
reclamation is idempotent. Maintenance failure cannot roll back a committed
generation or convert a successful build into failure.

## Error Model

New typed errors distinguish writer contention, live-state corruption, watcher
capacity, daemon identity mismatch and daemon unavailability. Errors do not use
an empty result as success. Runtime status diagnostics are additive and never
promote derived state to KLL evidence.

## Verification

Deterministic tests use inert event injection and in-process TCP servers. One
small real watcher smoke test may be platform-gated, but correctness does not
depend on OS delivery timing. The fixture matrix covers event coalescing,
mid-sync arrival, failed sync, retry exhaustion and recovery, competing writers,
stale registry/PID reuse, client disconnect, old-generation reader retention,
abandoned staging cleanup, no-daemon parity and static MCP discovery.

The delivery remains opt-in. It does not change default MCP discovery, does not
add a remote service, does not run an LLM and does not alter KLL truth.
