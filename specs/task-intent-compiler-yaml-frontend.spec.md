spec: task
name: "Intent Compiler YAML Frontend"
tags: [intent-compiler, yaml, frontend, import]
satisfies: [REQ-INTENT-COMPILER-YAML-FRONTEND]
depends: [task-requirements-compiler-plan-dag]
estimate: 1w
---

## Intent

Add a YAML dialect frontend to the intent compiler: `requirements import` learns
to translate reference-project-style requirement trees (`requirements.yaml` with
FOLDER grouping nodes, ATOMIC leaves, dependencies, and GIVEN/WHEN/THEN
scenarios) into the existing Requirement IR under `knowledge/requirements/`.
The IR and every downstream stage stay frozen — this is a new source dialect,
not a new pipeline. It is the explicit import task the reference-validation
matrix non-goal anticipated.

## Decisions

- Extend `agent-spec requirements import --from <file>`: a `.yaml`/`.yml` extension routes to the YAML frontend; Markdown behavior is unchanged.
- Hand-write the parser for a documented YAML subset (two-space indentation, scalar strings, lists, maps with known keys); do not add `serde_yaml` or any new dependency.
- Reject anchors, aliases, flow style, and multi-document streams with a `yaml-unsupported-construct` diagnostic; never emit a partial import.
- Mapping is fixed: top-level FOLDER → one `knowledge/requirements/req-<slug>.md` with `id: REQ-<SLUG>`; ATOMIC leaf → one `[REQ-<SLUG>-<NODE>]` clause; leaf GIVEN/WHEN/THEN entries → `Scenario:` blocks; node `dependencies` → `## Dependencies` entries.
- A FOLDER may declare optional `status: proposed|accepted` (default `proposed`) mapped to frontmatter `status` — human acceptance lives in the human-owned YAML source; any other value is a diagnostic, and work units stay informational until the folder is accepted.
- Node ids pass the existing safe-id trust boundary; unsafe ids are diagnostics, never silent renames.
- Generated documents carry `source: imported-yaml` provenance frontmatter; import refuses to overwrite any existing file lacking that marker.
- Re-import of an unchanged source regenerates byte-identical files (whole-file regeneration, no clause-level merging).
- Document the accepted subset and the mapping table in `docs/intent-compiler/yaml-frontend-v1.md`.
- Fixture `fixtures/requirements-yaml/` holds a tree with two FOLDER nodes, ATOMIC leaves, a cross-folder dependency, scenarios with negative cases, and golden imported output.
- Scenario keywords accept the same bilingual (English/Chinese) forms as the Markdown DSL.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- fixtures/requirements-yaml/**
- docs/intent-compiler/yaml-frontend-v1.md
- docs/intent-compiler/reference-validation-matrix.md
- knowledge/requirements/req-intent-compiler-yaml-frontend.md
- specs/roadmap/task-intent-compiler-yaml-frontend.spec.md
- skills/agent-spec-tool-first/**
- skills/agent-spec-intent-compiler/**
- README.md
- AGENTS.md
- CHANGELOG.md

### Forbidden
- Do not add dependencies; the subset parser is hand-written.
- Do not change the Requirement IR schema, `requirements graph`, `work-units`, `plan`, or any downstream stage.
- Do not overwrite human-authored knowledge files.
- Do not interpret prose outside the documented node fields.
- Do not add network calls.

## Out of Scope

- Full YAML specification support (anchors, aliases, block scalars beyond the subset)
- Reference-project runtime, template, or test import
- Bidirectional export (IR → YAML)
- Watch mode or auto-sync when the YAML source changes

## Completion Criteria

<!-- lint-ack: output-mode-coverage — file side effects are asserted directly by the idempotence and ownership scenarios -->
<!-- lint-ack: flag-combination-coverage — `proposed|accepted` is a status value set, not CLI flags; the import command has a single output-affecting flag (--check) -->


### Rule: yaml-subset-parsing — 子集解析与信任边界

Scenario: Reference-style tree parses into the IR
  Test:
    Filter: test_yaml_frontend_parses_folder_atomic_tree
    Level: integration
  Given `fixtures/requirements-yaml/requirements.yaml` with FOLDER nodes, ATOMIC leaves, dependencies, and scenarios
  When `requirements import` runs with the YAML source
  Then generated requirement documents appear under the output directory
  And each document carries the `source: imported-yaml` provenance marker
  And each document carries the folder's declared `status` with default `proposed`

Scenario: Constructs outside the subset are rejected whole
  Test:
    Filter: test_yaml_frontend_rejects_unsupported_yaml
    Level: integration
  Given a YAML source using anchors and flow-style collections
  When `requirements import` runs
  Then the exit code is non-zero with diagnostic `yaml-unsupported-construct`
  And no generated file is written

Scenario: Unsafe node ids are diagnostics, not renames
  Test:
    Filter: test_yaml_frontend_rejects_unsafe_node_ids
    Level: integration
  Given a YAML node whose id contains path separators
  When `requirements import` runs
  Then the import fails with a safe-id diagnostic naming the node
  And no generated file is written

### Rule: ir-mapping — 固定映射到 Requirement IR

Scenario: Folders become documents and leaves become clauses
  Test:
    Filter: test_yaml_frontend_maps_folders_to_docs_and_leaves_to_clauses
    Level: integration
  Given the fixture tree has two top-level FOLDER nodes with ATOMIC leaves
  When `requirements import` runs
  Then each folder yields one `req-<slug>.md` with `id: REQ-<SLUG>`
  And each leaf yields one `[REQ-<SLUG>-<NODE>]` clause with its GIVEN/WHEN/THEN entries as `Scenario:` blocks

Scenario: Dependencies survive into the requirement graph
  Test:
    Filter: test_yaml_frontend_maps_dependencies_into_graph
    Level: integration
  Given the fixture tree declares a cross-folder dependency
  When `requirements import` runs and `requirements graph --gate` runs on the output
  Then the generated `## Dependencies` entries resolve and the gate exits 0

Scenario: Dangling dependencies are caught by the existing gate
  Test:
    Filter: test_yaml_frontend_import_then_graph_reports_dangling_dependency
    Level: integration
  Given a YAML tree referencing a dependency id that no node defines
  When `requirements import` runs and `requirements graph --gate` runs on the output
  Then the gate exits non-zero reporting the dangling dependency

### Rule: ownership-and-idempotence — 生成物所有权

Scenario: Re-import is byte-identical
  Test:
    Filter: test_yaml_frontend_reimport_is_idempotent
    Level: integration
  Given an unchanged YAML source already imported once
  When `requirements import` runs again
  Then every generated file is byte-identical to the first import

Scenario: Human-authored files are never overwritten
  Test:
    Filter: test_yaml_frontend_refuses_overwriting_unmarked_files
    Level: integration
  Given the output directory contains a requirement file without the provenance marker at a colliding path
  When `requirements import` runs
  Then the import fails with an ownership diagnostic naming the file
  And the existing file is byte-identical to its state before the import
