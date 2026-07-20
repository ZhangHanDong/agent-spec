---
kind: requirement
id: REQ-ATLAS-CONCURRENT-QUERY-SERVING
title: "Rust Atlas Concurrent Query Serving"
status: accepted
liveness: auto
tags: [atlas, daemon, query, concurrency, backpressure, snapshot, receipt]
---

# Rust Atlas Concurrent Query Serving

## Problem

The D3 daemon processes accepted control connections in one loop and Atlas queries still execute in
the caller. Routing CPU-heavy B5 retrieval and source projection through that loop without explicit
isolation would block status, stop and MCP transport, while unbounded concurrency would multiply
query-index memory and reader leases. Atlas needs an opt-in concurrent serving contract that keeps
direct local reads as the default and makes overload, cancellation, crash and snapshot behavior
machine-readable.

## Requirements

[REQ-ATLAS-QUERY-DIRECT-DEFAULT] Zero-worker direct execution MUST remain the default for CLI, MCP, CI and single-client use; D4 MUST NOT change the default MCP tool list.

[REQ-ATLAS-QUERY-OPT-IN] Worker serving MUST require explicit configuration and MUST use fixed workers; it MUST NOT create one thread per request or derive a worker count from host cores.

[REQ-ATLAS-QUERY-CONTROL-LIVENESS] Daemon status and stop plus MCP initialize, ping and discovery MUST remain serviceable while a query or graph sync is slow.

[REQ-ATLAS-QUERY-SNAPSHOT] Every admitted worker query MUST pin one `GraphSnapshot` before queueing and retain its reader lease through execution and response serialization; retry MUST NOT resolve `CURRENT.json` again.

[REQ-ATLAS-QUERY-FROZEN] Worker queries MUST be frozen read-only operations and MUST NOT trigger graph refresh, generation publication, KLL mutation or source writes.

[REQ-ATLAS-QUERY-BOUNDS] Worker count, queue capacity, queue timeout, execution deadline, estimated memory budget and retry delay MUST be validated explicit configuration and visible in service status and receipts.

[REQ-ATLAS-QUERY-QUEUE] Admission MUST use a bounded queue. A full queue or exhausted memory reservation MUST return typed `atlas-query-busy` with `retry_after_ms`; it MUST NOT block admission or return a successful empty query shape.

[REQ-ATLAS-QUERY-DEADLINE] A job that expires in the queue MUST NOT start. An executing job MUST observe cooperative cancellation or deadline checks at bounded retrieval and projection checkpoints and return typed `cancelled` or `timeout`.

[REQ-ATLAS-QUERY-CRASH] An idempotent read MAY retry one worker panic exactly once against the same pinned snapshot. A repeated panic MUST open a circuit, reject new worker jobs as typed `degraded`, and remain open until daemon restart.

[REQ-ATLAS-QUERY-FALLBACK] Direct fallback MUST be explicit. It MUST discard any partial worker result, pin a complete direct query independently, and record worker outcome plus worker and fallback generation identities; it MUST NOT merge evidence across generations.

[REQ-ATLAS-QUERY-OUTCOME] Service outcomes MUST distinguish `success`, `busy`, `timeout`, `cancelled`, `degraded`, `failed` and `unavailable`; non-success outcomes MUST NOT carry a success-shaped context result.

[REQ-ATLAS-QUERY-RECEIPT] Every worker response MUST record request id, serving mode, configured limits, admission reservation, queue and execution observations, attempt count, outcome, retry delay, selected generation, graph fingerprint, B5 load profile, response digest and fallback state.

[REQ-ATLAS-QUERY-EVAL] D4 MUST add a versioned direct-versus-worker concurrency receipt and deterministic fixture matrix. Semantic parity, snapshot identity, queue bounds, typed outcomes and lease cleanup are correctness gates; throughput, latency, heartbeat lag, CPU and RSS are measurements only.

[REQ-ATLAS-QUERY-PROMOTION] Worker mode MUST remain opt-in until E1 records versioned direct-versus-worker burst trials with no correctness regression, no stale-as-fresh result, all failed runs retained and an accepted baseline-derived benefit.

[REQ-ATLAS-QUERY-NEGATIVE] Satisfying specs MUST cover full queue, memory rejection, queue expiry, executing cancellation, repeated worker panic, circuit-open rejection, writer publish during a query, daemon stop, unavailable daemon, explicit fallback, response oversize and concurrent worktree isolation.

## Dependencies

- REQ-ATLAS-LIVE-RUNTIME
- REQ-ATLAS-QUERY-CONTEXT-COMPILER
- REQ-ATLAS-QUERY-QUALITY-REGRESSION

## State Machine

State: direct
  On opt-in-valid-config -> warming

State: warming
  On fixed-workers-ready -> ready
  On worker-start-failure -> degraded
  On daemon-stop -> unavailable

State: ready
  On admitted-query -> queued
  On queue-or-memory-full -> busy
  On repeated-worker-panic -> degraded
  On daemon-stop -> unavailable

State: queued
  On worker-start-before-queue-deadline -> running
  On cancel -> cancelled
  On queue-deadline -> timeout
  On daemon-stop -> unavailable

State: running
  On complete -> success
  On cancel-checkpoint -> cancelled
  On execution-deadline-checkpoint -> timeout
  On first-worker-panic -> retrying
  On daemon-stop -> cancelled

State: retrying
  On retry-complete -> success
  On repeated-worker-panic -> degraded
  On deadline -> timeout

State: degraded
  On request -> degraded
  On daemon-restart-with-valid-config -> warming
  On daemon-stop -> unavailable

## Scenarios

Scenario: A burst cannot starve control traffic
  Given two workers, a four-entry queue and slow context queries
  When status, stop and MCP ping arrive during the burst
  Then control responses remain independent of worker completion and excess queries receive typed busy outcomes

Scenario: A writer cannot change an admitted query snapshot
  Given a queued query has pinned generation A
  When a writer publishes generation B before the query completes
  Then the result and receipt contain only generation A facts and reclamation retains A until the response releases its lease

Scenario: Cancellation prevents stale work from masquerading as success
  Given queued and executing queries with cancellation tokens
  When cancellation or deadline fires
  Then queued work never starts, executing work stops at a bounded checkpoint and neither response contains a context result

Scenario: Repeated worker failure opens a visible circuit
  Given one idempotent query panics in worker execution
  When its single retry panics again
  Then the service becomes degraded, rejects later worker requests and reports an explicit direct-fallback option

Scenario: Direct fallback is generation-honest
  Given worker service is busy or degraded and explicit fallback is enabled
  When the caller executes a direct query
  Then the complete direct result has one newly pinned generation and the receipt records both identities without merging evidence

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track D4
- predecessor runtime: knowledge/requirements/req-atlas-live-runtime.md
- predecessor context compiler: knowledge/requirements/req-atlas-query-context-compiler.md
- reference method: /Users/zhangalex/Work/Projects/consult/codegraph/src/mcp/query-pool.ts
- human approval: latest reviewed roadmap implementation goal, 2026-07-21
- contract: specs/task-atlas-concurrent-query-serving.spec.md
