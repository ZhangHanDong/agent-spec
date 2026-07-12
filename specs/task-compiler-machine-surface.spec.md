spec: task
name: "Compiler Machine Surface"
tags: [intent-compiler, machine-surface, json]
satisfies: [REQ-COMPILER-MACHINE-SURFACE]
depends: [task-requirement-governance-transitions, task-requirement-status-query]
estimate: 1d
---

## Intent

Give orchestrators a machine-readable compiler surface without adopting any
orchestrator's domain concepts: governance actions gain `--format json` output
carrying document digests, and a new `requirements traceability` command
projects requirement → clauses → specs → tests → verdicts → liveness as one
deterministic JSON document. External systems bind approvals to the reported
digests; the compiler itself never models approval, identity, or policy.

## Decisions

- `requirements transition <ID> --to <status> --format json` prints `{id, from, to, document, document_digest}` on success; `from` is `null` when the document had no status; digests are blake3 of the rewritten file.
- `requirements supersede <OLD> --by <NEW> --format json` prints both documents as `{id, status, document, document_digest}` entries after the atomic rewrite.
- Failures keep today's semantics: non-zero exit, diagnostic on stderr, nothing on stdout — no partial JSON.
- The JSON objects contain no actor, authority, approval, or policy fields.
- `requirements traceability <ID> [--knowledge] [--specs] [--code] [--format json|text] [--out <file>]` renders `requirement-traceability-v1`: clauses, satisfying specs, scenarios, bound test selectors, latest verdicts from run/trace records, and derived liveness.
- Schema file `docs/intent-compiler/schemas/requirement-traceability-v1.schema.json` with an `agent-spec/intent-compiler/` `$id`; all arrays sorted by id/path for byte-stable output.
- Traceability is a read: knowledge documents stay byte-identical.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- docs/intent-compiler/schemas/**
- fixtures/requirements-noteapp/**
- knowledge/requirements/req-compiler-machine-surface.md
- specs/roadmap/task-compiler-machine-surface.spec.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Forbidden
- Do not add actor, authority, approval, or policy fields to any output or schema.
- Do not change the human text output of transition/supersede.
- Do not mutate knowledge documents from the traceability command.
- Do not add dependencies.

## Out of Scope

- Approval storage or approval envelopes (external systems own them)
- Bundle emission (`requirements compile` — parity contract)
- MCP write tools (MCP stays read-only)

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when promoted -->

Scenario: Transition emits digest-bearing JSON
  Test:
    Filter: test_requirements_transition_json_reports_digest
    Level: integration
  Given a requirement document with `status: proposed`
  When `requirements transition` runs with `--to accepted --format json`
  Then stdout is one JSON object with id, from `proposed`, to `accepted`, document path, and the blake3 of the rewritten file
  And the object contains no actor, authority, approval, or policy field

Scenario: Illegal transitions print no partial JSON
  Test:
    Filter: test_requirements_transition_json_illegal_keeps_stdout_empty
    Level: integration
  Given a requirement document with `status: accepted`
  When `requirements transition` runs with `--to proposed --format json`
  Then the exit code is non-zero with the diagnostic on stderr
  And stdout is empty
  And the document is unchanged

Scenario: Supersede JSON covers both documents atomically
  Test:
    Filter: test_requirements_supersede_json_reports_both_documents
    Level: integration
  Given two requirement documents where the old one is `accepted`
  When `requirements supersede` runs with `--format json`
  Then the JSON lists both documents with their statuses and post-rewrite digests

Scenario: Traceability projects the full evidence chain deterministically
  Test:
    Filter: test_requirements_traceability_projection_is_byte_stable
    Level: integration
  Given a requirement with a satisfying spec, bound tests, and stored trace evidence
  When `requirements traceability` runs twice with `--format json --out`
  Then both files validate against `requirement-traceability-v1.schema.json`
  And the two runs are byte-identical

Scenario: Unknown requirement ids are rejected
  Test:
    Filter: test_requirements_traceability_rejects_unknown_id
    Level: integration
  Given no requirement document declares id `REQ-GHOST`
  When `requirements traceability` runs for `REQ-GHOST`
  Then the exit code is non-zero with a diagnostic naming the id

Scenario: Traceability leaves knowledge byte-identical
  Test:
    Filter: test_requirements_traceability_keeps_knowledge_byte_identical
    Level: integration
  Given a knowledge tree snapshot
  When `requirements traceability` runs
  Then every knowledge document is byte-identical afterwards
