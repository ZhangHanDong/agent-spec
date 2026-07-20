# Atlas Evaluation And Query Regression

Atlas evaluation provides a reproducible comparison harness for an Atlas-assisted
agent arm and a baseline arm. It defines inputs, run scheduling, receipts, and
summary statistics. It does not run an agent by default, contact a model, or
access the network.

The checked-in corpus is `benchmarks/atlas/corpus.json`. It is an offline
fixture, not evidence that a real model evaluation has run or that Atlas yields
a performance improvement.

Query-quality regression is a second, deterministic layer. Its checked-in
inputs are `benchmarks/atlas/query-corpus.json` and
`benchmarks/atlas/query-results.json`. They detect retrieval regressions; they
are not fresh observations from the pinned repository.

B5 context cases additionally preserve an expected profile and a strict
retrieval/projection receipt. They distinguish graph retrieval loss from
context projection loss instead of reducing both to a generic truncation bit.

## Live Runtime Conditions

D3's optional watcher and daemon do not change query correctness. Evaluation
must record whether an arm ran in no-daemon mode or through the daemon, the
pinned generation, pending watermark, live state, retry counts, reader lease,
and whether the runtime was `degraded`. Never compare a warmed daemon arm with
a cold baseline without labeling that condition. E1 concurrent experiments
must also report queue and transport behavior; live status does not replace
graph freshness, KLL, or lifecycle authority. See
[Rust Atlas Live Runtime](atlas-live-runtime.md).

## Workflow

Start by validating the corpus, then compile its paired plan:

```bash
cargo run -- atlas benchmark validate --corpus benchmarks/atlas/corpus.json
cargo run -- atlas benchmark plan --corpus benchmarks/atlas/corpus.json --out plan.json
```

Run the plan only through the explicit opt-in boundary described below. The
runner writes a receipt candidate; `summarize` is the validated receipt
boundary and must grade every parsed receipt for correctness before producing
performance aggregates:

```bash
cargo run -- atlas benchmark summarize --receipts receipts.ndjson --out summary.json
```

The comparison is correctness-first. A summary is rejected when any receipt
lacks a correctness verdict. Do not treat lower file reads, calls, duration,
context, or cost as a benefit unless the corresponding runs have been graded.

Score ranked query observations against the versioned golden corpus separately:

```bash
cargo run -- atlas benchmark score \
  --corpus benchmarks/atlas/query-corpus.json \
  --results benchmarks/atlas/query-results.json \
  --out query-regression.json
```

## Corpus Schema

A corpus is a JSON object with `schema` equal to
`agent-spec/atlas-eval/corpus-v1`, plus the following fields:

| Field | Type | Meaning |
|---|---|---|
| `model` | string | Model identifier held constant for every compiled run. |
| `prompt` | string | Prompt held constant for every compiled run. |
| `cases` | array | Evaluation cases. Unknown fields are rejected. |

Each case has these fields:

| Field | Type | Values or rule |
|---|---|---|
| `id` | string | Non-empty and unique within the corpus. |
| `size` | string | `small`, `medium`, or `large`. |
| `task_class` | string | `symbol`, `flow`, `impact`, `implementation`, `stale`, `scip-unavailable`, `compile-failing`, or `worktree`. |
| `repository` | string | Repository or fixture location. |
| `revision` | string | Non-empty pinned revision. |
| `trials_per_arm` | integer | At least `3`. |
| `rubric` | array of strings | Non-empty, with no empty item. |
| `permissions` | string | `read-only` or `workspace-write`. |
| `cache_condition` | string | `cold` or `warm`. |

`validate` reads and validates this schema. `plan` validates the same corpus
before compiling runs. On success `validate` prints JSON such as
`{"valid":true,"cases":8}`. It has no `--out` option. On a validation error,
it produces no normal JSON result.

## Run Plan Schema

`plan` validates the corpus, then emits a JSON object with
`schema: "agent-spec/atlas-eval/run-plan-v1"` and a `runs` array. For every
case it creates both `atlas` and `baseline` runs, numbered from trial `1`
through `trials_per_arm`. Therefore every valid case receives at least three
runs for each arm.

Each run contains:

```json
{
  "case_id": "symbol-lookup",
  "arm": "atlas",
  "trial": 1,
  "model": "offline-fixture-model",
  "prompt": "Produce an evidence-backed answer.",
  "repository": "fixtures/atlas/basic",
  "revision": "a2d4282",
  "permissions": "read-only",
  "cache_condition": "cold"
}
```

The plan records the inputs required to make paired runs comparable. It does
not execute the runs or claim any result.

## Receipts And Summaries

`summarize` accepts either a JSON array of receipts or newline-delimited JSON
(NDJSON), with one receipt object per non-empty line. Unknown receipt fields
are rejected. A receipt has this shape:

```json
{
  "case_id": "symbol-lookup",
  "arm": "atlas",
  "trial": 1,
  "correctness": { "passed": true },
  "file_reads": 12,
  "graph_calls": 3,
  "tool_calls": 9,
  "duration_ms": 842,
  "context_bytes": 12000,
  "cost_usd": 0.04,
  "query_metrics_schema": "agent-spec/atlas-eval/query-metrics-v1",
  "response_bytes": 24000,
  "read_back_calls": 1,
  "follow_up_queries": 2,
  "truncated_queries": 0,
  "atlas_serving_mode": "worker",
  "concurrent_query_receipt_path": "benchmarks/atlas/concurrent-query-receipt-v1.json",
  "concurrent_query_receipt_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
}
```

`arm` is `atlas` or `baseline`; `trial` is a positive integer. The
`correctness` object is required for summarization and contains `passed`.
`cost_usd` is optional, but when present it must be finite and non-negative.
The remaining measurements are unsigned integers. Current external agent
commands must emit `query_metrics_schema` and all four query measurements,
including explicit zeroes:

| Field | Meaning |
|---|---|
| `response_bytes` | Total raw bytes returned by exploration and query tools during the run. |
| `read_back_calls` | Source-read calls made after an `atlas explore` response to retrieve source already represented by that response. |
| `follow_up_queries` | Exploration or graph queries issued after the run's initial exploration query. |
| `truncated_queries` | Query responses that reported truncation or another response limit. |

For backward compatibility, receipts created before these fields were added
deserialize omitted query measurements as zero. They are counted under
`legacy_query_metrics_receipts` and excluded from the four query metric
distributions, so missing measurements cannot improve an A/B result. A receipt
with only some fields, an unknown schema, or non-zero unversioned values is
rejected. New producers must report the schema and all measured values.

D4 evaluation metadata is backward compatible but all-or-none. New direct or
worker trials record `atlas_serving_mode`, the concurrent-query receipt path,
and its 64-character SHA-256 hash together. A partial triplet, empty path, or
invalid hash is rejected. This ties an E1 run to a reviewed concurrency matrix
without turning that matrix's latency observations into E1 thresholds.

The result contains total `receipts`, total `correctness` counts, aggregate
`metrics`, and an `arms` object keyed by the populated arms. Each metric has
`samples`, `median`, and `mad` (median absolute deviation). Metrics are
`file_reads`, `graph_calls`, `tool_calls`, `duration_ms`, `context_bytes`, and
optional `cost_usd`, plus `response_bytes`, `read_back_calls`,
`follow_up_queries`, and `truncated_queries`. The optional cost metric is absent
when no receipt reports a cost. Aggregate and per-arm metrics use the same
calculation. Each metrics object also reports `query_metrics_receipts` and
`legacy_query_metrics_receipts`; a valid E1 comparison requires zero legacy
query-metric receipts in both arms.

## Query Quality Corpus

The E3 corpus uses schema `agent-spec/atlas-eval/query-corpus-v1` and carries a
non-empty `version`. It has two required tiers:

- `deterministic-fixture` cases point under `fixtures/` and run in the default
  offline test suite.
- `pinned-repository` cases identify a true Rust repository with a full 40-hex
  Git revision and a `paired_fixture`. Validation and scoring never clone,
  fetch, build, or query that repository.

Every case records:

| Field | Meaning |
|---|---|
| `query` | The exact user question being evaluated. |
| `expected_symbols` | Canonical symbols that must all be returned. |
| `expected_paths` | Complete ordered symbol paths that must all be returned. |
| `forbidden_symbols`, `forbidden_paths` | Known wrong answers; any hit fails correctness. |
| `required_evidence` | Exact evidence labels that the observation must retain. |
| `required_diagnostics` | Exact `{kind, code}` boundaries; kinds are `capability`, `stale`, `worktree-mismatch`, `truncated`, `degraded`, or `runtime-boundary`. |
| `allowed_ambiguity` | Maximum extra returned symbols plus paths, bounded to `0..=64`; it never permits a forbidden hit. |
| `rubric` | Human answer-quality criteria retained for later Agent A/B review. |
| `source_ref` | Requirement, issue, or roadmap evidence that introduced the case. |
| `paired_fixture` | Required fixture case id for a pinned-repository case. |
| `context_expectation` | Optional B5 profile, limits, required loss/read-back behavior, and omission classes. |

Unknown fields, duplicate ids or golden entries, empty required lists,
expected/forbidden conflicts, mutable pinned revisions, pinned cases that point
back into `fixtures/`, and invalid fixture links are rejected. This schema is
independent of the E0 A/B corpus, so E0 plans and historical receipts remain
compatible.

## Query Observations And Scoring

`score` consumes `agent-spec/atlas-eval/query-results-v1`. The results name the
target `corpus_version` and provide exactly one observation for every corpus
case. An observation preserves ranked symbols, complete paths, evidence,
typed diagnostics, response bytes, duration, source read-back calls, and
follow-up queries. B5 observations also carry a strict `context_receipt` with
retrieval totals/hard-cap loss, projection relevance/retention loss, serialized
bytes, omission classes, graph fingerprint, read-back, follow-ups, and load
profile. Missing, duplicate, unknown, wrong-version, or malformed
observations fail before a receipt is emitted.

The output schema is `agent-spec/atlas-eval/query-regression-v1`. Per-case
correctness and aggregate metrics use these deterministic rules:

- symbol recall is matched expected symbols divided by expected symbols;
- reciprocal rank is `1 / rank` of the first expected symbol, and aggregate
  MRR is the mean across cases;
- path precision and recall use exact ordered canonical paths;
- evidence recall uses exact required evidence labels;
- forbidden-hit rate is forbidden symbol and path hits divided by all returned
  symbols and paths;
- response bytes, latency, read-back calls, and follow-up queries report median
  and median absolute deviation;
- capability and stale diagnostic counts remain visible in the aggregate;
- runtime-boundary diagnostics are matched exactly per case and remain query
  hints rather than graph facts.
- context receipt counts must be internally possible, response bytes and
  follow-up counts must agree with the observation, and required profile,
  omission, loss, and read-back behavior must match the case.

The receipt also records a BLAKE3 `corpus_fingerprint` and preserves each
case's observed typed diagnostic codes. A structurally valid observation set
always produces a receipt. If any case fails correctness, `score` writes that
receipt first and then exits non-zero with `atlas-query-regression`, so CI blocks
without discarding failure evidence.

A case fails when any expected symbol or path is missing, a forbidden item is
returned, required evidence or diagnostics are missing, or extra results exceed
`allowed_ambiguity`. Therefore a symbol hit cannot hide a wrong path or stale
authority. The scorer exposes measurements and case-local correctness; it does
not import benchmark percentages from another project.

Default live probes rebuild both the basic Atlas fixture and the
runtime-boundary fixture. The latter projects the current disconnected `flow`
result into an observation and requires the expected continuation path, source
evidence, and `{kind: runtime-boundary, code: atlas-flow-runtime-boundary}`.
This catches scanner or output regressions without treating a heuristic
candidate as a persisted call edge.

## Regression Promotion Loop

When a production answer is wrong:

1. Record the issue or requirement in `source_ref`.
2. Reduce the failure to a deterministic fixture case.
3. Add or update a pinned-repository case linked through `paired_fixture`.
4. Capture a fresh observation explicitly, then run `atlas benchmark score`.
5. Keep the regression in the checked-in corpus after the fix.
6. Run the opt-in Agent A/B workflow when the change alters the default query
   or MCP surface.

Fresh pinned-repository observations and real Agent runs are explicit external
steps. Default tests rebuild the checked-in Rust fixture, project current
`rust_atlas::search` and `rust_atlas::flow` output into observations, and run the
same scorer. The pinned-repository observation remains checked-in data; no
default test performs network access or repository execution.

## Output Contract

`plan`, `summarize`, and `score` print pretty JSON followed by a newline when
`--out` is not supplied:

```bash
cargo run -- atlas benchmark plan --corpus benchmarks/atlas/corpus.json
cargo run -- atlas benchmark summarize --receipts receipts.json
cargo run -- atlas benchmark score --corpus benchmarks/atlas/query-corpus.json --results benchmarks/atlas/query-results.json
```

With `--out PATH`, they atomically replace `PATH` and keep stdout empty:

```bash
cargo run -- atlas benchmark plan --corpus benchmarks/atlas/corpus.json --out plan.json
cargo run -- atlas benchmark summarize --receipts receipts.ndjson --out summary.json
cargo run -- atlas benchmark score --corpus benchmarks/atlas/query-corpus.json --results benchmarks/atlas/query-results.json --out query-regression.json
```

Schema and parsing errors occur before JSON output. A structurally valid score
run always emits its receipt; correctness failures then return non-zero with
`atlas-query-regression`.

## Opt-In Runner

`scripts/atlas-eval/run-opt-in.sh` is intentionally separate from the CLI.
Default commands and tests do not invoke a real agent or model, or access the
network.
The runner requires `jq`, an explicit `ATLAS_EVAL_AGENT_COMMAND`, an explicit
`ATLAS_EVAL_D4_MODE` of `direct` or `worker`, and a readable
`ATLAS_EVAL_D4_RECEIPT`. The command variable is one external executable: an
explicit path or a name that resolves through `PATH` to a path containing `/`.
It cannot be a shell builtin, shell function, or command string containing
arguments; the runner never evaluates it as shell source.

```bash
export ATLAS_EVAL_AGENT_COMMAND=/absolute/path/to/evaluation-agent
export ATLAS_EVAL_D4_MODE=worker
export ATLAS_EVAL_D4_RECEIPT=benchmarks/atlas/concurrent-query-receipt-v1.json
bash scripts/atlas-eval/run-opt-in.sh plan.json receipts.ndjson -- --agent-flag value
```

The runner usage is `run-opt-in.sh PLAN RECEIPTS [-- AGENT_ARG...]`. Before
starting the executable, it rejects a missing command, a command containing a
newline or arguments, a shell builtin or function, an unavailable executable,
a missing `jq`, a malformed or empty run plan, an invalid D4 mode, a missing D4
receipt, and a missing SHA-256 utility. It exports the selected serving mode,
receipt path, and computed hash to the external process. It passes the plan
path and any literal arguments supplied after `--` to the executable, captures
its stdout in a temporary file, and atomically moves that file to the receipt
path only when the executable succeeds. The saved stdout is a receipt
candidate. The runner preserves those stdout bytes without parsing, adding
default query measurements, validating, or reconciling receipt output against
that plan.

`atlas benchmark summarize` is the validated receipt boundary. It typed-parses
the candidate as a JSON array or NDJSON, rejects unknown fields and empty input,
and refuses to calculate aggregates when any parsed receipt lacks
`correctness`. It does not consume the run plan, so it cannot verify that every
planned run has exactly one receipt or that the plan and receipt set are
complete matches. The agent command must produce a complete receipt candidate,
and that candidate must pass `summarize`; plan-receipt reconciliation is not
implemented.

## D4 Concurrent Query Matrix

`benchmarks/atlas/concurrent-query-receipt-v1.json` is the checked-in D4
correctness and measurement receipt. Its strict parser rejects unknown fields,
wrong schema, duplicate request ids, mixed generation/fingerprint state,
missing failed runs, invalid outcome/digest combinations, retained reader
leases, non-idle final pool state, and incomplete worktree or fallback
identity. It covers all four B5 load profiles and all seven query outcomes.

Semantic direct/worker parity, snapshot identity, typed outcome shape, resource
cleanup, and matrix completeness are correctness gates. Queue wait, service
time, heartbeat lag, response bytes, CPU, and RSS are measurements only. The
checked-in test intentionally replaces those measurements with `u64::MAX` and
still requires the correctness gate to pass, proving there is no hidden
performance threshold. See
[Rust Atlas Concurrent Query Serving](atlas-concurrent-query-serving.md).

## D2 Incremental Correctness Matrix

`fixtures/atlas/incremental-hardening/matrix.json` is the checked-in D2 case
inventory. The bound production-API test runs isolated cold, zero-change,
single-file edit, deletion, manifest-content edit, frontier-overflow, overlay,
cancellation, generation-commit-failure, and orphan-recovery cases.

Every case emits the same deterministic receipt shape: outcome, generation,
input-plan result, touched shards, resolved/unresolved edge delta, bounded
working bytes, fallback reason, and orphan recovery count. Correctness,
artifact identity, and bounded counters gate the test. Duration and operating
system RSS may be observed externally but are not cross-machine pass/fail
thresholds.

## Limits

These are evaluation and regression harnesses, not benchmark results. The
repository provides offline corpora, deterministic plan compilation, typed
receipt validation, query scoring, and robust summary statistics. It does not
include a fresh real-repository observation, a real-model run, a completed
study, or evidence of an Atlas performance gain.
