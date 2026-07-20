# Atlas D2 Incremental Hardening Implementation Plan

> **Execution rule:** follow the active agent-spec Task Contract and implement
> each task test-first. A passing legacy incremental test is not sufficient for
> generation safety or dependent-frontier correctness.

**Goal:** Replace Rust Atlas's in-place, full-graph incremental refresh with a
recoverable, bounded build transaction that publishes one complete generation
and makes zero-change builds a true fast path.

**References:**

- `docs/atlas-roadmap.md`, Track D2
- CodeGraph v1.3.1 `src/index.ts`, `src/db/**`, and
  `__tests__/sync.test.ts` for changed-file resolution, orphan sweeping,
  transaction boundaries, and zero-change behavior
- Existing Rust Atlas MIR shard transaction, layered freshness, query index,
  and worktree identity contracts

## Non-Negotiable Invariants

1. Readers pin one committed generation for meta, shards, and query index.
2. A failed or cancelled build never changes the committed generation pointer.
3. Source extraction, syn resolution, overlays, validation, meta, and index are
   completed in staging before publication.
4. Changed declarations re-resolve every directly affected reverse dependent.
5. Frontier overflow is an explicit full-build fallback, never a partial pass.
6. Interrupted resolution leaves a recoverable orphan work item.
7. A healthy zero-change build does not run full resolution or validation and
   does not rewrite graph authority files.
8. Input-plan reuse is content-addressed by manifests, toolchain, feature,
   target/cfg, provider, and schema inputs; mtimes are not authority.
9. Batches, cancellation checks, and an explicit working-memory ceiling apply
   before publication.
10. Metrics describe actual touched shards, edge deltas, generation identity,
    cache result, fallback reason, orphan recovery, and bounded working bytes.

## Task 1: Establish Governance And Failure Matrix

**Files:**

- Add `knowledge/requirements/req-atlas-incremental-hardening.md`
- Add `specs/task-atlas-incremental-hardening.spec.md`
- Modify `docs/atlas-roadmap.md`

Create one accepted D2 requirement and one active Task Contract. Bind every
normative rule to a deterministic selector before implementation. Keep D3
watching, daemon lifecycle, retry scheduling, and pending watermarks out of
scope.

## Task 2: Introduce Committed Generation Snapshots

**Files:**

- Add `crates/rust-atlas/src/generation.rs`
- Modify `crates/rust-atlas/src/lib.rs`
- Modify `crates/rust-atlas/src/index.rs`
- Modify `crates/rust-atlas/src/status.rs`

Add a versioned `CURRENT.json` pointer and immutable
`generations/<generation-id>/` directories. A generation contains shards,
`meta.json`, `query-index.json`, and `generation.json`. The manifest records
artifact hashes, graph fingerprint, capability, base generation, and input-plan
fingerprint.

Readers resolve `CURRENT.json` once per operation and pass the pinned data path
through status, shard, and index reads. A graph without `CURRENT.json` is a
legacy generation and is migrated only by a successful build. Generation ids
and manifest paths pass strict safe-component validation.

Build staging hard-links unchanged shards when possible and copies them when a
hard link is unavailable. Every staged shard rewrite uses atomic replacement so
the base generation's inode cannot be modified through a link. Publication
renames staging into `generations/` and atomically replaces only the pointer.
Cleanup of abandoned staging and old generations is best-effort maintenance
after authority is committed.

Tests must inject staging-write, final-rename, and pointer-write failures and
prove the prior generation remains queryable and byte-identical.

## Task 3: Add Content-Addressed Cargo Input Plans

**Files:**

- Add `crates/rust-atlas/src/input_plan.rs`
- Modify `crates/rust-atlas/src/lib.rs`
- Modify `src/main.rs`

Persist the Cargo target plan separately from source module ownership. The key
contains canonical manifest paths plus content hashes, `rustc -vV`, schema and
provider versions, requested features, target, cfg values, and relevant Cargo
environment. Reusing the Cargo target plan must still rebuild source-unit module
ownership from current Rust source.

Add CLI options for feature, target, and cfg inputs without executing a shell.
The build report records `hit`, `miss`, or `disabled`. A content change with a
preserved mtime must miss; an mtime-only change with identical bytes must hit.

## Task 4: Implement The Bounded Dependency Frontier

**Files:**

- Add `crates/rust-atlas/src/incremental.rs`
- Modify `crates/rust-atlas/src/lib.rs`
- Modify `crates/rust-atlas/src/dynamic_dispatch.rs`

Compare old and newly extracted declaration identities for dirty, added,
removed, or ownership-changed files. The frontier includes:

- each changed shard;
- shards whose resolved target id was removed or replaced;
- shards whose `target_text` matches a changed canonical symbol;
- shards with bare targets whose last-segment candidate set changed;
- implementation/trait dependents affected by changed impl relations.

Resolve only the frontier against the complete staged declaration index.
Process frontier shards in canonical batches. If the distinct frontier exceeds
the configured limit, restart staging as an explicit full-resolution fallback
and record `dependency-frontier-overflow`. Dynamic-dispatch recomputation uses
the same changed impl signal or records its own full fallback.

Regression tests cover declaration rename, deletion, same-name ambiguity,
module ownership movement, impl changes, canonical ordering, and overflow.

## Task 5: Persist And Recover Orphan Work

**Files:**

- Modify `crates/rust-atlas/src/incremental.rs`
- Modify `crates/rust-atlas/src/lib.rs`

Before resolution, atomically persist a bounded work queue containing the base
generation, source-plan fingerprint, affected files, changed symbols, and
reason. Merge a prior queue into every build, including zero-change builds.

A successfully resolved edge and a deterministically unresolved/external edge
both consume work. Cancellation or process failure leaves the queue intact.
Clear it only after the generation pointer commit. Invalid, unsafe, oversized,
or mismatched queue records fail closed with a typed diagnostic.

Tests inject interruption after queue persistence and mid-batch, then run a
zero-change build to prove recovery and eventual queue removal.

## Task 6: Add Fast Path, Cancellation, And Resource Gates

**Files:**

- Modify `crates/rust-atlas/src/lib.rs`
- Modify `src/main.rs`

Extend `BuildOptions` with frontier, batch, and working-byte limits plus an
optional atomic cancellation token for library callers. Validate all limits.
Check cancellation before extraction batches, frontier batches, overlay work,
validation batches, and pointer publication.

When source hashes, input plan, requested capabilities, and committed artifacts
are unchanged and no orphan exists, return before staging, resolution,
validation, or writes. Maintenance runs after success and is advisory; its
failure becomes a warning, not a failed completed build.

Tests snapshot every authority/control artifact around a zero-change build,
instrument resolution/validation counters, and prove no bytes or counters
change. Resource overflow and cancellation leave the old generation active.

## Task 7: Preserve Overlay And Query Authority

**Files:**

- Modify `crates/rust-atlas/src/mir.rs`
- Modify `crates/rust-atlas/src/dynamic_dispatch.rs`
- Modify `crates/rust-atlas/src/status.rs`
- Modify `crates/rust-atlas/src/index.rs`

Apply SCIP, MIR, and dynamic-dispatch changes only inside staging. Capability
metadata and source/artifact fingerprints are published with the same
generation as their edges. A failed overlay may degrade according to its
existing contract, but no reader can observe new capability with old shards or
new shards with old index.

Status and every query surface report the pinned generation id. Query-index
validation remains strict and schema/worktree errors retain precedence.

## Task 8: Add The D2 Acceptance Matrix

**Files:**

- Add `fixtures/atlas/incremental-hardening/**`
- Modify `src/atlas_eval.rs`
- Modify `src/main.rs`
- Modify `docs/atlas-evaluation.md`

Create deterministic fixture measurements for cold build, zero-change build,
single-file declaration edit, deletion, manifest-content edit, frontier
overflow, large overlay, cancellation, generation commit failure, and orphan
recovery. Receipts record touched shards, resolved/unresolved edge delta,
bounded working bytes, generation id, input-plan result, and fallback reason.

Timing and operating-system RSS are observational fields, not deterministic
pass/fail thresholds. Correctness, bounded counts, artifact identity, and
fallback diagnostics are the gate.

## Task 9: Publish, Dogfood, And Review

**Files:**

- Modify `README.md`
- Modify `AGENTS.md`
- Modify `skills/agent-spec-tool-first/SKILL.md`
- Modify `CHANGELOG.md`
- Modify `docs/atlas-roadmap.md`
- Modify `.agent-spec/wiki/**`

Document the generation layout only through supported commands; generated graph
files remain derived state. Rebuild the dogfood graph, bind Contract symbols,
run KLL graph/plan gates, lifecycle with run logging, requirement replay, trace
graph, workspace tests, Clippy, docs lint, and wiki check.

Run an independent review focused on pointer atomicity, hard-link copy-on-write,
frontier false negatives, orphan consumption, cancellation windows, zero-change
writes, path validation, and semantic capability mixing. Resolve every blocking
or correctness finding before marking D2 delivered.
