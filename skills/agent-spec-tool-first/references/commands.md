# agent-spec CLI Command Reference

## All Commands

```
agent-spec <COMMAND>

Commands:
  parse               Parse .spec/.spec.md files and show AST
  lint                Analyze spec quality (detect smells)
  verify              Verify code against specs
  matrix              Render the coverage matrix (Rule × Scenario × Test × Verdict × Provenance)
  audit               Audit a spec library's health (counts, unproven rules, open questions)
  discover            Reverse-engineer a draft task spec from a codebase's existing tests
  check-structure     Mechanical structural check: forbid a reference within a file glob
  gen-integrations    Generate per-tool integration files from a single source
  promote             Promote a passing task Rule into a capability spec (living-spec library)
  init                Create a starter .spec.md file (or --workspace for the knowledge tree)
  trace               Trace a decision/requirement to satisfying specs and report liveness (KLL)
  lint-knowledge      Lint the knowledge corpus (per-doc + governance); text/json/sarif (KLL)
  requirements        Import, validate, plan, and draft from KLL requirements
  archive             Archive completed specs with latest passing lifecycle evidence
  wiki                Maintain a repo-local code live wiki with source trace and architecture inventory
  mcp                 Serve the knowledge layer over MCP (read-only, stdio) (KLL)
  lifecycle           Run full lifecycle: lint -> verify -> report
  brief               Compatibility alias for the contract view
  contract            Render an explicit Task Contract for agent execution
  guard               Git guard: lint all specs + verify against change scope
  graph               Generate dependency graph from spec files (DOT/SVG)
  explain             Generate a human-readable contract review summary
  stamp               Preview git trailers for a verified contract
  checkpoint          Preview or create a VCS checkpoint
  resolve-ai          Merge external AI decisions into a verification report
  measure-determinism [Experimental] Measure contract verification determinism
  install-hooks       Install git hooks for automatic spec checking
```

## Core Flow

```bash
# 1. Read the contract
agent-spec contract specs/task.spec

# 2. Implement code...

# 3. Verify
agent-spec lifecycle specs/task.spec --code . --format json

# 4. Repo-wide guard
agent-spec guard --spec-dir specs --code .
```

## contract

```bash
agent-spec contract <spec> [--format text|json]
```

Renders the Task Contract with: Intent, Must/Must NOT, Decisions, Boundaries, Completion Criteria.

## lifecycle

```bash
agent-spec lifecycle <spec> --code <dir> \
  [--change <path>]... \
  [--change-scope none|staged|worktree|jj] \
  [--ai-mode off|stub] \
  [--min-score 0.6] \
  [--format text|json|md] \
  [--run-log-dir <dir>] \
  [--adversarial] \
  [--layers lint,boundary,test,ai,complexity] \
  [--resume[=conservative]] \
  [--review-mode auto|strict]
```

Full pipeline: lint -> verify -> report. Default format is `json`.

`lifecycle` honors `--format json` (machine-readable) and `--format md`/`markdown`;
any other value (including `compact`/`diagnostic`) renders as plain text. Use
`--format json` for retry-loop parsing.

New flags:
- `--resume` — skip already-passed scenarios (incremental mode)
- `--resume=conservative` — rerun all but detect regressions
- `--review-mode auto` (default) — treat `pending_review` as pass
- `--review-mode strict` — treat `pending_review` as non-passing

## guard

```bash
agent-spec guard \
  [--spec-dir specs] \
  [--code .] \
  [--change <path>]... \
  [--change-scope staged|worktree] \
  [--min-score 0.6]
```

Scans all `*.spec` and `*.spec.md` files in `--spec-dir`, runs lint + verify on each. Default change scope is `staged`.

## verify

```bash
agent-spec verify <spec> --code <dir> \
  [--change <path>]... \
  [--change-scope none|staged|worktree] \
  [--ai-mode off|stub] \
  [--format text|json|md]
```

Raw verification without lint quality gate. Default change scope is `none`.

## explain

```bash
agent-spec explain <spec> \
  [--code .] \
  [--format text|markdown] \
  [--history]
```

Human-readable contract review summary. Use `--format markdown` for PR descriptions. Use `--history` to include run log history. In jj repos, `--history` also shows file-level diffs between adjacent runs via operation IDs.

## stamp

```bash
agent-spec stamp <spec> [--code .] [--dry-run]
```

Preview git trailers (`Spec-Name`, `Spec-Passing`, `Spec-Summary`). Currently only `--dry-run` is supported.

In jj repositories, also outputs `Spec-Change:` trailer with the current jj change ID.

## lint

```bash
agent-spec lint <files>... [--format text|json|md] [--min-score 0.0]
```

Built-in linters: VagueVerb, Unquantified, Testability, Coverage, Determinism, ImplicitDep, ExplicitTestBinding, Sycophancy.

## init

```bash
agent-spec init [--level org|project|task] [--name <name>] [--lang zh|en|both]
agent-spec init --workspace
```

`--workspace` ignores the spec flags and instead scaffolds the canonical KLL
knowledge tree (`knowledge/decisions|requirements|proposals|guidance|context`,
`knowledge/standards/canon/artifact-types.md`, `.agent-spec/config.yaml`), each
with a README + template. Idempotent: existing files are left untouched.

## Change Set Defaults

| Command | `--change-scope` default |
|---------|-------------------------|
| verify | `none` |
| lifecycle | `none` |
| guard | `staged` |

## resolve-ai

```bash
agent-spec resolve-ai <spec> \
  [--code .] \
  --decisions <decisions.json> \
  [--format text|json]
```

Merges external AI decisions into a verification report. Used as step 2 of the caller mode protocol:
1. `lifecycle --ai-mode caller` emits pending requests to `.agent-spec/pending-ai-requests.json`
2. Agent analyzes scenarios and writes `ScenarioAiDecision` JSON
3. `resolve-ai` merges decisions, replacing Skip verdicts with AI verdicts

The decisions file format:
```json
[
  {
    "scenario_name": "场景名称",
    "model": "claude-agent",
    "confidence": 0.92,
    "verdict": "pass",
    "reasoning": "All steps verified"
  }
]
```

Cleans up `pending-ai-requests.json` after successful merge.

## AI Mode

- `off` (default) - No AI verification layer
- `stub` - Returns `uncertain` for all scenarios (testing/scaffolding)
- `caller` - Agent-as-verifier: emits `AiRequest` JSON, resolved via `resolve-ai`
- `external` - Reserved for host-injected `AiBackend` trait implementations

## Verification Layers

Use `--layers` to select which verification layers to run:

```bash
# Only lint and boundary checking
agent-spec lifecycle specs/task.spec --code . --layers lint,boundary

# Skip lint, run structural + boundary + test
agent-spec lifecycle specs/task.spec --code . --layers boundary,test
```

Available layers: `lint`, `boundary`, `test`, `ai`, `complexity`

## graph

```bash
agent-spec graph \
  [--spec-dir specs] \
  [--format dot|svg]
```

Scans all spec files in `--spec-dir`, extracts `depends` and `estimate` from frontmatter, and generates a DOT dependency graph.

- Nodes use `box` shape (pending) or `doubleoctagon` (completed, tagged `done`/`completed`)
- Node labels include spec name + estimate (e.g., `"Goal Gate\n[0.5d]"`)
- Edges represent dependency relationships
- Critical path edges highlighted in red (`color=red, penwidth=2.0`)
- `--format svg` pipes DOT through system `dot` command (requires graphviz installed)

Example:

```bash
# Generate DOT and view
agent-spec graph --spec-dir specs/roadmap

# Generate SVG
agent-spec graph --spec-dir specs/roadmap --format svg > deps.svg
```

## BDD-spine Commands (0.3.0)

Additive commands from the BDD-spine release. Verdict semantics and `is_passing`
are unchanged; these are sensors (lint / report / audit), not new gates.

### matrix

```bash
agent-spec matrix <SPEC> \
  --code <CODE> \
  [--change <PATH>] [--change-scope none|staged|worktree] \
  [--ai-mode off|stub|caller] \
  [--format text|json|markdown]
```

Renders the coverage matrix: **Rule × Scenario × Test × Verdict × Provenance**.
Provenance is `Computational` (mechanical evidence) vs `Inferential` (AI). Shares
`verify`'s change-set and ai-mode flags and default semantics. Scenarios with no
matching test surface as orphan rows.

### promote

```bash
agent-spec promote <SPEC> \
  --rule <RULE_ID> \
  --to <CAPABILITY_NAME> \
  --code <CODE>
```

Promotes a passing task Rule into `specs/capabilities/<name>.spec.md` (the
living-spec library). The promote gate requires the Rule's Examples to pass and
at least one Example to exist. The Rule's stable `id` is preserved across the
lift — only its scope changes (Task → Capability). The capability name is
path-traversal-checked.

### audit

```bash
agent-spec audit [--spec-dir specs] [--format text|json]
```

Mechanically aggregates spec-library health: `spec_count`, `rule_count`,
`scenario_count`, `unproven_rules` (Rules with no proving Example),
`ungrouped_scenarios` (scenarios under no Rule), `open_questions`,
`malformed_rules`. **Observability only — never gates / never changes exit code
on health.** Reuses the same resolved/malformed definitions as lint/parser.

### discover

```bash
agent-spec discover --from-codebase \
  --code <DIR> \
  --name <SPEC_NAME> \
  [--out <FILE>]
```

Reverse-engineers a draft task spec from existing Rust test functions: one
`测试:`-bound scenario per test, placeholder When/Then steps, plus a `## Questions`
seed flagging the draft as auto-generated and needing human refinement. The draft
is guaranteed parseable. Cold-start aid only — it is NOT a finished contract.
Prints to stdout unless `--out` is given.

### check-structure

```bash
agent-spec check-structure \
  --code <DIR> \
  --forbid <SUBSTRING> \
  --in <FILE_GLOB>
```

Mechanical layering guard (dependency-cruiser-lite): fails (non-zero exit) if any
file matching `--in` contains `--forbid`. Example: forbid `clients/**` from
referencing `crate::services`:

```bash
agent-spec check-structure --code src --forbid crate::services --in "clients/**"
```

`**` matches across directories, `*` matches a single path segment.

### gen-integrations

```bash
agent-spec gen-integrations \
  [--target agents|cursor|claude|all] \
  [--out <DIR>] \
  [--check] \
  [--with-guidance <KNOWLEDGE_DIR>]
```

Generates per-tool integration files (agents / cursor / claude) from a single
source. `--check` compares on-disk files to what would be generated and exits
non-zero on drift — use it as a CI drift gate. Write and check share the same
renderer, so "check passes" is equivalent to "write is a no-op".

`--with-guidance <knowledge>` projects the `knowledge/guidance/` artifacts (KLL)
into each generated file as a delimited `<!-- agent-spec:guidance -->` block —
scope, instructions, applies-to globs, and designated skills.

## Knowledge & Liveness Layer (0.4.0 KLL)

KLL adds a typed knowledge layer beside specs. Artifacts live under
`knowledge/` (one kind per subdirectory); specs stay in `specs/` and link back
with a `satisfies:` frontmatter edge to decisions or requirements.
**Liveness is derived, never stored** — recomputed from current spec verdicts on
every `trace`.

### Artifact kinds

| Kind | Dir | Required sections | Notes |
|------|-----|-------------------|-------|
| `decision` | `knowledge/decisions/` | `## Context · ## Decision · ## Consequences` | MADR shape; Accepted ⇒ `## Alternatives Considered`; Consequences must name both sides |
| `requirement` | `knowledge/requirements/` | `## Problem · ## Requirements` | `[REQ-NNN] … MUST/SHOULD/MAY …` clauses; BCP-14 / ISO-29148 / EARS quality lint (warnings) |
| `guidance` | `knowledge/guidance/` | `## Scope · ## Instructions` | `## Applies To` globs + `## Skills`; `liveness: n/a`; projected via `gen-integrations --with-guidance` |
| `proposal` | `knowledge/proposals/` | `## Context · ## Decision · ## Consequences` | `liveness: n/a`; `## Produces: ADR-NNN` edge to spawned decisions |
| context | `knowledge/context/` | — | free-form, untyped, unlinted escape hatch |

Knowledge frontmatter (identity only):

```yaml
---
kind: decision            # decision | requirement | guidance | proposal
id: ADR-001               # canonical; falls back to <letters>-<digits> filename prefix
title: "Decision title"   # required for generated requirement work units
status: Accepted          # Proposed | Accepted | Superseded | Deprecated | Rejected
supersedes: ADR-000       # optional
liveness: auto            # auto (verifiable) | n/a (governance, never code-gated)
tags: [rust]              # optional; used by guidance scoping
---
```

The spec→decision edge, in spec frontmatter:

```yaml
satisfies: [ADR-001, REQ-002]
```

### trace

```bash
agent-spec trace <ID> \
  [--knowledge knowledge] \
  [--specs specs] \
  [--code .] \
  [--format text|json] \
  [--gate]
```

Resolves a decision or requirement id (case-insensitive) to the specs that
`satisfy:` it, runs verification on each, and rolls the verdicts up into a
**liveness** state. `--gate` exits 2 on `violated` and warns (exit 0) on
`unproven`.

**Liveness ladder** (precedence, total and mutually exclusive):
1. declared `n/a` → `na`
2. any satisfying spec `Fail` → `violated`
3. none, or any not-`Pass` → `unproven`
4. all `Pass` → `honored`

### lint-knowledge

```bash
agent-spec lint-knowledge \
  [--knowledge knowledge] \
  [--format text|json|sarif] \
  [--gate]
```

Lints the whole knowledge corpus: per-doc rules (required sections + forcing
functions, by kind) **plus** governance integrity — `knowledge-id-conflict`
(Error), `supersession-dangling` (Error), `supersession-target-not-marked`
(Warning), `references-superseded` (Warning), `produces-dangling` (Warning).
Malformed knowledge files are reported as `knowledge-parse-error` (Error);
they are never silently dropped from the gate.
`--gate` exits 2 on any Error-level finding. `--format sarif` emits SARIF 2.1.0
for GitHub Code Scanning. `README.md` and `*-template.md` are exempt
(self-referential exemption).

### requirements

```bash
agent-spec requirements import \
  --from <PRD_OR_ISSUE_MD> \
  [--out knowledge/requirements] \
  [--check]

agent-spec requirements graph \
  [--knowledge knowledge] \
  [--format text|json] \
  [--gate]

agent-spec requirements work-units \
  [--knowledge knowledge] \
  [--out .agent-spec/work_units.json] \
  [--format text|json]

agent-spec requirements draft-specs \
  [--knowledge knowledge] \
  [--out specs/generated] \
  [--check]
```

Requirements intake is the PRD/issue → KLL → executable unit → Task Contract
bridge:

```text
PRD/issue
  -> knowledge/requirements/*.md
  -> requirements graph
  -> work_units.json
  -> specs/generated/task-*.spec.md
```

#### `docs/` vs `knowledge/`

`docs/` is for human-facing explanatory material: PRDs, issue writeups, design
notes, plans, retrospectives, tutorials, and background context. It can contain
raw source material, but it is not governed KLL truth: files there do not need
stable IDs, KLL frontmatter, or schema-valid sections.

`knowledge/` is for machine-consumable project truth. Typed artifacts under
`knowledge/decisions`, `knowledge/requirements`, `knowledge/guidance`, and
`knowledge/proposals` carry stable IDs and frontmatter, are checked by
`lint-knowledge`, can be served over MCP, and can be connected to specs through
`satisfies: [ADR-*|REQ-*]` for `trace` liveness.

Pipeline rule: keep raw PRD/issue prose in `docs/` if useful, but generate work
units and Task Contract drafts from imported `knowledge/requirements/*.md`, not
directly from raw prose. Exception: `knowledge/context/` is a free-form KLL
escape hatch served by MCP; it is intentionally untyped, unlinted, and not
trace-gated.

`import` consumes explicitly marked Markdown blocks (below) or the constrained
YAML dialect for `.yaml`/`.yml` sources (`docs/intent-compiler/yaml-frontend-v1.md`):

```markdown
<!-- agent-spec:requirement id=REQ-101 title="User Login" tags=auth,web source=issue:#123 -->
## Problem

Users with existing accounts need to authenticate.

## Requirements

[REQ-101] The authentication service MUST create a login session when valid
credentials are submitted.

## Scenarios

Scenario: Valid login
  Given the visitor has a valid persisted account
  When the visitor submits valid credentials
  Then the system establishes a login session

## Dependencies

- REQ-100

## Open Questions

None.
<!-- /agent-spec:requirement -->
```

Consumable requirement artifacts use `kind: requirement`, an explicit `id`, a
frontmatter `title`, `liveness: auto`, and these sections when known:
`## Problem`, `## Requirements`, `## Scenarios`, `## Dependencies`,
`## Child Requirements`, `## Source Trace`, and `## Open Questions`.

`graph --gate` fails on parse errors and Error-level graph diagnostics. Open
questions and missing leaf scenarios are warnings; work units with open questions
or no scenarios are blocked rather than emitted as ready implementation work.

`draft-specs` renders reviewable task spec drafts with `satisfies: [REQ-*]`.
Generated drafts intentionally use `pending_...` test selectors; `lifecycle` is
expected to fail until a human replaces them with real tests.

### mcp

```bash
agent-spec mcp \
  [--knowledge knowledge] \
  [--specs specs] \
  [--code .]
```

Serves the knowledge layer over MCP — JSON-RPC 2.0 over newline-delimited
stdio (`initialize`, `tools/list`, `tools/call`). Read-only and deterministic:
**no RAG, no embeddings, no model calls**. Six tools:

| Tool | Args | Returns |
|------|------|---------|
| `knowledge.find` | `id` \| `tag` \| `path` | matching artifacts (id, kind, status, path) |
| `knowledge.governing` | `path` | decisions guarding the path (via satisfying-spec boundaries) + live liveness |
| `liveness.status` | `id` | the trace report (satisfying specs + verdicts + liveness) |
| `spec.contract` | `name` | the task contract for a spec |
| `guidance.for` | `path` \| `stack` | guidance + designated skills for the scope |
| `context.read` | `path` (optional) | free-form context file, or the file listing |

`context.read` serves only regular files under `knowledge/context/`; path
traversal and symlinks are rejected before reading.

## Frontmatter: depends and estimate

Spec-level dependency and effort fields in frontmatter:

```yaml
spec: task
name: "检查点与增量重跑"
inherits: project
tags: [bootstrap, lifecycle, phase8]
depends: [task-goal-gate, task-context-fidelity]
estimate: 1d
---
```

- `depends`: list of spec file stems or spec names this spec depends on
- `estimate`: effort estimate string (`0.5d`, `1d`, `2d`, `1w`, `4h`)
- Both fields are optional; specs without them still work normally
- Used by `agent-spec graph` to generate dependency visualization and critical path

## Six Verdicts

| Verdict | Meaning | Action |
|---------|---------|--------|
| `pass` | Scenario verified | No action needed |
| `fail` | Scenario failed verification | Read evidence, fix code |
| `skip` | Test not found or not run | Check `Test:` selector matches a real test name |
| `uncertain` | AI stub / manual review needed | Review manually or enable AI backend |
| `pending_review` | Test passed but needs human review | Human reviews, or `--review-mode auto` treats as pass |

## Scenario DSL Extensions

### Critical tags (Goal Gate)

```spec
场景: 用户注册成功（critical）
  标签: critical
```

- `critical` scenarios failing → `gate_blocked=true` in JSON, exit code 2
- Name suffix `（critical）`/`(critical)` also works as shorthand

### Review mode

```spec
场景: 安全审核
  审核: human
```

- `审核: human` / `Review: human` → verdict becomes `pending_review` when test passes
- `--review-mode auto` (default): treats as pass; `--review-mode strict`: treats as non-pass

### Optimize mode

```spec
场景: 性能优化
  模式: optimize
```

- `模式: optimize` / `Mode: optimize` → scenario listed in `optimization_candidates` when pass
- Fail still blocks `passed: false` (optimize is a floor, not a ceiling)

### Scenario dependencies

```spec
场景: 用户登录
  前置: 用户注册
```

- `前置:` / `Depends:` → lifecycle executes in topological order
- Prerequisite fail → dependent scenario auto-skipped with evidence
- Circular dependencies detected by lint

## Intent Compiler Commands

```bash
agent-spec requirements plan \
  [--knowledge knowledge] \
  [--specs specs] \
  [--format text|json] \
  [--gate]

agent-spec requirements test-obligations \
  [--knowledge knowledge] \
  [--specs specs] \
  [--format text|json] \
  [--out .agent-spec/test_obligations.json]

agent-spec requirements worktrees \
  [--knowledge knowledge] \
  [--specs specs] \
  [--base main] \
  [--path-prefix ../agent-spec-worktrees] \
  [--format text|json] \
  [--out .agent-spec/worktrees.json]

agent-spec requirements trace REQ-123 \
  [--trace-dir .agent-spec/trace] \
  [--format text|json]

agent-spec requirements replay REQ-123 \
  [--trace-dir .agent-spec/trace] \
  [--format text|json]

agent-spec requirements explain-failure REQ-123 \
  [--trace-dir .agent-spec/trace] \
  [--format text|json]

agent-spec requirements trace-graph REQ-123 \
  [--trace-dir .agent-spec/trace] \
  [--format mermaid|json]

agent-spec requirements questions \
  [--knowledge knowledge] \
  [--specs specs] \
  [--format text|json]

agent-spec archive \
  [--spec-dir specs] \
  [--archive-dir .agent-spec/archive/specs] \
  [--summary knowledge/context/spec-archives.md] \
  [--run-log-dir .] \
  [--dry-run] \
  [--check]
```

`plan --gate` fails on parse errors and Error-level plan diagnostics such as dangling requirement dependencies, requirement cycles, and ready requirements without satisfying specs.

`test-obligations` emits requirement/scenario/test-selector obligations without reading implementation code. It carries QA class evidence requirements so Class A/B/C work can demand different review strength.

`worktrees` emits deterministic branch/path/spec entries for ready work units only. It skips blocked, missing-scenario, and grouping-only work units and does not mutate git state.

`trace` queries stored requirement-level trace records. `replay` reconstructs every scenario from the latest known run; one satisfied requirement owns all spec scenarios, while multi-requirement specs use KLL scenario names for disambiguation. `explain-failure` filters that chain to non-pass scenario evidence. `trace-graph` renders the same chain as Mermaid or JSON for visualization. These commands replay evidence, not LLM execution.

`questions` emits deterministic reverse interview prompts derived from blocking open questions and ambiguity diagnostics. It does not call a model and does not edit files.

`archive` summarizes completed specs and moves them out of the active specs scan path when not in `--dry-run` mode. A spec is archiveable only when it is tagged `done` or `completed` and the latest lifecycle run log under `--run-log-dir` is passing and matches the current canonical spec path and content fingerprint. Missing, failing, name-only, or stale lifecycle evidence is reported as an archive diagnostic, and all targets are preflighted before any move. Archived specs are historical evidence; default active `guard`, `trace`, and `requirements plan` scans do not treat them as current liveness guards.

For agent-spec itself, dogfood these commands against the repository's own KLL requirements and task specs before using fixtures as external demonstrations.

Command docs must preserve these exact terms for documentation tests: QA class, state-machine, reverse interview, active specs, dogfood.

## Documentation Engineering Commands

Use Lore-style doc types, canon, and operational review before publishing substantial agent-spec documentation. Run the pre-publish docs lint gate:

```bash
bash scripts/docs-lint.sh
```

The docs lint script runs Harper for English prose when installed, always runs
agent-spec's built-in Chinese docs lint, and also runs markdownlint and lychee
when installed. It is separate from KLL/spec gates: docs lint checks
readability, rendered preview, structure, style, and links; `lint-knowledge`,
`requirements plan`, `lifecycle`, and `trace` check machine-consumable truth
and behavior.

## wiki

```bash
agent-spec wiki init --code . --wiki .agent-spec/wiki
agent-spec wiki seed --code . --wiki .agent-spec/wiki
agent-spec wiki seed --code . --wiki .agent-spec/wiki --check
agent-spec wiki status --code . --wiki .agent-spec/wiki
agent-spec wiki query "intent compiler" --wiki .agent-spec/wiki
agent-spec wiki inspect src/spec_wiki/live.rs --code . --wiki .agent-spec/wiki
agent-spec wiki inventory --code . --format json
agent-spec wiki inventory --code . --format mermaid
agent-spec wiki index --wiki .agent-spec/wiki
agent-spec wiki lint --code . --wiki .agent-spec/wiki
agent-spec wiki check --code . --wiki .agent-spec/wiki
agent-spec wiki meta update --code . --wiki .agent-spec/wiki
```

`wiki init` scaffolds the code live wiki under `.agent-spec/wiki` and writes
`_index.md`, `_architecture.md`, `_patterns.md`, `_log.md`, `_meta.json`,
`architecture/inventory.json`, `architecture/workspace.mmd`, and
`architecture/modules.mmd`. `wiki seed` creates focused draft module, concept,
and decision pages without overwriting maintained articles; `--check` reports
missing seed pages without writing files. `wiki status` compares dirty, staged,
and untracked current worktree files against article `source_files`
frontmatter and reports stale articles. `wiki query` searches titles, tags,
source files, and article bodies. `wiki inspect <path>` lists related wiki
pages, KLL requirements, and task specs. `wiki inventory` emits the Rust architecture inventory
and module graphs from Cargo metadata when available and falls back to a
generic source inventory for non-Rust repositories. `wiki index` rebuilds
`_index.md`; `wiki lint` rejects missing required files, missing
`source_files`, unsafe source paths, broken internal links, and stale index
content. `wiki check` combines index freshness, lint, and current worktree stale
status. In CI clean checkouts it is the tracked wiki structure gate. `wiki meta
update` records the current repository metadata.

The wiki is agent working memory, not KLL truth and not human docs. Durable
requirements still live in `knowledge/`, executable contracts live in `specs/`,
and published prose lives in `docs/`. Track `.agent-spec/wiki/**` when the
project wants live wiki memory in git, but keep `.agent-spec/runs`,
`.agent-spec/trace`, temp files, and runtime state ignored. Use the code live
wiki before reading raw source broadly, then update the affected article and run
`wiki index` plus `wiki lint` or `wiki check`. Archive old wiki material into
`learnings/` or `archive/` summaries with source links instead of deleting it
abruptly. Non-goals: no built-in LLM long-form generation, no web UI, and no
replacement for KLL.
