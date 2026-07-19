---
kind: requirement
id: REQ-ATLAS-AGENT-EVALUATION
title: "Rust Atlas Agent Evaluation Baseline"
status: accepted
liveness: auto
tags: [atlas, evaluation, benchmark, agent]
---

# Rust Atlas Agent Evaluation Baseline

## Problem

Rust Atlas currently proves graph invariants and fixture behavior, but it does not
measure whether a real coding agent answers more accurately or uses fewer source
reads and tool calls. Query-surface changes therefore lack an adoption baseline
and can optimize graph output without improving agent work.

## Requirements

[REQ-ATLAS-EVAL-CORPUS] The repository MUST contain a versioned benchmark corpus manifest covering small, medium, and large Rust workspaces.

[REQ-ATLAS-EVAL-QUESTIONS] The corpus MUST cover symbol discovery, flow reconstruction, change impact, implementation, stale graph, unavailable SCIP, compilation failure, and alternate-worktree cases.

[REQ-ATLAS-EVAL-VALIDATE] A deterministic validator MUST reject malformed manifests, duplicate case ids, missing repository revisions, absent expected-answer rubrics, and unsupported size or task classes.

[REQ-ATLAS-EVAL-PLAN] The evaluator MUST compile a manifest into a machine-readable run plan that fixes model, prompt, repository revision, permissions, Atlas arm, baseline arm, and cold or warm condition.

[REQ-ATLAS-EVAL-TRIALS] Each A/B case MUST schedule at least three trials per arm.

[REQ-ATLAS-EVAL-RECEIPTS] Run receipts MUST record answer-correctness verdict, file reads, graph calls, total tool calls, wall-clock duration, context size, cost when available, and failure diagnostics.

[REQ-ATLAS-EVAL-SUMMARY] Deterministic summarization MUST report medians and dispersion per arm while keeping correctness failures visible instead of averaging them away.

[REQ-ATLAS-EVAL-OPT-IN] Real model execution MUST remain opt-in and MUST NOT run from the default test suite or require network access for deterministic validation and summarization.

[REQ-ATLAS-EVAL-NO-COPIED-THRESHOLDS] Atlas acceptance thresholds MUST be derived from its own Rust baseline and MUST NOT copy codegraph benchmark percentages.

[REQ-ATLAS-EVAL-NEGATIVE] Satisfying specs MUST cover invalid corpus data, too few trials, and incomplete receipts.

## Dependencies

- REQ-RUST-ATLAS

## Scenarios

Scenario: Valid corpus compiles into paired runs
  Given a manifest with one case in each Rust workspace size and three trials
  When the deterministic evaluator compiles the run plan
  Then every case has matched Atlas and baseline arms with fixed variables

Scenario: Duplicate case id is rejected
  Given two corpus entries share one case id
  When the manifest is validated
  Then validation fails naming the duplicate id

Scenario: Too few A/B trials is rejected
  Given a case requests two trials per arm
  When the run plan is compiled
  Then compilation fails because the minimum is three

Scenario: Incomplete receipt is rejected
  Given a run receipt omits answer correctness
  When results are summarized
  Then summarization fails and emits no aggregate result

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track E0/E1
- reference methodology: codegraph v1.3.1, commit e552dc2
- human approval: roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-agent-evaluation.spec.md
