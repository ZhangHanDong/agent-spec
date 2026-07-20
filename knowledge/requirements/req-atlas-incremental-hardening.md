---
kind: requirement
id: REQ-ATLAS-INCREMENTAL-HARDENING
title: "Rust Atlas Incremental Hardening"
status: accepted
liveness: auto
tags: [atlas, incremental, generation, frontier, recovery, performance]
---

# Rust Atlas Incremental Hardening

## Problem

Before D2, Rust Atlas hashed source files incrementally but mutated live shards,
re-resolved and validated the complete graph, then published metadata and the
query index separately. A crash could expose mixed authority, a declaration
edit could require unchanged callers to be re-resolved, and a healthy
zero-change build still paid a full-graph floor. D3 watch or daemon behavior
would amplify those correctness and liveness failures.

## Requirements

[REQ-ATLAS-INCREMENTAL-GENERATION] A build MUST publish exactly one complete graph artifact set as one committed generation.

[REQ-ATLAS-INCREMENTAL-POINTER] A reader MUST pin one committed generation before reading graph artifacts.

[REQ-ATLAS-INCREMENTAL-FAILURE] Any build that does not commit MUST leave the previous generation readable.

[REQ-ATLAS-INCREMENTAL-BASELINE-BYTES] Any build that does not commit MUST leave the previous generation artifact digest set unchanged.

[REQ-ATLAS-INCREMENTAL-LEGACY] A graph without a generation pointer MUST remain readable until a successful build migrates it.

[REQ-ATLAS-INCREMENTAL-PATHS] Generation ids plus manifest paths MUST pass strict safe-component validation.

[REQ-ATLAS-INCREMENTAL-CLEANUP] Repeating cleanup of transaction-owned uncommitted staging MUST produce the same retained path set.

[REQ-ATLAS-INCREMENTAL-CLEANUP-ACTIVE] Transaction-owned staging cleanup MUST NOT delete any committed generation.

[REQ-ATLAS-INCREMENTAL-INPUT-KEY] Cargo input-plan reuse MUST compare the canonical input-plan fingerprint defined by the D2 Task Contract.

[REQ-ATLAS-INCREMENTAL-MTIME] Cargo input-plan reuse MUST NOT treat file mtime as authority.

[REQ-ATLAS-INCREMENTAL-MODULE-OWNERSHIP] Reusing Cargo metadata MUST NOT skip reconstruction of source module ownership.

[REQ-ATLAS-INCREMENTAL-REFRESH-INPUTS] Automatic stale-query refresh MUST reuse the committed features, target, and cfg input-plan configuration.

[REQ-ATLAS-INCREMENTAL-FRONTIER] A changed declaration MUST re-resolve directly affected reverse dependents in unchanged shards.

[REQ-ATLAS-INCREMENTAL-REMOVAL] A removed or renamed declaration MUST invalidate incoming resolved targets that no longer exist.

[REQ-ATLAS-INCREMENTAL-BARE] A changed last-segment candidate set MUST re-resolve affected bare targets.

[REQ-ATLAS-INCREMENTAL-IMPL] A changed implementation relation MUST recompute every implementation-dependent derived edge in its frontier.

[REQ-ATLAS-INCREMENTAL-FRONTIER-LIMIT] A dependency frontier MUST have a configurable positive upper bound.

[REQ-ATLAS-INCREMENTAL-FALLBACK] Frontier overflow MUST restart as an explicit full-resolution fallback.

[REQ-ATLAS-INCREMENTAL-ORPHAN-WRITE] Resolution work MUST be persisted atomically before processing begins.

[REQ-ATLAS-INCREMENTAL-ORPHAN-RECOVERY] A later build MUST process prior orphan work even when source files have no new changes.

[REQ-ATLAS-INCREMENTAL-ORPHAN-CONSUME] Successful resolution plus deterministic unresolved or external classification MUST consume an orphan item.

[REQ-ATLAS-INCREMENTAL-ORPHAN-FAILURE] Cancellation or process failure MUST preserve uncommitted orphan work.

[REQ-ATLAS-INCREMENTAL-FAST-PATH] A healthy zero-change build MUST NOT run full resolution or full validation.

[REQ-ATLAS-INCREMENTAL-NO-REWRITE] A healthy zero-change build MUST NOT rewrite committed authority artifacts.

[REQ-ATLAS-INCREMENTAL-CANCEL] Every build batch boundary MUST check an explicit cancellation token.

[REQ-ATLAS-INCREMENTAL-BATCH] Resolution plus validation MUST use configurable positive batch limits.

[REQ-ATLAS-INCREMENTAL-MEMORY] A build MUST enforce a configurable positive working-byte ceiling over source, serialized graph, and overlay admission before publication.

[REQ-ATLAS-INCREMENTAL-CAPABILITY] Semantic overlay capability metadata MUST share a generation id with its graph evidence.

[REQ-ATLAS-INCREMENTAL-REPORT] A build report MUST include generation id, touched shards, edge deltas, bounded working bytes, input-plan result, orphan recovery count, plus fallback reason.

[REQ-ATLAS-INCREMENTAL-MAINTENANCE] Post-commit maintenance failure MUST produce a warning diagnostic.

[REQ-ATLAS-INCREMENTAL-MAINTENANCE-COMMIT] Post-commit maintenance failure MUST NOT change the committed generation pointer.

[REQ-ATLAS-INCREMENTAL-MAINTENANCE-RECOVERY] A retained post-commit orphan queue MUST remain consumable by the next build.

[REQ-ATLAS-INCREMENTAL-MATRIX] The deterministic acceptance matrix MUST cover cold, zero-change, edit, delete, manifest, overflow, overlay, cancellation, commit failure, plus orphan recovery cases.

[REQ-ATLAS-INCREMENTAL-NON-GOAL] D2 MUST NOT enable filesystem watching, daemon ownership, retry scheduling, or pending-event watermarks.

## Dependencies

- REQ-ATLAS-EDGE-EVIDENCE-INDEX
- REQ-ATLAS-WORKTREE-FRESHNESS

## Scenarios

Scenario: Reader pins a complete committed generation
  Given a query begins while another build is ready to publish a new generation
  When the current pointer changes between query phases
  Then the query contract test exits zero only when meta, shards, and index all come from its original generation

Scenario: Failed generation publication preserves baseline authority
  Given a readable baseline graph and injected staging, rename, or pointer failures
  When Atlas attempts an incremental build
  Then the fault-injection test exits zero only when every query JSON plus artifact digest equals its baseline value

Scenario: Legacy graph migrates only after success
  Given a readable graph without a generation pointer
  When one build fails and a later build succeeds
  Then the failure keeps legacy reads available and only the successful build publishes a pointer

Scenario: Transaction-owned staging cleanup is idempotent
  Given an active generation and one uncommitted transaction staging directory
  When transaction cleanup is repeated
  Then the staging path stays absent and the active generation remains byte-identical

Scenario: Cargo input plan is content addressed
  Given unchanged manifest bytes with changed mtimes and changed bytes with restored mtimes
  When Atlas builds after each mutation
  Then the receipt JSON contains `input_plan: hit` for identical bytes and `input_plan: miss` for changed bytes

Scenario: Source module ownership is never cached as Cargo metadata
  Given an unchanged Cargo plan and a source edit that moves one module through a path attribute
  When Atlas incrementally builds
  Then the new generation contains only the new canonical module ownership and ids

Scenario: Automatic refresh preserves committed Cargo inputs
  Given a graph was built with explicit features, target, or cfg inputs
  When a non-frozen query refreshes stale source
  Then the new input-plan artifact retains the committed input configuration and fingerprint

Scenario: Declaration rename repairs unchanged callers
  Given an unchanged caller has a resolved edge to a declaration in another file
  When the declaration is renamed
  Then the report JSON lists caller plus callee under `touched_shards` and the caller edge has `resolution: unresolved`

Scenario: Bare-name ambiguity change repairs unchanged callers
  Given an unchanged caller has a uniquely resolved bare target
  When another file adds a same-name declaration
  Then the query JSON contains `resolution: unresolved` for the caller edge and omits the obsolete target id

Scenario: Frontier overflow is an explicit complete fallback
  Given changed symbols affect more shards than the configured frontier limit
  When Atlas builds
  Then the report JSON contains `fallback_reason: dependency-frontier-overflow` and `validation: pass`

Scenario: Interrupted resolution is recovered by zero-change sync
  Given a build stops after persisting part of its resolution queue
  When a later build sees no additional source edit
  Then the report JSON contains numeric field `orphans_recovered` with value at least one
  And report field `generation` differs from the baseline generation exactly once
  And filesystem path `.agent-spec/graph/orphans.json` does not exist

Scenario: Deterministic unresolved work is consumed
  Given an orphan target cannot resolve and is not external
  When the recovery build classifies it as unresolved
  Then the queue path is absent and the query JSON contains `resolution: unresolved`

Scenario: Healthy zero-change build performs no graph work
  Given a committed graph with matching input plan, source hashes, capabilities, and no orphan queue
  When Atlas builds without `--full`
  Then the report JSON contains `resolved_shards: 0` and `validated_shards: 0`
  And report field `generation` equals the baseline generation
  And the sorted set of authority artifact BLAKE3 digests equals the baseline set

Scenario: Cancellation discards staging only
  Given cancellation becomes true during a configured build batch
  When Atlas observes the token
  Then it returns a cancellation diagnostic, preserves the active generation, and retains orphan work

Scenario: Working-byte ceiling fails before publication
  Given a frontier batch exceeds the configured working-byte ceiling
  When Atlas builds
  Then it returns a resource-limit diagnostic without changing the active generation pointer

Scenario: Overlay capability is generation atomic
  Given a semantic overlay changes edges and capability fingerprints
  When publication succeeds or fails at the pointer boundary
  Then each query JSON contains one generation id shared by capability metadata and semantic edges

Scenario: Post-commit maintenance remains recoverable
  Given pointer publication succeeds and orphan queue deletion fails
  When the next zero-change build runs
  Then the first build reports a warning without rollback and the next build consumes the rebased queue

Scenario: Build receipt records the deterministic D2 matrix
  Given the checked-in incremental-hardening fixture matrix
  When the evaluator captures each build report
  Then the receipt-schema test exits zero only when every required metric field is present in every case

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track D2
- implementation plan: docs/superpowers/plans/2026-07-20-atlas-d2-incremental-hardening.md
- reference method: codegraph v1.3.1 `src/index.ts`, `src/db/**`, and `__tests__/sync.test.ts`, commit e552dc2
- human approval: latest-roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-incremental-hardening.spec.md
