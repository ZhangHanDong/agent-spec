spec: task
name: "Intent Compiler YAML Export"
tags: [intent-compiler, yaml, export, projection]
satisfies: [REQ-INTENT-COMPILER-YAML-EXPORT]
depends: [task-intent-compiler-yaml-frontend]
estimate: 2d
---

## Intent

Add the export direction to the intent compiler's YAML dialect:
`requirements export` projects hand-owned canonical requirement documents into
the constrained `requirements.yaml` dialect that `requirements import` already
consumes. The exported file is a derived projection for YAML-world interop —
never the source of truth — and the round-trip law (export → import → export
is byte-identical) anchors correctness. The IR and all downstream stages stay
frozen.

## Decisions

- New subcommand `agent-spec requirements export --knowledge knowledge --out <file>` with the `.yaml`/`.yml` extension required; optional repeatable `--id REQ-X` filters the exported set.
- Scope: `kind: requirement` documents with status `proposed` or `accepted`; superseded, deprecated, and rejected documents are excluded with a diagnostic naming each exclusion.
- Inverse mapping is fixed: document → top-level FOLDER (id lowercased from `REQ-<SLUG>`, title, status, Problem text reflowed to a single-line `description`); clause `[REQ-<DOC>-<SUFFIX>]` → ATOMIC leaf `id: <suffix-lowercase>` with the clause text as `statement`; leaf `title` synthesized deterministically from the suffix (hyphens to spaces, first letter capitalized).
- Document-level `## Dependencies` and `## Scenarios` export as FOLDER-level `dependencies` and `scenarios`; the frontend gains symmetric support for both FOLDER-level keys, rendered to the same document sections the exporter reads.
- Round-trip fixpoint law: `export → import → export` produces byte-identical YAML; deterministic rendering (stable ordering, double-quoted scalars) makes the projection reproducible.
- A clause id that does not extend its document id, or scalar content the dialect cannot carry (double quotes, backslashes, content requiring block scalars), fails the whole export with a diagnostic; no file is written.
- Content the dialect cannot express (Source Trace, tags, lint acknowledgements, multi-paragraph formatting) is listed in a `yaml-export-lossy` diagnostic instead of vanishing silently.
- The exported file is derived: export overwrites the target; `--check` re-renders and exits non-zero on drift, mirroring `import --check`.
- No new dependencies; the exporter is hand-written beside the frontend in `src/spec_knowledge/`, and export never modifies knowledge documents.
- Document the inverse mapping and dialect v1.1 (FOLDER-level `dependencies`/`scenarios`) in `docs/intent-compiler/yaml-frontend-v1.md`.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- fixtures/requirements-yaml/**
- docs/intent-compiler/yaml-frontend-v1.md
- knowledge/requirements/req-intent-compiler-yaml-export.md
- specs/roadmap/task-intent-compiler-yaml-export.spec.md
- skills/agent-spec-tool-first/**
- skills/agent-spec-intent-compiler/**
- README.md
- AGENTS.md
- CHANGELOG.md

### Forbidden
- Do not add dependencies.
- Do not modify any knowledge document during export or change the Requirement IR schema.
- Do not change `requirements graph`, `work-units`, `plan`, or any downstream stage.
- Do not treat the exported YAML as a source of truth for confirmed requirements.
- Do not add network calls.

## Out of Scope

- Exporting decisions, guidance, proposals, or non-requirement knowledge kinds
- Watch mode or auto-export on requirement change
- Carrying Source Trace, tags, or lint acknowledgements into the dialect
- Bidirectional merge (export remains a one-way derived projection)

## Completion Criteria

<!-- lint-ack: output-mode-coverage — file side effects are asserted directly by the fixpoint and check-drift scenarios -->
<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when the spec is promoted to specs/ -->
<!-- lint-ack: flag-combination-coverage — the id-filter scenario exercises `--id` and `--out` together; the heuristic does not recognize the phrasing -->

Scenario: Corpus exports to the dialect tree
  Test:
    Filter: test_yaml_export_renders_corpus_tree
    Level: integration
  Given requirement documents with clauses, document-level scenarios, dependencies, and statuses `proposed` and `accepted`
  When `requirements export` runs to a `.yaml` target
  Then the output holds one FOLDER per document with leaves, synthesized titles, `status`, FOLDER-level `dependencies` and `scenarios`

Scenario: Id filter restricts the exported tree
  Test:
    Filter: test_yaml_export_id_filter_restricts_output
    Level: integration
  Given a corpus with three exportable requirement documents
  When `requirements export` runs with two `--id` filters and an `--out` target
  Then the output holds exactly the two named FOLDER nodes
  And an unknown `--id` value fails the export with a diagnostic

Scenario: Round-trip is a fixpoint
  Test:
    Filter: test_yaml_export_import_roundtrip_fixpoint
    Level: integration
  Given an exported `requirements.yaml`
  When the file is imported and the resulting documents are exported again
  Then the second export is byte-identical to the first

Scenario: Frontend accepts FOLDER-level dependencies and scenarios
  Test:
    Filter: test_yaml_frontend_accepts_folder_dependencies_and_scenarios
    Level: integration
  Given a YAML tree with FOLDER-level `dependencies` and `scenarios`
  When `requirements import` runs
  Then the generated document carries matching `## Dependencies` entries and a `## Scenarios` section

Scenario: Nonconforming clause ids fail the export whole
  Test:
    Filter: test_yaml_export_rejects_nonconforming_clause_ids
    Level: integration
  Given a document whose clause id does not extend the document id
  When `requirements export` runs
  Then the exit code is non-zero with a diagnostic naming the clause
  And no file is written

Scenario: Inexpressible scalars fail the export whole
  Test:
    Filter: test_yaml_export_rejects_inexpressible_scalars
    Level: integration
  Given a clause statement containing a double quote
  When `requirements export` runs
  Then the exit code is non-zero with a diagnostic naming the document
  And no file is written

Scenario: Excluded statuses are reported, not silently dropped
  Test:
    Filter: test_yaml_export_reports_excluded_statuses
    Level: integration
  Given a superseded requirement document in the corpus
  When `requirements export` runs
  Then the output omits the superseded document
  And a diagnostic names the exclusion

Scenario: Check mode detects projection drift
  Test:
    Filter: test_yaml_export_check_detects_drift
    Level: integration
  Given an exported file that was manually edited afterwards
  When `requirements export` runs with `--check`
  Then the exit code is non-zero naming the drifted target
