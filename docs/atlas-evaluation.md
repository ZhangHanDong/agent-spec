# Atlas Evaluation Baseline

Atlas evaluation provides a reproducible comparison harness for an Atlas-assisted
agent arm and a baseline arm. It defines inputs, run scheduling, receipts, and
summary statistics. It does not run an agent by default, contact a model, or
access the network.

The checked-in corpus is `benchmarks/atlas/corpus.json`. It is an offline
fixture, not evidence that a real model evaluation has run or that Atlas yields
a performance improvement.

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
  "response_bytes": 24000,
  "read_back_calls": 1,
  "follow_up_queries": 2,
  "truncated_queries": 0
}
```

`arm` is `atlas` or `baseline`; `trial` is a positive integer. The
`correctness` object is required for summarization and contains `passed`.
`cost_usd` is optional, but when present it must be finite and non-negative.
The remaining measurements are unsigned integers. Current external agent
commands must emit all four query measurements, including explicit zeroes:

| Field | Meaning |
|---|---|
| `response_bytes` | Total raw bytes returned by exploration and query tools during the run. |
| `read_back_calls` | Source-read calls made after an `atlas explore` response to retrieve source already represented by that response. |
| `follow_up_queries` | Exploration or graph queries issued after the run's initial exploration query. |
| `truncated_queries` | Query responses that reported truncation or another response limit. |

For backward compatibility, receipts created before these fields were added
deserialize omitted query measurements as zero. Omission is reserved for those
legacy receipts; new producers must report the measured values explicitly.

The result contains total `receipts`, total `correctness` counts, aggregate
`metrics`, and an `arms` object keyed by the populated arms. Each metric has
`samples`, `median`, and `mad` (median absolute deviation). Metrics are
`file_reads`, `graph_calls`, `tool_calls`, `duration_ms`, `context_bytes`, and
optional `cost_usd`, plus `response_bytes`, `read_back_calls`,
`follow_up_queries`, and `truncated_queries`. The optional cost metric is absent
when no receipt reports a cost. Aggregate and per-arm metrics use the same
calculation.

## Output Contract

`plan` and `summarize` print pretty JSON followed by a newline when `--out` is
not supplied:

```bash
cargo run -- atlas benchmark plan --corpus benchmarks/atlas/corpus.json
cargo run -- atlas benchmark summarize --receipts receipts.json
```

With `--out PATH`, they atomically replace `PATH` and keep stdout empty:

```bash
cargo run -- atlas benchmark plan --corpus benchmarks/atlas/corpus.json --out plan.json
cargo run -- atlas benchmark summarize --receipts receipts.ndjson --out summary.json
```

Validation, parsing, and correctness errors occur before normal JSON output.

## Opt-In Runner

`scripts/atlas-eval/run-opt-in.sh` is intentionally separate from the CLI.
Default commands and tests do not invoke a real agent or model, or access the
network.
The runner requires `jq` and an explicit `ATLAS_EVAL_AGENT_COMMAND` value.
That variable is one external executable: an explicit path or a name that
resolves through `PATH` to a path containing `/`. It cannot be a shell builtin,
shell function, or command string containing arguments; the runner never
evaluates it as shell source.

```bash
export ATLAS_EVAL_AGENT_COMMAND=/absolute/path/to/evaluation-agent
bash scripts/atlas-eval/run-opt-in.sh plan.json receipts.ndjson -- --agent-flag value
```

The runner usage is `run-opt-in.sh PLAN RECEIPTS [-- AGENT_ARG...]`. Before
starting the executable, it rejects a missing command, a command containing a
newline or arguments, a shell builtin or function, an unavailable executable,
a missing `jq`, and a malformed or empty run plan. It passes the plan path and
any literal arguments supplied after `--` to the executable, captures its
stdout in a temporary file, and atomically moves that file to the receipt path
only when the executable succeeds. The saved stdout is a receipt candidate. The
runner preserves those stdout bytes without parsing, adding default query
measurements, validating, or reconciling receipt output against that plan.

`atlas benchmark summarize` is the validated receipt boundary. It typed-parses
the candidate as a JSON array or NDJSON, rejects unknown fields and empty input,
and refuses to calculate aggregates when any parsed receipt lacks
`correctness`. It does not consume the run plan, so it cannot verify that every
planned run has exactly one receipt or that the plan and receipt set are
complete matches. The agent command must produce a complete receipt candidate,
and that candidate must pass `summarize`; plan-receipt reconciliation is not
implemented.

## Limits

This is a baseline harness, not a benchmark result. The repository provides an
offline corpus, deterministic plan compilation, receipt validation, and robust
summary statistics. It does not include a real-model run, a completed study, or
evidence of an Atlas performance gain.
