# Changelog

All notable changes to `agent-spec` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.2.0] - 2026-07-22

The **evidence-aware Atlas** release: Rust Atlas grows from a symbol graph into
an incrementally published, freshness-gated code evidence service, and
agent-spec connects affected code back to requirements, scenarios, tests,
quality policy, worktrees, and replayable failure evidence. External Code Graph
providers now have a standalone adapter SDK and conformance gate. Advanced MCP
and concurrent serving surfaces remain opt-in until real Agent A/B evidence is
accepted.

### Added

- Atlas F1 external Code Graph provider kit: a standalone Rust SDK defines
  strict provider manifests, opt-in project registrations, separate extraction
  and semantic-enrichment schemas, canonical projection, worktree/freshness
  validation, host-derived fingerprints, literal-argv bounded process execution,
  cancellation, and atomic publication. `atlas provider validate|conformance`
  exposes an eight-check local matrix without adding a non-Rust provider or
  changing Rust Atlas defaults.
- Atlas E1 adoption harness: strict versioned experiment, Agent A/B/C plan,
  receipt, and gate schemas separate Atlas primitive value from B5 context
  value; an independent four-profile direct/worker burst gate covers D4.
  Environment symmetry, at least three trials, failed-run retention,
  correctness/freshness-first decisions, raw-session hashes, complete query
  metrics, and baseline median/MAD comparisons are enforced. External runners
  require explicit executables and remain outside default tests. No real E1
  receipt is checked in and no MCP, B5, or worker default is promoted.
- Atlas D4 concurrent query serving: an opt-in fixed worker pool, bounded queue
  and memory admission, pinned generation leases, cooperative timeout/cancel,
  one panic retry plus circuit state, isolated maintenance/control lanes,
  daemon protocol v2, direct/worker/fallback CLI modes, and a hidden concurrent
  MCP `atlas_context` route. A strict 20-run fixture receipt covers four B5
  load profiles, all seven typed outcomes, publish/stop behavior, and two
  worktrees. Correctness gates semantic parity, snapshot identity, bounds, and
  lease cleanup; latency, heartbeat, CPU, RSS, and response bytes remain
  measurements. Direct execution and default MCP discovery remain unchanged
  pending E1 real Agent A/B.
- Atlas B5 query context compiler: additive `atlas context` profiles separate
  scored retrieval from relevance and byte-bounded projection. Results carry
  hash-verified source spans, required-evidence protection, omission manifests,
  fingerprint-bound continuation argv, dual-loss receipts, and deterministic
  D4 load profiles. E3 now validates context receipts and rebuilds all four
  profiles plus projection pressure offline; default MCP remains unchanged.
- Atlas D3 optional live runtime: a shared build/watch scope, bounded platform
  watcher, persisted pending watermark, separate writer/ordinary retry budgets,
  typed `warming`/`healthy`/`pending`/`degraded`/`unavailable` states, and a
  loopback identity handshake provide explicit local daemon supervision. Every
  query pins its immutable generation with a reader lease; single-writer
  reclamation retains active or ambiguous generations and cleans abandoned
  staging idempotently. MCP discovery remains static and no-daemon queries read
  the same graph facts. Live state remains derived and cannot replace graph
  freshness, KLL, Task Contract, or lifecycle authority.
- Atlas D2 incremental hardening: builds now publish immutable committed
  generations behind one atomic pointer; read surfaces pin and report the same
  generation. A content-addressed Cargo input plan, source-owned module
  reconstruction, bounded reverse-dependent frontier, explicit overflow
  fallback, recoverable orphan queue, batch cancellation, working-byte limit,
  and byte-inert zero-change fast path replace in-place graph refresh. A
  deterministic 10-case fixture matrix covers cold, zero, edit, delete,
  manifest, overflow, overlay, cancellation, commit failure, and recovery.
  Automatic refresh preserves committed Cargo inputs; capability changes use
  explicit full frontiers, shard phases stream by batch, and byte admission
  covers source, serialized graph, and overlays. Failed post-commit orphan
  cleanup remains recoverable. D3 composes these primitives without changing
  their publication contract.
- Atlas disconnected `flow` queries now explain fresh Rust runtime boundaries
  for async tasks, channels, callback registries, reflection, and framework
  routes. The bounded AST-derived results are explicitly heuristic query hints:
  they do not mutate graph shards or participate in impact, affected, binding,
  lifecycle, or archive authority. The E3 live fixture scores their expected
  continuation, source evidence, and exact diagnostic. Scans bind to a unique
  function AST so same-line siblings cannot leak sites, and Rust-relative
  candidate paths resolve from their source package, module, or impl context.
  Qualified signatures share canonicalization; qualified-self candidates retain
  type, trait, and generic identity. Receiver roles inspect only the AST access
  chain that produces the receiver, ignoring arguments, index values, and
  literals while requiring token boundaries. This avoids silent misses and
  suffix lookalikes such as `ctx`/`tx`. Stale source nodes and stale SCIP/MIR
  edges cannot expand the scan frontier, and default trait methods resolve
  lowercase `self`/`super` plus qualified `Self` candidates from the trait
  declaration. Framework-route hints preserve handlers from both two-argument
  `route` calls and one-argument `service` calls; generic reflection text
  resolves to its indexed type declaration without losing the displayed type.
  Bare candidates prefer source-module exact matches, and a shared per-file
  source cache applies node and byte limits before additional frontier reads.
  Callable associated paths resolve through canonical inherent-impl symbols;
  Rust type/value namespace filtering excludes impossible same-symbol nodes
  before candidate fan-out is measured.
- Atlas query-quality regression gate: a strict two-tier corpus combines
  deterministic fixtures with immutable true-repository revisions and scores
  ranked symbols, exact paths, required evidence, forbidden hits, ambiguity,
  stale/capability diagnostics, response bytes, latency, read-backs, and
  follow-up queries. `atlas benchmark score` emits a fingerprinted receipt and
  exits non-zero after preserving any failed cases. Default tests rebuild the
  Rust fixture and score current search/flow output without network access;
  fresh pinned-repository observations and real Agent A/B remain opt-in.
- Atlas Wave 2: deterministic compact/deep `atlas explore`, explainable
  `atlas flow`, reverse `atlas impact`, and changed-file `atlas affected`
  queries share bounded traversal, evidence paths, layered freshness, and
  worktree authority. Source excerpts require matching graph hashes, affected
  results do not infer tests from filenames, and `atlas_explore` is frozen and
  unavailable over MCP unless `AGENT_SPEC_MCP_ATLAS_EXPLORE=1`. Evaluation
  receipts now record response bytes, read-backs, follow-up queries, and
  truncation counts; no real Agent A/B improvement is claimed.
- Atlas Wave 1: schema v6 preserves edge occurrence/evidence facts, an atomic
  derived query index powers deterministic `atlas search`, and `atlas status`
  reports graph identity plus independent syn/SCIP/MIR freshness. Worktree
  mismatch and stale available semantic authority fail closed for definitive
  provider, binding, lifecycle-symbol, and typed trace evidence. Users rebuild
  with `atlas build` after schema or query-index diagnostics; `atlas check`
  remains the syn stale-file gate. MCP keeps `atlas_search` hidden by default;
  start `agent-spec mcp` with `AGENT_SPEC_MCP_ATLAS_SEARCH=1` to list it.
- Atlas evaluation baseline: versioned offline corpus and paired run-plan
  compilation, typed correctness receipts, robust per-arm and aggregate median/
  MAD summaries, and `atlas benchmark validate|plan|summarize`. `plan` and
  `summarize` honor an atomic `--out` contract. The separate opt-in runner
  requires `jq` and an explicit single executable in
  `ATLAS_EVAL_AGENT_COMMAND`; default commands do not invoke a model or network.
  The harness records evaluation inputs and results but does not claim an
  executed real-model benchmark or an Atlas performance improvement. See
  `docs/atlas-evaluation.md`.

### Changed

- `agent-spec` is now **1.2.0** and depends on `rust-atlas` **0.3.0**. Atlas
  graph storage advances from schema v4 to schema v6 to retain edge evidence,
  graph identity, committed generations, and query-index integrity. Existing
  `.agent-spec/graph` directories are rebuildable derived data: run
  `agent-spec atlas build --full` after upgrading rather than migrating shards.
- The provider-neutral SDK is published for the first time as
  `agent-spec-code-graph-provider` **0.1.0**. Publish it before `agent-spec` so
  the root crate's registry dependency is resolvable; `rust-atlas` remains an
  independent package and is published before the root crate as well.

## [1.1.0] - 2026-07-19

The **interop deepening** release: compiled requirements now flow both ways
with the reference compiler's native format, the code graph gains an
optional semantic layer from rust-analyzer, and the 1.0 book ships. All
changes are additive — the 1.0 compatibility promise holds untouched.

### Added

- **The agent-spec 1.0 Book** (`REQ-AGENT-SPEC-BOOK`, Chinese edition):
  a spec-driven mdbook under `book/` — preface with reading paths and a
  knowledge map, nineteen chapters across five parts (getting started,
  the contract, the intent compiler, knowledge & ecosystem, philosophy &
  architecture), and four appendices including the book's own contract
  (dogfood) and two end-to-end traces. Every chapter carries a
  positioning anchor, the 1.0.0 baseline, and at least one Mermaid
  diagram — all guarded by seven bound structure tests. Rendered with
  mdbook-mermaid; built at Pages deploy time (never committed) and
  served at `/book/`. English edition is declared follow-up work.

- Atlas SCIP semantic overlay (`REQ-INTENT-CODE-LINKER`, Phase 2 of
  `docs/atlas-roadmap.md`): rust-atlas accepts rust-analyzer's protobuf
  `index.scip` and layers `Calls` / `UsesType` / `References` edges over
  the syn baseline — additive `provenance=Scip` edges only, the offline
  syn layer is never mutated and survives incremental refresh. `ImplsTrait`
  / `ImplFor` resolve precisely via SCIP symbol descriptors (fixing syn's
  re-export/cross-crate blind spot). New `agent-spec atlas scip-gen`
  subcommand invokes rust-analyzer to produce the index. Phase 1 syn
  hardening ships alongside: bare-name unique-suffix resolution, exclude
  respect, five new node kinds (`Static`, `Union`, `TraitAlias`,
  `AssocConst`, `AssocType`), declaration-head signatures.

- ARC-native requirements dialect (`REQ-ARC-NATIVE-DIALECT`): agent-spec
  compiled requirements are now directly consumable as the reference
  compiler's input `requirements.yaml`, and reference-native trees import
  for governance. `requirements import` auto-detects the single-root
  ARC-native shape (with `root:`/`requirement:` wrapper tolerance) and maps
  `name`→title, ATOMIC `description`→clause statement, `steps` scenarios,
  and dotted ids (normalized with `source-id:` fidelity annotations); the
  subset parser gains folded/literal block scalars and empty flow lists.
  `requirements export --dialect arc-native [--root-name N]` projects the
  IR as a reference-loadable single-root tree, restoring dotted ids; the
  round-trip law holds (export → import → export byte-identical, verified
  even on dotted-id corpora). The verbatim reference ticketbooking file is
  a fixture and the parity input-conformance test now binds it.

### Changed

- rust-atlas crate bumped to **0.2.0** (graph `schema_version` 4): node
  ids now carry `#` disambiguation suffixes and the node-kind vocabulary
  grew — a breaking format change versus the published 0.1.0, versioned
  per the 1.0 compatibility promise (atlas `schema_version` gates the
  format; `### Symbols` bare-name references still resolve).

### Fixed

- Requirement clause extraction skips HTML comment paragraphs (e.g.
  `<!-- source-id: ... -->` annotations) instead of treating them as
  id-less clauses.

## [1.0.0] - 2026-07-13

**The compatibility promise begins.** Every surface in the README's
"Stability" section — the CLI command families, the machine formats (five
verdicts, `is_passing`, all schema `$id`s, YAML dialect v1.1, provenance
manifests v1/v2, traceability projection, compile bundle layouts,
code-bindings and execution-bundle schemas, typed trace targets, atlas
`schema_version`), and the governance semantics (requirement state machine,
execution ladder, derived-never-stored liveness) — now breaks only with a
major version.

### Removed

- The `brief` command (deprecated 0.4.0). `agent-spec contract` renders the
  identical output — an internal parity test keeps pinning the two shapes
  equal.

### Fixed

- 1.0 machine-format audit: schema file `$id`s unified to the resolvable
  `https://agent-spec.dev/schemas/agent-spec/intent-compiler/*.schema.json`
  form (artifact-embedded logical ids unchanged); documentation verdict
  counts corrected to the real five (`pass`, `fail`, `skip`, `uncertain`,
  `pending_review`); a stale `agent-spec/requirements-compiler/` namespace
  mention dropped from the stability promise.

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

[Unreleased]: https://github.com/ZhangHanDong/agent-spec/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/ZhangHanDong/agent-spec/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/ZhangHanDong/agent-spec/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/ZhangHanDong/agent-spec/compare/v0.6.0...v1.0.0
[0.6.0]: https://github.com/ZhangHanDong/agent-spec/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/ZhangHanDong/agent-spec/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/ZhangHanDong/agent-spec/releases/tag/v0.4.0
[0.3.0]: https://github.com/ZhangHanDong/agent-spec/releases/tag/v0.3.0
