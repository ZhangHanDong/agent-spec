# Changelog

All notable changes to `agent-spec` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-06-04

The **BDD-spine** release. agent-spec absorbs living-spec-library (OpenSpec) and
scaffolding/governance (Spec Kit) capabilities under one model — Discovery →
Formulation → Automation — while staying BDD-core and single-binary. Verdict
semantics are unchanged: the six verdicts (`pass`/`fail`/`skip`/`uncertain`/
`pending_review` + gate) and `is_passing` are untouched; every new check is a
sensor (lint / report / audit), never a silent change to pass/fail.

### Added

- **Rule → Example BDD semantics (Phase 1).** First-class `Rule:` / `规则:`
  grouping over scenarios, with a stable keystone identity `RuleKey { scope, id }`
  — `id` is stable kebab-case, the display `name` is mutable. `Example` is
  recognized as a synonym for `Scenario` (incl. `示例` / `例子`, fullwidth colon).
  Four `bdd-*` linters with agent-readable self-correction guidance.
- **Coverage matrix (Phase 2).** `agent-spec matrix` renders
  Rule × Scenario × Test × Verdict × Provenance (text / json / markdown).
  New `EvidenceProvenance` (Computational vs Inferential) stamped per result.
- **Capability level + promote (Phase 3).** `spec: capability` specs and
  `agent-spec promote`, lifting a passing task Rule into a capability spec
  **without changing its `id`**. Promotion gate requires ≥1 proving Example;
  capability names are path-traversal-checked.
- **Discovery questions (Phase 4).** `## Questions` / `问题` / `待澄清` sections
  parse to a structured `Questions` section; `open-question` lint (non-blocking).
- **lint-ack + dimensions (Phase 5).** `<!-- lint-ack: CODE reason -->` lets
  authors acknowledge a Warning/Info **with a mandatory reason** (never an Error);
  acknowledged counts stay visible. Lint codes classify into dimensions.
- **Single-source integrations (Phase 6).** `agent-spec gen-integrations`
  renders agents / cursor / claude integration files from one source, with a
  `--check` drift gate (write and check share the same renderer).
- **Probe abstraction (Phase 6.5).** Unifies evidence sources as
  `Test / Static / Benchmark / External / Inferential` (scaffolding; runner
  execution for Benchmark/External is deferred).
- **Structural check (Phase 7).** `agent-spec check-structure --forbid X --in glob`
  — mechanical layering guard (dependency-cruiser-lite); non-zero exit on violation.
- **Library health audit (Phase 8).** `agent-spec audit` aggregates spec/rule/
  scenario counts, unproven rules, ungrouped scenarios, open questions, malformed
  rules (text / json). Observability only — never gates.
- **Cold-start reverse spec (Phase 9).** `agent-spec discover --from-codebase`
  drafts a parseable task-spec skeleton from existing test functions (one bound
  scenario per test) plus a `## Questions` seed flagging it for human refinement.

### Changed

- README command table now documents the full BDD-spine command surface.
- `ReviewMode` now derives `Default` (`Auto`); no behavior change.

### Fixed

- Cleared `cargo clippy --all-targets --all-features -- -D warnings` (the CI gate):
  derivable impl, `slice::from_ref` over `&[x.clone()]` in tests, and missing
  `unwrap_used`/`expect_used` allows on two test modules.

### Notes

- 342 tests pass; `guard` 37/37 specs; clippy CI gate clean.
- Verification semantics and `is_passing` are unchanged from 0.2.x.

## [0.2.7] and earlier

See git history. 0.2.x established the core contract pipeline: `parse`, `lint`,
`verify`, `lifecycle`, `guard`, `explain`, `stamp`, `contract`, `plan`, `graph`,
the four verifier layers, run logging, and VCS-aware checkpoints.

[0.3.0]: https://github.com/ZhangHanDong/agent-spec/releases/tag/v0.3.0
