---
kind: requirement
id: REQ-REQUIREMENT-STATUS-QUERY
title: "Requirement Three-Axis Status Query"
status: accepted
liveness: auto
tags: [intent-compiler, status, governance, liveness]
---

# Requirement Three-Axis Status Query

## Problem

One `status` field cannot represent governance, delivery, and current
correctness. Today answering "where is REQ-X?" takes three commands and mental
math: frontmatter for governance, guard/spec location for execution progress,
and trace for liveness. The target architecture requires one aggregate query
exposing all three axes without conflating them.

## Requirements

[REQ-REQUIREMENT-STATUS-QUERY-COMMAND] `agent-spec requirements status <ID>` MUST report the governance, execution, and liveness axes for one requirement.

[REQ-REQUIREMENT-STATUS-QUERY-GOVERNANCE] The governance axis MUST come from the persisted KLL frontmatter status, with `missing` reported explicitly.

[REQ-REQUIREMENT-STATUS-QUERY-EXECUTION] The execution axis MUST be derived by the fixed ladder: archived when an archived spec satisfies the id; verified when an active spec satisfies it and liveness is honored; active when an active spec satisfies it; ready when the work unit is ready; planned when only a staged roadmap spec satisfies it; otherwise unplanned.

[REQ-REQUIREMENT-STATUS-QUERY-LIVENESS] The liveness axis MUST be recomputed from current spec verdicts, never stored.

[REQ-REQUIREMENT-STATUS-QUERY-EVIDENCE] The report MUST list the active, staged, and archived satisfying spec paths and the work-unit state behind the derivation.

[REQ-REQUIREMENT-STATUS-QUERY-UNKNOWN] An id with no requirement document MUST be a diagnostic.

[REQ-REQUIREMENT-STATUS-QUERY-NEGATIVE] Satisfying specs MUST include negative scenarios covering unknown ids and the unplanned execution state.

## Dependencies

- REQ-REQUIREMENT-GOVERNANCE

## Source Trace

- target architecture: docs/intent-compiler/architecture.md (Three Independent State Axes; delivery boundary 5, first slice)
- contract: specs/task-requirement-status-query.spec.md
