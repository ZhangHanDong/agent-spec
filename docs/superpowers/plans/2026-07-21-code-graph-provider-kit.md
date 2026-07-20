# Code Graph Provider Adapter Kit Implementation Plan

> Execute with TDD and the repository's agent-spec lifecycle workflow. Check off each step only
> after its command has passed.

**Goal:** Deliver roadmap F1 as a reusable Rust SDK, bounded local process adapter, conformance
harness, CLI surface, and documented provider-author contract without implementing F2.

**Requirement:** `REQ-CODE-GRAPH-PROVIDER-KIT`

**Task Contract:** `specs/task-code-graph-provider-kit.spec.md`

## Task 1: Establish Governance

- [x] Add accepted KLL requirement, task contract, design, and this plan.
- [x] Run spec parse/lint plus knowledge graph and requirements plan gates.
- [x] Commit governance before production code.

## Task 2: Create The Standalone SDK

- [x] Write RED tests for manifest role/capability/schema/limit validation and opt-in registration.
- [x] Add `crates/code-graph-provider` to the workspace with strict serde wire types.
- [x] Implement stable diagnostic codes and manifest/registration validation.
- [x] Run focused tests and commit.

## Task 3: Project Extraction And Enrichment

- [x] Write RED projection tests for stable ids, canonical order, unsafe paths, freshness, worktree,
  and additive enricher evidence.
- [x] Implement separate extraction and enrichment payload validators.
- [x] Derive canonical BLAKE3 graph/enrichment fingerprints and published artifacts.
- [x] Run focused tests and commit.

## Task 4: Bound Process Execution And Publication

- [ ] Write RED tests for unknown schema, stdout/stderr limits, timeout, cancellation, and rollback.
- [ ] Implement literal-argv process execution with bounded readers and child reaping.
- [ ] Implement same-directory atomic publication after full validation only.
- [ ] Run focused tests and commit.

## Task 5: Deliver The Conformance Harness

- [ ] Add provider-neutral source fixture, manifest, registration, and local fixture executable.
- [ ] Write RED matrix test proving all required F1 checks are represented and enforced.
- [ ] Implement strict conformance receipt and deterministic harness.
- [ ] Run fixture conformance tests and commit.

## Task 6: Expose Explicit CLI Commands

- [ ] Write RED CLI test for `atlas provider validate` and `atlas provider conformance`.
- [ ] Add nested commands with quiet JSON stdout, atomic `--out`, and nonzero blocked exit.
- [ ] Verify no default command, build, test, or requirements flow launches a provider.
- [ ] Run focused tests and commit.

## Task 7: Publish F1 And Dogfood

- [ ] Document schemas, projection, freshness, diagnostics, execution, and F2 author workflow.
- [ ] Update roadmap, README, AGENTS, skill, changelog, and Code Live Wiki without promoting wiki
  content to KLL truth.
- [ ] Run fmt, clippy, full workspace tests, docs lint, wiki check, KLL graph/plan gates, and task
  lifecycle with no skip or uncertain verdict.
- [ ] Commit publication, rerun post-commit lifecycle, requirement replay, and trace graph.
