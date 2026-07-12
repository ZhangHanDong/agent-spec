spec: task
name: "Reference Compiler Parity Layout"
tags: [intent-compiler, parity, compat]
satisfies: [REQ-REFERENCE-COMPILER-PARITY]
depends: [task-compiler-machine-surface, task-provenance-run-hardening]
estimate: 2d
---

## Intent

Let agent-spec stand in for the reference requirements compiler end to end:
`requirements compile` emits a per-requirement bundle (requirement document,
draft spec, traceability projection, compilation manifest) in a neutral
default layout, and `--layout arc-v1` projects the same content under the
reference-compatible file names so existing consumers work unchanged.
Compatibility lives at the edge: core schemas and the neutral layout stay
provider-neutral, enforced mechanically.

## Decisions

- `requirements compile --out <dir> [--id REQ-*] [--layout agent-spec-v1|arc-v1] [--force]`; the default layout is `agent-spec-v1` with file names `<id>/requirements.md`, `<id>/spec.md`, `<id>/traceability.json`, `<id>/compilation.json`.
- `--layout arc-v1` writes `<id>.requirements.md`, `<id>.spec.md`, `<id>.arc.traceability.json`, `<id>.arc.compilation.json` into `--out` directly; content is identical to the neutral layout modulo file naming.
- Bundle rendering is atomic: every artifact for every selected requirement renders and validates in memory before the first write; any failure writes nothing.
- Existing bundle files fail the run without `--force`; the diagnostic names each colliding path.
- The compilation manifest records per-artifact blake3 digests plus a bundle digest defined as the blake3 of the newline-joined `path:digest` lines sorted by path.
- A repository test walks `docs/intent-compiler/schemas/` and a rendered `agent-spec-v1` bundle asserting the forbidden reference token appears in neither.
- Parity fixture `fixtures/requirements-parity/` models the reference ticketbooking tree shape (FOLDER/ATOMIC, dependencies, scenarios) with golden bundles for both layouts.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- docs/intent-compiler/**
- fixtures/requirements-parity/**
- knowledge/requirements/req-reference-compiler-parity.md
- specs/roadmap/task-reference-compiler-parity.spec.md
- skills/agent-spec-tool-first/**
- README.md
- CHANGELOG.md

### Forbidden
- Do not put reference-project names into core schemas or the `agent-spec-v1` layout.
- Do not leave a partial bundle behind on any failure.
- Do not mutate knowledge documents during compile.
- Do not add dependencies.

## Out of Scope

- Executing reference-project runtime tests (validation-matrix non-goal stands)
- Orchestrator-side bundle consumption, approval binding, or certification
- Byte-identity with the reference implementation's own renderer (compatibility is schema/consumer-level, pinned by the parity goldens)

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when promoted -->

Scenario: Neutral bundles render with digests
  Test:
    Filter: test_compile_emits_neutral_bundle_with_bundle_digest
    Level: integration
  Given the parity fixture knowledge tree
  When `requirements compile` runs with `--out`
  Then each selected requirement gets requirements.md, spec.md, traceability.json, and compilation.json
  And the manifest records per-artifact digests and the bundle digest over sorted `path:digest` lines

Scenario: The arc-v1 layout matches reference file names
  Test:
    Filter: test_compile_arc_layout_matches_reference_names
    Level: integration
  Given the parity fixture knowledge tree
  When `requirements compile` runs with `--layout arc-v1`
  Then the bundle files are named `<id>.requirements.md`, `<id>.spec.md`, `<id>.arc.traceability.json`, `<id>.arc.compilation.json`
  And the goldens are byte-identical across two runs

Scenario: Unknown layouts are rejected
  Test:
    Filter: test_compile_rejects_unknown_layout
    Level: integration
  Given any knowledge tree
  When `requirements compile` runs with `--layout nonsense-v9`
  Then the exit code is non-zero and the diagnostic lists the accepted layouts

Scenario: Existing bundles refuse overwrite without --force
  Test:
    Filter: test_compile_refuses_overwrite_without_force
    Level: integration
  Given a previously compiled bundle in the target directory
  When `requirements compile` runs again without `--force`
  Then the exit code is non-zero naming each colliding file
  And every pre-existing file is byte-identical afterwards

Scenario: A failing render writes nothing
  Test:
    Filter: test_compile_atomic_failure_leaves_no_partial_bundle
    Level: integration
  Given an accepted fixture requirement whose draft spec cannot render
  When `requirements compile` runs
  Then the exit code is non-zero
  And the target directory contains no file from the failed run

Scenario: Core schemas stay provider-neutral
  Test:
    Filter: test_core_schemas_and_neutral_layout_carry_no_reference_token
    Level: integration
  Given the schema directory and a rendered neutral bundle
  When the vocabulary check scans both
  Then the forbidden reference token appears in neither

Scenario: Reference-style trees import before compiling
  Test:
    Filter: test_parity_fixture_reference_tree_imports_cleanly
    Level: integration
  Given the parity fixture `requirements.yaml` in the reference tree shape
  When `requirements import --from` runs
  Then the import succeeds with zero `yaml-unsupported-construct` diagnostics
