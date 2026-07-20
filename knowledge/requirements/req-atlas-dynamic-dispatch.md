---
kind: requirement
id: REQ-ATLAS-DYNAMIC-DISPATCH
title: "Atlas Bounded Dynamic Dispatch"
status: accepted
liveness: auto
tags: [atlas, code-graph, dynamic-dispatch, static-analysis]
---

# Atlas Bounded Dynamic Dispatch

## Problem

An exact reference to a Rust trait method identifies the declaration but not the
runtime implementation. Treating every implementation as an exact compiler fact
would be unsound, while omitting the bounded alternatives breaks flow and impact
queries at a common dispatch boundary.

## Requirements

[REQ-ATLAS-DYNAMIC-ISOLATION] Dynamic-dispatch inference MUST run as a deterministic whole-graph enrichment pass outside the source parser and MUST be explicitly enabled.

[REQ-ATLAS-DYNAMIC-TRAIT] A resolved SCIP call to a trait method MUST preserve the exact declaration edge and MAY add one inferred call edge whose candidates are the matching methods of resolved `impl Trait for Type` nodes.

[REQ-ATLAS-DYNAMIC-HONESTY] Every inferred edge MUST use `dispatch: trait`, `confidence: bounded-candidates`, unresolved candidate semantics, a dedicated extractor identity, and explanatory evidence; it MUST NOT use MIR provenance or exact confidence.

[REQ-ATLAS-DYNAMIC-BOUND] Candidate lists MUST be sorted, deduplicated, and capped at 64. A larger fan-out MUST emit `dynamic-dispatch-truncated` and MUST NOT write a partial candidate edge.

[REQ-ATLAS-DYNAMIC-INERT] A repository without a resolved trait-method call anchor MUST gain no inferred dynamic edge even when the enricher is enabled.

[REQ-ATLAS-DYNAMIC-REVERSIBLE] Rebuilding without dynamic enrichment MUST remove previously inferred edges while preserving syn, SCIP, and MIR facts.

[REQ-ATLAS-DYNAMIC-TRAVERSAL] Flow and impact consumers MUST traverse bounded implementation candidates through the existing provider-neutral candidate-edge contract.

[REQ-ATLAS-DYNAMIC-PRECEDENCE] When an exact edge and a bounded candidate identify the same concrete adjacent target, the derived query index MUST select the exact edge for traversal while canonical shards retain both evidence records.

## Dependencies

- REQ-ATLAS-MIR-OVERLAY
- REQ-ATLAS-EXPLORE-FLOW-IMPACT

## Scenarios

Scenario: Trait call exposes bounded implementations
  Given one SCIP call resolves to a trait method with two resolved implementation methods
  When dynamic dispatch enrichment runs
  Then one inferred edge lists both implementation ids in canonical order and flow can reach either candidate

Scenario: Unanchored implementations stay inert
  Given traits and implementations exist but no SCIP call resolves to a trait method
  When dynamic dispatch enrichment runs
  Then no inferred dynamic edge is written

Scenario: Excessive fan-out is explicit
  Given one trait method has more than 64 implementation candidates
  When dynamic dispatch enrichment runs
  Then no partial candidate edge is written and the build reports dynamic-dispatch-truncated

Scenario: Dynamic enrichment is reversible
  Given a graph contains an inferred dynamic edge
  When Atlas rebuilds without dynamic enrichment
  Then the inferred edge is removed and non-dynamic evidence remains

Scenario: Exact evidence dominates a candidate alternative
  Given one caller has an exact MIR edge and a bounded dynamic candidate to the same implementation method
  When the derived query index builds adjacent traversal locators
  Then incoming and outgoing adjacency select the exact MIR edge while canonical shards retain the dynamic evidence

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track A4
- design precedent: codegraph whole-graph callback synthesizers, bounded fan-out, inert mechanism gates, and dynamic-boundary honesty
- human approval: latest Atlas roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-dynamic-dispatch.spec.md
