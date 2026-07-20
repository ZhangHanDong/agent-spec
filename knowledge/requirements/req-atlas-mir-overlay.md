---
kind: requirement
id: REQ-ATLAS-MIR-OVERLAY
title: "Atlas Optional MIR Overlay"
status: accepted
liveness: auto
tags: [atlas, code-graph, mir, static-analysis]
---

# Atlas Optional MIR Overlay

## Problem

The syn and SCIP layers cannot provide compiler-level call and control-flow
facts. Pulling a nightly compiler API into the default Atlas dependency graph
would make the stable baseline fragile, while silently treating failed or stale
MIR extraction as current evidence would make impact analysis unsound.

## Requirements

[REQ-ATLAS-MIR-DEFAULT] The default feature set MUST build and run on the repository stable toolchain without compiling or invoking a MIR extractor.

[REQ-ATLAS-MIR-PROTOCOL] The optional MIR consumer MUST accept a versioned `rust-atlas/mir-overlay-v1` artifact with extractor identity, source fingerprint, function facts, call sites, and CFG summaries.

[REQ-ATLAS-MIR-CALLS] A uniquely resolved MIR call MUST add an exact `calls` edge with `mir` provenance while preserving lower-provenance syn and SCIP facts in canonical shards.

[REQ-ATLAS-MIR-CFG] A resolved function fact MUST attach a deterministic CFG summary containing basic-block, edge, exit, and loop-header counts.

[REQ-ATLAS-MIR-GENERIC] Generic MIR calls MUST preserve the generic target form and MUST carry `generic: true`; Atlas MUST NOT expand monomorphized instances.

[REQ-ATLAS-MIR-PRECEDENCE] Query consumers MUST prefer the highest-provenance relation for an otherwise equivalent source-target relation without deleting lower-provenance stored evidence.

[REQ-ATLAS-MIR-FAILURE] A missing, malformed, schema-incompatible, version-incompatible, or non-zero extractor result MUST leave a valid syn plus optional SCIP graph, remove previously stored MIR facts, and return a typed `mir-extraction-failed` diagnostic.

[REQ-ATLAS-MIR-FRESHNESS] Capability metadata and `atlas status` MUST report MIR availability, extractor identity, overlay fingerprint, source fingerprint, and freshness independently from syn and SCIP.

[REQ-ATLAS-MIR-DRIVER] The optional external driver MUST be invoked directly with fixed argv and MUST NOT be interpreted through a shell.

## Dependencies

- REQ-ATLAS-EDGE-EVIDENCE-INDEX
- REQ-ATLAS-EXPLORE-FLOW-IMPACT
- REQ-ATLAS-WORKTREE-FRESHNESS

## Scenarios

Scenario: MIR overlay adds precise call and CFG facts
  Given a valid overlay for a caller and callee in the current source snapshot
  When Atlas builds with the MIR feature and overlay enabled
  Then the stored graph contains an exact mir call edge and the caller CFG summary

Scenario: Query prefers MIR without deleting lower evidence
  Given the same source-target relation exists in syn, SCIP, and MIR evidence
  When Atlas queries the caller
  Then the query reports the MIR relation while the canonical shard retains all evidence

Scenario: Extraction failure degrades explicitly
  Given a previously overlaid graph and a failing or malformed MIR producer
  When Atlas rebuilds with MIR enabled
  Then the build exits successfully with mir-extraction-failed and no stale MIR facts remain

Scenario: MIR freshness is independent
  Given syn is refreshed after a recorded MIR source snapshot
  When Atlas status runs
  Then syn is fresh and MIR is stale with its recorded and current fingerprints

Scenario: Default build excludes MIR
  Given the repository stable toolchain and default Cargo features
  When Atlas builds without MIR options
  Then no MIR producer is invoked and MIR capability is unavailable

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track A3
- rejected extractor: Charon requires a nightly toolchain and does not satisfy the repository stable-toolchain compatibility gate, evaluated 2026-07-20
- selected boundary: separately versioned `rustc_public` producer emitting `rust-atlas/mir-overlay-v1`; the producer is runtime/feature opt-in and is not a default library dependency
- human approval: latest Atlas roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-mir-layer.spec.md
