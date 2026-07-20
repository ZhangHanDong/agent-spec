# Atlas Live Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver roadmap D3 as an optional, bounded Rust Atlas watcher/daemon that preserves pending events, single-writer generation publication, typed degradation, no-daemon query correctness and reader-safe reclamation.

**Architecture:** `rust-atlas` owns portable scope, pending/retry state, writer/reader leases, watch planning and reclamation. The agent-spec binary owns the localhost daemon process and control protocol. The daemon only schedules the existing D2 build; all CLI/MCP graph queries continue to read immutable local generations.

**Tech Stack:** Rust 2024, `notify 8.2`, `fs2 0.4`, serde JSON, std TCP/process/thread primitives, clap and the existing D2 generation transaction.

## Global Constraints

- The authoritative requirement is `REQ-ATLAS-LIVE-RUNTIME`; the executable contract is `specs/task-atlas-live-runtime.spec.md`.
- Runtime files live only under `<graph>/.runtime/` and never enter generation manifests or graph fingerprints.
- Daemon mode is opt-in. Existing no-daemon queries and default MCP discovery remain available without a daemon.
- A daemon never proxies graph facts. It only watches, records pending work and invokes the existing D2 build.
- Only loopback TCP is allowed. No remote service, LLM call, KLL write or user Agent configuration write is allowed.
- Persistent runtime JSON uses strict schema/version parsing, atomic writes, safe relative paths and a 16 MiB limit.
- Tests use deterministic watch planning and inert events; correctness does not depend on OS event timing.
- Every build takes the graph writer lock before reading the base generation and holds it through publish and maintenance.
- Pending events are acknowledged only after success, only through the snapshot watermark, and never for `BuildReport.unparsed` paths.
- Reader ambiguity fails closed. Post-commit maintenance failure is warning-only.
- Use TDD for every named selector and do not modify or stage `.superpowers/`.

---

### Task 1: Commit The Approved D3 Governance Surface

**Files:**
- Create: `docs/superpowers/specs/2026-07-20-atlas-live-runtime-design.md`
- Create: `knowledge/requirements/req-atlas-live-runtime.md`
- Create: `specs/task-atlas-live-runtime.spec.md`
- Create: `docs/superpowers/plans/2026-07-20-atlas-live-runtime.md`

**Interfaces:**
- Consumes: reviewed roadmap D3.1-D3.7 and D2 implementation.
- Produces: accepted requirement, 21 selectors and this implementation plan.

- [ ] **Step 1: Parse and lint the contract**

```bash
target/debug/agent-spec parse specs/task-atlas-live-runtime.spec.md
target/debug/agent-spec lint specs/task-atlas-live-runtime.spec.md --min-score 0.7
```

Expected: 21 scenarios, no parse errors, quality at least 0.7.

- [ ] **Step 2: Gate KLL and the requirement graph**

```bash
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected: no error diagnostics; the requirement maps to one active Task Contract.

- [ ] **Step 3: Commit governance artifacts**

```bash
git add docs/superpowers/specs/2026-07-20-atlas-live-runtime-design.md \
  docs/superpowers/plans/2026-07-20-atlas-live-runtime.md \
  knowledge/requirements/req-atlas-live-runtime.md specs/task-atlas-live-runtime.spec.md
git commit -m "docs(atlas): define optional live runtime contract"
```

---

### Task 2: Share Atlas Build And Watch Scope

**Files:**
- Create: `crates/rust-atlas/src/scope.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Modify: `crates/rust-atlas/src/input_plan.rs`

**Interfaces:**
- Produces `AtlasScope::discover`, `classify`, `source_files`, `watch_directories` and `ScopeEntryKind`.
- Consumers: pending ingestion and watcher tasks.

- [ ] **Step 1: Add failing `test_atlas_live_scope_is_shared_by_build_and_watcher`**

The fixture contains `.rs`, nested `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, toolchain,
ignored, target, workspace-excluded, graph-runtime and outside-root paths. Assert source walk,
input-plan inputs and watcher classification agree.

- [ ] **Step 2: Run RED**

```bash
cargo test -p rust-atlas test_atlas_live_scope_is_shared_by_build_and_watcher -- --exact
```

Expected: missing `scope` interfaces.

- [ ] **Step 3: Implement the shared interfaces**

```rust
pub(crate) enum ScopeEntryKind { RustSource, CargoInput, Ignored }
pub(crate) struct AtlasScope {
    code_root: PathBuf,
    graph_root: PathBuf,
    workspace_excludes: Vec<PathBuf>,
}
impl AtlasScope {
    pub(crate) fn discover(code: &Path, graph: &Path) -> Result<Self, AtlasError>;
    pub(crate) fn classify(&self, path: &Path) -> Result<ScopeEntryKind, AtlasError>;
    pub(crate) fn source_files(&self) -> Vec<PathBuf>;
    pub(crate) fn watch_directories(&self) -> Vec<PathBuf>;
}
```

Canonicalize roots once, reuse `ignore::WalkBuilder`, and reject symlink escape. Cargo inputs are
`Cargo.toml`, `Cargo.lock`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config` and
`.cargo/config.toml`.

- [ ] **Step 4: Route build and input-plan discovery through `AtlasScope`**

Preserve D2 ordering and content hashes; remove duplicated filters.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p rust-atlas test_atlas_live_scope_is_shared_by_build_and_watcher -- --exact
cargo test -p rust-atlas test_atlas_input_plan
cargo test -p rust-atlas test_atlas_incremental
git add crates/rust-atlas/src/scope.rs crates/rust-atlas/src/lib.rs crates/rust-atlas/src/input_plan.rs
git commit -m "refactor(atlas): share build and watch scope"
```

---

### Task 3: Persist Pending Watermarks And Retry State

**Files:**
- Create: `crates/rust-atlas/src/live.rs`
- Modify: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Produces `PendingJournal`, `PendingSnapshot`, `PendingEvent`, `RetryPolicy`, `RetryState`, `RetryClass`, `LiveRuntimeState` and `LiveRuntimeStatus`.

- [ ] **Step 1: Add failing tests**

```text
test_atlas_pending_watermark_preserves_mid_sync_events
test_atlas_failed_sync_preserves_pending_events
test_atlas_retry_budget_degrades_without_dropping_pending
test_atlas_retry_state_resets_after_successful_sync
```

Assert sequence-1 snapshot plus sequence-2 arrival retains sequence 2; four failure classes leave
pending bytes unchanged; retry delays are `100,200,400,800,1600`, attempt 6 degrades, and success
resets both counters.

- [ ] **Step 2: Run RED**

```bash
cargo test -p rust-atlas test_atlas_pending_watermark -- --nocapture
cargo test -p rust-atlas test_atlas_retry_ -- --nocapture
```

- [ ] **Step 3: Implement strict journal and retry types**

```rust
pub struct PendingEvent {
    pub path: String,
    pub first_sequence: u64,
    pub latest_sequence: u64,
    pub first_seen_ms: u64,
    pub latest_seen_ms: u64,
}
pub struct PendingSnapshot { pub watermark: u64, pub events: Vec<PendingEvent> }
pub enum RetryClass { WriterLock, Ordinary }
pub struct RetryPolicy { pub max_attempts: u32, pub base_delay_ms: u64, pub max_delay_ms: u64 }
pub enum LiveRuntimeState { Warming, Healthy, Pending, Degraded, Unavailable }
```

Use schema ids `rust-atlas/pending-events-v1` and `rust-atlas/live-status-v1`,
`deny_unknown_fields`, 100000 events and safe-path validation. `acknowledge` removes only entries
at/below the snapshot and not in the `unparsed` set. Degraded transition never clears the journal.

- [ ] **Step 4: Verify strict corruption cases and commit**

```bash
cargo test -p rust-atlas live::tests
git add crates/rust-atlas/src/live.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): persist live pending watermarks"
```

---

### Task 4: Enforce One Build Writer And Coordinate Sync

**Files:**
- Create: `crates/rust-atlas/src/locking.rs`
- Create: `crates/rust-atlas/src/sync.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Modify: `crates/rust-atlas/Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Produces `WriterLease::try_acquire`, `sync_once(SyncRequest) -> SyncReceipt` and `AtlasError::WriterBusy`.

- [ ] **Step 1: Add `fs2 = "0.4"` and failing writer tests**

Add `test_atlas_writer_lock_allows_only_one_build` and
`test_atlas_dead_writer_lock_is_recoverable`. Contention must leave staging, pointer and pending
bytes untouched; dropping a locked handle permits recovery even though the lock file remains.

- [ ] **Step 2: Run RED**

```bash
cargo test -p rust-atlas test_atlas_writer_lock -- --nocapture
cargo test -p rust-atlas test_atlas_dead_writer -- --nocapture
```

- [ ] **Step 3: Implement the writer and acquire it first in `build_with_meta`**

```rust
pub(crate) struct WriterLease { file: fs::File }
impl WriterLease {
    pub(crate) fn try_acquire(graph_root: &Path) -> Result<Self, AtlasError>;
}
```

Reject symlink runtime/lock paths. Map contention to `WriterBusy`; preserve other I/O. Acquire
before base generation/orphan reads and hold through pending acknowledgement and maintenance.

- [ ] **Step 4: Implement `sync_once`**

```rust
pub struct SyncRequest<'a> {
    pub code_root: &'a Path,
    pub graph_root: &'a Path,
    pub build_options: &'a BuildOptions,
}
pub struct SyncReceipt {
    pub snapshot_watermark: u64,
    pub pending_before: usize,
    pub pending_after: usize,
    pub build: BuildReport,
    pub runtime: LiveRuntimeStatus,
}
```

Snapshot before build; acknowledge on success through watermark excluding `unparsed`; classify
errors into writer/ordinary retry without altering pending. Daemon refresh restores committed
features/target/cfg.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p rust-atlas test_atlas_writer_
cargo test -p rust-atlas test_atlas_dead_writer_
cargo test -p rust-atlas test_atlas_failed_sync_
cargo test -p rust-atlas test_atlas_incremental
git add crates/rust-atlas/Cargo.toml Cargo.lock crates/rust-atlas/src/locking.rs \
  crates/rust-atlas/src/sync.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): serialize live graph writers"
```

---

### Task 5: Surface Pending And Degraded State Per Query

**Files:**
- Modify: `crates/rust-atlas/src/status.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Modify: `crates/rust-atlas/src/explore.rs`
- Modify: `crates/rust-atlas/src/flow.rs`
- Modify: `crates/rust-atlas/src/impact.rs`
- Modify: `crates/rust-atlas/src/affected.rs`
- Modify: `src/spec_knowledge/code_graph.rs`

**Interfaces:**
- Produces additive `AtlasStatus.live` and `scope_live_status`.

- [ ] **Step 1: Add failing `test_atlas_live_status_scopes_pending_paths_to_query`**

Pending contains `a.rs` and `b.rs`. Query, flow and impact outputs assert only represented pending
files; global status asserts both.

- [ ] **Step 2: Run RED and implement projection scoping**

```bash
cargo test -p rust-atlas test_atlas_live_status_scopes_pending_paths_to_query -- --exact
```

```rust
pub(crate) fn scope_live_status(
    status: &mut AtlasStatus,
    represented_files: impl IntoIterator<Item = String>,
);
```

Keep live state separate from freshness authority. Tree/global status use all pending. Other query
families collect returned node, edge-site, path and source files before intersection.

- [ ] **Step 3: Verify and commit**

```bash
cargo test -p rust-atlas test_atlas_live_status_
cargo test -p rust-atlas status::tests
cargo test -p rust-atlas flow::tests
cargo test -p rust-atlas impact::tests
git add crates/rust-atlas/src/status.rs crates/rust-atlas/src/lib.rs \
  crates/rust-atlas/src/explore.rs crates/rust-atlas/src/flow.rs \
  crates/rust-atlas/src/impact.rs crates/rust-atlas/src/affected.rs \
  src/spec_knowledge/code_graph.rs
git commit -m "feat(atlas): report live pending query scope"
```

---

### Task 6: Add The Bounded Filesystem Watcher

**Files:**
- Create: `crates/rust-atlas/src/watch.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Modify: `crates/rust-atlas/Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Produces `WatchPlatform`, `WatchPlan`, `WatchCoverage`, `AtlasWatcher` and inert event injection.

- [ ] **Step 1: Add `notify = "8.2"` and failing tests**

```text
test_atlas_watch_plan_is_bounded_per_platform
test_atlas_watcher_records_source_and_manifest_events
test_atlas_watch_capacity_reports_partial_or_degraded
```

- [ ] **Step 2: Run RED**

```bash
cargo test -p rust-atlas test_atlas_watch_ -- --nocapture
```

- [ ] **Step 3: Implement deterministic planning and event ingestion**

```rust
pub enum WatchPlatform { MacOs, Windows, Linux, Other }
pub enum WatchMode { Recursive, NonRecursive }
pub struct WatchTarget { pub path: PathBuf, pub mode: WatchMode }
pub struct WatchPlan { pub targets: Vec<WatchTarget>, pub coverage: WatchCoverage }
```

Mac/Windows produce one recursive root; Linux/Other select sorted directories through the cap.
Production owns `notify::RecommendedWatcher`; callback events enter a bounded channel, then shared
scope classification and persisted pending journal. New Linux directories consume remaining cap.

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p rust-atlas test_atlas_watch_
cargo check --workspace --all-targets --all-features
git add crates/rust-atlas/Cargo.toml Cargo.lock crates/rust-atlas/src/watch.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): watch bounded project inputs"
```

---

### Task 7: Implement Daemon Identity, Control And Supervision

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/rust-atlas/src/live.rs`
- Modify: `crates/rust-atlas/src/locking.rs`
- Modify: `crates/rust-atlas/src/sync.rs`
- Create: `src/atlas_daemon.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces CLI `atlas daemon start|serve|status|sync|stop` and strict localhost protocol v1.

- [ ] **Step 1: Add failing daemon tests**

```text
test_atlas_daemon_identity_rejects_wrong_root_version_and_nonce
test_atlas_daemon_single_writer_recovers_stale_registry_and_pid_reuse
test_atlas_daemon_client_disconnect_does_not_stop_server
test_atlas_daemon_reports_warming_pending_degraded_unavailable
```

Use port 0, in-process server threads and deterministic nonce/time seams. PID reuse reuses a
numeric PID with another nonce and must fail handshake.

- [ ] **Step 2: Run RED**

```bash
cargo test test_atlas_daemon_ -- --nocapture
```

- [ ] **Step 3: Implement strict identity and protocol**

```rust
struct DaemonIdentity {
    schema_id: String,
    schema_version: u32,
    worktree_root: String,
    graph_root: String,
    pid: u32,
    started_at_ms: u64,
    startup_nonce: String,
    tool_version: String,
    atlas_schema_version: u32,
    endpoint: String,
}
enum DaemonCommand { Status, Sync, Stop }
```

Limit lines to 1 MiB, apply I/O timeouts, require loopback and exact identity. Hold distinct
`.runtime/daemon.lock` for server lifetime. Registry is discovery only; handshake is authority.

- [ ] **Step 4: Implement serve/start/status/sync/stop**

`serve` owns watcher and nonblocking control loop. `start` spawns the current executable with fixed
argv and null stdio, then waits at most 5 seconds for verified handshake. `stop` is bounded. Client
disconnect only closes one connection. Stale registry is replaced only while singleton lock is free.

- [ ] **Step 5: Verify CLI grammar and commit**

```bash
cargo test test_atlas_daemon_
cargo run -- atlas daemon --help
cargo run -- atlas daemon status --help
git add Cargo.toml Cargo.lock crates/rust-atlas/src/live.rs \
  crates/rust-atlas/src/locking.rs crates/rust-atlas/src/sync.rs \
  src/atlas_daemon.rs src/main.rs
git commit -m "feat(atlas): supervise optional local daemon"
```

---

### Task 8: Add Reader Leases And Safe Reclamation

**Files:**
- Modify: `Cargo.lock`
- Modify: `crates/rust-atlas/Cargo.toml`
- Modify: `crates/rust-atlas/src/locking.rs`
- Modify: `crates/rust-atlas/src/generation.rs`
- Modify: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Produces `ReaderLease`, lease-backed `GraphSnapshot` and `safe_reclaim(&WriterLease)`.

- [x] **Step 1: Add failing tests**

```text
test_atlas_reader_lease_preserves_old_generation_until_drop
test_atlas_reclamation_retains_generations_for_ambiguous_lease
test_atlas_safe_reclamation_cleans_abandoned_staging_idempotently
```

- [x] **Step 2: Run RED**

```bash
cargo test -p rust-atlas test_atlas_reader_lease_ -- --nocapture
cargo test -p rust-atlas test_atlas_reclamation_ -- --nocapture
cargo test -p rust-atlas test_atlas_safe_reclamation_ -- --nocapture
```

- [x] **Step 3: Add process-bound reader lease**

`resolve_snapshot` creates `.runtime/readers/<pid>-<nonce>.json`, obtains a shared file lock and
keeps it in `Arc<ReaderLeaseInner>`. Snapshot clones share the lease; final drop unlocks/removes it.
Custom equality compares only data dir/generation.

- [x] **Step 4: Implement fail-closed reclamation**

Under `WriterLease`, exclusive-lockable reader files are stale and removable; a contended valid
lease protects its generation. A contended malformed/unreadable lease retains all old generations.
Always retain current. Remove abandoned `.staging/*` only under the writer lease.

- [x] **Step 5: Integrate warning-only maintenance, verify and commit**

```bash
cargo test -p rust-atlas generation::tests
cargo test -p rust-atlas test_atlas_reader_lease_
cargo test -p rust-atlas test_atlas_reclamation_
cargo test -p rust-atlas test_atlas_safe_reclamation_
git add Cargo.lock crates/rust-atlas/Cargo.toml crates/rust-atlas/src/locking.rs \
  crates/rust-atlas/src/generation.rs crates/rust-atlas/src/lib.rs \
  docs/superpowers/plans/2026-07-20-atlas-live-runtime.md
git commit -m "feat(atlas): reclaim generations with reader leases"
```

---

### Task 9: Preserve MCP Discovery And No-Daemon Parity

**Files:**
- Verify: `src/spec_mcp/tools.rs`
- Verify: `src/spec_mcp/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes additive `AtlasStatus.live`.
- Produces static discovery and local frozen queries with explicit runtime status.

- [x] **Step 1: Add failing `test_atlas_mcp_discovery_is_static_and_no_daemon_queries_match`**

Capture `tool_specs()` under unavailable/warming/pending/degraded files and assert byte identity.
Query one generation with/without a daemon registry and compare facts after excluding additive
runtime status. A test connector panics if discovery tries network access.

- [x] **Step 2: Run RED, preserve purity and verify**

```bash
cargo test test_atlas_mcp_discovery_is_static_and_no_daemon_queries_match -- --exact
```

Do not call the daemon from `tool_specs` or `atlas_tool`; existing tools remain frozen local reads.
`atlas daemon status` independently reports warming/unavailable before a graph exists.

```bash
cargo test test_mcp_atlas_
cargo test test_atlas_
git add src/spec_mcp/tools.rs src/spec_mcp/mod.rs src/main.rs
git commit -m "feat(atlas): expose live state without query coupling"
```

---

### Task 10: Add Acceptance Matrix, Docs, Wiki And Dogfood

**Files:**
- Create: `fixtures/atlas/live-runtime/**`
- Create: `docs/atlas-live-runtime.md`
- Modify: `docs/atlas-incremental-builds.md`
- Modify: `docs/atlas-evaluation.md`
- Modify: `docs/atlas-roadmap.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `.agent-spec/wiki/**`
- Modify: `CHANGELOG.md`
- Modify: `src/main.rs`

- [x] **Step 1: Add failing matrix and docs tests**

```text
test_atlas_live_runtime_acceptance_matrix_records_receipts
test_atlas_live_runtime_docs_match_cli_and_authority_boundaries
```

Matrix cases cover coalescing, mid-sync, failed sync, both retry exhaustions, transient recovery,
competing writer, stale registry, disconnect, active reader, ambiguous lease, abandoned staging and
no-daemon parity. Receipts contain watermark, pending before/after, retries, state, generation,
writer identity and reclamation.

- [x] **Step 2: Run RED, add fixtures/docs, then mark roadmap D3 delivered**

```bash
cargo test test_atlas_live_runtime_acceptance_matrix_records_receipts -- --exact
cargo test test_atlas_live_runtime_docs_match_cli_and_authority_boundaries -- --exact
```

Document exact commands/defaults/state/recovery/authority boundaries. Keep B5/E1/F1 pending.

- [x] **Step 3: Refresh wiki and run docs gates**

```bash
target/debug/agent-spec wiki meta update --code . --wiki .agent-spec/wiki
target/debug/agent-spec wiki index --wiki .agent-spec/wiki
target/debug/agent-spec wiki check --code . --wiki .agent-spec/wiki
bash scripts/docs-lint.sh
```

- [x] **Step 4: Dogfood start/status/sync/stop and no-daemon query**

```bash
target/debug/agent-spec atlas build --code . --graph .agent-spec/graph
target/debug/agent-spec atlas daemon start --code . --graph .agent-spec/graph
target/debug/agent-spec atlas daemon status --code . --graph .agent-spec/graph
target/debug/agent-spec atlas daemon sync --code . --graph .agent-spec/graph
target/debug/agent-spec atlas daemon stop --code . --graph .agent-spec/graph
target/debug/agent-spec atlas query rust_atlas::build --code . --graph .agent-spec/graph --frozen
```

- [x] **Step 5: Commit**

```bash
git add fixtures/atlas/live-runtime docs/atlas-live-runtime.md docs/atlas-incremental-builds.md \
  docs/atlas-evaluation.md docs/atlas-roadmap.md README.md AGENTS.md \
  skills/agent-spec-tool-first/SKILL.md .agent-spec/wiki CHANGELOG.md src/main.rs
git commit -m "docs(atlas): publish live runtime workflow"
```

---

### Task 11: Run Completion Gates And Independent Review

**Files:**
- Modify only files required by findings.

- [x] **Step 1: Run workspace gates**

```bash
cargo fmt --all -- --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

- [x] **Step 2: Run lifecycle and governance evidence**

```bash
target/debug/agent-spec lifecycle specs/task-atlas-live-runtime.spec.md \
  --code . --run-log-dir .agent-spec/runs --format json
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
target/debug/agent-spec requirements bind --knowledge knowledge --specs specs --code . \
  --graph .agent-spec/graph --out .agent-spec/code-bindings.json
target/debug/agent-spec requirements status REQ-ATLAS-LIVE-RUNTIME \
  --knowledge knowledge --specs specs --code . --format json
target/debug/agent-spec requirements replay REQ-ATLAS-LIVE-RUNTIME --format json
target/debug/agent-spec requirements trace-graph REQ-ATLAS-LIVE-RUNTIME --format mermaid
```

Expected: 21/21 pass and accepted/verified/honored with continuous trace.

- [x] **Step 3: Review high-risk invariants**

Lead findings by severity. Cover acknowledgement and `unparsed`, writer acquisition order,
mid-sync races, runtime limits, handshake identity, loopback binding, detached argv, disconnect,
local stale scope, active reader retention, ambiguity fail-closed, warning-only maintenance, static
discovery and no-daemon parity. Fix every blocking/important finding and rerun affected gates.

- [x] **Step 4: Commit review fixes**

```bash
git add -u
git commit -m "fix(atlas): close live runtime review gaps"
```

Expected: only unrelated `.superpowers/` remains untracked. B5/E1/F1 remain pending.

## Plan Self-Review

- D3.1: Tasks 2 and 6.
- D3.2: Tasks 3 through 5.
- D3.3: Tasks 3 and 4.
- D3.4: Tasks 4 and 7.
- D3.5: Tasks 7 and 9.
- D3.6: Task 7.
- D3.7: Task 8.
- Roadmap completion definition items 1-10: Tasks 1, 5, 9, 10 and 11.
- Every public interface consumed by a later task is defined above; no placeholder implementation step remains.
