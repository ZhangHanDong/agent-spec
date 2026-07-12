---
kind: requirement
id: REQ-INTENT-COMPILER-YAML-FRONTEND
title: "Intent Compiler YAML Frontend"
status: proposed
liveness: auto
tags: [intent-compiler, yaml, frontend, import]
---

# Intent Compiler YAML Frontend

## Problem

The intent compiler imports requirements only from Markdown files carrying
explicit marked blocks. Teams arriving from reference-project-style tooling
hold requirement trees in a constrained `requirements.yaml` dialect (FOLDER
grouping nodes, ATOMIC executable leaves, dependencies, GIVEN/WHEN/THEN
scenarios). They need a deterministic frontend that translates that dialect
into the existing Requirement IR without changing the IR or any downstream
compiler stage.

## Requirements

[REQ-INTENT-COMPILER-YAML-FRONTEND-INPUT] `agent-spec requirements import` MUST accept a `.yaml`/`.yml` source file and route it to the YAML frontend by file extension.

[REQ-INTENT-COMPILER-YAML-FRONTEND-SUBSET] The frontend MUST parse only the documented YAML subset (two-space indentation, scalar strings, lists, and maps with known keys).

[REQ-INTENT-COMPILER-YAML-FRONTEND-SUBSET-REJECT] Constructs outside the documented subset (anchors, aliases, flow style, multi-document streams) MUST produce a `yaml-unsupported-construct` diagnostic instead of a partial import.

[REQ-INTENT-COMPILER-YAML-FRONTEND-MAP-DOCS] Each top-level FOLDER node MUST map to one generated requirement document under `knowledge/requirements/`.

[REQ-INTENT-COMPILER-YAML-FRONTEND-MAP-CLAUSES] Each ATOMIC leaf MUST map to one `[REQ-*]` clause inside its folder's document.

[REQ-INTENT-COMPILER-YAML-FRONTEND-MAP-SCENARIOS] GIVEN/WHEN/THEN entries on a leaf MUST map to `Scenario:` blocks in the generated document.

[REQ-INTENT-COMPILER-YAML-FRONTEND-MAP-DEPS] Node `dependencies` MUST map to requirement-level `## Dependencies` entries resolvable by `requirements graph`.

[REQ-INTENT-COMPILER-YAML-FRONTEND-IDS] Node ids MUST pass the existing safe-id trust boundary; unsafe ids MUST be rejected with diagnostics rather than silently renamed.

[REQ-INTENT-COMPILER-YAML-FRONTEND-MARKER] Generated documents MUST carry an imported-from-YAML provenance marker in frontmatter.

[REQ-INTENT-COMPILER-YAML-FRONTEND-OWNERSHIP] Import MUST NOT overwrite an existing file that lacks the provenance marker.

[REQ-INTENT-COMPILER-YAML-FRONTEND-IDEMPOTENT] Re-importing an unchanged YAML source MUST produce byte-identical generated documents.

[REQ-INTENT-COMPILER-YAML-FRONTEND-IR-FROZEN] The frontend MUST NOT change the Requirement IR schema or any downstream stage; imported documents flow through `lint-knowledge`, `graph`, `work-units`, and `plan` unchanged.

[REQ-INTENT-COMPILER-YAML-FRONTEND-NEGATIVE] Satisfying specs MUST include negative scenarios covering unsupported YAML constructs, unsafe node ids, dangling dependencies, and overwrite refusal.

## Dependencies

- REQ-REQUIREMENTS-COMPILER-PLAN-DAG

## Source Trace

- decision origin: terminology/positioning session 2026-07-12 — YAML dialect frontend keeps the IR frozen
- supersedes the matrix non-goal "parse reference-project YAML directly in the CLI" via the explicit import task it anticipated: docs/intent-compiler/reference-validation-matrix.md
- contract: specs/task-intent-compiler-yaml-frontend.spec.md
