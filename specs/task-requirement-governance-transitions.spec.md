spec: task
name: "Requirement Governance Gate and Transitions"
tags: [intent-compiler, governance, kll]
satisfies: [REQ-REQUIREMENT-GOVERNANCE]
depends: [task-requirements-compiler-plan-dag]
estimate: 2d
---

## Intent

Implement delivery boundary 1 of the target intent-compiler architecture
(`docs/intent-compiler/architecture.md`): requirement governance status becomes
a real gate — missing status fails instead of scheduling, Markdown intake emits
`status: proposed` candidates, and human governance actions become explicit
CLI transitions (`requirements transition`, `requirements supersede`) that
validate the state machine and rewrite frontmatter precisely. Compilation
itself never mutates governance status.

## Decisions

- `requirements graph` validation emits error diagnostic `requirement-governance-missing` for a requirement without status; `requirements graph --gate` and `plan --gate` therefore fail on missing status.
- `build_work_units` treats a missing status like a non-accepted status: the unit is `informational`, never `ready`.
- Markdown intake (`render_requirement_artifact`) emits `status: proposed` in generated frontmatter; the noteapp fixture goldens regenerate accordingly.
- New subcommand `agent-spec requirements transition <ID> --to <status>` with the fixed state machine: `proposed → accepted | rejected`; `accepted → deprecated`; missing → `proposed | accepted`; all other transitions are diagnostics. `superseded` is reachable only through the supersede command.
- The transition rewrite is line-precise: only the frontmatter `status:` line changes (inserted after `title:` when absent); every other byte of the document is preserved.
- The transition command prints `<ID>: <old> -> <new>` for auditability.
- New subcommand `agent-spec requirements supersede <OLD> --by <NEW>`: sets `status: superseded` on the old document and writes `supersedes: <OLD>` into the new document's frontmatter, satisfying the existing governance lint pair check.
- Supersession is atomic: both new contents are computed and validated before the first write, and a failed second write restores the first file.
- Both supersession ids must resolve to existing requirement documents; a replacement that is itself `superseded` or `rejected` is a diagnostic.
- Repository self-hosting: `req-kll-work-units.md` and `req-requirements-compiler-plan-dag.md` gain `status: accepted`; a test walks `knowledge/requirements/` and fails on any document without a status.
- Compiler reads stay pure: `graph`, `work-units`, and `plan` leave knowledge documents byte-identical.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- fixtures/requirements-noteapp/**
- fixtures/requirements-yaml/**
- knowledge/requirements/req-requirement-governance.md
- knowledge/requirements/req-kll-work-units.md
- knowledge/requirements/req-requirements-compiler-plan-dag.md
- specs/task-requirement-governance-transitions.spec.md
- specs/roadmap/README.md
- docs/intent-compiler/**
- skills/agent-spec-tool-first/**
- README.md
- AGENTS.md
- CHANGELOG.md

### Forbidden
- Do not add dependencies.
- Do not let `import`, `graph`, `work-units`, or `plan` mutate governance status.
- Do not change verdict semantics or any lifecycle gate.
- Do not rewrite any frontmatter line other than `status:` and `supersedes:` in the transition and supersede commands.
- Do not add network calls.

## Out of Scope

- Aggregate `requirements status <ID>` three-axis query (delivery boundary 5)
- Code Graph IR, Intent-Code Linker, Quality Planning, Execution Bundles (delivery boundaries 2–4)
- Governance audit log files beyond the printed transition line
- Decision/guidance/proposal transitions (requirements only)

## Completion Criteria

<!-- lint-ack: output-mode-coverage — file side effects are asserted directly by the line-precise rewrite, atomic supersession, and byte-identical purity scenarios -->
<!-- lint-ack: precedence-fallback-coverage — `<old> -> <new>` is an audit print format, not a precedence chain; the transition scenario asserts the printed line -->

### Rule: governance-gate — 缺失状态必须失败

Scenario: Missing status is an error diagnostic in the requirement graph
  Test:
    Filter: test_requirement_graph_reports_missing_governance_status
    Level: integration
  Given a requirement document without a frontmatter status
  When `requirements graph` validation runs
  Then diagnostics include error `requirement-governance-missing` naming the requirement

Scenario: Missing status demotes the work unit to informational
  Test:
    Filter: test_work_units_demote_missing_status_to_informational
    Level: integration
  Given a requirement node without a governance status
  When `build_work_units` runs
  Then the unit status is `informational`, not `ready`

Scenario: Markdown intake emits proposed candidates
  Test:
    Filter: test_markdown_intake_emits_proposed_status
    Level: integration
  Given a marked requirement block in a PRD
  When `render_requirement_artifact` renders it
  Then the frontmatter contains `status: proposed`

Scenario: Repository requirements all declare governance status
  Test:
    Filter: test_repo_requirements_declare_governance_status
    Level: integration
  Given every document under `knowledge/requirements/`
  When the self-hosting check parses each frontmatter
  Then every document declares a governance status

### Rule: transitions — 显式且合法的人类动作

Scenario: Accepting a proposed requirement rewrites only the status line
  Test:
    Filter: test_requirements_transition_rewrites_only_status_line
    Level: integration
  Given a requirement document with `status: proposed`
  When `requirements transition` runs with `--to accepted`
  Then the document's `status:` line reads `accepted`
  And every other byte of the document is unchanged
  And the command prints the id with old and new status

Scenario: Illegal transitions are rejected
  Test:
    Filter: test_requirements_transition_rejects_illegal_transition
    Level: integration
  Given a requirement document with `status: accepted`
  When `requirements transition` runs with `--to proposed`
  Then the exit code is non-zero with a diagnostic naming the illegal transition
  And the document is unchanged

Scenario: Unknown requirement ids are rejected
  Test:
    Filter: test_requirements_transition_rejects_unknown_id
    Level: integration
  Given no requirement document declares id `REQ-GHOST`
  When `requirements transition` runs for `REQ-GHOST`
  Then the exit code is non-zero with a diagnostic naming the unknown id

### Rule: supersession — 原子替换链

Scenario: Supersession updates both documents and satisfies governance lint
  Test:
    Filter: test_requirements_supersede_updates_both_documents
    Level: integration
  Given two requirement documents where the old one is `accepted`
  When `requirements supersede` runs with `--by` the replacement id
  Then the old document's status is `superseded`
  And the replacement's frontmatter records `supersedes` with the old id
  And corpus governance lint reports no supersession error

Scenario: A failed supersession changes neither document
  Test:
    Filter: test_requirements_supersede_rejects_unknown_target_atomically
    Level: integration
  Given the `--by` id resolves to no document
  When `requirements supersede` runs
  Then the exit code is non-zero
  And both candidate documents are byte-identical to their prior state

### Rule: compiler-immutability — 编译只读治理状态

Scenario: Compiler reads leave knowledge documents byte-identical
  Test:
    Filter: test_compiler_reads_do_not_mutate_governance_status
    Level: integration
  Given a knowledge tree with requirements in several governance states
  When `requirements graph`, `build_work_units`, and `requirements plan` run
  Then every knowledge document is byte-identical to its prior state
