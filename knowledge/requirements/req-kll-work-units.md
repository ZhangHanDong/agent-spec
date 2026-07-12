---
kind: requirement
id: REQ-KLL-WORK-UNITS
title: "Requirements Intake And Work Units"
status: accepted
liveness: auto
tags: [kll, requirements, work-units]
---

## Problem

agent-spec can verify Task Contracts and trace specs to KLL decisions or requirements, but it does not yet provide a deterministic path from PRD or issue material into long-lived requirement artifacts, executable work units, and reviewable Task Contract drafts.

## Requirements

[REQ-KLL-WORK-UNITS] agent-spec MUST import explicitly marked PRD or issue requirement blocks into `knowledge/requirements/*.md` artifacts.
[REQ-KLL-WORK-UNITS-GRAPH] agent-spec MUST validate KLL requirement artifacts as a dependency graph before generating executable work units.
[REQ-KLL-WORK-UNITS-DRAFTS] agent-spec MUST generate reviewable Task Contract drafts with `satisfies: [REQ-*]` links for ready work units.

## Scenarios

Scenario: Import a marked requirement block
  Given a PRD or issue file contains an explicit `agent-spec:requirement` block
  When the requirements import command runs
  Then agent-spec writes a KLL requirement artifact preserving the id, title, requirements, scenarios, dependencies, source trace, and open questions

Scenario: Generate a work unit from a ready requirement
  Given a KLL requirement has scenarios and no blocking open questions
  When the work-unit command runs
  Then agent-spec emits a ready work unit linked to the source requirement id

Scenario: Draft a Task Contract from a ready work unit
  Given a ready work unit exists for a KLL requirement
  When the draft-spec command runs
  Then agent-spec writes a reviewable `.spec.md` draft with `satisfies: [REQ-*]`

## Dependencies

None.

## Source Trace

- product decision: drawer_agent_spec_default_91c5ee38
- reference project: acknowledged once in README (Intent Compiler Workflow section)

## Open Questions

None.
