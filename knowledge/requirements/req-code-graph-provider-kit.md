---
kind: requirement
id: REQ-CODE-GRAPH-PROVIDER-KIT
title: "External Code Graph Provider Adapter Kit"
status: accepted
liveness: auto
tags: [atlas, code-graph, provider, adapter, conformance]
---

# External Code Graph Provider Adapter Kit

## Problem

The provider-neutral Code Graph IR has a Rust Atlas consumer, but an external language or semantic
tool has no versioned producer contract or conformance gate. Direct integrations would otherwise
invent incompatible process protocols, freshness claims, identifiers, limits, and failure behavior.

## Requirements

[REQ-PROVIDER-KIT-MANIFEST] A strict `ProviderManifest` MUST declare provider id and version,
language, compatible IR schema range, provider role, capabilities, startup protocol, freshness
inputs, resource limits, deterministic behavior, and no-daemon support.

[REQ-PROVIDER-KIT-ROLES] Extraction providers and semantic enrichers MUST use separate output
schemas. Extraction providers MAY publish nodes, containment, and basic references; enrichers MUST
NOT publish or replace nodes and MAY only add evidence-bearing edges or query hints.

[REQ-PROVIDER-KIT-PROJECTION] Published extraction facts MUST use stable provider-scoped node ids,
normalized repository-relative paths, explicit provenance, normalized diagnostics, and a host-
derived graph fingerprint over canonical content and freshness inputs.

[REQ-PROVIDER-KIT-FRESHNESS] Every response MUST bind to the requested worktree identity and MUST
report fresh, stale, or partial state honestly. Stale and partial states MUST retain affected paths
and diagnostics rather than becoming fresh evidence.

[REQ-PROVIDER-KIT-EXECUTION] The process adapter MUST invoke an explicit executable with literal
argv, enforce timeout and stdout/stderr limits, support cancellation, and MUST NOT use shell
interpolation, an implicit daemon, an installer, or network discovery.

[REQ-PROVIDER-KIT-ATOMIC] A provider artifact MUST be validated completely before same-directory
atomic publication. Invalid schema, malformed facts, cancellation, timeout, and oversized output
MUST leave the previous artifact unchanged.

[REQ-PROVIDER-KIT-CONFORMANCE] A provider-neutral conformance fixture and harness MUST cover stable
ids, repeated-build determinism, partial parse, stale/worktree mismatch, unknown schema, bounded
output, cancellation, and atomic publication with machine-readable diagnostics.

[REQ-PROVIDER-KIT-OPTIONAL] External adapters MUST be disabled unless selected in project
configuration. The kit MUST remain vendor-, runtime-, language-, installer-, and orchestrator-
neutral, and MUST NOT give any future provider a privileged protocol role.

## Dependencies

- REQ-CODE-GRAPH-IR
- REQ-INTENT-AWARE-AFFECTED
- REQ-ATLAS-WORKTREE-FRESHNESS
- REQ-ATLAS-INCREMENTAL-HARDENING

## Scenarios

Scenario: Valid extraction provider publishes canonical graph evidence
  Given a valid extractor manifest, explicit project registration, and fresh fixture response
  When the adapter runs and publishes the artifact
  Then stable nodes, normalized paths, freshness inputs, provenance, and graph fingerprint are present

Scenario: Semantic enricher remains additive
  Given an enricher response with evidence-bearing semantic edges and query hints
  When the response is validated
  Then no node or KLL mutation surface exists and the base graph fingerprint remains explicit

Scenario: Invalid execution never replaces prior evidence
  Given a previously published artifact
  When the provider times out, is cancelled, exceeds a limit, or returns an unknown schema
  Then the prior artifact is byte-identical and a normalized diagnostic is returned

Scenario: Conformance detects dishonest freshness
  Given a stale, partial, or wrong-worktree fixture response
  When the conformance harness evaluates it
  Then stale and partial evidence remains visible and wrong-worktree evidence is rejected

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track F1 and delivery item 16
- consumer contract: knowledge/requirements/req-code-graph-ir.md
- implementation precedent: crates/rust-atlas and local codegraph review recorded in the roadmap
- human approval: implement the latest reviewed roadmap, 2026-07-21
- contract: specs/task-code-graph-provider-kit.spec.md

