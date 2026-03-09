# agent-spec

[![Crates.io](https://img.shields.io/crates/v/agent-spec.svg)](https://crates.io/crates/agent-spec)
[![docs.rs](https://docs.rs/agent-spec/badge.svg)](https://docs.rs/agent-spec)
[![CI](https://github.com/ZhangHanDong/agent-spec/actions/workflows/contract-guard.yml/badge.svg)](https://github.com/ZhangHanDong/agent-spec/actions/workflows/contract-guard.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`agent-spec` is an AI-native BDD/spec verification tool for task execution.

The core idea is simple:

- humans review the contract
- agents implement against the contract
- the machine verifies whether the code satisfies the contract

The primary planning surface is the **Task Contract**. The older `brief` view remains available as a compatibility alias, but new workflows should use `contract`.

## Task Contract

A task contract is a structured spec with four core parts:

- `Intent`: what to do, and why
- `Decisions`: technical choices that are already fixed
- `Boundaries`: what may change, and what must not change
- `Completion Criteria`: BDD scenarios that define deterministic pass/fail behavior

The DSL supports English and Chinese headings and step keywords.

## Example

```spec
spec: task
name: "User Registration API"
tags: [api, contract]
---

## Intent

Implement a deterministic user registration API contract that an agent can code against
and a verifier can check with explicit test selectors.

## Decisions

- Use `POST /api/v1/users/register` as the only public entrypoint
- Persist a new user only after password hashing succeeds

## Boundaries

### Allowed Changes
- crates/api/**
- tests/integration/register_api.rs

### Forbidden
- Do not change the existing login endpoint contract
- Do not create a session during registration

## Completion Criteria

Scenario: Successful registration
  Test: test_register_api_returns_201_for_new_user
  Given no user with email "alice@example.com" exists
  When client submits the registration request:
    | field    | value             |
    | email    | alice@example.com |
    | password | Str0ng!Pass#2026  |
  Then response status should be 201
  And response body should contain "user_id"
```

Chinese authoring is also supported:

```spec
## 意图
## 已定决策
## 边界
## 完成条件

场景: 全额退款保持现有返回结构
  测试: test_refund_service_keeps_existing_success_payload
  假设 存在一笔金额为 "100.00" 元的已完成交易 "TXN-001"
  当 用户对 "TXN-001" 发起全额退款
  那么 响应状态码为 202
```

## Workflow

### 1. Author a task contract

Start from a template:

```bash
cargo run -q --bin agent-spec -- init --level task --lang en --name "User Registration API"
```

Or study the examples in [`examples/`](examples).

### AI Agent Skills

This repo ships three agent skills under [`skills/`](skills):

- **`agent-spec-tool-first`**: the default integration path — tells the agent to use `agent-spec` as a CLI tool and drive tasks through `contract`, `lifecycle`, and `guard`.
- **`agent-spec-authoring`**: the authoring path — helps write or revise Task Contracts in the DSL.
- **`agent-spec-estimate`**: the estimation path — maps Task Contract elements (scenarios, decisions, boundaries) to round-based effort estimates.

#### Install for Claude Code

```bash
# Copy to your global skills directory
cp -r skills/agent-spec-tool-first ~/.claude/skills/
cp -r skills/agent-spec-authoring ~/.claude/skills/
cp -r skills/agent-spec-estimate ~/.claude/skills/
```

Or symlink for auto-updates:

```bash
ln -s "$(pwd)/skills/agent-spec-tool-first" ~/.claude/skills/
ln -s "$(pwd)/skills/agent-spec-authoring" ~/.claude/skills/
ln -s "$(pwd)/skills/agent-spec-estimate" ~/.claude/skills/
```

#### Install for Codex

The equivalent guidance for Codex lives in [`AGENTS.md`](AGENTS.md). Copy it to your project root:

```bash
cp AGENTS.md /path/to/your/project/
```

#### Install for Cursor

Copy [`.cursorrules`](.cursorrules) to your project root.

#### Workflow

1. Use `agent-spec-tool-first` to inspect the target spec and render `agent-spec contract`.
2. Implement code against the rendered Task Contract.
3. Run `agent-spec lifecycle` for the task-level gate.
4. Run `agent-spec guard` for repo-level validation when needed.

This keeps the main integration mode tool-first. Library embedding remains available for advanced Rust-host integration, but it is not the default path.

### 2. Render the contract for agent execution

```bash
cargo run -q --bin agent-spec -- contract specs/my-task.spec
```

Use `--format json` if another tool or agent runtime needs structured output.

### 3. Run the full quality gate

```bash
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --format json
```

`lifecycle` runs:

- lint
- verification
- reporting

The run fails if:

- lint emits an `error`
- any scenario fails
- any scenario is still `skip` or `uncertain`
- the quality score is below `--min-score`

### 4. Use the repo-level guard

```bash
cargo run -q --bin agent-spec -- guard --spec-dir specs --code .
```

`guard` is intended for pre-commit / CI use. It lints all specs in `specs/` and verifies them against the current change set.

### 5. Contract Acceptance (replaces Code Review)

```bash
cargo run -q --bin agent-spec -- explain specs/my-task.spec --code . --format markdown
```

`explain` renders a reviewer-friendly summary of the Contract + verification results. Use `--format markdown` for direct PR description paste. Use `--history` to include retry trajectory from run logs.

The reviewer judges two questions: (1) Is the Contract definition correct? (2) Did all verifications pass?

### 6. Stamp for traceability

```bash
cargo run -q --bin agent-spec -- stamp specs/my-task.spec --code . --dry-run
```

Outputs git trailers (`Spec-Name`, `Spec-Passing`, `Spec-Summary`) for the commit message. Currently only `--dry-run` is supported.

## Explicit Test Binding

Task-level scenarios should declare an explicit `Test:` / `测试:` selector.

```spec
Scenario: Duplicate email is rejected
  Test: test_register_api_rejects_duplicate_email
```

If package scoping matters, use the structured selector block:

```spec
Scenario: Duplicate email is rejected
  Test:
    Package: user-service
    Filter: test_register_api_rejects_duplicate_email
```

```spec
场景: 超限退款返回稳定错误码
  测试:
    包: refund-service
    过滤: test_refund_service_rejects_refund_exceeding_original_amount
```

This is the default quality rule for self-hosting and new task specs. The older `// @spec:` source annotation is still accepted as a compatibility fallback, but it should not be the primary authoring path.

## Boundaries And Change Sets

`Boundaries` can contain both natural-language constraints and path constraints. Path-like entries are mechanically enforced against a change set.

Examples:

```spec
## Boundaries

### Allowed Changes
- crates/spec-parser/**
- crates/spec-gateway/src/lifecycle.rs

### Forbidden
- tests/golden/**
- docs/archive/**
```

The relevant commands accept repeatable `--change` flags:

```bash
cargo run -q --bin agent-spec -- verify specs/my-task.spec --code . --change crates/spec-parser/src/parser.rs
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --change crates/spec-parser/src/parser.rs
```

Single-task commands also support optional VCS-backed change discovery:

```bash
cargo run -q --bin agent-spec -- verify specs/my-task.spec --code . --change-scope staged
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --change-scope worktree
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --change-scope jj
```

Available scopes: `none` (default for verify/lifecycle), `staged`, `worktree`, `jj`.

When a `.jj/` directory is detected (even colocated with `.git/`), use `--change-scope jj` to discover changes via `jj diff --name-only`. The `stamp` command also outputs a `Spec-Change:` trailer with the jj change ID, and `explain --history` shows file-level diffs between adjacent runs via jj operation IDs.

## AI Verifier Skeleton

`agent-spec` now includes a minimal AI verifier surface intended to make `uncertain` results explicit and inspectable before a real model backend is wired in.

The relevant commands accept:

```bash
cargo run -q --bin agent-spec -- verify specs/my-task.spec --code . --ai-mode stub
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --ai-mode stub
```

Available modes:

- `off`: default, preserves the current mechanical-verifier-only behavior
- `stub`: turns otherwise-uncovered scenarios into `uncertain` results with `AiAnalysis` evidence
- `caller`: the calling Agent acts as the AI verifier (two-step protocol)

`caller` mode enables the Agent running `agent-spec` to also serve as the AI verifier. When `lifecycle --ai-mode caller` finds skipped scenarios, it writes `AiRequest` objects to `.agent-spec/pending-ai-requests.json`. The Agent reads the requests, analyzes each scenario, writes `ScenarioAiDecision` JSON, then calls `resolve-ai --decisions <file>` to merge decisions back into the report.

`stub` mode does not claim success. It is only a scaffold for:

- explicit `uncertain` semantics
- structured AI evidence in reports
- future integration of a real model-backed verifier

Internally, the AI layer now uses a pluggable backend shape:

- `AiRequest`: structured verifier input
- `AiDecision`: structured verifier output
- `AiBackend`: provider abstraction used by `AiVerifier`
- `StubAiBackend`: built-in backend for deterministic local behavior

No real model provider is wired in yet. The current value is that the contract/reporting surface is now stable enough to add a real backend later without redesigning the verification pipeline.

Provider selection and configuration are intentionally out of scope for `agent-spec` itself. The intended embedding model is:

- the host agent owns provider/model/auth/timeout policy
- the host agent injects an `AiBackend` into `spec-gateway`
- `agent-spec` stays focused on contracts, evidence, and verification semantics

`guard` resolves change paths in this order:

1. explicit `--change` arguments
2. auto-detected git changes according to `--change-scope`, if the current workspace is inside a git repo
3. an empty change set, if no git repo is available

`guard` defaults to `--change-scope staged`, which keeps pre-commit behavior stable.

If you want stronger boundary checks against the full current workspace, use:

```bash
cargo run -q --bin agent-spec -- guard --spec-dir specs --code . --change-scope worktree
```

`worktree` includes:

- staged files
- unstaged tracked changes
- untracked files

This makes `guard` practical for both pre-commit usage and broader local worktree validation without forcing users to enumerate changed files manually.

For consistency, `verify` and `lifecycle` use the same precedence when `--change-scope` is provided. The practical default is:

- `verify`: `none`
- `lifecycle`: `none`
- `guard`: `staged`

## Commands

- `parse`: parse `.spec` files and show the AST
- `lint`: analyze spec quality
- `verify`: verify code against a single spec
- `contract`: render the Task Contract view
- `lifecycle`: run lint + verify + report
- `guard`: lint all specs and verify them against the current change set
- `explain`: generate a human-readable contract review summary (for Contract Acceptance)
- `stamp`: preview git trailers for a verified contract (`--dry-run`)
- `resolve-ai`: merge external AI decisions into a verification report (caller mode)
- `checkpoint`: preview VCS-aware checkpoint status
- `install-hooks`: install git hooks for automatic checking
- `brief`: compatibility alias for `contract`
- `measure-determinism`: [experimental] measure contract verification variance

## Examples

See [`examples/`](examples):

- [`examples/user-registration-contract.spec`](examples/user-registration-contract.spec)
- [`examples/refactor-payment-service.spec`](examples/refactor-payment-service.spec)
- [`examples/refund.spec`](examples/refund.spec)
- [`examples/no-unwrap.spec`](examples/no-unwrap.spec)

## Current Status

The current system is strongest when the contract can be checked by:

- explicit tests selected from `Completion Criteria`
- structural checks
- boundary checks against an explicit or staged change set

More advanced verifier layers can still be added, but the current model is already sufficient for self-hosting `agent-spec` with task contracts.

## Contributing

agent-spec is self-bootstrapping: the project uses itself to govern its own development. When you contribute, you follow the same Contract-driven workflow that agent-spec teaches.

### The contribution flow

Every change starts with a Task Contract. Before writing code, create a `.spec` file in `specs/` that defines what you're building — the intent, the technical decisions that are already fixed, the files you'll touch, and the BDD scenarios that define "done." Then implement against the Contract and verify with `lifecycle`.

```bash
# 1. Create a task contract for your change
agent-spec init --level task --lang en --name "my-feature"
# Edit the generated spec: fill in Intent, Decisions, Boundaries, Completion Criteria

# 2. Check that the contract itself is well-written
agent-spec lint specs/my-feature.spec --min-score 0.7

# 3. Implement your change

# 4. Verify against the contract
agent-spec lifecycle specs/my-feature.spec --code . --change-scope worktree --format json

# 5. Run the repo-wide guard before committing
agent-spec guard --spec-dir specs --code .

# 6. Generate the PR description
agent-spec explain specs/my-feature.spec --code . --format markdown
```

The `guard` pre-commit hook is installed via `agent-spec install-hooks`. It checks all specs in `specs/` against your staged changes — your commit will be blocked if any contract fails.

### Project-level rules

The file `specs/project.spec` defines constraints that every task spec inherits. Read it before writing your first Contract — it tells you what the project enforces globally (e.g. "all public CLI behavior must have regression tests," "verification results must distinguish pass/fail/skip/uncertain").

### Roadmap specs

Future work lives in `specs/roadmap/`. These are real Task Contracts but they are not checked by the default `guard` run. When a roadmap spec is ready for implementation, promote it to the top-level `specs/` directory. See `specs/roadmap/README.md` for the promotion rule.

### Using AI agents to contribute

If you use Claude Code, Codex, Cursor, or another AI coding agent, install the skills from the [`skills/`](skills) directory (see [AI Agent Skills](#ai-agent-skills) above).

The `agent-spec-tool-first` skill tells the agent to read the Contract first, implement within its Boundaries, run `lifecycle` to verify, and retry on failure without modifying the spec. The `agent-spec-authoring` skill helps the agent draft or revise Task Contracts in the DSL.

For agents without skill support, the project includes `AGENTS.md` (Codex), `.cursorrules` (Cursor), and `.aider.conf.yml` (Aider) with the essential command reference.

### What we review

Pull requests are evaluated through Contract Acceptance, not line-by-line code review. The reviewer checks two things: is the Contract definition correct (does it capture the right intent and edge cases), and did all verifications pass (lifecycle reports all-green). If both are yes, the PR is approved.

This means the quality of your Contract matters as much as the quality of your code. A well-written Contract with thorough exception-path scenarios is a stronger contribution than clever code with a thin spec.
