# Code Live Wiki

This index is generated from article frontmatter. Read it before opening individual wiki pages.

## Architecture

- [Architecture](_architecture.md) — `Cargo.lock`, `Cargo.toml`, `build.rs`, `crates/rust-atlas/Cargo.toml`, `crates/rust-atlas/src/lib.rs`, `src/main.rs`
- [Rust Atlas Architecture](architecture/atlas.md) — `crates/rust-atlas/src/lib.rs`, `crates/rust-atlas/src/index.rs`, `crates/rust-atlas/src/generation.rs`, `crates/rust-atlas/src/input_plan.rs`, `crates/rust-atlas/src/incremental.rs`, `crates/rust-atlas/src/status.rs`, `crates/rust-atlas/src/traversal.rs`, `crates/rust-atlas/src/flow.rs`, `crates/rust-atlas/src/runtime_boundary.rs`, `crates/rust-atlas/src/impact.rs`, `crates/rust-atlas/src/affected.rs`, `crates/rust-atlas/src/explore.rs`, `src/main.rs`, `src/spec_mcp/tools.rs`, `specs/task-atlas-explore-flow-impact.spec.md`, `specs/task-atlas-runtime-boundary-hints.spec.md`, `specs/task-atlas-incremental-hardening.spec.md`

## Concept

- [Atlas Graph Authority](concepts/atlas-authority.md) — `crates/rust-atlas/src/status.rs`, `crates/rust-atlas/src/lib.rs`, `crates/rust-atlas/src/generation.rs`, `crates/rust-atlas/src/incremental.rs`, `crates/rust-atlas/src/explore.rs`, `crates/rust-atlas/src/flow.rs`, `crates/rust-atlas/src/runtime_boundary.rs`, `crates/rust-atlas/src/impact.rs`, `crates/rust-atlas/src/affected.rs`, `src/spec_knowledge/code_graph.rs`, `src/spec_verify/atlas_symbols.rs`, `docs/atlas-roadmap.md`, `docs/atlas-runtime-boundaries.md`, `docs/atlas-incremental-builds.md`
- [Cross-Project Wiki](concepts/cross-project-wiki.md) — `knowledge/requirements/req-cross-project-wiki.md`, `specs/task-cross-project-wiki.spec.md`, `src/spec_wiki/project_map.rs`, `src/spec_wiki/live.rs`
- [Intent Compiler](concepts/intent-compiler.md) — `knowledge/requirements/req-requirements-compiler-plan-dag.md`, `src/spec_knowledge/requirement_plan.rs`, `src/spec_knowledge/trace_ledger.rs`, `src/spec_knowledge/test_obligations.rs`, `src/spec_knowledge/worktrees.rs`, `src/main.rs`
- [Knowledge Liveness Layer](concepts/knowledge-liveness-layer.md) — `knowledge/requirements`, `src/spec_knowledge`
- [Lifecycle](concepts/lifecycle.md) — `src/spec_gateway/lifecycle.rs`, `src/spec_verify`
- [Task Contract](concepts/task-contract.md) — `README.md`, `AGENTS.md`, `skills/agent-spec-tool-first/SKILL.md`
- [Trace And Replay](concepts/trace-replay.md) — `src/spec_knowledge/trace_ledger.rs`, `src/spec_knowledge/trace.rs`
- [Wiki Working Memory](concepts/wiki-working-memory.md) — `skills/agent-spec-wiki/SKILL.md`, `.agent-spec/wiki/_index.md`

## Decision

- [Atlas Derived Authority](decisions/atlas-derived-authority.md) — `crates/rust-atlas/src/lib.rs`, `crates/rust-atlas/src/index.rs`, `crates/rust-atlas/src/status.rs`, `crates/rust-atlas/src/flow.rs`, `crates/rust-atlas/src/runtime_boundary.rs`, `src/spec_mcp/tools.rs`, `docs/atlas-roadmap.md`, `docs/atlas-runtime-boundaries.md`
- [Deterministic CLI](decisions/deterministic-cli.md) — `specs/task-code-live-wiki.spec.md`, `specs/task-code-live-wiki-deepening.spec.md`, `src/spec_wiki`
- [Knowledge Versus Docs](decisions/knowledge-vs-docs.md) — `skills/agent-spec-wiki/SKILL.md`, `AGENTS.md`
- [Wiki Path](decisions/wiki-path.md) — `knowledge/requirements/req-code-live-wiki.md`, `knowledge/requirements/req-code-live-wiki-deepening.md`, `.gitignore`

## External-project

- [agent-spec](projects/agent-spec.md) — `Cargo.toml`, `src/spec_wiki`
- [codewiki](projects/codewiki.md) — `knowledge/requirements/req-code-live-wiki.md`, `src/spec_wiki/live.rs`
- [symposium](projects/symposium.md) — `knowledge/requirements/req-code-live-wiki.md`, `specs/task-code-live-wiki.spec.md`, `src/spec_wiki/architecture.rs`

## Module

- [Code Live Wiki](modules/code-live-wiki.md) — `src/spec_wiki/live.rs`, `src/spec_wiki/architecture.rs`, `src/spec_wiki/model.rs`, `src/main.rs`, `knowledge/requirements/req-code-live-wiki.md`, `knowledge/requirements/req-code-live-wiki-deepening.md`, `specs/task-code-live-wiki.spec.md`, `specs/task-code-live-wiki-deepening.spec.md`, `skills/agent-spec-wiki/SKILL.md`
- [Intent Compiler](modules/intent-compiler.md) — `src/spec_knowledge/parser.rs`, `src/spec_knowledge/requirement_graph.rs`, `src/spec_knowledge/work_units.rs`, `src/spec_knowledge/requirement_plan.rs`, `knowledge/requirements/req-kll-work-units.md`, `knowledge/requirements/req-requirements-compiler-plan-dag.md`, `specs/task-requirements-compiler-plan-dag.spec.md`
- [Main CLI](modules/main-cli.md) — `src/main.rs`
- [Spec Archive](modules/spec-archive.md) — `src/spec_archive.rs`
- [Spec Knowledge](modules/spec-knowledge.md) — `src/spec_knowledge`
- [Spec Lint](modules/spec-lint.md) — `src/spec_lint`
- [Spec Parser](modules/spec-parser.md) — `src/spec_parser`
- [Spec Verify](modules/spec-verify.md) — `src/spec_verify`
- [Spec Wiki](modules/spec-wiki.md) — `src/spec_wiki`
- [Verification Lifecycle](modules/verification-lifecycle.md) — `src/spec_verify/mod.rs`, `src/spec_verify/boundaries.rs`, `src/spec_verify/test_verifier.rs`, `src/main.rs`, `AGENTS.md`, `skills/agent-spec-tool-first/SKILL.md`

## Patterns

- [Patterns](_patterns.md) — `Cargo.lock`, `Cargo.toml`, `build.rs`, `crates/rust-atlas/Cargo.toml`, `crates/rust-atlas/src/lib.rs`, `src/main.rs`

## Project-flow

- [agent-spec adapts codewiki methodology](flows/agent-spec-to-codewiki.md) — `knowledge/requirements/req-code-live-wiki.md`, `src/spec_wiki/live.rs`, `skills/agent-spec-wiki/SKILL.md`
- [agent-spec adapts symposium metadata model](flows/agent-spec-to-symposium.md) — `knowledge/requirements/req-code-live-wiki.md`, `specs/task-code-live-wiki.spec.md`, `src/spec_wiki/architecture.rs`

