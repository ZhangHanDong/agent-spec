spec: task
name: "Provenance Run Hardening"
tags: [intent-compiler, provenance, replay]
satisfies: [REQ-PROVENANCE-RUN-HARDENING]
depends: [task-intent-compiler-provenance-manifest]
estimate: 2d
---

## Intent

Make the compiler's determinism claim executable: manifests record the exact
build (crate version + embedded build commit) and the effective command
configuration, every artifact-emitting `requirements` command can emit a
manifest, and `requirements verify-run` replays a recorded compilation and
byte-compares the outputs. Manifests stay records of deterministic facts —
no approval or authority semantics.

## Decisions

- Build identity is embedded at compile time via `option_env!("AGENT_SPEC_BUILD_COMMIT")` populated by `build.rs` from `git rev-parse HEAD` when available; the fallback value is the literal `unknown`. No new dependencies.
- `compilation-provenance-v2.schema.json` adds `build_commit`, `command` (subcommand string), and `config` (ordered flag/value pairs); v1 manifests keep verifying through the existing `verify-provenance` path.
- `--provenance <file>` lands on `requirements graph`, `plan`, `work-units`, `test-obligations`, and `traceability` (import/export already have it); the manifest records input corpus digest and every `--out` artifact digest.
- `requirements verify-run --manifest <file>` re-executes the recorded subcommand with the recorded config against a temporary output target, byte-compares each output against the recorded digests, and exits non-zero naming every drifted file.
- Replay is sandboxed to the temporary target: the recorded original outputs are read-only inputs to the comparison.
- Manifest emission and verification leave knowledge documents byte-identical.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- build.rs
- Cargo.toml
- docs/intent-compiler/schemas/**
- fixtures/requirements-yaml/**
- knowledge/requirements/req-provenance-run-hardening.md
- specs/roadmap/task-provenance-run-hardening.spec.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Forbidden
- Do not add approval, authority, actor, or policy fields to any manifest schema.
- Do not break verification of existing v1 manifests.
- Do not let replay write outside its temporary target.
- Do not add dependencies.

## Out of Scope

- Bundle-level digests (parity contract)
- Signing or attestation of manifests (external certification layers own signatures)

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when promoted -->
<!-- lint-ack: precedence-fallback-coverage — the build-commit fallback resolves at compile time of the binary (option_env!), so runtime tests can only observe the baked value; the build-identity scenario asserts the field carries a commit or the literal `unknown` -->

Scenario: Manifests record build identity and configuration
  Test:
    Filter: test_provenance_v2_records_build_and_config
    Level: integration
  Given a requirements export with `--provenance`
  When the manifest is read
  Then it validates against `compilation-provenance-v2.schema.json`
  And it records the crate version, a build commit or `unknown`, the subcommand, and the effective flags

Scenario: Artifact-emitting commands emit manifests
  Test:
    Filter: test_provenance_coverage_for_out_commands
    Level: integration
  Given a knowledge tree
  When `requirements graph`, `plan`, `work-units`, `test-obligations`, and `traceability` run with `--out` and `--provenance`
  Then each manifest records the input corpus digest and the output artifact digest

Scenario: verify-run confirms a reproducible compilation
  Test:
    Filter: test_verify_run_passes_on_reproducible_outputs
    Level: integration
  Given a manifest recorded from an unchanged knowledge tree
  When `requirements verify-run` runs with that manifest
  Then the exit code is zero and the report names zero drifted files

Scenario: verify-run names drifted outputs
  Test:
    Filter: test_verify_run_fails_naming_drifted_files
    Level: integration
  Given a recorded manifest and one knowledge document edited afterwards
  When `requirements verify-run` runs
  Then the exit code is non-zero
  And the report names each output whose fresh digest differs

Scenario: Missing manifests are a diagnostic
  Test:
    Filter: test_verify_run_rejects_missing_manifest
    Level: integration
  Given no file exists at the manifest path
  When `requirements verify-run` runs
  Then the exit code is non-zero with a diagnostic naming the path

Scenario: v1 manifests keep verifying
  Test:
    Filter: test_verify_provenance_still_accepts_v1_manifests
    Level: integration
  Given a stored v1 import/export manifest from the current fixtures
  When provenance verification runs
  Then the manifest verifies without schema errors

Scenario: Hardening leaves knowledge byte-identical
  Test:
    Filter: test_provenance_hardening_keeps_knowledge_byte_identical
    Level: integration
  Given a knowledge tree snapshot
  When manifests are emitted and verified
  Then every knowledge document is byte-identical afterwards
