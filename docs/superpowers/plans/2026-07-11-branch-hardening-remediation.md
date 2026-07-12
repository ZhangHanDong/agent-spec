# Branch Hardening Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for every production change.

**Goal:** Close the security, false-green, DAG, QA, trace, archive, symlink, parser, and legacy-Wiki findings from the full `feat/knowledge-liveness-layer` review.

**Architecture:** Introduce shared strict validation at compiler boundaries, propagate diagnostics through every lowering pass, represent specs as real plan-DAG nodes, and bind persisted evidence to exact spec bytes. Keep deterministic CLI behavior and add no dependencies.

**Tech Stack:** Rust 2024, clap, serde/serde_json, existing agent-spec lifecycle and KLL modules.

## Global Constraints

- Add no dependencies.
- Reject invalid input before writing files.
- A gate must fail when a required root is missing or unreadable.
- Generated paths must remain inside their declared output root.
- Symlinked repository inputs must not be traversed.
- Existing verdict distinctions remain unchanged.
- Every production fix starts with a failing regression test.

---

### Task 1: Strict KLL IDs, Frontmatter, and Input Roots

**Files:**
- Modify: `src/spec_knowledge/parser.rs`
- Modify: `src/spec_knowledge/governance.rs`
- Modify: `src/spec_knowledge/intake.rs`
- Modify: `src/spec_knowledge/draft_specs.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produce `validate_knowledge_id(&str) -> Result<(), String>` and strict directory-kind collection diagnostics.
- Ensure import and draft output paths are checked before any write.

- [ ] Add tests rejecting absolute/traversing IDs, duplicate/malformed frontmatter, directory-kind mismatches, and missing roots.
- [ ] Run each targeted test and confirm it fails.
- [ ] Implement validation and diagnostic propagation.
- [ ] Re-run targeted tests and KLL/requirements gates.

### Task 2: Complete Requirement/Work-Unit/Spec DAG and QA Gate

**Files:**
- Modify: `src/spec_knowledge/requirement_plan.rs`
- Modify: `src/spec_knowledge/test_obligations.rs`
- Modify: `src/spec_knowledge/worktrees.rs`
- Modify: `src/spec_qa.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Plan edges include requirement -> work unit -> spec and spec dependency edges.
- Unknown `satisfies` targets and ambiguous multi-spec coverage are diagnostics.
- Lifecycle/guard enforce QA evidence required by `risk`.

- [ ] Add failing tests for satisfies/spec-depends edges, dangling coverage, propagated diagnostics, invalid risk classes, and Class A/B enforcement.
- [ ] Implement real DAG nodes/edges and QA evidence checks.
- [ ] Verify plan schemas and fixture goldens against the new output.

### Task 3: Exact Trace Replay and Evidence-Safe Archive

**Files:**
- Modify: `src/spec_knowledge/trace_ledger.rs`
- Modify: `src/spec_archive.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Scenario-to-requirement trace is explicit and never inferred as a Cartesian product.
- Replay returns every record from the latest run.
- Run logs carry spec path and content fingerprint; archive requires an exact match.

- [ ] Add failing multi-requirement/multi-scenario trace tests.
- [ ] Add failing archive tests for edited specs, duplicate names, target collisions, and partial-apply prevention.
- [ ] Implement explicit trace mapping and preflighted archive application.
- [ ] Re-run lifecycle trace and archive tests.

### Task 4: Symlink-Safe Walkers and Legacy Wiki Removal

**Files:**
- Modify: `src/spec_wiki/sources.rs`
- Modify: `src/spec_wiki/live.rs`
- Delete: `src/spec_wiki/plan.rs`
- Delete: `src/spec_wiki/render.rs`
- Delete: `src/spec_wiki/check.rs`
- Delete: `src/spec_wiki/github.rs`
- Delete: `src/spec_wiki/assets.rs`
- Delete: `src/spec_wiki/ci.rs`
- Modify: `src/spec_wiki/mod.rs`
- Modify: `src/main.rs`
- Delete: `specs/task-autowiki-living-docs.spec.md`

**Interfaces:**
- Every recursive walker rejects symlink entries and remains under its root.
- Public and hidden CLI surfaces contain only the maintained Code Live Wiki.

- [ ] Add failing symlink-file and symlink-directory tests.
- [ ] Replace recursive traversal with `DirEntry::file_type` checks and containment diagnostics.
- [ ] Remove legacy command variants, handlers, modules, tests, and obsolete docs references.

### Task 5: Contracts, Documentation, and Full Verification

**Files:**
- Modify: `knowledge/requirements/req-requirements-compiler-plan-dag.md`
- Modify: `knowledge/requirements/req-code-live-wiki.md`
- Modify: `specs/task-requirements-compiler-plan-dag.spec.md`
- Modify: `specs/task-code-live-wiki.spec.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CHANGELOG.md`
- Modify: `.gitignore`

- [ ] Bind every new regression test in the relevant Task Contract.
- [ ] Update docs to describe strict IDs, real DAG edges, QA gates, exact trace mapping, archive evidence, and symlink policy.
- [ ] Remove fixture `target/` artifacts and ignore nested fixture build output.
- [ ] Run `cargo fmt --check`, `cargo test --quiet`, clippy, KLL gate, requirements plan gate, Wiki lint/check, docs lint, and repo guard.
