# Atlas Real Agent A/B Gate

The E1 gate evaluates whether Atlas improves real Agent work. The repository
ships deterministic schemas, plan compilers, receipt validators, and promotion
gates. It does not ship a model driver, run a model from tests, or claim that an
Atlas candidate has passed.

## Agent A/B/C

The checked-in experiment is
`benchmarks/atlas/agent-ab-experiment-v1.json`. It fixes environment controls
and three tool surfaces:

| Arm | Surface | Decision |
|---|---|---|
| `baseline` | Built-in Read and Grep | Reference only |
| `atlas-primitives` | Baseline plus current Atlas primitives and explore | B versus A decides the Atlas default-surface candidate |
| `atlas-context` | B plus B5 context | C versus B decides the B5 candidate |

Compile the accepted E0 corpus into the versioned 72-run plan:

```bash
cargo run -- atlas benchmark agent-plan \
  --corpus benchmarks/atlas/corpus.json \
  --experiment benchmarks/atlas/agent-ab-experiment-v1.json \
  --out benchmarks/atlas/agent-ab-plan-v1.json
```

Each case has three trials in each arm. Model, prompt, repository revision,
permissions, cache condition, prompt hooks, MCP config, user skills, tool
instructions, judge version, and session-retention root are fingerprinted and
matched. Only the declared tool set changes.

Run a host-specific driver through the explicit boundary:

```bash
ATLAS_EVAL_AGENT_COMMAND=/absolute/path/to/agent-driver \
  scripts/atlas-eval/run-agent-ab-opt-in.sh \
  benchmarks/atlas/agent-ab-plan-v1.json \
  .agent-spec/evaluation/agent-receipts.json
```

The driver receives the plan path as its first argument and writes one strict
`agent-receipts-v1` JSON object to stdout. The runner validates the top-level
schema and atomically replaces the receipt path. It never evaluates a shell
command string.

Gate the complete receipt bundle:

```bash
cargo run -- atlas benchmark agent-gate \
  --plan benchmarks/atlas/agent-ab-plan-v1.json \
  --receipts .agent-spec/evaluation/agent-receipts.json \
  --out .agent-spec/evaluation/agent-gate.json
```

A structurally invalid or incomplete bundle produces no gate receipt. A valid
but blocked experiment writes `agent-gate-v1` first and then exits non-zero.
Failed, timed-out, and cancelled runs remain in the evidence and cannot be
removed from the denominator.

## Correctness And Metrics

Every run records its outcome, rubric result and rationale, judge version,
rubric fingerprint, raw-session path/hash, answer hash, tool-trace hash,
stale-as-fresh flag, and these metrics:

- file reads, Grep calls, graph calls, total tool calls, and round trips;
- wall-clock duration, response bytes, and context bytes;
- read-backs, follow-up queries, and truncation count;
- cost when the host exposes it.

The current query-metric schema and all fields are mandatory. Legacy or partial
receipts fail before aggregation.

Correctness and freshness gate first. A candidate failure, incorrect answer, or
stale-as-fresh result blocks that case even when it uses fewer tools. For each
matched case, the reference arm supplies a median and median absolute deviation
(MAD):

- medium and large cases must improve Read+Grep, round trips, and total tool
  calls beyond the reference MAD;
- small cases report `improved`, `tie`, or `blocked`; overhead inside the MAD
  band remains a visible tie.

The algorithm derives its noise band from Atlas's own baseline trials. It does
not import a percentage from CodeGraph or another benchmark. Gate output stays
grouped by task class and workspace size.

## Direct Versus Worker

D4 worker adoption uses a separate serving experiment. The checked-in
`benchmarks/atlas/serving-ab-experiment-v1.json` is intentionally disabled and
contains replacement values. `serving-plan` rejects it until
`execution_ready=true`, the repository is outside `fixtures/`, the revision is
a nonzero 40-character Git id, and query/config fingerprints identify the real
driver inputs.

```bash
cargo run -- atlas benchmark serving-plan \
  --experiment path/to/real-serving-experiment.json \
  --out .agent-spec/evaluation/serving-plan.json

ATLAS_EVAL_SERVING_COMMAND=/absolute/path/to/burst-driver \
  scripts/atlas-eval/run-serving-ab-opt-in.sh \
  .agent-spec/evaluation/serving-plan.json \
  .agent-spec/evaluation/serving-receipts.json

cargo run -- atlas benchmark serving-gate \
  --plan .agent-spec/evaluation/serving-plan.json \
  --receipts .agent-spec/evaluation/serving-receipts.json \
  --out .agent-spec/evaluation/serving-gate.json
```

The plan covers `light`, `traversal`, `source-heavy`, and `mixed`, with at
least three direct and three worker bursts per profile. Promotion requires all
logical results, one snapshot per run, direct/worker semantic and graph parity,
no stale result or queue timeout, heartbeat within the declared budget, batch
duration improvement beyond direct MAD, and no p95 regression beyond direct
MAD. CPU and RSS remain observations.

## Authority

Machine `passed` means the receipt met its versioned gate. It is not permission
to edit defaults. Human acceptance must separately decide the affected question
classes and candidate. Until real receipts exist and are accepted:

- default MCP discovery remains unchanged;
- B5 keeps its current profile behavior;
- D4 remains direct by default;
- checked-in manifests and plans are experimental inputs, not performance
  evidence.
