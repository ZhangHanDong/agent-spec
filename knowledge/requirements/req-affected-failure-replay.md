---
kind: requirement
id: REQ-AFFECTED-FAILURE-REPLAY
title: "Affected Failure Explanation and Replay"
status: accepted
liveness: auto
tags: [atlas, intent-compiler, trace]
---

# Affected Failure Explanation and Replay

## Problem

Current requirement replay follows lifecycle records, but it cannot replay the complete
code-impact chain or the quality decisions used for an affected implementation run.

## Requirements

[REQ-AFFECTED-FAILURE-REPLAY-LEDGER] Trace schema v2 MUST store an intent-impact digest, complete affected paths and spans, link gaps, explicit selectors, planned worktree, observed VCS reference, and normalized quality outcomes.

[REQ-AFFECTED-FAILURE-REPLAY-COMPAT] Existing trace schema v1 records MUST remain readable and MUST be identified as lacking affected context.

[REQ-AFFECTED-FAILURE-REPLAY-QUERY] Requirement replay, failure explanation, and trace graph MUST return the latest saved requirement-to-code chain together with lifecycle and quality failures.

[REQ-AFFECTED-FAILURE-REPLAY-MERGE] Re-recording the same run MUST preserve existing bundle and quality evidence; conflicting immutable evidence MUST be rejected.

[REQ-AFFECTED-FAILURE-REPLAY-HONESTY] Replay MUST read stored deterministic evidence only; it MUST NOT rerun a provider, test command, quality tool, skill, or model.

[REQ-AFFECTED-FAILURE-REPLAY-MISSING] Missing or malformed affected trace evidence MUST produce a typed diagnostic rather than a reconstructed guess.

## Dependencies

- REQ-INTENT-AWARE-AFFECTED
- REQ-AFFECTED-EXECUTION-BUNDLE
- REQ-REQUIREMENTS-COMPILER-PLAN-DAG

## Scenarios

Scenario: Latest affected chain replays end to end
  Given stored lifecycle and affected trace v2 records
  When requirement replay runs
  Then it returns the latest requirement, work unit, scenario, selector, code path, worktree, VCS reference, and verdict chain

Scenario: Quality failure is explained with code context
  Given a stored non-pass quality outcome for an affected requirement
  When failure explanation runs
  Then it names the quality failure and the saved affected nodes and paths

Scenario: Link gaps survive replay
  Given a stored affected report containing binding and selector gaps
  When replay runs
  Then the same typed gaps are returned without inference

Scenario: Trace graph includes affected evidence
  Given a saved affected path with worktree, VCS, selector, and quality evidence
  When the trace graph is rendered
  Then the graph connects the requirement and work unit through the saved scenario and code path to authority and quality nodes

Scenario: Missing trace is not regenerated
  Given no affected trace for a requirement
  When affected replay is requested
  Then a trace-missing diagnostic is returned and no provider or model is invoked

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track C3
- prerequisite: knowledge/requirements/req-intent-aware-affected.md
- human approval: implement the latest Atlas roadmap, 2026-07-20

## Open Questions

None.
