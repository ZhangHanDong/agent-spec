---
kind: requirement
id: REQ-ATLAS-AGENT-AB-GATE
title: "Rust Atlas Real Agent A/B Gate"
status: accepted
liveness: auto
tags: [atlas, evaluation, benchmark, agent, adoption]
---

# Rust Atlas Real Agent A/B Gate

## Problem

Atlas has deterministic graph and query-quality receipts, but those receipts do not prove that a
real coding Agent becomes more correct or more efficient. The default MCP surface, B5 context
profile, and D4 worker mode need a versioned, reproducible gate that preserves failed runs and
separates each candidate's incremental value.

## Requirements

[REQ-ATLAS-AB-THREE-ARMS] The Agent experiment MUST compile every corpus case into three matched arms: built-in Read/Grep, Atlas primitives plus explore, and the same Atlas surface plus B5 context.

[REQ-ATLAS-AB-SYMMETRY] Model, prompt, repository revision, permissions, cache condition, prompt hooks, MCP configuration, user skills, tool instructions, judge version, and session-retention policy MUST be identical across matched arms except for the declared tool-surface ablation.

[REQ-ATLAS-AB-TRIALS] Every case MUST schedule at least three trials per arm and every concurrent load profile MUST schedule at least three direct and three worker burst trials.

[REQ-ATLAS-AB-CORRECTNESS] The gate MUST judge correctness before efficiency, reject candidate-arm correctness regression, and reject any run that presents stale evidence as fresh.

[REQ-ATLAS-AB-METRICS] Strict receipts MUST record file reads, grep calls, graph calls, total tool calls, round trips, wall-clock duration, response bytes, context bytes, optional cost, run outcome, judge evidence, and raw-session location and hash.

[REQ-ATLAS-AB-RETENTION] The gate MUST require exactly one receipt for every planned run, reject duplicate or unknown runs, retain timeout/cancelled/failed runs, and MUST NOT aggregate only successful samples.

[REQ-ATLAS-AB-NO-LEGACY] Formal E1 receipts MUST carry the current query-metric schema; legacy or partially populated query metrics MUST block promotion rather than become zero-valued samples.

[REQ-ATLAS-AB-BASELINE-DERIVATION] Benefit and tie-zone decisions MUST be derived from the matched baseline trial median and median absolute deviation; the implementation MUST NOT copy benchmark percentages from another project.

[REQ-ATLAS-AB-SCOPED-PROMOTION] B versus A MUST decide only the Atlas primitive/default-surface candidate, while C versus B MUST decide only the B5 context candidate, with results retained by question class and workspace size.

[REQ-ATLAS-AB-CONCURRENCY] D4 promotion MUST use a separate real-repository direct-versus-worker burst plan and receipt that proves semantic parity, one-snapshot answers, transport heartbeat, bounded queue behavior, zero stale-as-fresh results, and no lost logical query.

[REQ-ATLAS-AB-OPT-IN] Real Agent and burst execution MUST remain opt-in, require an explicit external command, stay out of default tests, and never contact a model or repository network endpoint during deterministic validation.

[REQ-ATLAS-AB-HONESTY] Until complete real receipts pass and a human accepts the result, the CLI and documentation MUST report the candidate as pending or blocked and MUST NOT change default MCP discovery, B5 profile, or worker mode.

## Dependencies

- REQ-ATLAS-AGENT-EVALUATION
- REQ-ATLAS-QUERY-QUALITY-REGRESSION
- REQ-ATLAS-QUERY-CONTEXT-COMPILER
- REQ-ATLAS-CONCURRENT-QUERY-SERVING

## Scenarios

Scenario: Three matched Agent arms compile
  Given a valid E0 corpus and a symmetric E1 experiment manifest
  When the Agent plan is compiled
  Then each case has three trials for arms A, B, and C with one declared surface difference

Scenario: Asymmetric environment is rejected
  Given one arm changes a prompt hook, skill, MCP config, or tool instruction outside the surface manifest
  When the experiment is validated
  Then validation fails before a plan is emitted

Scenario: Missing and failed runs block promotion
  Given a plan contains all required trials
  When a receipt bundle omits a run or marks a run failed
  Then the gate retains the failure and does not produce a passing promotion

Scenario: Candidate correctness regression blocks promotion
  Given a matched candidate run is incorrect or presents stale evidence as fresh
  When the Agent receipt is gated
  Then the affected comparison is blocked before efficiency metrics are considered

Scenario: Surface conclusions remain scoped
  Given B improves on A but C does not improve on B
  When the Agent receipt is gated
  Then only the Atlas primitive comparison may pass and B5 remains blocked

Scenario: Real direct and worker bursts are isolated
  Given matched burst runs from a pinned non-fixture repository
  When the concurrency receipt is gated
  Then semantic parity, snapshot identity, heartbeat, tail latency, queue outcomes, and all logical results are evaluated independently of Agent A/B/C

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track E1 and delivery item 15
- baseline requirement: knowledge/requirements/req-atlas-agent-evaluation.md
- D4 requirement: knowledge/requirements/req-atlas-concurrent-query-serving.md
- human approval: implement the latest reviewed roadmap, 2026-07-21
- contract: specs/task-atlas-agent-ab-gate.spec.md

