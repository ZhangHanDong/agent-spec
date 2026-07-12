spec: task
name: "Requirements Intake And Work Units"
tags: [kll, requirements, work-units]
satisfies: [REQ-KLL-WORK-UNITS]
---

## Intent

Build a deterministic pipeline that imports marked PRD/issue requirement blocks into `knowledge/requirements/*.md`, validates those artifacts as a graph, generates executable work units, and drafts reviewable Task Contracts linked by `satisfies: [REQ-*]`.

## Decisions

- Use explicit `<!-- agent-spec:requirement ... -->` blocks for deterministic PRD/issue intake.
- Keep raw unmarked PRD prose outside automated import.
- Reuse KLL `kind: requirement` artifacts as the long-lived source of truth.
- Generate work units from KLL requirement docs, not directly from PRD source files.
- Generate `.spec.md` drafts only; do not mark generated drafts as verified or accepted.
- Block executable work-unit generation when `## Open Questions` contains content other than `None.`.

## Boundaries

### Allowed Changes
- src/main.rs
- src/spec_knowledge/**
- knowledge/requirements/req-kll-work-units.md
- README.md
- specs/task-requirements-intake-work-units.spec.md

### Forbidden
- Do not add network or LLM dependencies.
- Do not add serde_yaml.
- Do not copy Python code from the reference project.
- Do not change existing verification verdict semantics.

## Completion Criteria

Scenario: Import marked PRD requirement blocks into KLL requirement files
  Test: test_requirements_import_parses_block_and_renders_artifact
  Given a Markdown source containing one `agent-spec:requirement` block
  When the import parser processes the source
  Then it returns one requirement artifact with frontmatter `kind: requirement`, the declared id, title-derived filename, source trace, requirements, scenarios, dependencies, and open questions preserved

Scenario: Reject malformed requirement import blocks
  Test: test_requirements_import_rejects_missing_id
  Given a Markdown source containing an `agent-spec:requirement` block without an id
  When the import parser processes the source
  Then it returns an error explaining that `id` is required

Scenario: Build a graph from KLL requirements
  Test: test_requirement_graph_extracts_dependencies_scenarios_and_open_questions
  Given two KLL requirement docs where one depends on the other
  When the requirement graph builder reads the knowledge directory
  Then it produces two nodes, records the dependency edge, parses scenarios, and records no dangling-dependency diagnostic

Scenario: Detect invalid requirement graph edges
  Test: test_requirement_graph_reports_dangling_dependency_and_cycle
  Given KLL requirement docs with one missing dependency and one dependency cycle
  When the requirement graph validator runs
  Then it reports both a dangling dependency diagnostic and a cycle diagnostic

Scenario: Generate work units only for executable requirements
  Test: test_work_units_skip_grouping_and_block_open_questions
  Given one requirement with scenarios, one grouping requirement with child ids but no scenarios, and one requirement with open questions
  When work units are generated
  Then the scenario-backed requirement is `ready`, the grouping requirement is `grouping_only`, and the open-question requirement is `blocked_questions`

Scenario: Draft task specs link back to requirements
  Test: test_draft_specs_render_satisfies_and_bdd_scenarios
  Given a ready work unit for `REQ-101`
  When the draft-spec renderer runs
  Then the draft spec includes `satisfies: [REQ-101]`, an Intent from `Problem`, and BDD scenarios from `## Scenarios`
