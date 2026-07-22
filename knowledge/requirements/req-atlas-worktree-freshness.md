---
kind: requirement
id: REQ-ATLAS-WORKTREE-FRESHNESS
title: "Atlas Worktree Identity and Layered Freshness"
status: accepted
liveness: auto
tags: [atlas, worktree, freshness, scip, lifecycle]
---

# Atlas Worktree Identity and Layered Freshness

## Problem

Source hashes detect stale syn shards, but Atlas does not expose one authoritative
identity and freshness contract for the current worktree and each semantic layer.
A graph from another worktree, or a refreshed syn layer combined with an old SCIP
index, can otherwise be consumed as if all evidence described the current code.

## Requirements

[REQ-ATLAS-WORKTREE-IDENTITY] Graph metadata MUST record canonical repository root, git common directory when available, worktree root, graph root, and current toolchain identity.

[REQ-ATLAS-WORKTREE-MISMATCH] Definitive query, binding, and lifecycle consumption MUST reject a graph whose recorded worktree differs from the current worktree while status output reports both identities.

[REQ-ATLAS-FRESHNESS-LAYERS] Atlas MUST report syn, scip, and mir freshness independently as fresh, stale, or unavailable with fingerprints and diagnostics.

[REQ-ATLAS-FRESHNESS-SYN] Syn freshness MUST continue to compare current source hashes with stored per-file hashes.

[REQ-ATLAS-FRESHNESS-SCIP] SCIP freshness MUST compare the index fingerprint and the source-set fingerprint captured when that index was explicitly supplied; source edits followed by syn refresh MUST leave the reused SCIP layer stale.

[REQ-ATLAS-FRESHNESS-MIR] MIR freshness MUST report unavailable until a MIR capability exists and MUST NOT be inferred from syn or SCIP state.

[REQ-ATLAS-FINGERPRINT] The provider graph fingerprint MUST include schema version, worktree identity, toolchain identity, source-set fingerprint, and available semantic-layer fingerprints.

[REQ-ATLAS-FRESHNESS-QUERY] Every tree, query, refs, impls, search, status, and future traversal result MUST carry the same identity and layered-freshness summary.

[REQ-ATLAS-FRESHNESS-GATE] Code binding and Contract symbol validation MUST consume the shared Atlas status result; stale semantic evidence or worktree mismatch MUST block definitive evidence.

[REQ-ATLAS-FRESHNESS-NOGIT] Outside git, Atlas MUST use the canonical code root as worktree identity and continue operating without fabricating a git common directory.

[REQ-ATLAS-FRESHNESS-NEGATIVE] Satisfying specs MUST cover borrowed-worktree rejection, source-newer-than-SCIP staleness, missing analyzer output, and no-git fallback.

## Dependencies

- REQ-RUST-ATLAS
- REQ-ATLAS-EDGE-EVIDENCE-INDEX

## Scenarios

Scenario: Fresh graph reports independent layers
  Given a graph built with an explicitly supplied current SCIP index
  When Atlas status runs in the same worktree
  Then syn and scip are fresh and mir is unavailable

Scenario: No-git project remains usable
  Given a Rust project outside git
  When Atlas builds and queries the graph
  Then worktree identity equals the canonical code root and git common directory is absent

Scenario: Borrowed worktree graph is rejected
  Given a graph built in one linked git worktree
  When a definitive query runs from another linked worktree using that graph
  Then the query fails with both worktree paths and does not return symbol facts

Scenario: Source edit leaves reused SCIP stale
  Given a fresh syn and SCIP graph
  When source changes and automatic refresh reuses the old SCIP index
  Then syn becomes fresh, scip remains stale, and definitive semantic binding is blocked

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track D1
- reference implementation: codegraph v1.3.1 worktree mismatch handling
- human approval: roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-worktree-layered-freshness.spec.md
