spec: task
name: "Intent Compiler Compilation Provenance Manifest"
tags: [intent-compiler, provenance, digest]
satisfies: [REQ-INTENT-COMPILER-PROVENANCE]
depends: [task-intent-compiler-yaml-export]
estimate: 4h
---

## Intent

Give the intent compiler's YAML transformations a verifiable compilation
provenance artifact: an opt-in `--provenance <path>.json` manifest on
`requirements import` and `requirements export` binding the compilation
direction, input digest, output digests, tool identity, dialect schema
version, and a reproducibility result — so a later verifier can prove the
artifact chain by recomputing digests instead of re-trusting the producer.
This aligns the compiler with the enterprise `*.compilation.json` requirement.

## Decisions

- `--provenance <path>` is opt-in on both `requirements import` and `requirements export`; the target must end in `.json` and any other extension is a diagnostic.
- Manifest fields: `manifest_version: 1`, `direction` (`import` | `export`), `tool` (`agent-spec` + crate version), `dialect_schema` (`yaml-frontend-v1.1`), `input` (`path`, `blake3`), `outputs` (list of `path` + `blake3`), `reproducible` (bool from re-rendering and digest comparison).
- Digests are blake3 hex over file bytes; the export input digest covers the knowledge corpus as the blake3 of the sorted (path, doc-digest) pairs.
- The manifest is written after the transformation outputs; a manifest write failure reports the path and leaves the outputs intact.
- `verify_provenance(manifest_path)` recomputes every digest and returns the drifted paths; an empty result means the chain still holds.
- The manifest shape is documented by `docs/intent-compiler/schemas/compilation-provenance-v1.schema.json` with a stable `$id` under `agent-spec/intent-compiler/`.
- Hashing is blake3, already present in the workspace tree via rust-atlas; the root manifest gains the workspace-shared `blake3` entry and nothing else.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- Cargo.toml
- Cargo.lock
- docs/intent-compiler/**
- knowledge/requirements/req-intent-compiler-provenance.md
- specs/task-intent-compiler-provenance-manifest.spec.md
- skills/agent-spec-tool-first/**
- README.md
- AGENTS.md
- CHANGELOG.md

### Forbidden
- Do not make provenance mandatory; without `--provenance` behavior is byte-identical to before.
- Do not modify knowledge documents during provenance emission.
- Do not add network calls.

## Out of Scope

- Signing or attestation of manifests (enterprise layer owns signatures)
- Provenance for Markdown intake or non-YAML transformations
- Registry or storage of manifests beyond the named file

## Completion Criteria

<!-- lint-ack: output-mode-coverage — manifest file emission and drift behavior are asserted directly by the scenarios -->
<!-- lint-ack: decision-coverage — the dependency-hygiene decision is structural (manifest diff), not scenario-verifiable -->

### Rule: manifest-emission — 双向清单

Scenario: Export writes a digest-complete manifest
  Test:
    Filter: test_export_provenance_manifest_binds_digests
    Level: integration
  Given a knowledge corpus and an export target
  When `requirements export` runs with `--provenance`
  Then the manifest records direction `export`, the corpus input digest, the yaml output digest, tool name and version, and `reproducible: true`

Scenario: Import writes a digest-complete manifest
  Test:
    Filter: test_import_provenance_manifest_binds_digests
    Level: integration
  Given a YAML source and an import output directory
  When `requirements import` runs with `--provenance`
  Then the manifest records direction `import`, the source digest, one digest per generated document, and `reproducible: true`

### Rule: verification — 摘要重算

Scenario: Verification detects output drift
  Test:
    Filter: test_provenance_verify_detects_drift
    Level: integration
  Given a written manifest whose output file is modified afterwards
  When `verify_provenance` recomputes the digests
  Then the drifted path is reported
  And an unmodified manifest verifies with no drifted paths

Scenario: Non-JSON provenance targets are rejected
  Test:
    Filter: test_provenance_rejects_non_json_target
    Level: integration
  Given a provenance target ending in `.yaml`
  When the export runs with `--provenance`
  Then the exit is a diagnostic naming the extension rule
  And no manifest is written

Scenario: Manifest write failure leaves outputs intact
  Test:
    Filter: test_provenance_write_failure_keeps_outputs
    Level: integration
  Given a provenance path inside a nonexistent directory
  When the export runs with `--provenance`
  Then the command reports the manifest path error
  And the exported yaml target still exists with its content
