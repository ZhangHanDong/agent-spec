---
kind: requirement
id: REQ-ATLAS-QUERY-QUALITY-REGRESSION
title: "Rust Atlas Query Quality Regression Loop"
status: accepted
liveness: auto
tags: [atlas, evaluation, query, regression]
---

# Rust Atlas Query Quality Regression Loop

## Problem

Atlas 的 E0 benchmark 能固定 Agent A/B 运行参数和汇总人工 correctness，但不能机器判断
ranking、traversal、dynamic boundary 或 context projection 的改动是否返回了正确 symbol、正确
path 和必要 evidence。只看“目标符号是否出现”会漏掉错误主路径、forbidden 结果和被隐藏的
stale/capability 边界。

## Requirements

[REQ-ATLAS-QUERY-CORPUS] The repository MUST contain a versioned, strictly parsed query-quality corpus with deterministic-fixture and pinned-repository tiers.

[REQ-ATLAS-QUERY-GOLDEN] Every query case MUST declare expected symbols, expected paths, forbidden symbols, forbidden paths, required evidence, ambiguity bounded to at most 64 extra items, exact diagnostic expectations, repository identity, and an answer rubric.

[REQ-ATLAS-QUERY-PINNED] A pinned-repository case MUST identify a repository outside `fixtures/`, use an immutable full Git revision, and link to a deterministic fixture case; validation and scoring MUST NOT fetch or execute that repository.

[REQ-ATLAS-QUERY-SCORE] Deterministic scoring MUST report per-case and aggregate symbol recall, mean reciprocal rank, path precision and recall, forbidden-hit rate, evidence recall, response bytes, latency, read-back calls, follow-up queries, and capability/stale diagnostics.

[REQ-ATLAS-QUERY-CORRECTNESS] A case MUST fail correctness when an expected symbol or path is absent, a forbidden item is returned, required evidence or diagnostics are absent, or unexpected symbols or paths exceed the declared ambiguity allowance.

[REQ-ATLAS-QUERY-RECEIPT] Scoring MUST emit a versioned, machine-readable receipt bound to the corpus version and MUST reject missing, duplicate, unknown, or wrong-version observations.

[REQ-ATLAS-QUERY-GATE] The score command MUST retain the corpus fingerprint and observed typed diagnostics, and MUST exit non-zero after atomically emitting a receipt when any case fails correctness.

[REQ-ATLAS-QUERY-OFFLINE] The deterministic fixture corpus and scorer MUST run in the default test suite without network access; producing fresh observations from a pinned repository or a real Agent remains explicit and opt-in.

[REQ-ATLAS-QUERY-PROMOTION] The documented regression loop MUST preserve the chain production issue to minimal fixture to paired pinned-repository case to scored regression and, when the default Agent surface changes, to Agent A/B evidence.

[REQ-ATLAS-QUERY-NO-COPIED-THRESHOLDS] The scorer MUST expose measurements and case-local correctness rules, and MUST NOT copy CodeGraph benchmark percentages as Atlas acceptance thresholds.

[REQ-ATLAS-QUERY-NEGATIVE] Satisfying specs MUST cover wrong paths, forbidden hits, missing evidence, hidden stale diagnostics, duplicate observations, missing observations, mutable pinned revisions, and fixture paths mislabeled as pinned repositories.

## Dependencies

- REQ-ATLAS-AGENT-EVALUATION
- REQ-ATLAS-EXPLORE-FLOW-IMPACT

## Scenarios

Scenario: Checked-in two-tier corpus scores offline
  Given deterministic fixture cases and a true Rust repository case pinned to a full revision
  When the checked-in observations are scored in the default test suite
  Then the receipt records all required retrieval and query-cost metrics without repository execution

Scenario: Wrong path cannot pass through symbol recall
  Given an observation returns every expected symbol but reports the wrong path
  When the scorer evaluates the case
  Then path recall is below one and correctness fails

Scenario: Stale boundary cannot be hidden
  Given a stale case requires a stale diagnostic
  When an observation omits that diagnostic
  Then correctness fails and the missing diagnostic is named

Scenario: Observation set must match the corpus
  Given results contain a duplicate case or omit a corpus case
  When the scorer validates the observation set
  Then scoring fails before emitting a receipt

Scenario: Query regression remains auditable when the gate fails
  Given a valid observation set with one incorrect answer
  When the score command writes its receipt
  Then the receipt identifies the corpus fingerprint and failed case before the command exits non-zero

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track E3
- reference methodology: codegraph v1.3.1, commit e552dc2, `__tests__/evaluation/**`
- human approval: roadmap implementation goal and CodeGraph-informed revision, 2026-07-20
- contract: specs/task-atlas-query-quality-regression.spec.md
