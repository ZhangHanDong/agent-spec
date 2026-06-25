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
  trace               Trace a decision to satisfying specs and report liveness (KLL)
  lint-knowledge      Lint the knowledge corpus (per-doc + governance); text/json/sarif (KLL)
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

## Knowledge & Liveness Layer (KLL)

KLL adds a typed knowledge layer beside specs. Artifacts live under
`knowledge/` (one kind per subdirectory); specs stay in `specs/` and link back
with a `satisfies:` frontmatter edge. **Liveness is derived, never stored** —
recomputed from current spec verdicts on every `trace`.

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

Resolves a decision id (case-insensitive) to the specs that `satisfy:` it, runs
verification on each, and rolls the verdicts up into a **liveness** state.
`--gate` exits 2 on `violated` and warns (exit 0) on `unproven`.

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
`--gate` exits 2 on any Error-level finding. `--format sarif` emits SARIF 2.1.0
for GitHub Code Scanning. `README.md` and `*-template.md` are exempt
(self-referential exemption).

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
