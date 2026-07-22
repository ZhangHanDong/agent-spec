---
kind: requirement
id: REQ-ATLAS-SCIP-SEMANTIC
title: "Atlas SCIP Semantic Overlay"
status: accepted
liveness: auto
tags: [atlas, code-graph, scip, semantic, rust-analyzer]
---

# Atlas SCIP Semantic Overlay

## Problem

The syn baseline can recover Rust declarations and local structure without a compiler, but it
cannot resolve every cross-file or cross-crate reference. Atlas needs an optional rust-analyzer
SCIP overlay that adds resolved semantic edges without weakening or replacing the offline syn
graph.

## Dependencies

- REQ-RUST-ATLAS

## Requirements

[REQ-ATLAS-SCIP-PROTOBUF] Atlas MUST ingest rust-analyzer protobuf `index.scip` files and retain support for the existing JSON fixture representation.

[REQ-ATLAS-SCIP-EDGE-KINDS] The overlay MUST classify callable targets as `calls`, type targets as `uses-type`, and other references as `references`.

[REQ-ATLAS-SCIP-IMPLS] The overlay MUST resolve repository-local trait and implementation relationships to graph node identifiers and MUST classify out-of-repository targets as external.

[REQ-ATLAS-SCIP-ADDITIVE] SCIP edges MUST be additive, carry `provenance=scip`, and MUST NOT rewrite or delete syn edges.

[REQ-ATLAS-SCIP-CAPABILITY] Graph metadata MUST record the supplied SCIP path and content fingerprint, and a schema version change MUST reject old graphs with a rebuild diagnostic.

[REQ-ATLAS-SCIP-REFRESH] Incremental syn refresh MUST reapply an available recorded SCIP index; if the index is missing, Atlas MUST remove SCIP edges and retain a valid syn-only graph.

[REQ-ATLAS-SCIP-GENERATE] The CLI MUST provide an opt-in rust-analyzer SCIP generation command that fails with an actionable diagnostic when the analyzer is missing or the project cannot be analyzed.

[REQ-ATLAS-SCIP-DEGRADE] A build without a SCIP index MUST remain a successful syn-only build and MUST NOT require rust-analyzer.

## Scenarios

Scenario: Protobuf overlay resolves semantic references
  Given rust-analyzer produced an `index.scip` for a Rust fixture
  When Atlas builds with that index
  Then the graph contains resolved SCIP edges classified by target symbol kind

Scenario: Refresh preserves an available overlay
  Given graph metadata records an existing SCIP index and its fingerprint
  When a source edit triggers incremental refresh
  Then Atlas reapplies the SCIP overlay and preserves its capability metadata

Scenario: Missing overlay degrades to syn
  Given graph metadata references a SCIP index that no longer exists
  When incremental refresh runs
  Then Atlas removes SCIP edges, reports the capability absent, and preserves syn facts

Scenario: Analyzer generation failure is explicit
  Given the configured rust-analyzer executable is unavailable
  When the SCIP generation command runs
  Then it exits non-zero with an actionable diagnostic and does not panic

## Source Trace

- delivered contract: specs/task-atlas-scip-semantic.spec.md
- canonical roadmap: docs/atlas-roadmap.md, Track A1
- implementation lineage: Rust Atlas schema v4 SCIP semantic overlay
