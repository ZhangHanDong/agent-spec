# Atlas Real Agent A/B Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the E1 real-Agent and concurrent-serving evaluation gates without running a model by default or changing any Atlas default.

**Architecture:** A new typed evaluation module compiles the accepted E0 corpus and one versioned experiment manifest into symmetric A/B/C Agent runs plus a separate direct/worker burst plan. Strict receipt validators preserve every run and produce correctness-first, baseline-variance-derived promotion candidates. Two opt-in runners execute external drivers without binding agent-spec to a vendor.

**Tech Stack:** Rust 2024, serde JSON, clap, BLAKE3, existing Atlas evaluation types, POSIX shell. No new crate dependency.

## Global Constraints

- Authority is `REQ-ATLAS-AGENT-AB-GATE`; execution is governed by `specs/task-atlas-agent-ab-gate.spec.md`.
- Agent arms are exactly baseline, atlas-primitives, and atlas-context; serving arms are exactly direct and worker.
- Every arm schedules at least three trials; no failed, timed-out, cancelled, missing, duplicate, or unknown run may be hidden.
- Environment controls are global and fingerprinted. Only the declared tool surface differs across A/B/C.
- Correctness, freshness, session evidence, and metric completeness gate before efficiency.
- Medium/large benefit and small tie zones derive from baseline median/MAD; do not add copied percentage thresholds.
- Real execution is opt-in and external. Default tests remain offline and deterministic.
- No code path automatically changes default MCP discovery, B5 profile, or D4 execution mode.
- Do not modify or stage `.superpowers/`.
- Use TDD for every named selector and observe each RED before implementation.

---

## Task 1: Gate E1 Governance

**Files:**
- Create: `knowledge/requirements/req-atlas-agent-ab-gate.md`
- Create: `specs/task-atlas-agent-ab-gate.spec.md`
- Create: `docs/superpowers/specs/2026-07-21-atlas-agent-ab-gate-design.md`
- Create: `docs/superpowers/plans/2026-07-21-atlas-agent-ab-gate.md`

**Interfaces:**
- Consumes: roadmap E1, E0 corpus, B5 and D4 accepted contracts.
- Produces: one accepted KLL requirement and one 16-scenario active Task Contract.

- [x] **Step 1: Parse and lint the Task Contract**

Run:

```bash
target/debug/agent-spec parse specs/task-atlas-agent-ab-gate.spec.md
target/debug/agent-spec lint specs/task-atlas-agent-ab-gate.spec.md --min-score 0.7
```

Expected: 16 scenarios, explicit selectors, score at least 0.7, no error diagnostic.

- [x] **Step 2: Gate KLL graph and work planning**

Run:

```bash
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected: `REQ-ATLAS-AGENT-AB-GATE` is an accepted ready leaf covered by exactly one task spec.

- [x] **Step 3: Commit governance files**

```bash
git add knowledge/requirements/req-atlas-agent-ab-gate.md \
  specs/task-atlas-agent-ab-gate.spec.md \
  docs/superpowers/specs/2026-07-21-atlas-agent-ab-gate-design.md \
  docs/superpowers/plans/2026-07-21-atlas-agent-ab-gate.md
git commit -m "docs(atlas): define real agent evaluation gate"
```

## Task 2: Compile Symmetric Agent Plans

**Files:**
- Create: `src/atlas_agent_eval.rs`
- Modify: `src/main.rs`
- Create: `benchmarks/atlas/agent-ab-experiment-v1.json`
- Create: `benchmarks/atlas/agent-ab-plan-v1.json`
- Test: `src/atlas_agent_eval.rs`

**Interfaces:**
- Consumes: `atlas_eval::Corpus`, its case rubric and E0 fixed conditions.
- Produces: `AgentExperiment`, `AgentRunPlan`, `compile_agent_plan`, and atomic JSON loaders/writers.

- [ ] **Step 1: Write RED plan tests**

Add `test_agent_ab_plan_builds_three_symmetric_arms`,
`test_agent_ab_plan_rejects_asymmetric_surface`, and
`test_agent_ab_plan_requires_three_trials`. Construct the manifest in Rust so tests assert exact
run count, arm order, shared environment fingerprint, B/A set difference, and C/B set difference.

- [ ] **Step 2: Run RED**

```bash
cargo test test_agent_ab_plan_builds_three_symmetric_arms -- --nocapture
cargo test test_agent_ab_plan_rejects_asymmetric_surface -- --nocapture
cargo test test_agent_ab_plan_requires_three_trials -- --nocapture
```

Expected: compile failure because `atlas_agent_eval` and plan APIs do not exist.

- [ ] **Step 3: Implement strict experiment and plan types**

Define public serde types with `deny_unknown_fields`, fixed arm enums, global symmetric controls,
tool surfaces, retention root, judge config, and stable fingerprints. `compile_agent_plan` validates
the E0 corpus, rejects `/tmp` retention, validates the exact surface set relationships, and emits
runs ordered by case, trial, then A/B/C.

- [ ] **Step 4: Add CLI and checked-in inputs**

Add:

```text
agent-spec atlas benchmark agent-plan --corpus PATH --experiment PATH [--out PATH]
```

The checked-in manifest pins disabled local hooks/skills and versioned tool/config fingerprints.
Generate `agent-ab-plan-v1.json` with the command itself. With `--out`, stdout is empty and output is
atomically replaced; without it, pretty JSON plus one newline is emitted.

Add `test_agent_ab_cli_writes_atomic_outputs` in `src/main.rs`; it invokes `agent-plan --out`,
parses the complete file, asserts empty stdout, and checks the destination directory has no
same-prefix temporary file.

- [ ] **Step 5: Verify and commit**

```bash
cargo test agent_ab_plan -- --nocapture
cargo run -- atlas benchmark agent-plan \
  --corpus benchmarks/atlas/corpus.json \
  --experiment benchmarks/atlas/agent-ab-experiment-v1.json \
  --out benchmarks/atlas/agent-ab-plan-v1.json
git add src/atlas_agent_eval.rs src/main.rs benchmarks/atlas/agent-ab-experiment-v1.json \
  benchmarks/atlas/agent-ab-plan-v1.json
git commit -m "feat(atlas): compile symmetric agent evaluation plans"
```

## Task 3: Validate Strict Agent Receipts

**Files:**
- Modify: `src/atlas_agent_eval.rs`
- Test: `src/atlas_agent_eval.rs`

**Interfaces:**
- Produces: `AgentReceiptBundle`, `AgentRunReceipt`, `validate_agent_receipts`.
- Consumed by: Task 4 Agent promotion gate.

- [ ] **Step 1: Write RED receipt tests**

Add `test_agent_ab_gate_requires_exact_planned_runs`,
`test_agent_ab_gate_retains_failed_runs`,
`test_agent_ab_gate_rejects_legacy_query_metrics`, and
`test_agent_ab_gate_validates_session_evidence`. Fixtures must exercise omitted, duplicate, unknown,
failed, partial metric, `/tmp` path, judge mismatch, and malformed hash variants.

- [ ] **Step 2: Run RED**

```bash
cargo test test_agent_ab_gate_requires_exact_planned_runs -- --nocapture
cargo test test_agent_ab_gate_retains_failed_runs -- --nocapture
cargo test test_agent_ab_gate_rejects_legacy_query_metrics -- --nocapture
cargo test test_agent_ab_gate_validates_session_evidence -- --nocapture
```

Expected: compile failure for missing receipt APIs.

- [ ] **Step 3: Implement strict receipt parsing and completeness**

Require all plan/run/surface/environment/rubric fingerprints, current query metric schema, complete
metrics, non-empty judge evidence, canonical session location and lowercase 64-hex hashes. Match the
set of receipt run ids exactly to the plan before aggregation. Non-completed outcomes require a
diagnostic and `correctness.passed=false`; they remain valid evidence but block promotion.

- [ ] **Step 4: Verify and commit**

```bash
cargo test agent_ab_gate_ -- --nocapture
git add src/atlas_agent_eval.rs
git commit -m "feat(atlas): validate complete agent evaluation receipts"
```

## Task 4: Produce Scoped Correctness-First Agent Gates

**Files:**
- Modify: `src/atlas_agent_eval.rs`
- Modify: `src/main.rs`
- Test: `src/atlas_agent_eval.rs`
- Test: `src/main.rs`

**Interfaces:**
- Produces: `AgentGateReceipt`, `gate_agent_receipts`, and `atlas benchmark agent-gate`.

- [ ] **Step 1: Write RED comparison tests**

Add `test_agent_ab_gate_blocks_correctness_and_stale_regression`,
`test_agent_ab_gate_derives_benefit_from_baseline_mad`,
`test_agent_ab_gate_keeps_small_tie_zone_visible`, and
`test_agent_ab_gate_scopes_surface_promotions`. Use three distinct baseline values so median and MAD
are observable; assert per-case diagnostics and independent B/A and C/B states.

- [ ] **Step 2: Run RED**

```bash
cargo test test_agent_ab_gate_blocks_correctness_and_stale_regression -- --nocapture
cargo test test_agent_ab_gate_derives_benefit_from_baseline_mad -- --nocapture
cargo test test_agent_ab_gate_keeps_small_tie_zone_visible -- --nocapture
cargo test test_agent_ab_gate_scopes_surface_promotions -- --nocapture
```

Expected: compile failure for missing gate APIs.

- [ ] **Step 3: Implement median/MAD and scoped decisions**

Compare matched case metrics only after all candidate runs pass correctness/freshness/evidence.
Medium/large require candidate median plus baseline MAD to be lower than baseline median for
Read+Grep, round trips, and total tool calls. Small results are improved below the lower bound, tie
within the MAD band, and blocked above the upper bound. Preserve duration, bytes, context, cost,
failures, question class, and size without using them as hidden gates.

- [ ] **Step 4: Add Agent gate CLI**

Add:

```text
agent-spec atlas benchmark agent-gate --plan PATH --receipts PATH [--out PATH]
```

It emits a strict `agent-gate-v1` receipt for valid complete evidence, including blocked results;
schema/completeness errors emit no normal JSON. A blocked gate writes the receipt first, then exits
non-zero with `atlas-agent-ab-blocked`.

- [ ] **Step 5: Verify and commit**

```bash
cargo test agent_ab_gate -- --nocapture
cargo test test_atlas_benchmark_agent_gate_cli -- --nocapture
git add src/atlas_agent_eval.rs src/main.rs
git commit -m "feat(atlas): gate agent surface promotion evidence"
```

## Task 5: Gate Real Direct Versus Worker Bursts

**Files:**
- Modify: `src/atlas_agent_eval.rs`
- Modify: `src/main.rs`
- Create: `benchmarks/atlas/serving-ab-experiment-v1.json`
- Test: `src/atlas_agent_eval.rs`

**Interfaces:**
- Produces: serving plan/receipt/gate types and `serving-plan`, `serving-gate` CLI actions.

- [ ] **Step 1: Write RED serving tests**

Add `test_agent_ab_serving_plan_builds_matched_profiles`,
`test_agent_ab_serving_gate_blocks_correctness_snapshot_and_timeout_regression`, and
`test_agent_ab_serving_plan_rejects_fixture_repository`. Valid tests use a non-fixture path and a
40-hex revision; negative cases mutate lost-query counts, stale flags, semantic/snapshot digests,
queue timeouts, and repository identity.

- [ ] **Step 2: Run RED**

```bash
cargo test test_agent_ab_serving_plan_builds_matched_profiles -- --nocapture
cargo test test_agent_ab_serving_gate_blocks_correctness_snapshot_and_timeout_regression -- --nocapture
cargo test test_agent_ab_serving_plan_rejects_fixture_repository -- --nocapture
```

Expected: compile failure for missing serving APIs.

- [ ] **Step 3: Implement serving schemas and gate**

Compile four profiles times two modes times at least three trials. Validate exact run coverage,
session evidence, all logical results, zero stale and queue timeout, matched semantic and snapshot
digests, and worker heartbeat budget. Require worker batch median plus direct MAD below direct median;
reject p95 regression beyond direct MAD. Preserve CPU/RSS as observations.

- [ ] **Step 4: Add serving CLI actions and checked-in manifest**

Add:

```text
agent-spec atlas benchmark serving-plan --experiment PATH [--out PATH]
agent-spec atlas benchmark serving-gate --plan PATH --receipts PATH [--out PATH]
```

The checked-in manifest intentionally uses a descriptive external repository/revision input and is
validated only when the user replaces it with a real pinned target; do not check in a passing plan
or receipt that claims a run occurred.

- [ ] **Step 5: Verify and commit**

```bash
cargo test agent_ab_serving -- --nocapture
cargo test test_atlas_benchmark_serving_cli -- --nocapture
git add src/atlas_agent_eval.rs src/main.rs benchmarks/atlas/serving-ab-experiment-v1.json
git commit -m "feat(atlas): gate concurrent serving promotion"
```

## Task 6: Add Explicit External Runners

**Files:**
- Create: `scripts/atlas-eval/run-agent-ab-opt-in.sh`
- Create: `scripts/atlas-eval/run-serving-ab-opt-in.sh`
- Modify: `src/atlas_agent_eval.rs`
- Test: `src/atlas_agent_eval.rs`

**Interfaces:**
- Consumes: checked plan plus one explicit external executable.
- Produces: atomically replaced strict receipt bundle candidate.

- [ ] **Step 1: Write RED runner test**

Add `test_agent_ab_opt_in_runners_require_explicit_commands`. Invoke both scripts with their command
environment variables removed; assert exit 2, empty stdout, no output file, and stable diagnostics.

- [ ] **Step 2: Run RED**

```bash
cargo test test_agent_ab_opt_in_runners_require_explicit_commands -- --nocapture
```

Expected: failure because scripts do not exist.

- [ ] **Step 3: Implement safe opt-in runners**

Each runner validates its plan schema with `jq`, resolves one executable without `eval`, writes to a
same-directory temporary file, and atomically renames only after the driver exits successfully and
the JSON bundle has the expected schema. Extra driver arguments follow `--`. Signal/error cleanup
removes only the temporary file.

- [ ] **Step 4: Verify and commit**

```bash
cargo test test_agent_ab_opt_in_runners_require_explicit_commands -- --nocapture
bash -n scripts/atlas-eval/run-agent-ab-opt-in.sh
bash -n scripts/atlas-eval/run-serving-ab-opt-in.sh
git add scripts/atlas-eval/run-agent-ab-opt-in.sh scripts/atlas-eval/run-serving-ab-opt-in.sh \
  src/atlas_agent_eval.rs
git commit -m "feat(atlas): add opt-in external evaluation runners"
```

## Task 7: Publish E1 Without Claiming A Result

**Files:**
- Create: `docs/atlas-agent-ab-gate.md`
- Modify: `docs/atlas-evaluation.md`
- Modify: `docs/atlas-roadmap.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `CHANGELOG.md`
- Modify: `.agent-spec/wiki/**`
- Modify: `docs/superpowers/plans/2026-07-21-atlas-agent-ab-gate.md`

**Interfaces:**
- Produces: exact command/schema guidance and an honest E1 status.

- [ ] **Step 1: Document manifests, runners, gates, and authority**

Document A/B/C symmetry, strict receipt fields, baseline MAD rules, failure retention, scoped
promotion, real-repository serving constraint, and human acceptance. State explicitly that no real
receipt is checked in and defaults remain unchanged.

- [ ] **Step 2: Update roadmap and working guidance**

Mark the E1 harness delivered but the real Agent conclusion pending. Add the four CLI commands and
two runners to README, AGENTS, the tool-first skill, changelog, and relevant wiki pages without
turning wiki text into KLL authority.

- [ ] **Step 3: Run full verification**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
bash scripts/docs-lint.sh
target/debug/agent-spec wiki check --code . --wiki .agent-spec/wiki
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
target/debug/agent-spec lifecycle specs/task-atlas-agent-ab-gate.spec.md --code . \
  --change-scope worktree --format json --run-log-dir .agent-spec/runs
```

Expected: all deterministic gates pass with no skip or uncertain verdict; E1 documentation still
states that real Agent acceptance has not occurred.

- [ ] **Step 4: Commit publication and record post-commit evidence**

```bash
git add docs/atlas-agent-ab-gate.md docs/atlas-evaluation.md docs/atlas-roadmap.md README.md \
  AGENTS.md skills/agent-spec-tool-first/SKILL.md CHANGELOG.md .agent-spec/wiki \
  docs/superpowers/plans/2026-07-21-atlas-agent-ab-gate.md
git commit -m "docs(atlas): publish real agent evaluation gate"
target/debug/agent-spec lifecycle specs/task-atlas-agent-ab-gate.spec.md --code . \
  --format json --run-log-dir .agent-spec/runs
target/debug/agent-spec requirements replay REQ-ATLAS-AGENT-AB-GATE --format text
target/debug/agent-spec requirements trace-graph REQ-ATLAS-AGENT-AB-GATE --format mermaid
```

Expected: post-commit lifecycle references the final commit; replay reaches all 16 scenarios and
the roadmap keeps default promotion pending.
