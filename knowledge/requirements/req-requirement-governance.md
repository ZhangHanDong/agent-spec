---
kind: requirement
id: REQ-REQUIREMENT-GOVERNANCE
title: "Requirement Governance Gate and Transitions"
status: accepted
liveness: auto
tags: [intent-compiler, governance, kll]
---

# Requirement Governance Gate and Transitions

## Problem

Requirement governance status answers whether a requirement is authorized to
enter executable lowering, but today the compiler treats a missing status as
schedulable, the Markdown intake emits candidates without any status, and the
only way a human can accept, reject, or supersede a requirement is hand-editing
frontmatter with no validation of legal transitions. The target architecture
(`docs/intent-compiler/architecture.md`) requires an explicit, auditable
governance gate: missing status fails rather than passing, and transitions are
CLI actions that compilation itself can never perform.

## Requirements

[REQ-REQUIREMENT-GOVERNANCE-GATE-MISSING] `requirements graph` validation MUST report a requirement without a governance status as an error diagnostic `requirement-governance-missing`.

[REQ-REQUIREMENT-GOVERNANCE-UNITS-MISSING] Work units for a requirement without a governance status MUST be `informational`, never `ready`.

[REQ-REQUIREMENT-GOVERNANCE-INTAKE-PROPOSED] Markdown intake MUST emit generated requirement documents with `status: proposed`.

[REQ-REQUIREMENT-GOVERNANCE-TRANSITION-CMD] `agent-spec requirements transition <ID> --to <status>` MUST rewrite only the frontmatter `status` line of the target requirement document.

[REQ-REQUIREMENT-GOVERNANCE-TRANSITION-LEGAL] Transitions MUST follow the state machine: `proposed` → `accepted` or `rejected`; `accepted` → `deprecated`; a document without status may transition to `proposed` or `accepted`; every other transition MUST be rejected with a diagnostic.

[REQ-REQUIREMENT-GOVERNANCE-TRANSITION-AUDIT] The transition command MUST print the requirement id with the old and new status.

[REQ-REQUIREMENT-GOVERNANCE-SUPERSEDE-CMD] `agent-spec requirements supersede <OLD> --by <NEW>` MUST set the old document's status to `superseded` and record `supersedes: <OLD>` in the new document's frontmatter.

[REQ-REQUIREMENT-GOVERNANCE-SUPERSEDE-ATOMIC] When any part of a supersession fails, neither document changes.

[REQ-REQUIREMENT-GOVERNANCE-SUPERSEDE-VALID] Both supersession ids MUST resolve to existing requirement documents, and the replacement MUST NOT be `superseded` or `rejected` itself.

[REQ-REQUIREMENT-GOVERNANCE-COMPILER-IMMUTABLE] `requirements graph`, `work-units`, and `plan` MUST leave every knowledge document byte-identical.

[REQ-REQUIREMENT-GOVERNANCE-SELF-HOSTING] Every requirement document in this repository MUST declare a governance status.

[REQ-REQUIREMENT-GOVERNANCE-NEGATIVE] Satisfying specs MUST include negative scenarios covering missing status, illegal transitions, unknown ids, and failed supersessions.

## Dependencies

- REQ-REQUIREMENTS-COMPILER-PLAN-DAG

## Source Trace

- target architecture: docs/intent-compiler/architecture.md (Requirement Governance Gate; delivery boundary 1)
- acceptance: user directive 2026-07-12 to implement toward the target architecture
- gap evidence: docs/intent-compiler/dogfood-rust-atlas.md (governance catches 1 and 3)
- contract: specs/task-requirement-governance-transitions.spec.md
