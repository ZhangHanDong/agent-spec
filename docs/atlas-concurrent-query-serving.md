# Rust Atlas Concurrent Query Serving

Atlas D4 adds an opt-in, fixed-size query service around the B5 context
compiler. It isolates slow retrieval and projection work from daemon control
and MCP transport while preserving one pinned immutable generation for the
complete request. Direct execution remains the default.

## Start The Worker Service

Build the graph, then start the daemon with an explicit worker configuration:

```bash
agent-spec atlas build --code . --graph .agent-spec/graph
agent-spec atlas daemon start --code . --graph .agent-spec/graph \
  --query-workers 2 \
  --query-queue-capacity 4 \
  --query-queue-timeout-ms 2000 \
  --query-deadline-ms 20000 \
  --query-memory-budget-bytes 268435456 \
  --query-retry-after-ms 100
agent-spec atlas daemon service-status --code . --graph .agent-spec/graph
```

`query-workers=0` is the default and disables worker admission. The prototype
accepts at most four workers; it never derives worker count from host cores and
never creates a thread per request. Starting against an existing daemon fails
when the requested configuration differs from the verified owner.

Use direct or worker execution explicitly:

```bash
agent-spec atlas context "compile_context calls projection" \
  --profile flow --execution direct \
  --code . --graph .agent-spec/graph --frozen

agent-spec atlas context "compile_context calls projection" \
  --profile flow --execution worker \
  --code . --graph .agent-spec/graph --frozen
```

`--fallback-direct` is valid only with `--execution worker`. It is not an
automatic default. When worker admission is busy, degraded, or unavailable,
the fallback runs a complete direct query and records worker and fallback
generation identity separately. Partial evidence is never merged.

## MCP Opt-In

The default MCP discovery remains unchanged. Enable the context tool and its
worker transport separately after starting the daemon above:

```bash
AGENT_SPEC_MCP_ATLAS_CONTEXT=1 \
AGENT_SPEC_MCP_ATLAS_QUERY_MODE=worker \
agent-spec mcp --code .
```

With only `AGENT_SPEC_MCP_ATLAS_CONTEXT=1`, `atlas_context` is visible but uses
the existing serial direct dispatcher. Worker mode uses two fixed MCP client
threads, a four-request context queue, and one stdout owner. Initialize,
`tools/list`, ping, and notifications remain on the transport lane, so a slow
context response may arrive after a later ping response. JSON-RPC ids preserve
request correlation.

The tool accepts only `query`, `profile`, `after`, `expect_graph`, and bounded
`max_bytes`. It always sets `frozen=true`; unknown fields and invalid bounds
fail before daemon admission. A full MCP client queue returns `isError: true`
with a typed Atlas `busy` response rather than an empty successful result.

## Service Contract

The daemon owns a bounded bulk listener queue, a reserved control lane, one
maintenance thread, and the configured query workers. Admission creates a
`PinnedContextSnapshot` before enqueueing. Queue wait, execution, retry, and
response serialization retain that generation and its reader lease.

The default prototype limits are:

| Setting | Default | Accepted range |
|---|---:|---:|
| Query workers | `0` | `0..=4` |
| Queue capacity | `4` | `1..=64` when enabled |
| Queue timeout | `2000 ms` | `10..=30000 ms` |
| Query deadline | `20000 ms` | `100..=60000 ms` |
| Memory budget | `268435456` bytes | `16777216..=2147483648` |
| Retry delay | `100 ms` | `1..=10000 ms` |

Memory admission reserves the pinned query index estimate, request bytes, and
maximum response bytes before enqueueing. Cancellation and deadline checks run
before index work, after retrieval, through projection, and before receipt
serialization. A queued timeout never starts the query runner.

One worker panic retries once against the same pinned snapshot. A second panic
returns `failed` and opens a circuit that remains degraded until daemon
restart. Manual and automatic sync use the fixed maintenance lane, so status
and stop continue to respond while a writer publishes.

## Typed Outcomes

| Outcome | Meaning | Caller action |
|---|---|---|
| `success` | A complete context and digest are present. | Consume the context and retain its receipt. |
| `busy` | Queue or memory admission rejected the request. | Honor `retry_after_ms`, reduce load, or choose explicit direct fallback. |
| `timeout` | Queue wait or cooperative execution exceeded its bound. | Narrow the query or increase an explicit bounded setting. |
| `cancelled` | The request was cancelled at a cooperative checkpoint. | Treat it as not executed; do not consume partial evidence. |
| `degraded` | The panic circuit is open. | Restart the daemon or use explicit direct fallback. |
| `failed` | Validation, execution, retry, or response bounds failed. | Read the diagnostic and fix the query, graph, or implementation. |
| `unavailable` | No matching worker owner or live transport exists. | Start the configured daemon or use the direct path. |

Only `success` carries context and `response_digest`. Non-success outcomes
carry a diagnostic; `busy` and `degraded` also carry `retry_after_ms`.

## Correctness Receipt

`benchmarks/atlas/concurrent-query-receipt-v1.json` uses schema
`agent-spec/atlas-eval/concurrent-query-v1`. The strict gate checks:

- four direct/worker semantic parity groups for B5 load profiles;
- all seven typed outcomes and retained expected failure runs;
- queue, memory, timeout, cancel, panic, circuit, publish, stop, fallback, and
  two-worktree cases;
- generation/fingerprint pairing, response digest shape, idle final counters,
  and zero retained reader leases;
- separate worktree root, daemon, generation, and graph identities.

The checked-in matrix contains 20 runs with two workers and queue capacity
four. It records 13 successes and seven expected non-success runs. Successful
response observations are 15951, 22152, 23617, and 29205 bytes. The largest
recorded queue wait, service time, heartbeat lag, CPU time, and RSS are 2001
ms, 91 ms, 1 ms, 42 ms, and 24117248 bytes.

Those values are measurements only. The gate validates their type and
presence but never compares them with a pass threshold. They are fixture
evidence, not a claim that worker mode is faster or more efficient.

For opt-in real Agent evaluation, set all three inputs:

```bash
export ATLAS_EVAL_AGENT_COMMAND=/absolute/path/to/evaluation-agent
export ATLAS_EVAL_D4_MODE=worker
export ATLAS_EVAL_D4_RECEIPT=benchmarks/atlas/concurrent-query-receipt-v1.json
bash scripts/atlas-eval/run-opt-in.sh plan.json receipts.ndjson
```

The runner exports the selected serving mode plus the D4 receipt path and
SHA-256 hash to the external agent. It does not manufacture a worker benefit,
drop failed trials, or promote a latency threshold. E1 real Agent A/B remains
required before changing defaults.

## Authority And Recovery

Worker mode improves isolation and bounded admission. It cannot improve graph
coverage or freshness. A stale or mismatched graph must still be rebuilt, and
an unavailable semantic layer remains unavailable. Query receipts are derived
runtime evidence; they do not become KLL truth, code bindings, requirement
state, scenario verdicts, or lifecycle evidence.

Use these recovery operations without changing authority:

```bash
agent-spec atlas daemon service-status --code . --graph .agent-spec/graph
agent-spec atlas daemon sync --code . --graph .agent-spec/graph
agent-spec atlas daemon stop --code . --graph .agent-spec/graph
agent-spec atlas build --code . --graph .agent-spec/graph --full
```

Stopping the daemon leaves the last committed generation available to direct,
frozen reads. Two worktrees must use separate canonical graph roots and daemon
identities; their request ids, counters, cancellation, snapshots, and receipts
are local to each service.
