# Atlas Query Context Compiler Implementation Plan

> **For agentic workers:** Implement each task with TDD and keep the Task Contract selectors as the acceptance surface.

**Goal:** Deliver roadmap B5 as a deterministic two-stage query context compiler with explicit profiles, stable continuations, omission manifests and separate retrieval/projection receipts.

**Architecture:** `rust-atlas` adds a provider-local compiler over one immutable query index. Retrieval constructs a scored candidate supergraph without using the response-byte budget. Projection applies profile relevance, verified source-span selection and optional-evidence pruning. The root binary exposes an additive `atlas context` command; existing explore and MCP surfaces remain unchanged.

**Tech Stack:** Rust 2024, serde JSON, clap, existing `QueryIndex`, traversal, impact, runtime-boundary and E3 scorer APIs.

## Global Constraints

- Authority is `REQ-ATLAS-QUERY-CONTEXT-COMPILER`; execution is governed by `specs/task-atlas-query-context-compiler.spec.md`.
- Use TDD for every named selector; do not edit selectors to accommodate implementation failures.
- Retrieval and projection are separate public data contracts and have separate loss metrics.
- Profiles, thresholds, caps, tie-breaks and continuation arguments are explicit and deterministic.
- Required evidence fails with a typed error if it cannot fit; it is never silently pruned.
- Source bytes require current hash equality with the selected graph generation.
- `atlas context` is additive. Do not change default MCP discovery or `explore-v1`.
- Do not modify or stage `.superpowers/`.

## Task 1: Commit And Gate Governance

- [x] Add the accepted KLL requirement, Task Contract, design and this plan.
- [x] Run `agent-spec parse`, `lint --min-score 0.7`, `lint-knowledge --gate`, `requirements graph --gate` and `requirements plan --gate`.
- [x] Confirm `satisfies` resolves to one active work unit before code changes.

## Task 2: Define Intent, Profiles And Retrieval Contract

- [x] Add failing parser and deterministic profile tests.
- [x] Add `context.rs` types for `QueryIntent`, `ContextProfile`, `ContextLimits`, evidence classes, candidates and scoring reasons.
- [x] Parse identifiers, paths and known relations without an LLM; retain typed diagnostics for unrecognized tokens.
- [x] Add retrieval tests for exact symbol/path, ambiguous suffix, caller/callee, implementation, primary/alternative path, boundary and impact evidence.
- [x] Build a stable candidate supergraph with hard-cap coverage accounting and no byte pruning.

## Task 3: Implement Priority, Source Projection And Omission

- [x] Add failing relevance-before-byte, verified-span, stale-source, required-preservation and required-overflow tests.
- [x] Add restricted test/generated/vendor source-body admission coverage.
- [x] Build profile-specific priority plans with stable class and evidence-id tie-breaks.
- [x] Project symbol and edge-site spans only after graph hash verification.
- [x] Apply relevance threshold, then skeletonize eligible siblings, then prune optional evidence to the byte ceiling.
- [x] Group omissions by class/reason and emit stable continuation argv.

## Task 4: Finalize Receipts And Continuations

- [x] Add failing receipt, cross-process continuation and fingerprint-mismatch tests.
- [x] Record retrieval coverage, projection retention, bytes, truncated classes, read-back, follow-up and load profile.
- [x] Implement `after` validation and `expect_graph`; reject missing cursor ids or changed fingerprints.
- [x] Prove JSON byte accounting converges and repeated compilation is byte-stable.

## Task 5: Add The Additive CLI

- [x] Add failing clap and finalized-byte tests for `atlas context`.
- [x] Add `--profile`, `--after`, `--expect-graph`, `--max-bytes` and `--frozen` with bounded validation.
- [x] Serialize only finalized results and keep typed failures non-zero.
- [x] Prove legacy explore output and MCP tool discovery are byte-stable.

## Task 6: Extend E3 And Documentation

- [x] Add fixed parser, profile, stale, forbidden, truncation and receipt observations to the query-quality corpus.
- [x] Extend the scorer only where required for retrieval/projection receipt assertions; retain strict schema/version checks.
- [x] Add `docs/atlas-query-context.md`, README, AGENTS, skill, wiki and changelog guidance.
- [x] Mark B5 delivered only after the fixed corpus passes; do not promote it to default MCP without E1.

## Task 7: Verify And Close The Contract

- [x] Run targeted tests, `cargo test --workspace --all-targets --all-features`, fmt and clippy with warnings denied.
- [x] Run docs lint, wiki check, KLL graph/plan gates and graph provider checks.
- [x] Run lifecycle with zero skip/uncertain, replay the requirement and inspect trace graph against the final commit.
- [x] Record real candidate, omission, byte and diagnostic numbers in docs and update roadmap status.
