# Reference Validation Matrix

agent-spec borrows compiler-style requirement invariants from an external
reference project (acknowledged once in the README, Intent Compiler Workflow
section), not as an executable dependency. The reference project's Python,
Node, VS Code, Playwright, and Android runtime tests are not imported into
agent-spec. This file is a persistent validation checklist: every borrowed
invariant maps to an agent-spec test, fixture assertion, command gate, or
explicit non-goal.

## References

Reference material, described by capability (paths live in the reference
project, not in this repository):

- ticketbooking demo requirement tree (`ticketbooking-demo/requirements.yaml`)
- traceability query service (`traceability/service.py`, agent-runtime `traceability.py`)
- test generation templates (`agents/test_generator.py`)
- generated app templates (`templates/web/backend`, `templates/android/app/src/test`)

## Validation Rows

| Reference method | Borrowed invariant | agent-spec evidence | Status |
| --- | --- | --- | --- |
| `ticketbooking-demo/requirements.yaml` | A requirement tree separates `FOLDER` grouping nodes from `ATOMIC` executable leaves. | `test_requirement_graph_extracts_dependencies_scenarios_and_open_questions`; `test_requirements_plan_json_includes_batches_edges_and_coverage`; `requirements graph`; `requirements plan`. | Covered |
| `ticketbooking-demo/requirements.yaml` | Requirement nodes carry explicit `dependencies` that form a DAG and affect execution order. | `test_requirement_graph_reports_dangling_dependency_and_cycle`; `test_requirements_plan_gate_fails_on_dangling_dependency`; `requirements graph --gate`; `requirements plan --gate`. | Covered |
| `ticketbooking-demo/requirements.yaml` | Executable leaves carry scenario-grounded GIVEN/WHEN/THEN behavior, including observable outcomes. | `test_requirements_test_obligations_json_contains_spec_derived_obligations`; `test_lint_requirement_warns_when_must_clause_has_no_scenario`; `test_lint_requirement_warns_on_weak_then_step`; `requirements test-obligations`. | Covered |
| `ticketbooking-demo/requirements.yaml` | negative cases such as duplicate accounts, invalid credentials, empty results, and missing booking context are first-class requirements. | `test_lint_requirement_warns_when_negative_behavior_lacks_negative_scenario`; fixture scenarios in `fixtures/requirements-noteapp`; `lint-knowledge --gate`. | Covered |
| `agents/test_generator.py` | test-first obligations derive from requirement focus and scenarios rather than implementation code. | `test_requirements_test_obligations_json_contains_spec_derived_obligations`; `test_requirements_compiler_schema_files_and_fixture_golden_outputs_are_stable`; `requirements test-obligations`. | Covered |
| `agents/test_generator.py` | Unit, integration, and E2E layers are selected from requirement intent and ownership. | `test_qa_class_a_requires_lifecycle_trace_targeted_tests_and_adversarial_review`; QA class A/B/C gates. agent-spec records required evidence strength; it does not generate app-layer tests. | Covered with non-goal boundary |
| `traceability/service.py` and agent-runtime `traceability.py` | traceability queries answer requirement-to-evidence questions across requirements, tests, call edges, node states, and code evidence. | `test_requirements_replay_uses_latest_trace_record_for_requirement`; `test_requirements_explain_failure_reports_non_pass_chain`; `test_requirements_trace_graph_mermaid_contains_evidence_nodes`; `requirements replay`; `requirements explain-failure`; `requirements trace-graph`. | Covered |
| `templates/web/backend` and `templates/android/app/src/test` | Generated app templates include runnable tests connected to requirements. | Non-goal: agent-spec validates contracts, work units, lifecycle evidence, and traceability; it does not import reference-project app templates or execute their runtime tests. | Explicit non-goal |

## ARC-Native Dialect Conformance (2026-07-19 refresh)

The reference project's real input format diverges from the v1.1 exchange
dialect: a single root node (not a `requirements:` list), `name:` (not
`title:`), folded block scalars, flow-style empty lists, `steps:
[{keyword, content}]` scenarios, ATOMIC `description:` statements, and
dotted hierarchical ids. The reference runtime now stores traceability in a
SQLite database (`traceability.db`) rather than file artifacts.

| Reference reality | agent-spec evidence | Status |
| --- | --- | --- |
| Verbatim `ticketbooking-demo/requirements.yaml` imports | `fixtures/arc-native/requirements.yaml` (byte-exact copy); `test_arc_native_real_ticketbooking_imports_cleanly`; `test_parity_fixture_reference_tree_imports_cleanly` | Covered |
| Reference loader consumes agent-spec exports | `requirements export --dialect arc-native`; `test_arc_native_export_is_reference_loadable`; loader semantics replicated (`yaml.safe_load` + wrapper unwrap + non-empty root id) | Covered |
| Dotted ids survive the round trip | `source-id:` frontmatter + clause comments; `test_arc_native_dotted_ids_normalize_with_source_id` | Covered |
| Traceability database (`traceability.db`) | Non-goal: a runtime artifact of the reference agent pipeline, not an input format; consumers read it directly | Explicit non-goal |

## Non-Goals

- Non-goal: execute reference-project runtime tests as part of agent-spec CI.
- Non-goal: depend on reference-project Python, Node, Playwright, VS Code, or Android runtime dependencies.
- Non-goal: parse reference-project YAML directly in the CLI; agent-spec stays with KLL Markdown artifacts unless a future explicit import task adds YAML support. That task shipped: `specs/task-intent-compiler-yaml-frontend.spec.md` (satisfies `REQ-INTENT-COMPILER-YAML-FRONTEND`) adds the constrained YAML dialect frontend.

## Acceptance Rule

The matrix is valid only when every borrowed reference invariant above has one of:

- A named agent-spec Rust test.
- A deterministic agent-spec command gate.
- A checked fixture assertion.
- An explicit non-goal row.
