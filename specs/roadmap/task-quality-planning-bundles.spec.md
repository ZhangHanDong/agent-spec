spec: task
name: "Quality Planning and Execution Bundles"
tags: [intent-compiler, quality, execution-bundle]
satisfies: [REQ-QUALITY-PLANNING]
depends: [task-code-graph-ir-bindings]
estimate: 1w
---

## Intent

Deliver target-architecture boundary 4: typed quality provider roles with
normalized outcomes (pass/fail/unavailable/error/authorized-skip), array-based
provider configuration, and `requirements bundle` producing an Execution
Bundle (work unit + contract + code bindings + quality profile + required
skills + fast checks + acceptance gates) so agents receive one complete,
verifiable execution context.

## Decisions

- Provider roles are a typed enum (code-intelligence, diagnostic, verification, transformation, agent-guidance); adapters expose capability detection, scoped planning, execution, and normalization.
- Normalized outcomes: pass | fail | unavailable | error | skip(policy); required-provider unavailability never counts as passing evidence.
- Provider configuration is executable + argv arrays with explicit cwd, timeout, and output limits; no interpolated shell strings.
- `requirements bundle --unit WU-REQ-X --out <file>.json` emits the Execution Bundle; schema at `docs/intent-compiler/schemas/execution-bundle-v1.schema.json`.
- Skill receipts record id, version, source, and content hash; deterministic tool output and lifecycle verdicts remain the only acceptance evidence.

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/spec_verify/**
- src/main.rs
- docs/intent-compiler/**
- knowledge/requirements/req-quality-planning.md
- specs/roadmap/task-quality-planning-bundles.spec.md
- CHANGELOG.md

### Forbidden
- Do not let unavailable providers pass gates.
- Do not use interpolated shell command strings for provider configuration.
- Do not treat skill receipts as acceptance evidence.
- Do not add dependencies.

## Out of Scope

- Actual third-party adapters beyond cargo/clippy/rustfmt baselines
- Enterprise signing of bundles

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — roadmap draft; scenarios will be grouped under Rules when promoted -->
<!-- lint-ack: output-mode-coverage — bundle emission is asserted by the bundle scenario -->

Scenario: Bundle packages the full execution context
  Test:
    Filter: test_execution_bundle_packages_context
    Level: integration
  Given a ready work unit with bindings and a quality profile
  When `requirements bundle` runs
  Then the bundle records work unit, contract, bindings, profile, required skills, fast checks, and acceptance gates

Scenario: Unavailable required providers never pass
  Test:
    Filter: test_required_provider_unavailable_is_not_pass
    Level: integration
  Given a required verification provider that is unavailable
  When quality outcomes normalize
  Then the outcome is `unavailable`, the gate fails, and no passing evidence is recorded

Scenario: Skill receipts are not acceptance evidence
  Test:
    Filter: test_skill_receipt_is_not_acceptance_evidence
    Level: integration
  Given a bundle with skill receipts and no passing verification outcomes
  When acceptance evidence is evaluated
  Then the receipts alone produce no acceptance

Scenario: Malformed bundle requests are diagnostics
  Test:
    Filter: test_bundle_rejects_unknown_work_unit
    Level: integration
  Given an unknown work unit id
  When `requirements bundle` runs
  Then the command fails with an error diagnostic naming the unknown unit
