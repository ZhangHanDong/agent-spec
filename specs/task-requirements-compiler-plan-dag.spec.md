spec: task
name: "Intent Compiler Plan DAG"
tags: [kll, requirements, compiler, planning]
satisfies: [REQ-REQUIREMENTS-COMPILER-PLAN-DAG]
depends: [task-requirements-intake-work-units]
risk: C
---

## Intent

Extend the current KLL requirements intake pipeline into a compiler-style plan DAG. The new workflow keeps CLI behavior deterministic while exposing requirement readiness, dependency batches, spec coverage, worktree execution entries, and clarification questions that an AI reverse-interview skill can use before implementation.

## Decisions

- Add `src/spec_knowledge/requirement_plan.rs` for cross-layer plan IR.
- Represent work units as first-class plan nodes with stable `WU-*` ids and explicit `work_unit` edges from source requirements.
- Add `src/spec_knowledge/questions.rs` for deterministic clarification question generation.
- Add `src/spec_knowledge/worktrees.rs` for deterministic work unit to git worktree execution manifests.
- Add `src/spec_knowledge/trace_ledger.rs` for requirement-level trace records, replay, failure explanation, and visual graph data.
- Add `src/spec_knowledge/test_obligations.rs` for spec-derived test obligations that are independent of implementation code.
- Add `src/spec_qa.rs` for DORA-inspired QA class A/B/C gate requirements.
- Add `agent-spec requirements plan` for machine-readable requirement/work-unit/spec planning.
- Add `agent-spec requirements questions` for reverse-interview inputs.
- Add `agent-spec requirements worktrees` for parallel implementation planning without mutating git state.
- Add `agent-spec requirements trace`, `requirements replay`, `requirements explain-failure`, and `requirements trace-graph` for requirement-level debugging and audit.
- Add `agent-spec requirements test-obligations` for Stream-D style spec-to-test-obligation output.
- Add versioned JSON Schema files and checked-in fixture golden outputs for the intent compiler's machine-readable artifacts.
- Keep AI generation outside CLI code and document it as a skill workflow.
- Define natural-language PRD intake in the intent compiler skill as a Candidate Requirement Block workflow, not as nondeterministic CLI parsing.
- Treat generated draft specs with `pending_...` selectors as review artifacts, not passing contracts.
- Treat the noteapp fixture as demonstration documentation, not as replacement for this repository's self-hosting dogfood gate.
- Add Lore-style documentation engineering standards under `knowledge/standards/` and a docs lint script that can run Harper, built-in Chinese docs lint, markdownlint, and lychee.
- Use the reference project's tests/templates (acknowledged once in the README) as a final validation checklist: requirement tree shape, dependency edges, executable scenarios, negative cases, test-first obligations, and requirement-to-evidence traceability must be covered by agent-spec tests or called out as non-goals.
- Require completed spec archival to preserve contract evidence and latest passing lifecycle evidence before moving specs out of active scans.
- Treat KLL and wiki roots as compiler trust boundaries: reject unsafe ids, malformed frontmatter, kind mismatches, missing roots, and symlink traversal.
- Include first-class spec nodes and `satisfies`/`spec_depends` edges in the plan IR; do not infer coverage from filenames.
- Derive requirement-to-scenario trace ownership from KLL scenario names and reject absent or ambiguous mappings instead of emitting a Cartesian product.
- Bind archive evidence to the canonical current spec path and content fingerprint, and preflight all moves before mutation.
- Enforce an explicitly declared QA class during lifecycle; `--adversarial` is an explicit operator assertion for the adversarial-review evidence layer, not an automatic multi-agent implementation.
- Count executed Cargo tests and classify a zero-match successful invocation as `skip`, preserving `skip != pass`.

## Boundaries

### Allowed Changes
- src/**
- ./Cargo.toml
- ./Cargo.lock
- src/spec_qa.rs
- src/main.rs
- .agent-spec/**
- scripts/docs-lint.sh
- ./README.md
- ./AGENTS.md
- ./CHANGELOG.md
- docs/intent-compiler/**
- docs/superpowers/plans/**
- skills/agent-spec-tool-first/**
- skills/agent-spec-intent-compiler/**
- .claude/skills/agent-spec-tool-first/**
- knowledge/requirements/**
- knowledge/standards/**
- knowledge/proposals/proposal-template.md
- specs/task-requirements-compiler-plan-dag.spec.md
- specs/task-requirements-intake-work-units.spec.md
- fixtures/requirements-noteapp/**

### Forbidden
- Do not add network or LLM calls to CLI code.
- Do not add serde_yaml.
- Do not remove existing requirements import, graph, work-units, or draft-specs commands.
- Do not weaken existing lifecycle, guard, trace, or lint-knowledge gates.

## Completion Criteria

Scenario: Requirement plan JSON includes DAG and coverage
  Test: test_requirements_plan_json_includes_batches_edges_and_coverage
  Given two KLL requirements where REQ-NOTE-LIST depends on REQ-NOTE-CREATE
  And specs satisfying both requirements
  When `cmd_requirements_plan` renders JSON
  Then the JSON contains requirement nodes, work-unit nodes, dependency edges, work-unit edges, execution batches, and spec coverage for both requirements

Scenario: Intent Compiler artifact contracts are versioned and stable
  Test: test_requirements_compiler_schema_files_and_fixture_golden_outputs_are_stable
  Given versioned JSON Schema files under docs/intent-compiler/schemas
  And checked-in noteapp fixture golden outputs under fixtures/requirements-noteapp/.agent-spec
  When repository tests regenerate the requirements plan, test obligations, worktree manifest, and clarification questions from the noteapp fixture
  Then the regenerated JSON exactly matches the golden outputs and every schema has a stable v1 identifier

Scenario: Requirement plan gate fails on hard diagnostics
  Test: test_requirements_plan_gate_fails_on_dangling_dependency
  Given a KLL requirement that depends on REQ-MISSING
  When `cmd_requirements_plan` runs with gate enabled
  Then it returns an error containing "requirements plan gate failed"

Scenario: Clarification questions JSON is deterministic
  Test: test_requirements_questions_json_reports_open_question
  Given a KLL requirement with an open question
  When `cmd_requirements_questions` renders JSON
  Then the output contains a blocking question tied to that requirement id and diagnostic code

Scenario: PRD intake and reverse interview skill contract is explicit
  Test: test_requirements_compiler_skill_defines_prd_intake_and_reverse_interview_contract
  Given the agent-spec intent compiler skill
  When documentation tests inspect the skill
  Then the skill defines Candidate Requirement Blocks, source excerpts, confidence, Open Questions, Reverse Interview Loop, Answer Integration, human-confirmed answers, and the rule that model inference is not accepted truth

Scenario: Worktree manifest includes ready work units only
  Test: test_requirements_worktrees_json_maps_ready_units_only
  Given a requirement plan with one ready work unit and one blocked work unit
  When `cmd_requirements_worktrees` renders JSON
  Then the output contains a branch, path, batch number, spec path, and requirement id for the ready work unit only

Scenario: Requirement trace ledger records lifecycle evidence
  Test: test_requirement_trace_ledger_records_req_to_scenario_test_code_and_vcs
  Given a requirement plan, a worktree manifest, and a lifecycle report for a spec satisfying REQ-NOTE-CREATE
  When `record_requirement_trace_run` writes trace records
  Then the trace record contains requirement id, work-unit id, spec path, scenario name, test selector, code targets, verdict, worktree path, branch, VCS reference, and run id

Scenario: Lifecycle requirement trace writes to the project trace directory
  Test: test_lifecycle_requirement_trace_writes_to_code_root_trace_dir
  Given lifecycle runs with a custom run log directory
  When requirement trace evidence is written for a satisfying spec
  Then the trace ledger appears under the project `.agent-spec/trace` directory rather than under the run log directory

Scenario: Requirement replay shows latest evidence chain
  Test: test_requirements_replay_uses_latest_trace_record_for_requirement
  Given two stored trace records for REQ-NOTE-CREATE
  When `cmd_requirements_replay` renders text
  Then the output uses the newest run and does not claim deterministic LLM replay

Scenario: Requirement explain-failure walks non-pass scenario to code target
  Test: test_requirements_explain_failure_reports_non_pass_chain
  Given a stored trace record with verdict `fail`
  When `cmd_requirements_explain_failure` renders JSON
  Then it contains requirement id, work-unit id, spec path, scenario name, test selector, code targets, worktree path, branch, VCS reference, and failure evidence

Scenario: Requirement explain-failure text answers the full trace chain
  Test: test_failure_text_reports_full_requirement_trace_chain
  Given a stored trace record with verdict `fail`
  When `format_requirement_failure_text` renders text
  Then it names the requirement, work unit, spec, scenario, test selector, code targets, worktree path, branch, VCS reference, and verdict

Scenario: Completed spec archive requires passing lifecycle evidence
  Test: test_archive_plan_uses_latest_passing_run_log_as_archive_evidence
  Given a completed task spec with multiple lifecycle run logs
  When `build_archive_plan_with_history` builds the archive plan
  Then only the latest passing lifecycle evidence makes the spec archiveable and the archive summary preserves source path, satisfies ids, scenarios, test selectors, and verification summary

Scenario: Completed spec archive blocks stale non-passing evidence
  Test: test_archive_plan_blocks_completed_specs_without_passing_lifecycle_evidence
  Given a completed task spec whose latest lifecycle run log is failing or missing
  When `build_archive_plan_with_history` builds the archive plan
  Then the spec is not selected for archive and the plan reports an archive diagnostic

Scenario: Requirement trace graph emits Mermaid
  Test: test_requirements_trace_graph_mermaid_contains_evidence_nodes
  Given a stored trace record for REQ-NOTE-CREATE
  When `cmd_requirements_trace_graph` renders Mermaid
  Then the graph contains nodes for requirement, work unit, spec, scenario, test, code target, worktree, and VCS reference

Scenario: Test obligations JSON is independent of code
  Test: test_requirements_test_obligations_json_contains_spec_derived_obligations
  Given a KLL requirement with a scenario and a satisfying task spec
  When `cmd_requirements_test_obligations` renders JSON
  Then the output contains requirement id, scenario name, suggested selector, and verification strength without reading source code files

Scenario: QA class A requires full evidence
  Test: test_qa_class_a_requires_lifecycle_trace_targeted_tests_and_adversarial_review
  Given a task spec with frontmatter `risk: A`
  When the QA class gate evaluates required evidence
  Then it requires lifecycle, trace, targeted tests, and adversarial review evidence

Scenario: State-machine transition without scenario is linted
  Test: test_lint_requirement_warns_on_uncovered_state_machine_transition
  Given a requirement with a `## State Machine` transition and no matching scenario
  When `lint_requirement` runs
  Then it reports `requirement-state-machine-transition-uncovered`

Scenario: Requirement lint catches missing scenario coverage
  Test: test_lint_requirement_warns_when_must_clause_has_no_scenario
  Given a requirement with a MUST clause and no scenarios
  When `lint_requirement` runs
  Then it reports `requirement-must-needs-scenario`

Scenario: Requirement lint catches weak observable outcome
  Test: test_lint_requirement_warns_on_weak_then_step
  Given a requirement scenario whose Then step says "the feature works"
  When `lint_requirement` runs
  Then it reports `requirement-weak-then`

Scenario: Requirement quality diagnostics feed clarification questions
  Test: test_collect_clarification_lint_diagnostics_surfaces_quality_convergence_rules
  Given a requirement with no source trace and a compound requirement clause
  When `collect_clarification_lint_diagnostics` runs
  Then the collected diagnostics include `requirement-source-trace-required` and `requirement-compound-clause`

Scenario: Documentation describes compiler workflow
  Test: test_docs_describe_requirements_compiler_plan_and_questions
  Given README, AGENTS, and tool-first skills
  When documentation tests inspect their content
  Then they mention `requirements plan`, `requirements test-obligations`, `requirements worktrees`, `requirements trace`, `requirements replay`, `requirements explain-failure`, `requirements trace-graph`, `requirements questions`, QA class gates, state-machine coverage, deterministic CLI, dogfood, and AI reverse interview as a skill workflow

Scenario: Documentation engineering standards are governed
  Test: test_docs_engineering_standards_include_lore_practices
  Given knowledge standards and proposal templates
  When documentation tests inspect their content
  Then they mention Tutorial, How-To, Reference, Explanation, Internals, ADR, Code-Standard, Landing, canon, operational checklist, tools, Harper, Chinese docs lint, markdownlint, lychee, pre-publish, compatibility, migration, security, privacy, risks, alternatives, and unresolved questions

Scenario: Self-hosting dogfood remains the primary acceptance gate
  Test: test_requirements_compiler_plan_dag_self_hosting_contract_is_traced
  Given the task spec satisfies REQ-REQUIREMENTS-COMPILER-PLAN-DAG
  When lifecycle writes requirement trace evidence for this repository
  Then replay and trace-graph can locate the requirement, work unit, spec scenario, test selector, code target, worktree or branch, and VCS reference for the agent-spec implementation

Scenario: Reference validation matrix covers borrowed invariants
  Test: test_reference_validation_matrix_covers_borrowed_invariants
  Given the reference project's ticketbooking demo requirements, traceability service, and test-generation templates are used as reference material
  When final verification compares agent-spec tests and fixture checks against the reference tree, dependency, scenario, test-first, negative-case, and traceability patterns
  Then each borrowed reference invariant is mapped to an agent-spec test, fixture assertion, or explicit non-goal without importing reference-project runtime tests

Scenario: Unsafe KLL ids and malformed frontmatter are rejected
  Test: test_knowledge_parser_rejects_unsafe_ids_and_malformed_frontmatter
  Given a knowledge artifact has a path-like id, duplicate key, malformed line, or delayed frontmatter
  When `parse_knowledge_str` parses it
  Then parsing fails before the id can become an output path

Scenario: Missing roots and kind mismatches block compiler gates
  Test: test_collect_knowledge_checked_rejects_missing_root_and_kind_mismatch
  Given a missing knowledge root or a requirement artifact under the decisions directory
  When checked knowledge collection runs
  Then it returns parse errors instead of an empty successful corpus

Scenario: Requirement plan includes spec nodes and cross-layer edges
  Test: test_requirement_plan_builds_batches_edges_and_coverage
  Given dependent requirements and dependent satisfying specs
  When `build_requirement_plan` runs
  Then it emits first-class spec nodes, work-unit-to-spec satisfies edges, and spec-to-spec dependency edges

Scenario: Requirement trace avoids multi-requirement Cartesian products
  Test: test_requirement_trace_avoids_multi_requirement_scenario_cartesian_product
  Given a spec satisfies two requirements with distinct KLL scenarios
  When `record_requirement_trace_run` records lifecycle results
  Then each requirement receives only the scenario declared by that requirement

Scenario: Ambiguous multi-requirement trace ownership is rejected
  Test: test_requirement_trace_rejects_ambiguous_scenario_ownership
  Given two satisfied requirements declare the same scenario name
  When `record_requirement_trace_run` assigns lifecycle results
  Then it emits Error-level ambiguity diagnostics and no false trace records

Scenario: Requirement replay returns the complete latest run
  Test: test_requirement_replay_returns_all_scenarios_from_latest_run
  Given the latest requirement trace run contains multiple scenarios
  When requirement replay is queried
  Then every scenario from that latest run is returned in deterministic order

Scenario: Archive rejects stale identity evidence
  Test: test_archive_plan_rejects_name_only_or_stale_lifecycle_evidence
  Given a completed spec has passing evidence matched only by display name or old content
  When `build_archive_plan_with_history` runs
  Then it reports `archive-lifecycle-stale` and does not select the spec

Scenario: Archive preflights every target before moving
  Test: test_apply_archive_plan_preflights_all_targets_before_moving_sources
  Given one target in a multi-entry archive plan already exists
  When `apply_archive_plan` runs
  Then it fails before moving any source file

Scenario: Lifecycle enforces declared QA classes
  Test: test_lifecycle_qa_gate_rejects_missing_class_a_evidence_and_invalid_risk
  Given lifecycle evaluates a class A spec without all required evidence or an invalid risk value
  When the QA evidence gate runs
  Then it reports the missing evidence or rejects the invalid value

Scenario: Zero-match Cargo filters are skipped rather than passed
  Test: test_cargo_test_executed_count_distinguishes_zero_match_from_mixed_targets
  Given Cargo output reports zero executed tests for a selector
  When the test verifier counts executed test targets
  Then the selector is distinguished from output where at least one test ran
