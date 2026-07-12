# agent-spec Integration for Codex / OpenAI Agents

> This file provides Codex with the same guidance that Claude Code gets via `.claude/skills/`.
> Two workflows: **Tool-First** (using the CLI) and **Authoring** (writing .spec/.spec.md files).

---

## Part 1: Tool-First Workflow

### Core Mental Model

**Review point displacement**: Human attention moves from "reading code diffs" to "writing contracts".

```
Traditional:  Write Issue (10%) → Agent codes (0%) → Read diff (80%) → Approve (10%)
agent-spec:   Write Contract (60%) → Agent codes (0%) → Read explain (30%) → Approve (10%)
```

### Quick Reference

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `agent-spec init` | Scaffold new spec | Starting a new task |
| `agent-spec contract <spec>` | Render Task Contract | Before coding - read the execution plan |
| `agent-spec lint <files>` | Spec quality check | After writing spec |
| `agent-spec lifecycle <spec> --code .` | Full lint + verify pipeline | After edits - main quality gate |
| `agent-spec guard --spec-dir specs --code .` | Repo-wide check | Pre-commit / CI - all specs at once |
| `agent-spec explain <spec> --format markdown` | PR-ready review summary | Contract Acceptance |
| `agent-spec explain <spec> --history` | Execution history | See retry count |
| `agent-spec stamp <spec> --dry-run` | Preview git trailers | Traceability |
| `agent-spec verify <spec> --code .` | Raw verification only | Verify without lint gate |
| `agent-spec resolve-ai <spec> --decisions <file>` | Merge AI decisions | Caller mode |
| `agent-spec requirements graph --gate` | Validate KLL requirements and dependency graph | After importing PRD/issue requirements |
| `agent-spec requirements transition <ID> --to accepted` | Explicit human governance transition | Accepting/rejecting a proposed requirement |
| `agent-spec requirements export --out requirements.yaml` | YAML projection of confirmed requirements (round-trip fixpoint, `--check` drift gate) | Interop with YAML-world tooling; derived, never source of truth |
| `agent-spec requirements supersede <ID> --by <NEW>` | Atomic supersession pair | Replacing an accepted requirement |
| `agent-spec wiki status` | Check stale code live wiki articles | Before broad source reading |
| `agent-spec wiki query <text>` | Search tracked live wiki articles | Before opening many source files |
| `agent-spec wiki check` | Live wiki lint + worktree status gate | Pre-commit / CI for tracked wiki |
| `agent-spec atlas build/tree/query/refs/impls/check` | Rust project graph: query structure instead of grepping | Before broad source reading; `check` gates staleness |

### KLL Requirements Intake

Use this when raw PRD/issue material needs to become executable, verifiable,
traceable agent-spec work:

```bash
agent-spec requirements import --from docs/prd.md --out knowledge/requirements
agent-spec requirements import --from requirements.yaml --out knowledge/requirements
agent-spec lint-knowledge --knowledge knowledge --gate
agent-spec requirements graph --knowledge knowledge --format json --gate
agent-spec requirements work-units --knowledge knowledge --out .agent-spec/work_units.json
agent-spec requirements draft-specs --knowledge knowledge --out specs/generated
```

Imported candidates carry `status: proposed`; a human accepts them with
`requirements transition <ID> --to accepted` before work units become ready —
missing status fails the governance gate.

`requirements import` consumes explicit `<!-- agent-spec:requirement ... -->`
Markdown blocks, or the constrained YAML dialect when the source ends in
`.yaml`/`.yml` (see `docs/intent-compiler/yaml-frontend-v1.md`). It never
interprets raw prose. Marked blocks need
blocks with `id` and `title`. The generated draft specs include
`satisfies: [REQ-*]` and placeholder `pending_...` test selectors; lifecycle is
expected to fail until those selectors are replaced with real tests.

### Code Live Wiki

agent-spec can maintain a repo-local code live wiki from code, KLL artifacts,
Task Contracts, docs, archive summaries, and lifecycle trace evidence. The
default location is `.agent-spec/wiki`.

```bash
agent-spec wiki init --code . --wiki .agent-spec/wiki
agent-spec wiki seed --code . --wiki .agent-spec/wiki
agent-spec wiki seed --code . --wiki .agent-spec/wiki --check
agent-spec wiki status --code . --wiki .agent-spec/wiki
agent-spec wiki query "intent compiler" --wiki .agent-spec/wiki
agent-spec wiki inspect src/spec_wiki/live.rs --code . --wiki .agent-spec/wiki
agent-spec wiki inventory --code . --format json
agent-spec wiki inventory --code . --format mermaid
agent-spec wiki project-map --code . --wiki .agent-spec/wiki --format json --out .agent-spec/wiki/architecture/project-map.json
agent-spec wiki project-map --code . --wiki .agent-spec/wiki --format mermaid --out .agent-spec/wiki/architecture/project-map.mmd
agent-spec wiki inspect-project brain-rs --code . --wiki .agent-spec/wiki --format text
agent-spec wiki index --wiki .agent-spec/wiki
agent-spec wiki lint --code . --wiki .agent-spec/wiki
agent-spec wiki check --code . --wiki .agent-spec/wiki
agent-spec wiki meta update --code . --wiki .agent-spec/wiki
```

Wiki pages are tracked agent working memory under `.agent-spec/wiki/**`, not
KLL truth and not published docs. Keep `.agent-spec/runs`, `.agent-spec/trace`,
temporary files, and other runtime state ignored. Each article declares
`source_files`; `wiki status` reports stale pages when those files change,
including dirty, staged, and untracked worktree changes. Run `wiki query` before
broad source reading and `wiki inspect <path>` to list related wiki pages, KLL
requirements, and task specs.

`wiki seed` creates focused draft module, concept, and decision pages without
overwriting maintained articles. `wiki inventory` emits Rust architecture inventory
and module graphs when Cargo metadata is available. `wiki check` combines index
freshness, lint, and current worktree stale status; in CI clean checkouts it is
the tracked wiki structure gate: `agent-spec wiki check --code .
--wiki .agent-spec/wiki`. Old wiki content should move into a `learnings/` or
`archive/` summary with source links rather than being deleted abruptly.
Non-goals: no built-in LLM long-form generation, no web UI, and no replacement
for `knowledge/`.

#### Cross-Project Wiki

Use project articles when the main repository depends on another important
project. Project articles live under `.agent-spec/wiki/projects/*.md` and use
stable `project_id` values. Use flow articles under `.agent-spec/wiki/flows/*.md`
to document working mechanisms and data flow between projects.
Project and flow articles must be regular Markdown files; symlinks are rejected
so discovery, copying, and stale checks use one deterministic file boundary.
Every field shown in the examples is required and non-empty. Invalid
frontmatter lines, duplicate keys, and incomplete articles fail project-map
validation instead of producing partial architecture records.

`source_files` stay repo-local and participate in stale article checks.
`external_sources` record outside project paths, URLs, or repo identifiers as
evidence labels; agent-spec performs no external repository scan by default.

Project article example:

```md
---
title: "brain-rs"
type: external-project
project_id: brain-rs
repo: rust-agents/brain-rs
role: "Context provider"
interfaces: [cli, json]
protocols: [stdio]
status: active
source_files:
  - src/integration/brain.rs
external_sources:
  - https://example.invalid/rust-agents/brain-rs
---
# brain-rs
```

Flow article example:

```md
---
title: "agent-spec to brain-rs context flow"
type: project-flow
flow_id: agent-spec-to-brain
projects:
  - agent-spec
  - brain-rs
kind: calls
protocols: [stdio, json]
requirements:
  - REQ-CROSS-PROJECT-WIKI
specs:
  - specs/task-cross-project-wiki.spec.md
source_files:
  - src/integration/brain.rs
external_sources:
  - https://example.invalid/rust-agents/brain-rs/src/lib.rs
---
# agent-spec to brain-rs context flow
```

The `projects` list is ordered; each adjacent pair becomes a directed edge.
`requirements` and `specs` resolve inside the current repository. Put paths,
URLs, and repository identifiers from outside the current repository in
`external_sources` only.

The `wiki project-map` command builds project-map JSON or Mermaid output under
`.agent-spec/wiki/architecture/`. The maintained truth remains the project
articles and flow articles; `project-map.json` and `project-map.mmd` are derived
artifacts. Use `wiki inspect-project <project-id>` to list the project article,
related flows, protocols, requirements, specs, and external source labels.
`wiki lint` and `wiki check` also require both derived artifacts to match the
maintained articles exactly.

### The Seven-Step Workflow

1. **Human writes Task Contract** — structured spec with Intent, Decisions, Boundaries, Completion Criteria
2. **Quality gate** — `agent-spec lint specs/task.spec --min-score 0.7`
3. **Agent reads Contract** — `agent-spec contract specs/task.spec`
4. **Agent self-checks with lifecycle** — retry loop until all scenarios pass
5. **Guard gate** — `agent-spec guard --spec-dir specs --code .` (pre-commit / CI)
6. **Contract Acceptance** — `agent-spec explain specs/task.spec --format markdown` (human reviews)
7. **Stamp and archive** — `agent-spec stamp specs/task.spec --dry-run`

### Retry Protocol

When `lifecycle` fails:

1. Run: `agent-spec lifecycle <spec> --code . --format json`
2. Parse JSON output, find each scenario's `verdict` and `evidence`
3. For `fail`: the bound test ran and failed — read evidence, fix code
4. For `skip`: test not found — check `Test:` selector matches a real test name
5. For `uncertain`: AI verification pending — review manually or enable AI backend
6. **Fix code based on evidence. Do NOT modify the spec file.**
7. Re-run lifecycle
8. After 3 consecutive failures on the same scenario, stop and escalate to the human

### Verdict Interpretation

| Verdict | Meaning | Action |
|---------|---------|--------|
| `pass` | Scenario verified | No action needed |
| `fail` | Scenario failed verification | Read evidence, fix code |
| `skip` | Test not found or not run | Add missing test or fix selector |
| `uncertain` | AI stub / manual review needed | Review manually or enable AI backend |

**Key rule: `skip` != `pass`**. All four verdicts are distinct.

### Change Set Options

| Flag | Behavior | Default |
|------|----------|---------|
| `--change <path>` | Explicit file/dir for boundary checking | (none) |
| `--change-scope staged` | Git staged files | guard default |
| `--change-scope worktree` | All git working tree changes | (none) |
| `--change-scope jj` | Jujutsu VCS changes | (none) |
| `--change-scope none` | No change detection | lifecycle/verify default |

### AI Verification: Caller Mode

When `--ai-mode caller` is used, the calling Agent acts as the AI verifier:

**Step 1**: `agent-spec lifecycle specs/task.spec --code . --ai-mode caller --format json`
- Output includes `"ai_pending": true` and `"ai_requests_file"` if scenarios need AI review

**Step 2**: Read pending requests, analyze each scenario, write decisions JSON, then merge:
```bash
agent-spec resolve-ai specs/task.spec --code . --decisions decisions.json
```

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| Guard reports N specs failing | Specs have lint or verify issues | Run `lifecycle` on each failing spec |
| `skip` verdict | Test selector doesn't match | Check `Test:` / `Filter:` in spec |
| Quality score below threshold | Lint warnings | Fix vague verbs, add quantifiers |
| Boundary violation | Changed file outside allowed paths | Update Boundaries or revert change |
| Agent keeps failing | Criteria too vague or strict | Improve Completion Criteria |

---

## Part 2: Authoring Workflow

### Spec File Structure

```spec
spec: task           # Level: org, project, task
name: "Task Name"
inherits: project    # Parent spec (optional)
tags: [feature, api]
---

## Intent
One focused paragraph: what to do and why.

## Decisions
- Specific fixed technical choices (tech, version, params)

## Boundaries

### Allowed Changes
- src/module/**
- tests/**

### Forbidden
- Do not add new dependencies
- Do not modify existing public API

## Out of Scope
- Feature X (deferred to next task)

## Completion Criteria

Scenario: Happy path
  Test: test_happy_path
  Given precondition
  When action
  Then expected result

Scenario: Error path 1
  Test: test_error_case
  Given error condition
  When action
  Then error response
```

### Section Reference

| Section | Chinese Header | English Header | Purpose |
|---------|---------------|----------------|---------|
| Intent | `## 意图` | `## Intent` | What to do and why |
| Constraints | `## 约束` | `## Constraints` | Must / Must NOT rules |
| Decisions | `## 已定决策` / `## 决策` | `## Decisions` | Fixed technical choices |
| Boundaries | `## 边界` | `## Boundaries` | Allowed / Forbidden / Out-of-scope |
| Acceptance Criteria | `## 验收标准` / `## 完成条件` | `## Acceptance Criteria` / `## Completion Criteria` | BDD scenarios |
| Out of Scope | `## 排除范围` | `## Out of Scope` | Explicitly excluded items |

### BDD Step Keywords

| English | Chinese | Usage |
|---------|---------|-------|
| `Given` | `假设` | Precondition |
| `When` | `当` | Action |
| `Then` | `那么` | Expected result |
| `And` | `并且` | Additional step |
| `But` | `但是` | Negative additional step |

### Test Selector Patterns

Simple: `Test: test_name`

Structured:
```spec
Test:
  Filter: test_specific_name
```

Chinese equivalents:
```spec
测试: test_name

测试:
  过滤: test_specific_name
```

### Key Authoring Rules

1. **Exception scenarios >= happy path scenarios** — forces edge-case thinking upfront
2. **Every scenario must have a `Test:` selector** — required for mechanical verification
3. **Decisions must be specific** (tech, version, params) — Agent shouldn't choose technology
4. **Boundaries must have path globs** — enables mechanical enforcement
5. **Use deterministic wording** — "returns 201" not "should return 201"
6. **Lint score >= 0.7** before handing to Agent

### Three-Layer Inheritance

```
org.spec(.md) → project.spec(.md) → task.spec(.md)
```

Constraints and decisions inherit downward. Both `.spec` and `.spec.md` extensions are supported; `.spec.md` is preferred for new files (enables Markdown preview in editors and GitHub).

### Conventions

- Task specs live in `specs/`
- Roadmap specs go in `specs/roadmap/`, promote to `specs/` when active
- Verdicts: pass, fail, skip, uncertain — all four are distinct
- **skip ≠ pass**: skipped scenarios block the pipeline

### Intent Compiler Workflow

> Terminology: **Intent Compiler（意图编译器）** is what agent-spec as a whole does. Requirements are the compiler's intermediate representation (IR): intent → structured requirements → Task Contracts. The artifact layer keeps the requirement noun (`agent-spec requirements`, `knowledge/requirements/`, `REQ-*`) because it names the IR, not the compiler; historical file stems are unchanged.

For raw PRDs or issues, keep the raw source in `docs/`, import or author stable requirements under `knowledge/requirements/`, then run:

If the source is unstructured prose, use the `agent-spec-intent-compiler` skill to draft human-reviewed Candidate Requirement Block entries first. Those entries must carry source excerpts, confidence, scenarios, and open questions; the CLI imports accepted blocks only and does not silently interpret raw prose.

```bash
agent-spec lint-knowledge --knowledge knowledge --gate
agent-spec requirements graph --knowledge knowledge --format json --gate
agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
agent-spec requirements test-obligations --knowledge knowledge --specs specs --format json --out .agent-spec/test_obligations.json
agent-spec requirements worktrees --knowledge knowledge --specs specs --base main --path-prefix ../agent-spec-worktrees --out .agent-spec/worktrees.json
agent-spec requirements replay REQ-123 --format text
agent-spec requirements explain-failure REQ-123 --format json
agent-spec requirements trace-graph REQ-123 --format mermaid
agent-spec requirements questions --knowledge knowledge --specs specs --format json
agent-spec archive --spec-dir specs --archive-dir .agent-spec/archive/specs --summary knowledge/context/spec-archives.md --run-log-dir . --dry-run
```

The CLI is deterministic. Use AI only to draft candidate KLL requirements or ask reverse interview questions generated by `requirements questions`; human-reviewed KLL artifacts remain the source of truth. Requirement replay is evidence replay from stored lifecycle/trace records and must not be described as deterministic LLM replay. It returns every scenario from the latest run; a sole satisfied requirement owns all spec scenarios, while multi-requirement specs use KLL scenario names for disambiguation instead of a Cartesian product.

Treat compiler roots as trust boundaries: unsafe ids, malformed or duplicate frontmatter, a missing root, a directory-kind mismatch, and symlinked compiler inputs are blocking errors. `requirements plan` contains first-class requirement, work-unit, and spec nodes with explicit `satisfies` and `spec_depends` edges.

For agent-spec's own development, dogfood the same compiler workflow before relying on fixture examples: create or update the repository KLL requirement, keep the task spec's `satisfies:` link current, run `requirements plan --gate`, run lifecycle, and confirm replay/trace-graph can walk from requirement to evidence.

QA class gates and state-machine lint apply before implementation. Completed specs should be archived out of the active specs scan set after contract acceptance. Run `agent-spec archive --run-log-dir . --dry-run` first, review the compressed summary, then archive only specs tagged `done` or `completed` whose latest lifecycle evidence is still passing. Missing, failing, name-only, or stale lifecycle evidence blocks archive application and appears as an archive diagnostic. Passing archive evidence must match the current canonical spec path and content fingerprint; all archive targets are preflighted before any source is moved.

### Documentation Engineering

agent-spec adopts Lore-style documentation engineering for human-facing docs:
doc types, canon, operational checklists, and docs lint tooling. Use `docs/`
for reader-facing prose and `knowledge/` for machine-consumable truth. Run
`bash scripts/docs-lint.sh` before publishing substantial documentation changes.
KLL gates and docs gates are complementary: docs gates check readability,
structure, English prose style, Chinese project style, and links with Harper,
agent-spec's built-in Chinese docs lint, markdownlint, and lychee; KLL/spec
gates check traceability and behavior. Treat these checks as pre-publish review
for substantial documentation changes. CI uses `DOCS_LINT_REQUIRE_EXTERNAL=all`
so Harper, markdownlint, and lychee must all run.
