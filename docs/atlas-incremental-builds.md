# Rust Atlas Incremental Builds

Rust Atlas publishes each successful build as one committed generation. Query
operations pin that generation before reading metadata, shards, capability
state, or the query index. A failed or cancelled build leaves the prior
generation active.

The graph remains derived working data under `.agent-spec/graph`; do not edit
`CURRENT.json`, generation directories, or `orphans.json`. Use `atlas build`,
`atlas status`, and query commands as the supported interface. KLL truth stays
under `knowledge/`.

## Build

```bash
agent-spec atlas build --code . --graph .agent-spec/graph \
  --features serde,tracing \
  --target x86_64-unknown-linux-gnu \
  --cfg tokio_unstable \
  --frontier-limit 2048 \
  --batch-size 256 \
  --working-byte-limit 536870912
```

`features`, `target`, and `cfg` participate in the content-addressed Cargo
input-plan key. The key also includes workspace manifest bytes, toolchain,
provider/schema versions, and relevant Cargo environment. File mtimes are not
authority. A cache hit reuses Cargo target metadata but reconstructs Rust
module ownership from current source. A non-frozen query that refreshes stale
source reuses the committed feature, target, and cfg configuration instead of
silently reverting to default Cargo inputs.

For source changes, Atlas compares old and new declaration identities and
builds a dependency frontier. It includes dirty shards plus unchanged reverse
dependents affected by removed targets, canonical symbols, bare-name candidate
sets, and implementation relations. A frontier above `--frontier-limit`
restarts as a complete pass and reports
`fallback_reason: dependency-frontier-overflow`.
Changing SCIP, MIR, or dynamic-dispatch capability also selects a complete
frontier and reports `semantic-overlay-change` or
`dynamic-dispatch-full-recompute`.

Resolution work is recorded in a bounded orphan queue before processing. A
cancelled or failed transaction keeps that queue and the prior committed
generation. The next build processes orphan work even when there is no new
source change; resolved, external, and deterministically unresolved edges all
consume their work item. The queue is removed only after pointer publication.
If post-commit deletion fails, Atlas reports a maintenance warning and rebases
the queue to the committed generation so the next build can recover it.

## Reports

The JSON build report includes the committed `generation`, `input_plan`
result, `touched_shards`, resolution and validation counts, resolved/unresolved
edge deltas, bounded `working_bytes`, `orphans_recovered`, and an optional
`fallback_reason`. `atlas status` and every query result also report the pinned
generation id.

`working_bytes` is the maximum admitted deterministic byte set across source,
serialized graph shards, and explicit SCIP/MIR overlay inputs. Frontier
planning, resolution, and validation stream shards in configured batches;
inputs above `--working-byte-limit` fail before publication.

A healthy zero-change build verifies committed artifact digests and returns
without staging, resolution, validation, or authority/control-file writes. It
keeps the current generation and reports zero work counters.

`atlas-cancelled`, `atlas-resource-limit`, and `atlas-orphan-queue` are typed
failures. Invalid zero limits fail before work starts. Cancellation and the
working-byte ceiling are checked before publication, so neither can expose a
partial generation.

## Live Runtime Boundary

D2 remains the deterministic build primitive. D3 now wraps it with an optional
watcher and daemon that persist a pending watermark, apply bounded retries, and
report `warming`, `healthy`, `pending`, `degraded`, or `unavailable`. See
[Rust Atlas Live Runtime](atlas-live-runtime.md) for commands, defaults, and
recovery. The no-daemon path continues to read the same immutable generations.

D2 removes staging owned by its current transaction. D3 cross-process
maintenance additionally requires the graph writer lease and a reader lease
scan before reclaiming old generations or abandoned staging. Ambiguous lease
state fails closed. Neither layer changes graph freshness, KLL, or lifecycle
authority, and maintenance warnings do not invalidate an already committed
generation.
