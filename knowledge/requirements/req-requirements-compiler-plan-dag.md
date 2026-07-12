---
kind: requirement
id: REQ-REQUIREMENTS-COMPILER-PLAN-DAG
title: "Intent Compiler Plan DAG"
status: accepted
liveness: auto
tags: [kll, requirements, compiler, planning]
---

## Problem

agent-spec can import explicit PRD requirement blocks into KLL artifacts and draft task specs, but it does not yet expose a full compiler-style plan DAG from requirements to work units to satisfying specs. Agents need a deterministic plan surface that identifies ready work, blocked requirements, missing coverage, and clarification questions before implementation starts.

## Requirements

[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-IR] The intent compiler MUST produce a deterministic requirement plan IR from KLL requirements and specs.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-WORK-UNIT-NODES] The intent compiler MUST expose work units as first-class plan DAG nodes with stable `WU-*` ids and explicit edges from source requirements.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-SCHEMA] The intent compiler MUST publish versioned JSON schemas and stable fixture golden outputs for its machine-readable compiler artifacts.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-GATE] The intent compiler MUST gate parse errors, graph errors, dangling spec coverage, dependency cycles, and executable requirements with unresolved blocking questions.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-TEST-OBLIGATIONS] The intent compiler MUST emit test obligations derived from requirements and specs independently of implementation code.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-QA-CLASS] The intent compiler MUST support QA class gates that map risk class A/B/C to required verification evidence.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-STATE-MACHINE] The knowledge linter MUST detect state-machine transitions that lack scenario or test-obligation coverage.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-WORKTREES] The intent compiler MUST map ready work units to deterministic git worktree execution entries for parallel implementation.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-TRACE-LEDGER] The intent compiler MUST persist and query requirement-level trace records from requirement id to work unit, spec, scenario, test selector, code targets, lifecycle verdict, worktree, and VCS reference.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-REPLAY] The intent compiler MUST replay the latest known evidence chain for a requirement without treating replay as deterministic LLM execution replay.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-FAILURE-EXPLAIN] The intent compiler MUST explain non-pass requirement states by walking from lifecycle scenario verdicts back to requirement ids, work units, tests, code targets, worktrees, and VCS references.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-QUESTIONS] The intent compiler MUST convert blocking ambiguity diagnostics into machine-readable clarification questions.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-PRD-INTAKE-SKILL] The intent compiler skill MUST define natural-language PRD intake as a human-confirmed Candidate Requirement Block workflow with source excerpts, confidence, scenarios, and open questions.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-LINT] The knowledge linter MUST detect missing scenario coverage, weak observable outcomes, missing source trace, compound clauses, and unmeasured non-functional requirements.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-DOCS] The tool documentation and skills MUST explain the deterministic compiler workflow and keep AI assistance outside the CLI core.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-DOC-ENGINEERING] agent-spec documentation governance MUST absorb Lore-style doc types, canon, operational checklists, tool guidance, documentation linting, and proposal template rigor.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-DOGFOOD] agent-spec development MUST dogfood the intent compiler on this repository's own KLL requirement and task spec before treating external examples as sufficient validation.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-REFERENCE-VALIDATION] Final validation MUST compare agent-spec's implementation against reference-project invariants from the ticketbooking requirement tree, traceability service, and test-generation templates without importing reference-project runtime tests.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-ARCHIVE] Completed spec archival MUST require latest passing lifecycle evidence and preserve the archived contract's source path, satisfying requirements, scenarios, test selectors, and verification summary.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-STRICT-INPUT] The intent compiler MUST reject unsafe ids, malformed or duplicate frontmatter keys, missing compiler roots, directory-kind mismatches, and symlinked knowledge inputs.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-SPEC-NODES] The plan DAG MUST include first-class spec nodes plus work-unit-to-spec `satisfies` edges and spec-to-spec `spec_depends` edges, and MUST reject dangling or ambiguous coverage.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-EXACT-TRACE] Requirement trace recording MUST assign all spec scenarios to a sole satisfied requirement, use KLL scenario names to disambiguate multi-requirement specs, reject ambiguous mappings, and replay every scenario from the latest run.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-BOUND-ARCHIVE] Archive evidence MUST match the current spec path and content fingerprint, and archive application MUST preflight every source and target before moving any file.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-QA-LIFECYCLE] Lifecycle MUST enforce declared QA classes A/B/C and fail when the required lifecycle, trace, targeted-test, or explicit adversarial-review evidence is absent.
[REQ-REQUIREMENTS-COMPILER-PLAN-DAG-ZERO-TEST] Lifecycle MUST classify a successful Cargo invocation that executes zero matching tests as `skip`, never `pass`.

## Scenarios

Scenario: Plan DAG reports batches and spec coverage
  Given KLL requirements with dependencies and specs that satisfy them
  When the operator runs `agent-spec requirements plan --format json`
  Then the output contains requirement nodes, work-unit nodes, spec coverage, dependency edges, and deterministic execution batches

Scenario: Compiler artifact schemas and golden outputs are stable
  Given the noteapp fixture and versioned intent compiler schemas
  When the repository test suite regenerates plan, test-obligation, worktree, and question outputs from the fixture
  Then the regenerated JSON exactly matches the checked-in golden outputs and every schema has a stable v1 identifier

Scenario: Plan gate blocks unresolved compiler errors
  Given a KLL requirement that depends on a missing requirement or has a dependency cycle
  When the operator runs `agent-spec requirements plan --gate`
  Then the command exits non-zero and reports Error-level diagnostics

Scenario: Clarification questions surface blocking ambiguity
  Given a KLL requirement with a real open question or weak observable scenario
  When the operator runs `agent-spec requirements questions --format json`
  Then the output contains a question with the requirement id, source diagnostic code, prompt text, and blocking flag

Scenario: PRD intake and reverse interview skill contract is governed
  Given the agent-spec intent compiler skill
  When documentation tests inspect the skill
  Then the skill defines Candidate Requirement Blocks, source excerpts, confidence, Open Questions, Reverse Interview Loop, Answer Integration, and human-confirmed answers

Scenario: Worktree manifest maps ready work units only
  Given a requirement plan with one ready work unit and one blocked work unit
  When the operator runs `agent-spec requirements worktrees --format json`
  Then the output contains a branch and worktree path for the ready work unit only

Scenario: Requirement trace ledger locates failing evidence chain
  Given a lifecycle report with a failing scenario for a spec that satisfies REQ-NOTE-CREATE
  And a worktree manifest entry for WU-REQ-NOTE-CREATE
  When the operator runs `agent-spec requirements explain-failure REQ-NOTE-CREATE --format json`
  Then the output contains the requirement id, work-unit id, spec path, scenario name, test selector, code targets, worktree path, branch, VCS reference, and non-pass verdict

Scenario: Failure explanation text answers the full trace chain
  Given a stored non-pass requirement trace record
  When the operator runs `agent-spec requirements explain-failure REQ-NOTE-CREATE --format text`
  Then the text output names the requirement, work unit, spec, scenario, test selector, code targets, worktree, branch, VCS reference, and verdict

Scenario: Completed spec archive requires passing lifecycle evidence
  Given a completed task spec with lifecycle run logs
  When the operator runs `agent-spec archive --run-log-dir . --dry-run`
  Then the archive summary includes source path, satisfies ids, scenarios, test selectors, and latest verification summary only for specs whose latest lifecycle evidence is passing

Scenario: Lifecycle auto trace writes to the project trace directory
  Given lifecycle runs with a custom run log directory
  When requirement trace evidence is written for a satisfying spec
  Then the trace ledger appears under the project `.agent-spec/trace` directory so default replay commands can find it

Scenario: Requirement replay reconstructs latest known evidence
  Given stored requirement trace records for REQ-NOTE-CREATE across multiple lifecycle runs
  When the operator runs `agent-spec requirements replay REQ-NOTE-CREATE --format text`
  Then the output shows the latest evidence chain and marks missing code targets as unknown instead of inferring them

Scenario: Requirement trace graph is visualizable
  Given a requirement trace ledger with a requirement, work unit, spec, scenario, test selector, code target, worktree, and VCS reference
  When the operator runs `agent-spec requirements trace-graph REQ-NOTE-CREATE --format mermaid`
  Then the output contains a deterministic graph from requirement to verification evidence

Scenario: Test obligations are derived from specs, not code
  Given a requirement with scenarios and a satisfying task spec
  When the operator runs `agent-spec requirements test-obligations --format json`
  Then the output contains expected test obligations without scanning implementation code

Scenario: QA class gate requires stronger evidence for high-risk work
  Given a task spec marked `risk: A`
  When the operator runs the QA class gate
  Then the gate requires lifecycle, trace, targeted tests, and adversarial review evidence

Scenario: State-machine transitions require coverage
  Given a requirement with a `## State Machine` section
  When a transition has no matching scenario or test obligation
  Then `agent-spec lint-knowledge --gate` reports a state-machine transition coverage diagnostic

Scenario: Requirement lint catches weak compiler inputs
  Given a requirement with a MUST clause, no source trace, and a weak Then step
  When the operator runs `agent-spec lint-knowledge --gate`
  Then the report includes deterministic requirement-quality diagnostics

Scenario: Requirement quality diagnostics feed clarification questions
  Given a requirement with no source trace and a compound requirement clause
  When the operator runs `agent-spec requirements questions --format json`
  Then the output includes source-trace and compound-clause diagnostics that can be resolved before task specs are generated

Scenario: agent-spec dogfoods its own compiler workflow
  Given this feature's KLL requirement and task spec are present in the agent-spec repository
  When the operator runs the self-hosting requirements plan, lifecycle, replay, explain-failure, and trace-graph commands
  Then the evidence chain proves REQ-REQUIREMENTS-COMPILER-PLAN-DAG is satisfied by agent-spec's own implementation before fixture validation is accepted

Scenario: Reference validation checks borrowed invariants
  Given the reference project's ticketbooking requirements, traceability service, and test-generation templates are available as reference material
  When final verification maps the reference requirement tree, dependency, scenario, test-first, and traceability invariants to agent-spec checks
  Then every borrowed invariant is covered by an agent-spec test, fixture assertion, or explicit non-goal without executing reference-project runtime tests

Scenario: Lore-style documentation engineering gate is present
  Given agent-spec documentation standards are scaffolded under knowledge/standards
  When documentation tests inspect the standards, proposal template, and docs lint script
  Then they find doc types, canon, operational checklist, tools guidance, pre-publish checks, proposal compatibility, security, privacy, risks, alternatives, Harper, Chinese docs lint, markdownlint, and lychee

Scenario: Compiler input boundary rejects unsafe artifacts
  Given unsafe ids, malformed frontmatter, a missing compiler root, a kind mismatch, or a symlinked knowledge entry
  When the deterministic parser and collector process the input
  Then they emit blocking diagnostics without reading or writing outside the declared roots

Scenario: Plan DAG contains executable spec topology
  Given dependent requirements and dependent satisfying task specs
  When the operator runs `agent-spec requirements plan --gate`
  Then the plan contains requirement, work-unit, and spec nodes with dependency, work-unit, satisfies, and spec-depends edges

Scenario: Requirement trace avoids false cross-products
  Given one task spec satisfies two requirements with one distinct scenario each
  When lifecycle records and replays requirement evidence
  Then each requirement is linked only to its own scenario and replay returns all scenarios from the latest run

Scenario: Archive evidence is content-bound and movement is preflighted
  Given a completed spec with name-only or stale lifecycle evidence, or an archive plan with an existing target
  When archive planning or application runs
  Then the archive is rejected before any source spec is moved

Scenario: Lifecycle enforces declared QA evidence
  Given a task spec declares risk class A, B, or C
  When lifecycle reaches its final gate
  Then missing evidence for that class makes lifecycle non-passing and invalid risk values are rejected

Scenario: Missing test selectors cannot pass lifecycle
  Given a Task Contract test selector matches zero Cargo tests
  When the test verifier evaluates successful Cargo output
  Then the scenario verdict is `skip` and the lifecycle remains non-passing

## Dependencies

- REQ-KLL-WORK-UNITS

## Source Trace

- Reference project: acknowledged once in README (Intent Compiler Workflow section); local checkout under ~/Work/Projects/consult
- Reference validation material: the reference project's ticketbooking demo requirement tree, traceability service, and test-generation templates, mapped in docs/intent-compiler/reference-validation-matrix.md
- Reference paper: /Users/zhangalex/Desktop/2602.13723v3.pdf
- Current implementation plan: docs/superpowers/plans/2026-07-08-requirements-compiler-plan-dag.md

## Open Questions

None.
