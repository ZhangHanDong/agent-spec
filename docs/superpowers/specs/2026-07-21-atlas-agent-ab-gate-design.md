# Atlas Real Agent A/B Gate Design

**Status:** Approved by the reviewed `docs/atlas-roadmap.md` E1 contract

**Requirement:** `REQ-ATLAS-AGENT-AB-GATE`

**Task Contract:** `specs/task-atlas-agent-ab-gate.spec.md`

## Purpose

E1 answers adoption questions that deterministic graph tests cannot answer. It decides whether a
real coding Agent benefits from Atlas primitives, whether B5 context adds value beyond those
primitives, and whether D4 worker serving improves concurrent use without changing answers. It does
not choose an Agent vendor, call a model from default tests, or promote any default automatically.

## Experiment Boundaries

Two experiments remain separate:

1. Agent A/B/C evaluates answer correctness and interaction cost. Arm A has built-in Read/Grep,
   arm B adds the current Atlas primitive and explore surface, and arm C adds only B5 context to B.
2. Serving direct/worker evaluates concurrent transport and scheduling. It uses four B5 load
   profiles and does not substitute for answer-quality trials.

This separation prevents architecture-answer gains from approving implementation or stale cases,
and prevents single-request Agent timing from approving a concurrent worker pool.

## Versioned Artifacts

The checked-in `agent-experiment-v1` manifest fixes environment symmetry and three tool surfaces.
It compiles with the existing E0 corpus into `agent-plan-v1`; every case/trial produces three runs.
The plan carries a corpus fingerprint, experiment fingerprint, environment fingerprint, surface
fingerprint, rubric fingerprint, and stable run id.

An external driver produces one strict `agent-receipts-v1` bundle. Every planned run appears exactly
once, including failed, timed out, and cancelled runs. Each run records:

- outcome and correctness;
- judge version and rubric fingerprint;
- raw-session location/hash, answer hash, and tool-trace hash;
- Read, Grep, graph, total tool, and round-trip counts;
- wall time, response bytes, context bytes, query metrics, and optional cost;
- whether stale evidence was displayed as fresh.

Canonical raw sessions may live outside Git, but their path must not be under `/tmp` and their
64-character lowercase hash remains in the receipt. The gate does not read or upload session
content.

The independent `serving-plan-v1` fixes a non-fixture repository at a 40-character revision, four
B5 profiles, direct/worker arms, burst width, heartbeat budget, and at least three trials per arm.
Its receipt records logical result counts, semantic/snapshot digests, overload outcomes, heartbeat,
tail latency, batch duration, CPU, RSS, and failure evidence.

## Symmetry And Ablation

Prompt hooks, MCP config, user skills, and tool instructions are either `disabled` or `pinned` with
a fingerprint. They are global experiment controls, not arm-local settings. The same model, prompt,
repository revision, permissions, cache condition, judge, and retention root are copied into each
matched run.

Tool surfaces are validated as sets:

- A contains `read` and `grep` and no `atlas-*` tool;
- B contains all of A plus `atlas-explore` and one or more Atlas primitives, but no
  `atlas-context`;
- C is exactly B plus `atlas-context`.

This makes B versus A the Atlas total-value comparison and C versus B the B5 incremental-value
comparison. A C result can never retroactively approve B, and a B result cannot approve C.

## Correctness-First Gate

Structural validation happens before aggregation. The plan and receipt fingerprints must agree;
run ids must be exact; all strict query metrics must be present; evidence hashes must be valid; and
non-success outcomes must retain diagnostics.

Correctness then blocks efficiency. Candidate failures, incorrect answers, stale-as-fresh results,
judge mismatches, or missing evidence block the affected comparison. Failed samples remain listed in
the gate receipt and remain part of completeness accounting.

Only after correctness passes does the gate compare metrics. For each matched case:

- baseline median is the reference;
- baseline median absolute deviation is the evidence-derived noise band;
- medium/large candidates must improve Read+Grep, round trips, and total tool calls by more than the
  corresponding baseline MAD;
- small candidates are `improved`, `tie`, or `blocked`; a candidate increase within baseline MAD is
  a visible tie, while a larger increase is blocked.

No percentage from CodeGraph or another project appears in the algorithm. Results remain grouped by
task class and workspace size. The gate emits independent `atlas_primitives` and `atlas_context`
promotion candidates, each with `passed`, `blocked`, or `pending` state and explicit diagnostics.

## Concurrent Gate

Serving receipts are valid only for a repository outside `fixtures/` with a full pinned Git
revision. Every direct/worker/profile/trial run must exist. A worker comparison blocks when it loses
a logical query, changes the semantic digest, reads another snapshot, reports stale evidence,
times out in the queue, or exceeds the declared heartbeat budget.

After correctness and liveness pass, worker batch duration must improve beyond the direct-arm MAD;
worker p95 latency may not regress beyond the direct MAD. CPU and RSS remain observations. A passing
machine gate is still only a candidate for human acceptance; it never edits defaults.

## Execution And Honesty

Two shell runners are opt-in. Each accepts one explicit executable through an environment variable,
validates the plan before launch, captures output atomically, and leaves gate execution explicit.
They do not embed an Agent SDK or shell-evaluate command strings.

The repository checks in manifests and deterministic plans, not invented run receipts. Until a real
driver fills every run and a human accepts the gate output, roadmap E1 remains pending, MCP discovery
does not change, B5 keeps its current default profile, and D4 remains direct by default.

## Non-Goals

- Built-in model invocation or judge implementation
- Selecting an Agent vendor or normalizing proprietary session formats
- Uploading raw sessions
- Treating deterministic fixture receipts as real Agent evidence
- Automatically rewriting MCP, context, or worker defaults
