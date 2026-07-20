---
kind: requirement
id: REQ-ATLAS-LIVE-RUNTIME
title: "Rust Atlas Optional Watch And Daemon Runtime"
status: accepted
liveness: auto
tags: [atlas, live, watcher, daemon, incremental, generation]
---

# Rust Atlas Optional Watch And Daemon Runtime

## Problem

D2 makes one incremental build correct and recoverable, but users must still invoke it manually.
A live runtime must shorten the stale window without introducing a second graph authority, losing
events during a sync, retrying forever, attaching to the wrong daemon, or deleting a generation
that an active query still reads.

## Requirements

[REQ-ATLAS-LIVE-OPTIONAL] Watch and daemon operation MUST remain opt-in and MUST NOT be required for correct no-daemon graph queries.

[REQ-ATLAS-LIVE-QUERY-AUTHORITY] Daemon mode MUST use the same immutable generation and query APIs as no-daemon mode.

[REQ-ATLAS-LIVE-SCOPE] Build and watch MUST share one source, Cargo-input, ignore, target-directory, graph-directory, workspace-exclude, and outside-root scope policy.

[REQ-ATLAS-LIVE-RECURSIVE] macOS and Windows watch planning MUST use one recursive worktree-root watch.

[REQ-ATLAS-LIVE-LINUX] Linux watch planning MUST use canonically ordered non-recursive directory watches with a configurable hard cap.

[REQ-ATLAS-LIVE-WATCH-CAP] Exceeding the directory-watch cap or backend watch capacity MUST report partial or degraded coverage and MUST NOT claim complete auto-sync.

[REQ-ATLAS-LIVE-INPUTS] Source files, Cargo manifests, Cargo.lock, `.cargo/config.toml`, Rust toolchain files and Atlas project config changes MUST enter the pending queue.

[REQ-ATLAS-LIVE-IGNORED] Ignored, target, graph-runtime, symlink-escaped and outside-root paths MUST NOT enter the pending queue.

[REQ-ATLAS-LIVE-SEQUENCE] Every accepted event MUST receive a monotonically increasing sequence and retain normalized path plus first/latest event time.

[REQ-ATLAS-LIVE-WATERMARK] A sync MUST snapshot its pending sequence watermark before starting the D2 build.

[REQ-ATLAS-LIVE-ACK] A successful committed sync MUST acknowledge only events whose latest sequence is at or below its snapshot watermark.

[REQ-ATLAS-LIVE-UNPARSED] A path named by the committed build report's `unparsed` set MUST remain pending.

[REQ-ATLAS-LIVE-MID-SYNC] Events accepted after the sync snapshot MUST remain pending after that sync commits.

[REQ-ATLAS-LIVE-FAILURE-PENDING] Lock contention, cancellation, extractor failure, I/O failure and degraded transition MUST retain pending events.

[REQ-ATLAS-LIVE-PERSISTENCE] Pending events and retry/degraded status MUST survive daemon restart in bounded, versioned, strictly parsed runtime files.

[REQ-ATLAS-LIVE-PATH-SAFETY] Runtime state MUST reject absolute, parent-traversal, control-character, over-limit, symlink and unknown-schema inputs.

[REQ-ATLAS-LIVE-LOCAL-STALE] Query results MUST identify the intersection between their represented source files and pending paths without marking unrelated results stale.

[REQ-ATLAS-LIVE-STATUS] Runtime status MUST distinguish `warming`, `healthy`, `pending`, `degraded`, and `unavailable`.

[REQ-ATLAS-LIVE-RETRY-CLASS] Writer-lock contention and ordinary sync failure MUST have separate consecutive retry counters.

[REQ-ATLAS-LIVE-RETRY-BOUND] Retry MUST use deterministic exponential backoff with configured base delay, maximum delay and maximum attempts.

[REQ-ATLAS-LIVE-RETRY-RESET] One successful sync MUST reset both retry counters.

[REQ-ATLAS-LIVE-DEGRADED] Retry exhaustion MUST enter typed degraded state, preserve pending events, retain a failure class and last error, and stop automatic retry.

[REQ-ATLAS-LIVE-BUILD-WRITER] Every D2 build MUST hold one process-bound exclusive graph writer lock through publish and post-commit maintenance.

[REQ-ATLAS-LIVE-BUILD-CONTENTION] A contended writer lock MUST return a typed retryable error without creating staging or changing the active generation.

[REQ-ATLAS-LIVE-DAEMON-SINGLETON] Concurrent daemon startup for one canonical worktree and graph MUST produce exactly one live daemon owner.

[REQ-ATLAS-LIVE-DAEMON-IDENTITY] Daemon identity MUST bind canonical worktree root, canonical graph root, PID, start time, random startup nonce, tool version, schema version and control endpoint.

[REQ-ATLAS-LIVE-DAEMON-HANDSHAKE] A client MUST verify live handshake identity and MUST NOT attach based only on a PID or registry file.

[REQ-ATLAS-LIVE-DAEMON-RECOVERY] Dead PID records, PID reuse, stale endpoint or lock metadata, worktree deletion/recreation and malformed registry data MUST be recovered or rejected explicitly.

[REQ-ATLAS-LIVE-DAEMON-VERSION] A live daemon with incompatible tool/schema version or different canonical worktree MUST be rejected without replacing it.

[REQ-ATLAS-LIVE-DISCOVERY] MCP tool discovery and CLI help MUST remain static and MUST NOT start, connect to, or wait for a daemon or graph warm-up.

[REQ-ATLAS-LIVE-SUPERVISION] Client disconnect MUST NOT stop a daemon that may serve other clients; daemon termination MUST return a typed unavailable result to later clients.

[REQ-ATLAS-LIVE-CONFIG] Watch/daemon MUST NOT write user Agent configuration or become a condition for graph correctness.

[REQ-ATLAS-LIVE-READER-LEASE] Each graph query MUST hold a process-bound reader lease for its pinned generation until graph reads complete.

[REQ-ATLAS-LIVE-RECLAIM-WRITER] Cross-process staging and generation reclamation MUST run only while the graph writer lock is held.

[REQ-ATLAS-LIVE-RECLAIM-ACTIVE] Reclamation MUST NOT remove `CURRENT.json` generation or any generation named by an active reader lease.

[REQ-ATLAS-LIVE-RECLAIM-STALE] Reclamation MAY remove only unlocked stale reader leases, abandoned staging and unleased non-current generations.

[REQ-ATLAS-LIVE-RECLAIM-FAIL-CLOSED] Unreadable or ambiguous lease/state data MUST retain candidate generations and emit a maintenance diagnostic.

[REQ-ATLAS-LIVE-RECLAIM-MAINTENANCE] Reclamation failure MUST NOT roll back a committed generation or convert a successful build into failure.

[REQ-ATLAS-LIVE-RECLAIM-IDEMPOTENT] Repeated safe reclamation MUST be idempotent.

[REQ-ATLAS-LIVE-MCP] Existing MCP Atlas tools MUST remain frozen local graph readers and MUST surface pending/degraded live status without changing the default tool list.

[REQ-ATLAS-LIVE-RECEIPT] The acceptance fixture MUST record event watermark, pending before/after, retry counters, runtime state, generation, writer identity and reclamation outcome.

[REQ-ATLAS-LIVE-NO-REMOTE] D3 MUST NOT add a remote graph service, network listener outside loopback, LLM call, KLL mutation or default MCP tool.

[REQ-ATLAS-LIVE-NEGATIVE] Satisfying specs MUST cover mid-sync events, failed sync, lock contention exhaustion, ordinary failure exhaustion, transient recovery, watch-cap overflow, malformed runtime state, competing writers, stale registry, PID reuse, identity/version mismatch, client disconnect, active reader retention, ambiguous lease retention, idempotent cleanup, static discovery and no-daemon parity.

## Dependencies

- REQ-ATLAS-INCREMENTAL-HARDENING
- REQ-ATLAS-WORKTREE-FRESHNESS
- REQ-ATLAS-QUERY-QUALITY-REGRESSION

## State Machine

State: unavailable
  On daemon-start -> warming

State: warming
  On initial-sync-success-without-pending -> healthy
  On initial-sync-success-with-new-events -> pending
  On retry-budget-exhausted -> degraded
  On daemon-stop -> unavailable

State: healthy
  On accepted-event -> pending
  On daemon-stop -> unavailable

State: pending
  On committed-sync-clears-watermark -> healthy
  On committed-sync-leaves-newer-events -> pending
  On retry-budget-exhausted -> degraded
  On daemon-stop -> unavailable

State: degraded
  On explicit-restart -> warming
  On daemon-stop -> unavailable

## Scenarios

Scenario: Event arriving during sync remains pending
  Given one pending source event and a sync snapshot watermark
  When a newer event is accepted before the generation commit is acknowledged
  Then only the older event is cleared and the newer sequence remains pending

Scenario: Failed sync preserves the complete pending set
  Given pending source and Cargo-input events
  When D2 build returns lock, cancellation, extractor or I/O failure
  Then no pending event is acknowledged

Scenario: Retry exhaustion is visible and recoverable
  Given bounded retry configuration and persistent pending work
  When lock or ordinary failures exceed their independent attempt budget
  Then runtime state is degraded with pending work and failure class preserved

Scenario: One successful sync resets retry state
  Given transient lock and ordinary failures below their limits
  When a later D2 build commits successfully
  Then both counters reset and acknowledged pending work is removed

Scenario: Exactly one daemon owns a worktree graph
  Given concurrent startup attempts for the same canonical worktree and graph
  When both attempt the daemon singleton lock and handshake
  Then exactly one serves requests and the other reports the live owner

Scenario: Registry identity cannot impersonate a daemon
  Given stale, PID-reused, wrong-worktree, wrong-version and wrong-nonce registry records
  When a client performs discovery and handshake
  Then it recovers or rejects each record without attaching to the wrong process

Scenario: Active query keeps its old generation readable
  Given a query holds a reader lease while a newer generation commits
  When safe reclamation runs under the writer lock
  Then the old generation remains until the query lease is released

Scenario: Ambiguous reclamation fails closed
  Given a malformed or locked reader lease whose generation cannot be proven unused
  When maintenance scans old generations and abandoned staging
  Then candidate generations remain and a maintenance diagnostic is returned

Scenario: No-daemon queries preserve graph semantics
  Given identical committed graph state with and without a live daemon
  When CLI and MCP query the same symbol
  Then graph facts and paths are identical while live status remains explicit

Scenario: Tool discovery is independent of runtime warm-up
  Given no graph, a warming daemon, a pending daemon and a degraded daemon
  When MCP lists tools and CLI renders help
  Then discovery remains byte-stable and performs no daemon connection

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track D3
- approved design: docs/superpowers/specs/2026-07-20-atlas-live-runtime-design.md
- reference project: codegraph v1.3.1, commit e552dc2, `src/sync/watcher.ts`, `src/mcp/daemon-*.ts`
- human approval: latest reviewed roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-live-runtime.spec.md
