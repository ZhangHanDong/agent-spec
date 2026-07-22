---
kind: requirement
id: REQ-INTENT-AWARE-AFFECTED
title: "Intent-Aware Affected Projection"
status: accepted
liveness: auto
tags: [atlas, intent-compiler, traceability]
---

# Intent-Aware Affected Projection

## Problem

Atlas can explain code impact, while agent-spec can explain requirements, work units,
contracts, scenarios, tests, guidance, worktrees, and VCS evidence. These facts are
currently queried separately, so an Agent cannot deterministically identify missing
links between changed code and governed intent.

## Requirements

[REQ-INTENT-AWARE-AFFECTED-PROVIDER] The projection MUST consume a provider-neutral typed impact result; Rust Atlas MUST be an adapter rather than a dependency of the intent data model.

[REQ-INTENT-AWARE-AFFECTED-CHAIN] Every bound affected node MUST project its impact path, binding fingerprint, requirement, leaf work unit, satisfying spec, scenario, explicit test selector, test obligation, planned worktree, and observed VCS reference when those facts exist.

[REQ-INTENT-AWARE-AFFECTED-GAPS] Missing or conflicting facts MUST produce sorted typed gaps and MUST NOT silently remove the affected path.

[REQ-INTENT-AWARE-AFFECTED-SELECTOR] Only a selector explicitly parsed from a Task Contract MAY be authoritative; a name or filename heuristic MUST NOT be reported as test evidence.

[REQ-INTENT-AWARE-AFFECTED-AUTHORITY] Provider unavailability, provider staleness, binding fingerprint mismatch, and impact truncation MUST remain visible in the machine-readable report.

[REQ-INTENT-AWARE-AFFECTED-DETERMINISM] Identical inputs MUST produce byte-identical JSON with stable schema, node, link, scenario, and gap ordering.

## Dependencies

- REQ-ATLAS-EXPLORE-FLOW-IMPACT
- REQ-CODE-GRAPH-IR
- REQ-REQUIREMENTS-COMPILER-PLAN-DAG

## Scenarios

Scenario: Full affected chain is projected
  Given one affected node with a current binding and an explicit contract selector
  When intent-aware affected joins compiler artifacts
  Then the report names the path, requirement, work unit, spec, scenario, selector, obligation, worktree, and VCS reference

Scenario: Unbound code remains visible
  Given an affected node absent from code bindings
  When intent-aware affected joins compiler artifacts
  Then the node remains in the report with an affected-node-unbound gap

Scenario: Missing selector is not inferred
  Given a bound scenario without an explicit Task Contract selector
  When intent-aware affected joins test obligations
  Then the report contains selector-missing and no authoritative selector

Scenario: Provider and truncation gaps survive projection
  Given unavailable, stale, fingerprint-mismatched, or truncated provider evidence
  When the report is serialized
  Then each authority condition remains a typed gap

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track C1
- prerequisite: knowledge/requirements/req-atlas-explore-flow-impact.md
- human approval: implement the latest Atlas roadmap, 2026-07-20

## Open Questions

None.
