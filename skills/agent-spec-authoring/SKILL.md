---
name: agent-spec-authoring
description: |
  CRITICAL: Use for writing and editing agent-spec .spec/.spec.md files. Triggers on:
  write spec, create spec, edit spec, new spec, spec authoring, task contract,
  .spec file, .spec.md file, BDD scenario, acceptance criteria, completion criteria,
  test selector, boundary, constraint, intent, decision, out of scope,
  "how to write a spec", "spec format", "spec syntax", "contract quality",
  写 spec, 创建规格, 编辑合约, 任务合约, 验收标准, 完成条件,
  BDD 场景, 测试选择器, 约束, 意图, 决策, 边界, 排除范围,
  "怎么写 spec", "spec 格式", "spec 语法", "合约质量"
---

# Agent Spec Authoring

> **Version:** 3.3.0 | **Last Updated:** 2026-06-08 | **Tracks agent-spec:** 0.3.0 (BDD-spine)

You are an expert at writing agent-spec Task Contracts. Help users by:
- **Creating specs**: Scaffold new `.spec.md` files with correct structure (`.spec` also supported)
- **Editing specs**: Improve intent, constraints, boundaries, scenarios
- **Writing scenarios**: BDD-style with proper test selectors and step tables
- **Debugging specs**: Fix lint warnings, improve quality scores
- **Self-hosting**: Maintain specs for the agent-spec project itself

## IMPORTANT: CLI Prerequisite Check

**Before running any `agent-spec` command, Claude MUST check:**

```bash
command -v agent-spec || cargo install agent-spec
```

If `agent-spec` is not installed, inform the user:
> `agent-spec` CLI not found. Install with: `cargo install agent-spec`

## Core Philosophy

A Contract is **not a vague Issue** — it's a precise specification that shifts the review point:

```
Traditional:  Human reviews 500 lines of code diff (slow, error-prone)
agent-spec:   Human writes 50-80 lines of Contract (fast, high-value)
              Machine verifies code against Contract (deterministic)
```

Writing a Contract is the **highest-value human activity** in the agent-spec workflow. You're defining "what is correct" — the machine handles "is the code correct".

## Quick Reference

| Section | Chinese Header | English Header | Purpose |
|---------|---------------|----------------|---------|
| Intent | `## 意图` | `## Intent` | What to do and why |
| Constraints | `## 约束` | `## Constraints` | Must / Must NOT rules |
| Decisions | `## 已定决策` / `## 决策` | `## Decisions` | Fixed technical choices |
| Boundaries | `## 边界` | `## Boundaries` | Allowed / Forbidden / Out-of-scope |
| Acceptance Criteria | `## 验收标准` / `## 完成条件` | `## Acceptance Criteria` / `## Completion Criteria` | BDD scenarios |
| Out of Scope | `## 排除范围` | `## Out of Scope` | Explicitly excluded items |
| Questions (Discovery) | `## 问题` / `## 待澄清` | `## Questions` | Unresolved items to clarify (Phase 4; non-blocking) |

## Hard Syntax Rules

- Use exactly one supported section header per line. Good: `## Intent` or `## 意图`. Bad: `## Intent / 意图`.
- Write scenarios as bare DSL lines under the acceptance section. Good: `Scenario:` / `场景:`. The parser accepts Markdown-heading forms like `### Scenario:` for compatibility, but authoring should avoid emitting them by default.
- Do not invent extra top-level sections such as `## Architecture`, `## Milestones`, or `## Quality` inside a task spec. Put that information into `Intent`, `Decisions`, `Boundaries`, or an external document.
- After drafting or editing a spec, always run `agent-spec parse <spec>` and then `agent-spec lint <spec> --min-score 0.7`.

## Documentation

Refer to the local files for authoring patterns and examples:
- `./references/patterns.md` - Complete authoring patterns with examples

## IMPORTANT: Documentation Completeness Check

**Before answering questions, Claude MUST:**
1. Read `./references/patterns.md` for authoring patterns
2. If file read fails: Inform user "references/patterns.md is missing, answering from SKILL.md patterns"
3. Still answer based on SKILL.md patterns + built-in knowledge

## Required Self-Check

After writing or editing a spec:

```bash
agent-spec parse specs/task.spec.md
agent-spec lint specs/task.spec.md --min-score 0.7
```

Do not hand a spec to an agent if:
- `agent-spec parse` shows `Acceptance Criteria: 0 scenarios`
- lint reports missing explicit test selectors
- lint score is below threshold

## Behavior Surface Checklist

When authoring a contract for CLI tools, MCP servers, protocols, or parity rewrites,
do not stop at the main happy path. Check these observable surfaces explicitly:

### Observable Behavior
- stdout vs stderr behavior
- `--json` or machine-readable output
- `-o/--output` and file side effects
- local vs remote behavior
- warm cache vs cold start
- fallback / precedence order
- partial failure vs hard failure
- on-disk state changes and persisted files

### Flag Combinations (lint: `flag-combination-coverage`)
- Multi-value parameters (multi-ID, batch) combined with output flags
- Single vs multiple entry behavior for `-o`, `--full`, `--json`
- If your command has 2+ output-affecting flags, add at least one scenario that tests a combination

### Platform-Specific Decisions (lint: `platform-decision-tag`)
- When copying decisions from a reference implementation, tag platform-specific terms
- Use markers like `[JS-only]`, `[platform-specific]`, or `不适用` to flag phantom requirements
- The linter flags untagged references to npm, pip, cargo install, dist/, bundled dist, etc.

### Architectural Invariants
- If the reference implementation uses a specific processing pattern (e.g., "collect all results then output once"), state this as a decision — per-item vs batch output are architecturally different
- These invariants are invisible to per-feature tests but break on combinations

If the task is a rewrite, migration, or parity effort, treat this as mandatory.
Do not hand the contract to an agent until these observable behaviors are either:
- covered by scenarios, or
- explicitly declared out of scope

For these tasks, prefer starting from the parity-aware scaffold instead of the generic task template:

```bash
agent-spec init --level task --template rewrite-parity --lang en --name "CLI Parity Contract"
```

## Before Writing a Contract

Not every task needs a Contract. Ask yourself:

| Question | If No |
|----------|-------|
| Can I define what "done" looks like? | Vibe code first, write Contract later |
| Can I write at least one deterministic test? | Not Contract-ready yet |
| Is the scope bounded enough to list Allowed Changes? | Split into smaller tasks |
| Do I know the key technical decisions? | Do a spike/prototype first |

If all "yes" — proceed with authoring. If not, doing exploratory work first is the right call.

## The Four Elements of a Contract

### 1. Intent — What and Why

One focused paragraph. Not a feature list — a clear statement of purpose.

```spec
## Intent

为现有的认证模块添加用户注册 endpoint。新用户通过邮箱+密码注册，
注册成功后发送验证邮件。这是用户体系的第一步，后续会在此基础上
添加登录和密码重置。
```

**Rules:**
- Focus on "what to do and why"
- Mention context (what already exists, where this fits)
- Keep it to 2-4 sentences
- Do not combine bilingual section labels on the same header line

### 2. Decisions — Fixed Technical Choices

Already-decided choices. Not aspirational. Not options to explore.

```spec
## Decisions

- 路由: POST /api/v1/auth/register
- 密码哈希: bcrypt, cost factor = 12
- 验证 Token: crypto.randomUUID(), 存数据库, 24h 过期
- 邮件: 使用现有 EmailService，不新建
```

**Rules:**
- Only choices that are **already fixed** — not "we should consider..."
- Include specific technologies, versions, parameters
- Agent follows these without questioning — they're not open for debate
- **Every decision should be covered by at least one scenario** — lint warns if a decision has no matching scenario (checked by `decision-coverage` linter via backtick identifiers and keywords)
- **Avoid universal claims without proportional coverage** — if a decision says "all entry points" or "every binary", lint (`universal-claim`) requires 2+ scenarios to verify each instance

### 3. Boundaries — What to Touch, What Not to Touch

Triple constraint: Allowed, Forbidden, Out-of-scope.

```spec
## Boundaries

### Allowed Changes
- crates/api/src/auth/**
- crates/api/tests/auth/**
- migrations/

### Forbidden
- 不要添加新的 npm/cargo 依赖
- 不要修改现有的登录 endpoint
- 不要在注册流程中创建 session

## Out of Scope

- 登录功能
- 密码重置
- OAuth 第三方登录
```

**Rules:**
- Path globs (`crates/auth/**`) are **mechanically enforced** by BoundariesVerifier
- Natural language prohibitions are checked by lint but not file-path enforced
- Out of Scope prevents scope creep — Agent knows what NOT to attempt
- **If Boundaries list 2+ entry points** (e.g. `bin/cli.rs`, `bin/server.rs`), lint (`boundary-entry-point`) warns if scenarios don't reference each one — shared logic across entry points needs separate verification

### 4. Completion Criteria — Deterministic Pass/Fail

BDD scenarios with explicit test bindings.

**Critical principle: Exception scenarios >= happy path scenarios.** Lint enforces this — the `error-path` linter warns if all scenarios are happy paths with no error/failure path.

```spec
## Completion Criteria

场景: 注册成功                                    ← 1 happy path
  测试: test_register_returns_201
  假设 不存在邮箱为 "alice@example.com" 的用户
  当 客户端提交注册请求:
    | 字段     | 值                |
    | email    | alice@example.com |
    | password | Str0ng!Pass#2026  |
  那么 响应状态码为 201
  并且 响应体包含 "user_id"

场景: 重复邮箱被拒绝                              ← exception path 1
  测试: test_register_rejects_duplicate_email
  假设 已存在邮箱为 "alice@example.com" 的用户
  当 客户端提交相同邮箱的注册请求
  那么 响应状态码为 409

场景: 弱密码被拒绝                                ← exception path 2
  测试: test_register_rejects_weak_password
  假设 不存在邮箱为 "bob@example.com" 的用户
  当 客户端提交密码为 "123" 的注册请求
  那么 响应状态码为 400

场景: 缺少必填字段                                ← exception path 3
  测试: test_register_rejects_missing_fields
  当 客户端提交缺少 email 字段的注册请求
  那么 响应状态码为 400
```

This forces you to think through edge cases **before coding begins**. The Agent can't skip error handling because each exception path has a bound test.

## Rewrite / Parity Contracts

For rewrite, migration, and parity tasks, write a behavior matrix before writing scenarios.
At minimum, ask whether the contract covers:

- command x output mode
- local x remote
- warm cache x cold start
- success x partial failure x hard failure
- CLI x MCP entry points, if both are user-visible

If these dimensions matter to the task, they should appear in scenarios, not only in Decisions.

## BDD-spine Authoring (0.3.0)

agent-spec 0.3.0 organizes authoring around Discovery → Formulation → Automation.
These constructs are additive — they never change verdict semantics. `Example` is
a synonym for `Scenario` (Cucumber alignment).

### Rule → Example grouping

Group related scenarios under a `Rule:` / `规则:` — a promise the system keeps,
proven by one or more Examples. A Rule has a **stable kebab-case id** (used for
references and promotion) and a mutable display name:

```spec
## Completion Criteria

### Rule: reject-invalid-input — 拒绝非法输入
场景: 空邮箱被拒绝
  测试: test_rejects_empty_email
  当 提交空邮箱
  那么 返回 400

场景: 弱密码被拒绝
  测试: test_rejects_weak_password
  当 提交密码 "123"
  那么 返回 400
```

- The id is the leading kebab-case token (`reject-invalid-input`); the text after
  `—` / `--` is the display name. **Never encode identity in the display name** —
  rename freely, the id is the anchor.
- `bdd-rule-id` lints malformed (non-kebab-case) ids; `bdd-rule-grouping` nudges
  ungrouped scenarios. A scenario binds to a Rule via `规则:` / by sitting under
  the Rule header.
- A Rule with no proving Example is "unproven" (surfaces in `audit`).

### Discovery: `## Questions`

Before a contract is fully formed, capture unresolved items in a `## Questions`
(`## 问题` / `## 待澄清`) section — bullet list. These are **non-blocking**
(`open-question` lint is Info/Warning, never an Error; they do NOT affect
`is_passing`). Mark resolved items with `[x]` / `[已解决]` / `RESOLVED`.

```spec
## Questions

- 折扣能否叠加?
- [x] 退款按折后价(已确认)
```

`agent-spec discover --from-codebase` seeds this section when reverse-engineering
a spec from tests — a cold-start draft is honestly "known-incomplete".

### lint-ack: acknowledging a warning with a reason

When a lint Warning/Info is a deliberate, justified exception, acknowledge it
inline **with a mandatory reason** instead of distorting the spec:

```spec
<!-- lint-ack: error-path 本任务是只读查询,无失败路径 -->
```

- Acknowledged lints are filtered from the report but **counted** (visible in
  `audit`) — the waiver is on the record, not silenced.
- **Errors can never be acknowledged** — only Warning/Info. A mechanical hard
  failure is not negotiable.

### capability specs and promotion

A matured, reusable Rule can be promoted out of a task spec into a capability
spec (`spec: capability`, the living-spec library) via
`agent-spec promote <task> --rule <id> --to <cap> --code .`. The promote gate
requires the Rule's Examples to pass (≥1 example). Authoring notes:

- Capability specs use header `spec: capability`; a task can declare which
  capability it contributes to with a `capability:` frontmatter field.
- Promotion preserves the Rule's `id` — task references stay valid.
- In a capability spec, an empty Rule (no Example yet) is allowed but flagged
  unproven by `audit`.

### Provenance (read when reviewing the matrix)

`agent-spec matrix` stamps each result's evidence provenance: `Computational`
(mechanical — tests, structural, boundary) vs `Inferential` (AI). When authoring,
prefer scenarios provable by Computational evidence; reserve AI-only scenarios for
genuinely non-mechanical intent, and never let Inferential evidence default to
pass.

## Spec File Structure

### Frontmatter (YAML)

```spec
spec: task                                    # Level: org, project, task
name: "Task Name"                             # Human-readable name
inherits: project                             # Parent spec (optional)
tags: [feature, api]                          # Tags for filtering
depends: [task-auth-base, task-db-migration]  # Spec dependencies (optional)
estimate: 1d                                  # Effort estimate (optional): 0.5d, 1d, 2d, 1w, 4h
---
```

- `depends`: list of spec file stems or spec names this spec depends on. Used by `agent-spec graph` to build the dependency DAG and critical path.
- `estimate`: effort estimate string. Used by `agent-spec graph` for critical path weighting and node labels.

### Three-Layer Inheritance

```
org.spec(.md) → project.spec(.md) → task.spec(.md)
```

| Layer | Scope | Example Content |
|-------|-------|-----------------|
| `org.spec.md` | Organization-wide | Coding standards, security rules, forbidden patterns |
| `project.spec.md` | Project-level | Tech stack decisions, API conventions, test requirements |
| `task.spec.md` | Single task | Intent, boundaries, specific acceptance criteria |

Both `.spec` and `.spec.md` extensions are recognized. `.spec.md` is preferred for new files (enables Markdown preview in editors and GitHub).

Constraints and decisions are **inherited downward**. Task specs inherit from project, which inherits from org.

## BDD Step Keywords

| English | Chinese | Usage |
|---------|---------|-------|
| `Given` | `假设` | Precondition |
| `When` | `当` | Action |
| `Then` | `那么` | Expected result |
| `And` | `并且` | Additional step (same type as previous) |
| `But` | `但是` | Negative additional step |

## Test Selector Patterns

### Simple selector

```spec
Scenario: Happy path
  Test: test_happy_path
  Given precondition
  When action
  Then result
```

### Structured selector (cross-crate)

```spec
Scenario: Cross-crate verification
  Test:
    Package: spec-gateway
    Filter: test_contract_prompt_format
  Given a task spec
  When verified
  Then passes
```

### Chinese equivalents

```spec
场景: 正常路径
  测试: test_happy_path

场景: 跨包验证
  测试:
    包: spec-gateway
    过滤: test_contract_prompt_format
```

## Step Tables

For structured inputs, use tables instead of inventing custom prose:

```spec
Scenario: Batch validation
  Test: test_batch_validation
  Given the following input records:
    | name  | email           | valid |
    | Alice | alice@test.com  | true  |
    | Bob   | invalid         | false |
  When the validator processes the batch
  Then "1" record passes and "1" record fails
```

## Boundary Patterns

### Machine-enforced (path globs)

```spec
### Allowed Changes
- crates/spec-parser/**
- tests/parser_contract.rs
```

BoundariesVerifier checks actual changed files against these globs.

### Natural language prohibitions

```spec
### Forbidden
- Do not break the existing JSON shape
- Do not introduce .unwrap()
```

Checked by lint, not mechanically enforced against file paths.

**Use both when needed.** Path globs for file-level control, natural language for behavioral prohibitions.

## Common Errors

| Lint Warning | Cause | Fix |
|-------------|-------|-----|
| `vague-verb` | "handle", "manage", "process", "处理" | Be specific: "validate email format" not "handle email" |
| `unquantified` | "fast", "efficient", "应该快速" | Add metrics: "respond within 200ms" not "respond quickly" |
| `testability` | Steps that can't be mechanically verified | Use observable assertions: "returns error code X" |
| `coverage` | Constraint with no covering scenario | Add a scenario that exercises the constraint |
| `determinism` | Non-deterministic step wording | Remove "should", "might"; use definitive assertions |
| `implicit-dep` | Missing `Test:` selector on scenario | Add `Test: test_name` or structured `Test:` block |
| `sycophancy` | Bug-finding bias language | Remove "find all bugs", "must find issues" |

## Authoring Checklist

Before handing a Contract to an Agent, verify:

| # | Check | Why |
|---|-------|-----|
| 1 | Intent is 2-4 focused sentences | Agent needs clear direction, not a novel |
| 2 | Decisions are specific (tech, version, params) | Agent shouldn't be choosing technology |
| 3 | Boundaries have path globs for Allowed Changes | Enables mechanical enforcement |
| 4 | Exception scenarios >= happy path scenarios | Forces edge-case thinking upfront |
| 5 | Every scenario has a `Test:` selector | Required for TestVerifier to run |
| 6 | Steps use deterministic wording | "returns 201" not "should return 201" |
| 7 | `agent-spec lint` score >= 0.7 | Quality gate before Agent starts |

## Deprecated Patterns (Don't Use)

| Deprecated | Use Instead | Reason |
|------------|-------------|--------|
| Scenarios without `Test:` | Always add `Test:` selector | Required for mechanical verification |
| Vague boundaries like "be careful" | Specific path globs or prohibitions | Must be mechanically checkable |
| "should" / "might" in steps | Definitive "returns" / "is" / "becomes" | Non-deterministic wording fails lint |
| `brief` command to preview | `contract` command | `brief` is a legacy alias |
| Only happy path scenarios | Include exception paths (>= happy) | Edge cases are where bugs live |

## Scenario DSL Extensions

### Critical tags (Goal Gate)

Mark must-pass scenarios with `critical` tag. Critical failures set `gate_blocked=true` and exit code 2.

```spec
场景: 用户注册成功（critical）
  标签: critical
  测试: test_register_returns_201
  ...
```

Name suffix `（critical）`/`(critical)` also works as shorthand.

### Review mode

Scenarios requiring human sign-off use `审核: human` / `Review: human`. Test pass → `pending_review` verdict.

```spec
场景: 安全审核
  审核: human
  测试: test_security_audit
  ...
```

`--review-mode auto` (default) treats as pass; `--review-mode strict` treats as non-passing.

### Optimize mode

Scenarios that represent optimization targets use `模式: optimize` / `Mode: optimize`. Pass → listed in `optimization_candidates`. Fail still blocks.

```spec
场景: 性能优化
  模式: optimize
  测试: test_performance_baseline
  ...
```

### Scenario dependencies

Use `前置:` / `Depends:` for execution order. Prerequisite fail → dependent auto-skipped.

```spec
场景: 用户登录
  前置: 用户注册
  测试: test_login
  ...
```

Circular dependencies are detected by lint.

## Dependency Graph Workflow

After writing multiple related specs, add `depends` and `estimate` to frontmatter, then visualize:

```bash
agent-spec graph --spec-dir specs
agent-spec graph --spec-dir specs --format svg > deps.svg
```

This helps identify the critical path and parallelizable work before starting implementation.

## Self-Hosting Rules

When authoring specs for the `agent-spec` project itself:

- Put task specs under `specs/`
- Roadmap specs go in `specs/roadmap/`, promote to `specs/` when active
- Update tests when DSL or verification behavior changes
- Preserve the six verdicts: `pass`, `fail`, `skip`, `uncertain`, `pending_review`
- Do not let a task spec rely on implicit test-name matching

## Escalation

**Authoring → Implementation**: Switch to `agent-spec-tool-first` after the Contract is drafted and passes `agent-spec lint` with score >= 0.7.

**Implementation → Authoring**: Switch back here if the Agent discovers during implementation that:
- A missing exception path needs to be added to Completion Criteria
- A Boundary is too restrictive and needs expanding
- A Decision was wrong and needs changing

Update the Contract first, re-lint, then resume implementation. The Contract is a living document until the task is stamped.
