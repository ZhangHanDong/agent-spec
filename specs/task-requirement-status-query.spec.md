spec: task
name: "Requirement Three-Axis Status Query"
tags: [intent-compiler, status, governance, liveness]
satisfies: [REQ-REQUIREMENT-STATUS-QUERY]
depends: [task-requirement-governance-transitions]
estimate: 4h
---

## Intent

Implement the first slice of target-architecture delivery boundary 5:
`agent-spec requirements status <ID>` aggregates the three independent state
axes — governance (persisted KLL), execution progress (derived), requirement
liveness (recomputed) — into one report with the satisfying-spec evidence
behind the derivation, so "where is REQ-X?" stops requiring three commands
and mental math.

## Decisions

- New subcommand `requirements status <ID> [--knowledge knowledge] [--specs specs] [--archive-dir .agent-spec/archive/specs] [--code .] [--format json|text]`.
- Governance axis: the persisted frontmatter status; a document without status reports `missing` (the governance gate already fails it elsewhere).
- Execution ladder, evaluated top-down: `archived` (an archived spec satisfies the id) → `verified` (active satisfying spec and recomputed liveness is honored) → `active` (active satisfying spec) → `ready` (work unit ready) → `planned` (staged roadmap spec satisfies the id) → `unplanned`.
- Liveness axis reuses the trace builder with an injectable verify function; the CLI injects the real spec rollup and tests inject verdict stubs.
- The report carries evidence: active/staged/archived satisfying spec paths and the work-unit state; JSON is the machine format and text is a three-line human summary.
- No new dependencies; the query is read-only and never mutates knowledge or graph state.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- knowledge/requirements/req-requirement-status-query.md
- specs/task-requirement-status-query.spec.md
- skills/agent-spec-tool-first/**
- README.md
- AGENTS.md
- CHANGELOG.md

### Forbidden
- Do not add dependencies.
- Do not store any derived axis; execution and liveness are recomputed per query.
- Do not mutate knowledge documents or graph state.
- Do not add network calls.

## Out of Scope

- Batch status over the whole corpus (single-id slice first)
- Decision/guidance/proposal axes
- Delivery boundaries 2–4 (Code Graph IR, Linker, Quality Planning)

## Completion Criteria

<!-- lint-ack: output-mode-coverage — json/text rendering is asserted directly by the aggregate-report scenario -->

### Rule: three-axes — 三轴聚合

Scenario: Verified requirement reports all three axes with evidence
  Test:
    Filter: test_requirement_status_reports_verified_axes
    Level: integration
  Given an accepted requirement with an active satisfying spec whose injected verdict is pass
  When `requirement_status` runs
  Then governance is `accepted`, execution is `verified`, liveness is `honored`
  And the active spec path appears in the evidence

Scenario: Staged-only coverage reports planned
  Test:
    Filter: test_requirement_status_reports_planned_for_staged_spec
    Level: integration
  Given an accepted requirement satisfied only by a spec under `specs/roadmap/`
  When `requirement_status` runs
  Then execution is `planned` and the staged spec path appears in the evidence

Scenario: Ready work unit without any spec reports ready
  Test:
    Filter: test_requirement_status_reports_ready_without_spec
    Level: integration
  Given an accepted requirement with scenarios and no satisfying spec
  When `requirement_status` runs
  Then execution is `ready` and liveness is `unproven`

Scenario: Proposed requirement without coverage reports unplanned
  Test:
    Filter: test_requirement_status_reports_unplanned
    Level: integration
  Given a proposed requirement with no satisfying spec and no scenarios
  When `requirement_status` runs
  Then governance is `proposed` and execution is `unplanned`
  And every knowledge document is byte-identical after the query (read-only)

Scenario: Archived coverage wins the ladder
  Test:
    Filter: test_requirement_status_reports_archived
    Level: integration
  Given an archived spec satisfies the requirement id
  When `requirement_status` runs
  Then execution is `archived` and the archived path appears in the evidence

Scenario: Unknown ids are diagnostics
  Test:
    Filter: test_requirement_status_rejects_unknown_id
    Level: integration
  Given no requirement document declares the id
  When `requirement_status` runs
  Then the query fails with an error diagnostic naming the id
