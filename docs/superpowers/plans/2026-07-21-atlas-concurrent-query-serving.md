# Atlas Concurrent Query Serving Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver roadmap D4 as an opt-in, bounded query service that preserves direct execution by default, pins one immutable generation per accepted query, isolates control/MCP transport, and reports typed overload, cancellation, panic and fallback evidence.

**Architecture:** `rust-atlas` first gains a pinned B5 query session whose reader lease spans retrieval through serialization. A binary-local fixed worker service owns bounded admission and failure state without owning graph semantics. Daemon protocol v2 and a fixed sync lane keep the loopback listener responsive; CLI and an opt-in MCP context tool consume the service. A versioned D4 receipt separates deterministic correctness gates from performance measurements.

**Tech Stack:** Rust 2024, serde JSON, clap, std TCP/thread/sync primitives, existing `GraphSnapshot`, B5 context compiler, D3 sync/runtime and E3 scorer APIs. No new crate dependency.

## Global Constraints

- Authority is `REQ-ATLAS-CONCURRENT-QUERY-SERVING`; execution is governed by `specs/task-atlas-concurrent-query-serving.spec.md`.
- Direct execution and the current MCP tool list remain the default. Worker serving requires explicit CLI or environment configuration.
- Workers are fixed `std::thread` instances behind bounded channels. Do not create per-request threads or infer worker count from host cores.
- Admission pins one immutable generation before queueing. Retry reuses that lease and never resolves `CURRENT.json` again.
- Worker operations are frozen reads and cannot build, refresh, publish, mutate KLL or write source.
- Busy, timeout, cancelled, degraded, failed and unavailable are distinct typed outcomes; none may carry success-shaped context.
- Cancellation is cooperative at documented bounded checkpoints. Do not claim forceful preemption of arbitrary OS calls.
- The checked-in numeric defaults and validation ranges in the design and Task Contract are exact.
- D4 latency, heartbeat, CPU and RSS are measurements, not correctness gates. Worker default promotion remains blocked on E1 evidence and human acceptance.
- Use TDD for every named selector. Do not modify a selector merely to accommodate implementation failure.
- Do not modify or stage `.superpowers/`.

---

## Task 1: Commit And Gate D4 Governance

**Files:**
- Create: `knowledge/requirements/req-atlas-concurrent-query-serving.md`
- Create: `specs/task-atlas-concurrent-query-serving.spec.md`
- Create: `docs/superpowers/specs/2026-07-21-atlas-concurrent-query-serving-design.md`
- Create: `docs/superpowers/plans/2026-07-21-atlas-concurrent-query-serving.md`

**Interfaces:**
- Consumes: accepted roadmap D4, `REQ-ATLAS-LIVE-RUNTIME`, `REQ-ATLAS-QUERY-CONTEXT-COMPILER`, `REQ-ATLAS-QUERY-QUALITY-REGRESSION`.
- Produces: one accepted requirement, one active Task Contract with 19 selectors, one approved design and this execution plan.

- [x] **Step 1: Parse and lint the Task Contract**

Run:

```bash
target/debug/agent-spec parse specs/task-atlas-concurrent-query-serving.spec.md
target/debug/agent-spec lint specs/task-atlas-concurrent-query-serving.spec.md --min-score 0.7
```

Expected: 19 scenarios, all explicit test selectors, quality at least 0.7, no error diagnostic.

- [x] **Step 2: Gate KLL and planning coverage**

Run:

```bash
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected: `REQ-ATLAS-CONCURRENT-QUERY-SERVING` is one ready leaf covered only by `task-atlas-concurrent-query-serving`; no error diagnostic.

- [x] **Step 3: Commit the governance surface**

```bash
git add knowledge/requirements/req-atlas-concurrent-query-serving.md \
  specs/task-atlas-concurrent-query-serving.spec.md \
  docs/superpowers/specs/2026-07-21-atlas-concurrent-query-serving-design.md \
  docs/superpowers/plans/2026-07-21-atlas-concurrent-query-serving.md
git commit -m "docs(atlas): define concurrent query serving contract"
```

---

## Task 2: Retain A Pinned Context Session Through Projection

**Files:**
- Modify: `crates/rust-atlas/src/context.rs`
- Modify: `crates/rust-atlas/src/generation.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Test: `crates/rust-atlas/src/context.rs`
- Test: `crates/rust-atlas/src/generation.rs`

**Interfaces:**
- Produces: `PinnedContextSnapshot`, `ContextExecutionControl`, `pin_context_snapshot`, `retrieve_context_pinned`, `project_context_controlled`.
- Consumers: Task 3 production `QueryRunner` and Task 4 daemon admission.

- [x] **Step 1: Write failing pinned-generation and cancellation tests**

Add `test_atlas_context_pinned_session_survives_writer_publish` and
`test_atlas_context_control_cancels_projection_at_checkpoint`. The first pins A, publishes B,
compiles from A, proves A remains during the session, drops it, then proves reclamation may remove A.
The second cancels after retrieval and asserts `AtlasError::QueryCancelled` before any source body is
projected.

- [x] **Step 2: Run RED**

```bash
cargo test -p rust-atlas test_atlas_context_pinned_session_survives_writer_publish -- --nocapture
cargo test -p rust-atlas test_atlas_context_control_cancels_projection_at_checkpoint -- --nocapture
```

Expected: compile failure for missing pinned/control APIs.

- [x] **Step 3: Add the pinned and control contracts**

Implement these exact public shapes with private fields:

```rust
pub struct PinnedContextSnapshot { /* GraphSnapshot + validated Meta */ }

impl PinnedContextSnapshot {
    pub fn generation(&self) -> Option<&str>;
    pub fn graph_fingerprint(&self) -> &str;
    pub fn estimated_index_bytes(&self) -> u64;
}

#[derive(Clone)]
pub struct ContextExecutionControl { /* cancel token + optional Instant */ }

impl ContextExecutionControl {
    pub fn unlimited() -> Self;
    pub fn with_deadline(deadline: std::time::Instant) -> Self;
    pub fn cancel(&self);
    pub fn checkpoint(&self) -> Result<(), AtlasError>;
}

pub fn pin_context_snapshot(graph_dir: &Path) -> Result<PinnedContextSnapshot, AtlasError>;
pub fn retrieve_context_pinned(
    code_root: &Path,
    graph_dir: &Path,
    snapshot: &PinnedContextSnapshot,
    intent: &QueryIntent,
    options: &ContextOptions,
    control: &ContextExecutionControl,
) -> Result<RetrievalCandidateSet, AtlasError>;
pub fn project_context_controlled(
    code_root: &Path,
    retrieval: &RetrievalCandidateSet,
    options: &ContextOptions,
    control: &ContextExecutionControl,
) -> Result<ContextResult, AtlasError>;
```

`pin_context_snapshot` reads `CURRENT.json`, creates the D3 reader lease, validates meta and records
the query-index file size; it does not hash the worktree or deserialize the index. Pinned retrieval
loads meta/index/status only from `snapshot.data_dir`. Add `QueryCancelled` and `QueryTimeout` Atlas
errors with prefixes `atlas-query-cancelled` and `atlas-query-timeout`.

- [x] **Step 4: Add bounded checkpoints and route direct compilation through the session**

Check control before index load, after status/index load, after every identifier/relation expansion,
after retrieval, for each projection candidate/source slice, and before receipt serialization.
Refactor `compile_context` to retain the final selected snapshot until `ContextResult` is finalized;
for non-frozen direct mode, perform any existing refresh before creating the final pinned session.

- [x] **Step 5: Verify parity and commit**

```bash
cargo test -p rust-atlas context::tests -- --nocapture
cargo test -p rust-atlas test_atlas_reader_lease_preserves_old_generation_until_drop -- --nocapture
cargo clippy -p rust-atlas --all-targets -- -D warnings
git add crates/rust-atlas/src/context.rs crates/rust-atlas/src/generation.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): retain pinned context sessions"
```

Expected: existing B5 tests remain byte-identical; new session/cancellation tests pass.

---

## Task 3: Build The Bounded Query Service Core

**Files:**
- Create: `src/atlas_query_service.rs`
- Modify: `src/main.rs`
- Test: `src/atlas_query_service.rs`

**Interfaces:**
- Consumes: Task 2 pinned/context control APIs.
- Produces: validated config, fixed pool, query request/reply/receipt/status, cancellation and completion polling.
- Task 4 owns all TCP mapping; this module has no socket or MCP dependency.

- [x] **Step 1: Add failing config, queue, memory, timeout and cancellation tests**

Add the exact selectors:

```text
test_atlas_query_service_defaults_direct_and_validates_bounds
test_atlas_query_service_rejects_full_queue_with_typed_busy
test_atlas_query_service_rejects_memory_reservation_over_budget
test_atlas_query_service_expires_queued_job_before_execution
test_atlas_query_service_cancels_queued_job_before_execution
test_atlas_query_service_times_out_executing_runner_at_checkpoint
test_atlas_query_service_cancels_running_projection_and_releases_lease
```

Use an injected `Clock` and `QueryRunner`; barriers and atomic counters replace wall-clock sleeps.

- [x] **Step 2: Run RED**

```bash
cargo test test_atlas_query_service_ -- --nocapture
```

Expected: module/types absent.

- [x] **Step 3: Implement exact service types**

```rust
pub(crate) struct QueryServiceConfig {
    pub workers: usize,
    pub queue_capacity: usize,
    pub queue_timeout_ms: u64,
    pub deadline_ms: u64,
    pub memory_budget_bytes: u64,
    pub retry_after_ms: u64,
}

pub(crate) enum QueryOutcome {
    Success, Busy, Timeout, Cancelled, Degraded, Failed, Unavailable,
}

pub(crate) struct QueryServiceRequest {
    pub request_id: String,
    pub query: String,
    pub options: rust_atlas::ContextOptions,
    pub snapshot: rust_atlas::PinnedContextSnapshot,
}

pub(crate) struct QueryServiceReply {
    pub outcome: QueryOutcome,
    pub context: Option<rust_atlas::ContextResult>,
    pub receipt: QueryServiceReceipt,
    pub diagnostic: Option<String>,
    pub retry_after_ms: Option<u64>,
}

pub(crate) trait QueryRunner: Send + Sync + 'static {
    fn run(
        &self,
        request: &QueryServiceRequest,
        control: &rust_atlas::ContextExecutionControl,
    ) -> Result<rust_atlas::ContextResult, rust_atlas::AtlasError>;
}
```

`QueryService::new`, `try_submit`, `cancel`, `try_completion`, `status` and `shutdown` own fixed
workers and bounded job/completion channels. Validate all exact ranges from the design. Validate
request ids as 1..=64 ASCII alphanumeric plus `-`/`_`. Reservation is checked with overflow-safe
arithmetic over index bytes times four, request bytes and B5 max output bytes.

- [x] **Step 4: Add panic retry and strict outcome tests**

Write RED tests:

```text
test_atlas_query_worker_panic_retries_once_then_opens_circuit
test_atlas_query_service_outcomes_are_typed_and_mutually_exclusive
```

Wrap only `QueryRunner::run` in `catch_unwind(AssertUnwindSafe(...))`. Retry once against the same
request object. Two consecutive panics open a process-local circuit until service drop. A successful
result never resets an already open circuit.

- [x] **Step 5: Verify and commit**

```bash
cargo test atlas_query_service::tests -- --nocapture
cargo clippy --bin agent-spec -- -D warnings
git add src/atlas_query_service.rs src/main.rs
git commit -m "feat(atlas): add bounded query service"
```

---

## Task 4: Integrate Protocol V2 And Keep Daemon Control Live

**Files:**
- Modify: `src/atlas_daemon.rs`
- Modify: `src/atlas_query_service.rs`
- Modify: `src/main.rs`
- Test: `src/atlas_daemon.rs`

**Interfaces:**
- Consumes: Task 3 `QueryService`.
- Produces: protocol-v2 context/cancel/service-status commands, pending response sockets and one fixed maintenance lane.

- [x] **Step 1: Write failing protocol and heartbeat tests**

Add:

```text
test_atlas_query_service_keeps_control_heartbeat_live_during_slow_query
test_atlas_query_worker_pins_generation_across_writer_publish
test_atlas_daemon_stop_cancels_queued_queries_and_drains_workers
test_atlas_daemon_maintenance_lane_keeps_status_live_during_sync
test_atlas_query_service_rejects_oversize_protocol_response
```

The tests use listener/worker barriers. Assert status/service-status/stop responses are observed
before releasing query or sync barriers; do not use a latency threshold as the assertion.

- [x] **Step 2: Run RED**

```bash
cargo test atlas_daemon::tests::test_atlas_query_ -- --nocapture
cargo test atlas_daemon::tests::test_atlas_daemon_maintenance_lane -- --nocapture
```

Expected: missing protocol/service integration.

- [x] **Step 3: Upgrade the strict protocol**

Set protocol and identity schema/version to v2. Extend `DaemonCommand` with `Context`, `Cancel` and
`ServiceStatus`. Add optional strict payloads whose command/payload combinations are validated before
admission. Add service response fields with `skip_serializing_if` so legacy status/sync response
fields retain their existing shape. Unknown fields and invalid request ids fail before pinning.

- [x] **Step 4: Make query completion asynchronous to the listener**

The listener pins a snapshot, calls `try_submit`, and stores accepted `TcpStream` by request id. It
drains `try_completion` each loop and writes one response. Busy/degraded/unavailable responses are
written immediately. Cancel uses a second connection and never waits for the target query.

- [x] **Step 5: Move sync into one fixed maintenance lane**

Use one bounded channel of capacity 1 and one lifetime worker. Both auto-sync and manual Sync enqueue
the existing `sync_once`; manual request sockets wait in the main-loop map. Status and stop remain
inline. Stop flips the existing build cancellation token, cancels query jobs, returns a verified
response, removes the registry and performs bounded cooperative drain without clearing pending events.

- [x] **Step 6: Verify daemon regressions and commit**

```bash
cargo test atlas_daemon::tests -- --nocapture
cargo test test_atlas_pending_watermark_preserves_mid_sync_events -- --nocapture
cargo test test_atlas_reader_lease_preserves_old_generation_until_drop -- --nocapture
cargo clippy --bin agent-spec -- -D warnings
git add src/atlas_daemon.rs src/atlas_query_service.rs src/main.rs
git commit -m "feat(atlas): isolate daemon query and sync work"
```

---

## Task 5: Add Explicit CLI Worker And Fallback Modes

**Files:**
- Modify: `src/main.rs`
- Modify: `src/atlas_daemon.rs`
- Test: `src/main.rs`

**Interfaces:**
- Produces: daemon worker config flags, `atlas daemon service-status`, `atlas context --execution worker`, `--fallback-direct`.
- Preserves: existing direct `atlas context` JSON bytes and exit behavior.

- [ ] **Step 1: Write failing clap and direct/worker parity tests**

Add `test_atlas_query_service_worker_matches_frozen_direct_result` and a clap test that covers every
daemon numeric flag, direct/worker values, fallback-with-direct rejection and invalid ranges.

- [ ] **Step 2: Run RED**

```bash
cargo test test_atlas_query_service_worker_matches_frozen_direct_result -- --nocapture
cargo test test_atlas_concurrent_query_cli_parse_contract -- --nocapture
```

- [ ] **Step 3: Implement CLI surfaces**

`atlas daemon start|serve` accept `--query-workers`, `--query-queue-capacity`,
`--query-queue-timeout-ms`, `--query-deadline-ms`, `--query-memory-budget-bytes` and
`--query-retry-after-ms`. Detached start forwards every value byte-for-byte. `service-status` prints
strict JSON.

`atlas context --execution direct|worker` defaults to direct. Worker mode forces frozen service
execution and emits `agent-spec/atlas-query-service-response-v1`. `--fallback-direct` is rejected in
direct mode; in worker mode it may handle busy/degraded/unavailable only after cancelling or proving
the worker job was not running.

- [ ] **Step 4: Preserve generation-honest fallback**

Add `test_atlas_query_worker_fallback_records_distinct_complete_generation`. The wrapper records
`worker_generation`, `worker_outcome`, `fallback_generation`, `fallback_graph_fingerprint` and one
digest of the complete fallback context. It never retains a partial worker context.

- [ ] **Step 5: Verify and commit**

```bash
cargo test test_atlas_concurrent_query_cli_parse_contract -- --nocapture
cargo test test_atlas_query_service_worker_matches_frozen_direct_result -- --nocapture
cargo test test_atlas_query_worker_fallback_records_distinct_complete_generation -- --nocapture
git add src/main.rs src/atlas_daemon.rs
git commit -m "feat(atlas): expose opt-in worker context queries"
```

---

## Task 6: Preserve MCP Discovery While Isolating Worker Transport

**Files:**
- Modify: `src/spec_mcp/mod.rs`
- Modify: `src/spec_mcp/tools.rs`
- Modify: `src/main.rs`
- Test: `src/spec_mcp/mod.rs`
- Test: `src/spec_mcp/tools.rs`

**Interfaces:**
- Produces: hidden `atlas_context` tool and opt-in concurrent stdio dispatcher.
- Preserves: default 11-tool discovery and serial direct handling.

- [ ] **Step 1: Write failing discovery and ping-liveness test**

Add `test_atlas_mcp_context_worker_mode_preserves_discovery_and_ping_liveness`. In one in-memory
stdio transcript, hold a context worker at a barrier, send ping, observe ping output first, release
the worker, then observe the context response. Assert default discovery remains exactly unchanged.

- [ ] **Step 2: Run RED**

```bash
cargo test test_atlas_mcp_context_worker_mode_preserves_discovery_and_ping_liveness -- --nocapture
```

- [ ] **Step 3: Add the hidden tool and strict arguments**

Expose `atlas_context` only when `AGENT_SPEC_MCP_ATLAS_CONTEXT=1`. Accept `query`, profile enum,
`after`, `expect_graph` and bounded `max_bytes`; force frozen mode. Without worker routing, dispatch
uses the direct pinned API and keeps the existing pure `handle_request` tests.

- [ ] **Step 4: Add opt-in concurrent serving**

When `AGENT_SPEC_MCP_ATLAS_QUERY_MODE=worker`, route only `atlas_context` calls through a fixed,
bounded client lane that waits for daemon protocol v2. The stdin reader continues handling
initialize/ping/tools-list/notifications. One writer owns stdout and may emit responses out of input
order by JSON-RPC id. Queue rejection is an MCP `isError: true` response containing the typed Atlas
outcome. Default env uses the old serial loop.

- [ ] **Step 5: Verify and commit**

```bash
cargo test spec_mcp::tests -- --nocapture
cargo test spec_mcp::tools::tests -- --nocapture
cargo clippy --bin agent-spec -- -D warnings
git add src/spec_mcp/mod.rs src/spec_mcp/tools.rs src/main.rs
git commit -m "feat(atlas): add opt-in concurrent MCP context"
```

---

## Task 7: Add The D4 Correctness And Measurement Receipt

**Files:**
- Modify: `src/atlas_eval.rs`
- Modify: `src/main.rs`
- Create: `fixtures/atlas/concurrent-query/Cargo.toml`
- Create: `fixtures/atlas/concurrent-query/src/lib.rs`
- Create: `benchmarks/atlas/concurrent-query-receipt-v1.json`
- Modify: `scripts/atlas-eval/run-opt-in.sh`
- Test: `src/atlas_eval.rs`

**Interfaces:**
- Produces: `ConcurrentQueryReceipt`, strict scorer/gate and checked-in direct/worker matrix.
- E1 consumes: serving mode plus receipt path/hash; B5 `QueryLoadProfile` remains the workload authority.

- [ ] **Step 1: Write failing strict receipt tests**

Add parse failures for wrong schema, unknown fields, duplicate request ids, missing failed runs,
success without context digest, non-success with digest, mixed generation/fingerprint and promoted
measurement thresholds. Add the acceptance selector
`test_atlas_concurrent_query_checked_in_receipt_is_passing`.

- [ ] **Step 2: Run RED**

```bash
cargo test atlas_eval::tests::test_atlas_concurrent_query_ -- --nocapture
```

- [ ] **Step 3: Implement schema and gate**

Use schema `agent-spec/atlas-eval/concurrent-query-v1`. Record workload/revision/config, per-request
B5 load profile, typed outcome, reservation, attempts, generation/fingerprint, response digest,
lease cleanup and pool counters. Store queue/service/heartbeat timing, response bytes, CPU and RSS in
a `measurements` object that the correctness gate validates for shape but never compares to a pass
threshold.

- [ ] **Step 4: Build the checked-in matrix**

Cover light, traversal, source-heavy and mixed direct/worker parity; queue and memory busy; queued
timeout; executing cancel; one retry/repeated panic; publish race; stop; unavailable; fallback; two
worktrees. Retain every failed injected run as an expected typed case. Generate stable request ids
and canonical response digests from the fixture.

- [ ] **Step 5: Extend the opt-in E1 hook without promoting defaults**

Add direct/worker serving mode and D4 receipt hash/path to `RunReceipt`. The opt-in shell runner
requires an explicit agent command and explicit D4 mode; it must not manufacture worker benefit or
drop failed trials. No threshold is added until E1 baseline evidence exists.

- [ ] **Step 6: Verify and commit**

```bash
cargo test atlas_eval::tests::test_atlas_concurrent_query_ -- --nocapture
cargo test test_atlas_context_compiler_checked_in_regression_receipt_is_passing -- --nocapture
git add src/atlas_eval.rs src/main.rs fixtures/atlas/concurrent-query \
  benchmarks/atlas/concurrent-query-receipt-v1.json scripts/atlas-eval/run-opt-in.sh
git commit -m "test(atlas): gate concurrent query serving"
```

---

## Task 8: Document, Dogfood And Close D4

**Files:**
- Create: `docs/atlas-concurrent-query-serving.md`
- Modify: `docs/atlas-live-runtime.md`
- Modify: `docs/atlas-query-context.md`
- Modify: `docs/atlas-evaluation.md`
- Modify: `docs/atlas-roadmap.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `CHANGELOG.md`
- Modify: `.agent-spec/wiki/**`
- Modify: `docs/superpowers/plans/2026-07-21-atlas-concurrent-query-serving.md`

**Interfaces:**
- Produces: final command/config guidance, measured fixture numbers, D4 delivered status and current trace evidence.

- [ ] **Step 1: Add reader-facing workflow and authority boundaries**

Document direct default, daemon opt-in startup, context worker/fallback, service status, MCP env gates,
typed outcomes, receipt interpretation and recovery. State that worker mode cannot improve graph
coverage/freshness and remains non-default pending E1.

- [ ] **Step 2: Update roadmap and working memory**

Mark D4 delivered only as an opt-in prototype. Keep E1 real Agent A/B pending. Record actual fixture
workers, queue capacity, profile counts, busy/timeout/cancel/crash cases, response bytes and
unavailable/degraded observations. Update wiki source links without promoting derived runtime facts to KLL.

- [ ] **Step 3: Run code and documentation gates**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/docs-lint.sh
target/debug/agent-spec wiki check --code . --wiki .agent-spec/wiki
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected: code, docs, wiki and KLL gates pass; missing optional local docs tools may be reported only according to the existing script policy.

- [ ] **Step 4: Run lifecycle and trace on the final commit**

```bash
target/debug/agent-spec atlas build --code . --graph .agent-spec/graph --full
target/debug/agent-spec lifecycle specs/task-atlas-concurrent-query-serving.spec.md \
  --code . --format json --run-log-dir .
target/debug/agent-spec requirements replay REQ-ATLAS-CONCURRENT-QUERY-SERVING --format text
target/debug/agent-spec requirements trace-graph REQ-ATLAS-CONCURRENT-QUERY-SERVING --format mermaid
```

Expected: 19/19 scenarios pass with zero skip/uncertain; trace reaches one current canonical symbol and the final Git commit.

- [ ] **Step 5: Commit final docs and evidence**

```bash
git add docs/atlas-concurrent-query-serving.md docs/atlas-live-runtime.md \
  docs/atlas-query-context.md docs/atlas-evaluation.md docs/atlas-roadmap.md \
  README.md AGENTS.md skills/agent-spec-tool-first/SKILL.md CHANGELOG.md \
  .agent-spec/wiki docs/superpowers/plans/2026-07-21-atlas-concurrent-query-serving.md
git commit -m "docs(atlas): publish concurrent query serving"
```
