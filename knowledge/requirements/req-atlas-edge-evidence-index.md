---
kind: requirement
id: REQ-ATLAS-EDGE-EVIDENCE-INDEX
title: "Atlas Edge Evidence and Derived Query Index"
status: accepted
liveness: auto
tags: [atlas, code-graph, evidence, search, index]
---

# Atlas Edge Evidence and Derived Query Index

## Problem

Atlas edges identify source, target, kind, resolution, and provenance, but they
discard the call site and analyzer reason. Queries also load and scan every JSON
shard and require callers to know a canonical symbol id. Explainable flow and
impact analysis need evidence-complete edges plus deterministic indexed lookup.

## Requirements

[REQ-ATLAS-EDGE-SCHEMA] Every non-containment edge MUST support optional site, extractor, dispatch, confidence, candidates, and evidence fields.

[REQ-ATLAS-EDGE-SITE] A site MUST identify repository-relative file and start/end line and column.

[REQ-ATLAS-EDGE-SCIP-SITE] Every SCIP call, type-use, and reference edge MUST preserve the originating occurrence site.

[REQ-ATLAS-EDGE-CONFIDENCE] Confidence MUST distinguish exact, bounded-candidates, and heuristic facts independently from syn, scip, and mir provenance.

[REQ-ATLAS-EDGE-DYNAMIC] An edge with more than one candidate MUST NOT carry exact confidence.

[REQ-ATLAS-EDGE-IDENTITY] Edge identity and deduplication MUST include source, target, kind, and site so distinct call sites remain queryable.

[REQ-ATLAS-EDGE-SCHEMA-GATE] The schema version MUST increase; graph load MUST reject the old version with the existing rebuild diagnostic, while optional fields within the current version use serde defaults.

[REQ-ATLAS-INDEX-DERIVED] The build MUST write an atomically replaceable derived query index without changing per-source JSON shards as the canonical graph store.

[REQ-ATLAS-INDEX-LOOKUP] The derived index MUST support node-by-id, symbol, file, incoming edge, and outgoing edge lookup without scanning every shard.

[REQ-ATLAS-SEARCH] Library, CLI, and MCP surfaces MUST provide deterministic symbol search with exact, qualified, segmented-identifier, and deterministic fuzzy matching.

[REQ-ATLAS-SEARCH-DISAMBIGUATE] Search MUST return ordered candidates with canonical id, symbol, kind, and location; equal inputs and graph fingerprints MUST produce byte-stable JSON order.

[REQ-ATLAS-INDEX-STALE] Loading a missing, schema-mismatched, or graph-fingerprint-mismatched derived index MUST fail with an actionable rebuild diagnostic rather than serving it as current.

[REQ-ATLAS-EDGE-NEGATIVE] Satisfying specs MUST cover invalid exact confidence, stale index rejection, ambiguous search, and old schema rejection.

## Dependencies

- REQ-RUST-ATLAS

## Scenarios

Scenario: SCIP occurrence is preserved on an edge
  Given a SCIP call occurrence with a source range
  When the overlay adds the call edge
  Then the edge site records the repository-relative file and exact range

Scenario: Search returns deterministic candidates
  Given symbols with exact, suffix, and segmented-name matches
  When Atlas search runs twice against one graph fingerprint
  Then both JSON results are byte-identical and ordered by match strength

Scenario: Multiple candidates cannot be exact
  Given an edge with two dynamic target candidates
  When graph validation runs
  Then validation fails naming the confidence invariant

Scenario: Stale derived index is rejected
  Given an index fingerprint that differs from the current graph fingerprint
  When a search query loads it
  Then the query fails with an actionable rebuild diagnostic

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track A2/B1
- reference implementation: codegraph v1.3.1 query indexes and edge metadata
- human approval: roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-edge-evidence-index.spec.md
