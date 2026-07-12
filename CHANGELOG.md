# Changelog

All notable changes to `agent-spec` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

## [0.6.0] - 2026-07-12

The **dual-IR convergence** release: all five delivery boundaries of the
target intent-compiler architecture are shipped. The Intent-Code Linker is
live — contracts pin real code symbols, lifecycle validates them against a
fresh Atlas graph, trace evidence carries typed stale-aware code targets,
and `requirements bundle` hands agents one complete, verifiable execution
context.

### Added

- Quality Planning and Execution Bundles (`REQ-QUALITY-PLANNING`, boundary
  4): typed provider roles (code-intelligence / diagnostic / verification /
  transformation / agent-guidance) with normalized outcomes (pass / fail /
  unavailable / error / authorized skip) — a required provider that is
  unavailable never counts as passing evidence. Provider configuration is
  executable + argv arrays with explicit cwd/timeout/output limits, never
  shell strings. `requirements bundle --unit WU-REQ-X --out <file>.json`
  emits the Execution Bundle: work unit, embedded contracts with digests,
  code bindings, quality profile, required skills with provenance receipts
  (never acceptance evidence), fast checks, and acceptance gates. Schema:
  `docs/intent-compiler/schemas/execution-bundle-v1.schema.json`.

- Intent-Code Linker, Atlas slice (`REQ-INTENT-CODE-LINKER`, boundary 3):
  Task Contracts declare code-graph references in a `### Symbols` boundary
  subsection (`- rust-atlas: <canonical::path>`); the lifecycle pipeline
  validates every reference against a fresh graph — `atlas-symbol-missing`
  for absent symbols, `atlas-stale` (before any lookup) for a lagging or
  missing graph, silence when everything resolves, and zero graph burden
  for contracts without symbols. Persisted trace evidence now records
  typed, stale-aware code targets (provider, node id, kind, file,
  provenance, graph fingerprint) alongside the untyped strings; the
  requirement-trace-ledger schema gains the additive `code_target_facts`
  field. Atlas access stays read-only and derived facts never become
  durable KLL truth.

- Code Graph IR bindings (`REQ-CODE-GRAPH-IR`, target-architecture boundary
  2): a provider-neutral `CodeGraphProvider` consumer contract (identity,
  staleness facts, graph fingerprint, symbol resolution) with Rust Atlas as
  the first provider, and `requirements bind` generating
  `.agent-spec/code-bindings.json` from ready work units' declared
  `### Symbols` contract entries. A stale graph blocks binding, naming the
  lagging files; unknown providers are diagnostics; bindings are derived
  working data — never KLL truth. Schema:
  `docs/intent-compiler/schemas/code-bindings-v1.schema.json`.

## [0.5.0] - 2026-07-12

The **orchestrator machine surface** release: agent-spec becomes consumable as
a deterministic compiler by any orchestrator — digest-bearing governance JSON,
a traceability projection, replayable v2 provenance, and per-requirement
compile bundles with an `arc-v1` reference-compat layout. Per ADR-001 the core
stays orchestrator-neutral: facts and digests only, approvals live outside.

### Added

- Reference-compiler parity layout (`REQ-REFERENCE-COMPILER-PARITY`):
  `requirements compile --out <dir>` emits per-requirement bundles
  (requirement document, draft task spec, traceability projection, and a
  compilation manifest carrying per-artifact digests plus a bundle digest)
  in the provider-neutral `agent-spec-v1` layout; `--layout arc-v1` projects
  the same content under reference-compatible file names
  (`<id>.requirements.md`, `<id>.spec.md`, `<id>.arc.traceability.json`,
  `<id>.arc.compilation.json`). Writes are atomic (a failed render writes
  nothing) and refuse to overwrite without `--force`. `verify-run` replays
  compile manifests per artifact. A mechanical vocabulary check keeps the
  reference token out of core schemas and the neutral layout.

- Provenance run hardening (`REQ-PROVENANCE-RUN-HARDENING`): compilation-run
  manifests v2 record the exact compiler build (crate version + git commit
  embedded at build time, `unknown` fallback) and the effective command
  configuration; `--out`-writing requirements commands (`graph`, `plan`,
  `work-units`, `test-obligations`, `traceability`) accept `--provenance`;
  `requirements verify-run --manifest <m>.json` replays the recorded
  compilation in memory and byte-compares against recorded digests, exiting
  non-zero naming each drifted output. `graph`/`plan` gain `--out`. v1
  manifests keep verifying unchanged. Schema:
  `docs/intent-compiler/schemas/compilation-provenance-v2.schema.json`.

- Compiler machine surface (`REQ-COMPILER-MACHINE-SURFACE`, ADR-001):
  `requirements transition`/`supersede` gain `--format json` emitting the
  post-rewrite blake3 document digests (facts only — no actor, authority,
  approval, or policy fields; external systems bind approvals to digests), and
  the new `requirements traceability <ID>` projects clauses → satisfying specs
  → scenarios → bound tests → latest recorded verdicts → derived liveness as
  one byte-stable JSON document (pure read over stored trace records). Schema:
  `docs/intent-compiler/schemas/requirement-traceability-v1.schema.json`.

## [0.4.0] - 2026-07-12

The **Knowledge & Liveness Layer (KLL)** release. agent-spec now has a durable
knowledge layer beside specs, so decisions and requirements can be traced to the
specs and tests that still guard them.

### Added

- `requirements status <ID>`: aggregate three-axis report (governance from KLL,
  execution ladder unplanned→planned→ready→active→verified→archived, liveness
  recomputed) with satisfying-spec evidence — target-architecture delivery
  boundary 5, first slice.
- `explain --history` now renders a tabular run history with per-run
  pass/fail/skip/uncertain counts and deltas against the previous run
  (completing the staged history-summary contract); all four remaining legacy
  roadmap contracts were finished and archived.

- Compilation provenance manifests (enterprise `*.compilation.json` alignment):
  opt-in `--provenance <path>.json` on `requirements import`/`export` binds the
  direction, blake3 input/output digests, tool identity, dialect schema version,
  and a reproducibility result; `verify_provenance` recomputes the chain and
  reports drift. Schema: `docs/intent-compiler/schemas/compilation-provenance-v1.schema.json`.

- Intent Compiler YAML export: `requirements export --out <file>.yaml [--id REQ-*]`
  projects confirmed requirements into the import dialect (round-trip fixpoint,
  governance-status scoping with exclusion diagnostics, explicit lossiness report,
  `--check` drift gate). Dialect v1.1 adds FOLDER-level `dependencies`/`scenarios`
  to the frontend for symmetry.

- Requirement governance gate and explicit transitions (target-architecture
  delivery boundary 1): missing `status` is now an error in `requirements graph`
  and never schedules work; Markdown intake emits `status: proposed` candidates;
  `requirements transition <ID> --to <status>` applies the legal state machine
  with a line-precise frontmatter rewrite; `requirements supersede <OLD> --by <NEW>`
  updates both documents atomically. Compilation never mutates governance status.
- Intent-compiler target architecture (`docs/intent-compiler/architecture.md`):
  dual IR (Requirement IR + Code Graph IR), Intent-Code Linker, Quality Planning,
  Execution Bundles, and three independent state axes.

- Intent Compiler YAML frontend: `requirements import --from <file>.yaml` translates
  reference-style FOLDER/ATOMIC requirement trees into Requirement IR documents with
  a hand-written subset parser (no new dependencies), `source: imported-yaml`
  provenance, overwrite refusal for human-authored files, and byte-identical
  re-imports. Subset and mapping documented in `docs/intent-compiler/yaml-frontend-v1.md`.
- Intent Compiler planning docs for `requirements plan` and `requirements questions`.
- DORA-inspired docs for spec-derived test obligations, QA class gates, and state-machine transition coverage.
- Worktree execution manifest docs for mapping ready work units to parallel git worktrees.
- Requirement trace replay docs for locating a requirement's work unit, spec scenario, test selector, code targets, worktree, and VCS reference when lifecycle evidence fails.
- A spec archival workflow that compresses completed contracts into historical summaries outside the active scan set.
- `agent-spec wiki init|seed|status|query|inspect|inventory|index|lint|check` for a tracked repo-local code live wiki under `.agent-spec/wiki`.
- `agent-spec wiki project-map` and `wiki inspect-project` for deterministic cross-project wiki maps from maintained project articles and flow articles.
- Cross-project wiki hardening now validates flow identities and local trace references, uses explicit code roots for inspection, and gates missing or drifted project-map JSON and Mermaid artifacts.
- A reverse-interview skill boundary: AI can draft and ask, while CLI gates remain deterministic.
- Clarified that example fixtures are demonstrations and agent-spec development must dogfood the compiler workflow on its own KLL requirement and task spec.
- Typed knowledge artifacts under `knowledge/`: `decision`, `requirement`,
  `guidance`, and `proposal`.
- `satisfies:` frontmatter on specs, building the edge from knowledge artifacts
  to verifiable contracts.
- Derived liveness (`honored` / `violated` / `unproven` / `n/a`) recomputed from
  current spec verdicts and never stored.
- `agent-spec trace`, `agent-spec lint-knowledge`, `agent-spec mcp`,
  `agent-spec init --workspace`, and `gen-integrations --with-guidance`.
- `agent-spec requirements import|graph|work-units|draft-specs` for turning
  marked PRD/issue blocks into KLL requirements, executable work units, and
  draft Task Contracts with `satisfies: [REQ-*]`.
- SARIF output for knowledge lint findings.


### Deprecated

- `brief` (legacy alias of `contract`) — removal planned for 1.0.

### Changed

- Terminology unified: **Intent Compiler（意图编译器）** is agent-spec's product
  positioning, renamed across docs, specs, skills, CLI help prose, and identifiers —
  the intake skill is now `agent-spec-intent-compiler`, planning docs live under
  `docs/intent-compiler/`, and schema `$id` URIs use `agent-spec/intent-compiler/`.
  Requirements are the compiler's intermediate representation (IR), so the artifact
  layer keeps the requirement noun: `agent-spec requirements`, `knowledge/requirements/`,
  and `REQ-*` ids are semantically correct, not legacy. Historical contract file stems
  are unchanged.
- External reference-project vocabulary removed from specs, knowledge, plans, and
  the validation matrix (now `docs/intent-compiler/reference-validation-matrix.md`);
  the README acknowledgement is the single remaining mention.
- 16 shipped roadmap contracts (phase0–6, plan-command, goal-gate, checkpoint-resume,
  complexity-gate, human-review, optimize-scenario-mode, scenario-dependencies,
  spec-dependency-graph, scenario-verification-metadata) verified passing and
  archived to `.agent-spec/archive/specs/`; archived contracts are now tracked in git.

### Fixed

- `apply_archive_plan` now rolls back already-moved files when creating an
  archive target directory fails, matching the rename-failure rollback path.
- Intent Compiler inputs now reject unsafe ids, malformed frontmatter, missing roots, kind mismatches, and symlink traversal; plan output includes real spec nodes and cross-layer edges.
- Requirement trace uses explicit scenario ownership, replay returns the complete latest run, and Error-level trace diagnostics no longer count as QA evidence.
- Cargo test filters that execute zero tests now produce `skip` instead of a false `pass`.
- Archive evidence is bound to the current spec path and content fingerprint, and archive moves are preflighted before mutation.
- Removed the superseded generated `docs/wiki` command path and implementation so the CLI exposes only the code live wiki model.
- Cross-project wiki validation now rejects incomplete or malformed project and
  flow articles, and `wiki init --check` no longer hides missing maintained
  directories by recreating them only in its temporary wiki.
- `lint-knowledge --gate` now reports malformed knowledge files as
  `knowledge-parse-error` instead of silently dropping them.
- `context.read` rejects symlinks from `knowledge/context/`.
- `trace` finds decisions recursively under `knowledge/decisions/**`.
- `trace` reports parse errors for matching malformed decision files instead of
  treating them as missing decisions.
- MCP knowledge/guidance tools and `gen-integrations --with-guidance` now report
  malformed knowledge artifacts instead of silently returning partial results.
- Knowledge SARIF metadata uses the package repository URL.

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

[0.4.0]: https://github.com/ZhangHanDong/agent-spec/releases/tag/v0.4.0
[0.3.0]: https://github.com/ZhangHanDong/agent-spec/releases/tag/v0.3.0
