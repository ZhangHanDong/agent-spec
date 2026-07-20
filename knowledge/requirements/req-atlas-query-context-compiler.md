---
kind: requirement
id: REQ-ATLAS-QUERY-CONTEXT-COMPILER
title: "Rust Atlas Query Context Compiler"
status: accepted
liveness: auto
tags: [atlas, query, context, projection, evidence, receipt]
---

# Rust Atlas Query Context Compiler

## Problem

`atlas explore` combines graph retrieval and response-size pruning in one operation. That makes it
hard to tell whether a missing fact was absent from retrieval or removed during projection, and its
fixed `compact`/`deep` profiles cannot preserve profile-specific evidence priorities. Atlas needs a
deterministic context compiler that keeps graph authority separate from the bounded representation
given to an Agent.

## Requirements

[REQ-ATLAS-CONTEXT-INTENT] Query intent MUST parse only explicit identifiers, repository paths, relation names and one explicit profile; Atlas MUST NOT call an LLM or write inferred intent into the graph.

[REQ-ATLAS-CONTEXT-PROFILES] The compiler MUST support explicit `symbol`, `flow`, `architecture` and `impact` profiles with versioned deterministic budgets, relevance thresholds and tie-break rules.

[REQ-ATLAS-CONTEXT-STAGES] Retrieval MUST produce a candidate supergraph with stable evidence ids, scores and scoring reasons before projection applies relevance or byte limits.

[REQ-ATLAS-CONTEXT-PRIORITY] Evidence priority MUST order user-named symbols and failure evidence before primary spine and boundary sites, then unique or representative implementations, adjacent structure and off-spine siblings.

[REQ-ATLAS-CONTEXT-SOURCE] Source projection MUST use graph-hash-verified line slices around symbol spans or edge sites and MUST NOT fill the budget with whole files.

[REQ-ATLAS-CONTEXT-RELEVANCE] A profile's relevance threshold MUST run before its byte cap; the byte budget is a ceiling and MUST NOT retain low-relevance evidence merely to fill output.

[REQ-ATLAS-CONTEXT-PRESERVE] User-named symbols, unique implementations, primary spine, runtime boundary sites, failure evidence and source spans MUST NOT be silently compressed or removed; if required evidence cannot fit, the compiler MUST return a typed budget error.

[REQ-ATLAS-CONTEXT-SKELETON] Compression MAY reduce only interchangeable off-spine sibling bodies that retain a representative implementation to signature skeletons.

[REQ-ATLAS-CONTEXT-OMISSION] Every omitted evidence class MUST report count, reason, highest-scored candidate and an executable continuation query.

[REQ-ATLAS-CONTEXT-CONTINUATION] Continuation MUST use a stable evidence id plus the expected graph fingerprint and MUST NOT depend on a process-local cursor or hidden mutable state.

[REQ-ATLAS-CONTEXT-RECEIPT] Every result MUST separate retrieval coverage from projection retention and record candidate counts, serialized bytes, truncated evidence classes, actual profile and limits, graph fingerprint, read-back requirement, follow-up count and a deterministic load profile.

[REQ-ATLAS-CONTEXT-HONESTY] Missing graph capability, stale source, unresolved endpoint, retrieval truncation and projection truncation MUST remain typed and MUST NOT be represented as an empty successful answer.

[REQ-ATLAS-CONTEXT-COMPATIBILITY] B5 MUST be additive: existing `explore`, search, flow, impact and default MCP discovery/output MUST remain unchanged until E1 separately approves a default surface change.

[REQ-ATLAS-CONTEXT-QUALITY] Parser, ranking, relevance, projection, omission and receipt changes MUST pass the E3 fixed query-quality corpus before delivery.

[REQ-ATLAS-CONTEXT-NEGATIVE] Satisfying specs MUST cover ambiguous suffixes, no match, stale source, low-relevance exclusion, byte pressure, required-evidence overflow, stable continuation, graph-fingerprint mismatch and deterministic tie ordering.

## Dependencies

- REQ-ATLAS-EXPLORE-FLOW-IMPACT
- REQ-ATLAS-QUERY-QUALITY-REGRESSION
- REQ-ATLAS-RUNTIME-BOUNDARY-HINTS

## Scenarios

Scenario: Retrieval and projection losses remain distinguishable
  Given a query whose candidate supergraph exceeds a profile relevance threshold and byte budget
  When the context compiler returns a bounded projection
  Then the receipt separately reports retrieval coverage and projection retention

Scenario: Required evidence survives projection
  Given a named symbol on the primary spine with a runtime boundary site
  When optional sibling evidence exceeds the byte budget
  Then the named symbol, spine and boundary span remain while siblings appear in the omission manifest

Scenario: Continuation is stable across processes
  Given an omission manifest from one committed graph fingerprint
  When a new process executes its continuation query against that fingerprint
  Then it resumes after the same stable evidence id with no hidden cursor

Scenario: Relevance is not confused with capacity
  Given low-scored candidates and unused output bytes
  When the profile relevance gate runs
  Then low-relevance candidates remain omitted even though the byte ceiling is not full

Scenario: Required overflow fails explicitly
  Given required evidence whose serialized form exceeds the configured profile ceiling
  When projection cannot preserve it
  Then a typed required-evidence budget error is returned instead of a partial success

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track B5
- prerequisite quality gate: knowledge/requirements/req-atlas-query-quality-regression.md
- predecessor query contract: knowledge/requirements/req-atlas-explore-flow-impact.md
- human approval: latest reviewed roadmap implementation goal, 2026-07-21
- contract: specs/task-atlas-query-context-compiler.spec.md

