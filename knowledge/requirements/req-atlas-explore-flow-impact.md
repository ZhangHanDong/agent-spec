---
kind: requirement
id: REQ-ATLAS-EXPLORE-FLOW-IMPACT
title: "Atlas Explore, Flow, Impact, and Affected Queries"
status: accepted
liveness: auto
tags: [atlas, explore, flow, impact, affected, evaluation]
---

# Atlas Explore, Flow, Impact, and Affected Queries

## Problem

Atlas exposes deterministic low-level search and adjacency queries, but an agent
still has to assemble architecture context, source excerpts, flow paths, and
change impact through repeated calls. The graph already contains enough indexed
facts for a bounded composition layer, but that layer must preserve evidence,
freshness, ambiguity, and output budgets instead of synthesizing untraceable
answers.

## Requirements

[REQ-ATLAS-EXPLORE-DETERMINISTIC] `atlas explore` MUST compose only indexed Atlas facts and repository source bytes; it MUST NOT invoke an LLM, access the network, or create a second canonical graph store.

[REQ-ATLAS-EXPLORE-INPUT] Explore input MUST deterministically extract code identifiers and repository paths, preserve ordered ambiguity candidates, and report a typed no-match diagnostic when no seed can be resolved.

[REQ-ATLAS-EXPLORE-BUDGET] Explore MUST provide fixed `compact` and `deep` budget profiles, report limits and usage, and mark deterministic truncation without emitting invalid or partially serialized JSON; if mandatory evidence alone exceeds the hard byte cap, the whole query MUST return a typed budget error.

[REQ-ATLAS-EXPLORE-SOURCE] A source excerpt MUST be emitted only when the selected file's current blake3 hash matches the graph's recorded hash; a stale, missing, symlink-escaped, or out-of-root file MUST produce a diagnostic and no excerpt.

[REQ-ATLAS-EXPLORE-EVIDENCE] Every returned relationship or path hop MUST preserve edge site, provenance, resolution, extractor, dispatch, confidence, candidates, and evidence.

[REQ-ATLAS-EXPLORE-STATUS] Explore, flow, impact, and affected results MUST embed the shared `AtlasStatus`, preserve the compatible top-level syn stale mirror, and reject worktree mismatch through the existing authority boundary.

[REQ-ATLAS-FLOW-PATHS] `atlas flow --from <symbol> --to <symbol>` MUST return deterministic shortest and highest-confidence paths within explicit depth and expansion limits.

[REQ-ATLAS-FLOW-THROUGH] `atlas flow --through <symbol>` MUST return bounded deterministic incoming-through-outgoing spines and retain alternative paths introduced by bounded candidates.

[REQ-ATLAS-FLOW-DIAGNOSTICS] Flow MUST resolve unknown or ambiguous endpoints before traversal and distinguish found, no-path, required-capability-unavailable, unknown-endpoint, ambiguous-endpoint, and traversal-truncated outcomes without presenting an incomplete search as no-path; an existing syn path MUST remain found when SCIP is unavailable, while an unresolved no-path query MUST be capability-unavailable until SCIP evidence exists.

[REQ-ATLAS-IMPACT] `atlas impact <symbol>` MUST reverse-traverse calls, references, type use, impl, and containment context, returning each affected node with minimum distance and an evidence path.

[REQ-ATLAS-IMPACT-CONTAINMENT] Impact MUST expand members of a changed container at the same dependency distance and MUST NOT climb from a leaf to its container and re-expand unrelated siblings.

[REQ-ATLAS-AFFECTED-INPUT] Affected input MUST normalize repository-relative, `./`-prefixed, and in-root absolute paths to the same key; parent traversal, out-of-root absolute paths, and escaping symlinks MUST be rejected.

[REQ-ATLAS-AFFECTED-VCS] The CLI MUST accept exactly one affected input mode from explicit paths, stdin, staged changes, worktree changes, or a commit range; VCS modes MUST use argv-based Git invocation without shell evaluation.

[REQ-ATLAS-QUERY-CLI] The CLI MUST constrain explore to `compact|deep`, flow to either paired from/to or one through endpoint, impact to one symbol, and affected to exactly one input mode before performing I/O.

[REQ-ATLAS-AFFECTED-HONESTY] Affected output MUST remain provider-neutral code impact and MUST NOT infer deterministic tests from filename patterns; test selectors remain the responsibility of the Intent-Code Linker and test obligations.

[REQ-ATLAS-EXPLORE-MCP] MCP `atlas_explore` MUST remain hidden by default and become discoverable only through an explicit opt-in environment variable until real Atlas A/B evidence approves a default surface change.

[REQ-ATLAS-EVAL-QUERY-METRICS] Atlas evaluation receipts MUST record a query-metric schema version together with response bytes, read-back calls, follow-up queries, and truncated queries; legacy receipts MAY remain readable but MUST be counted separately and MUST NOT contribute synthetic zero samples, while partially populated query metrics MUST be rejected.

[REQ-ATLAS-EXPLORE-NEGATIVE] Satisfying specs MUST cover unshrinkable output, stale/missing/escaping excerpt omission, ambiguous or unavailable flow, traversal exhaustion, worktree mismatch, zero/conflicting/failed affected VCS modes, option-like revisions, and forbidden filename-based test inference.

## Dependencies

- REQ-ATLAS-AGENT-EVALUATION
- REQ-ATLAS-EDGE-EVIDENCE-INDEX
- REQ-ATLAS-WORKTREE-FRESHNESS

## Scenarios

Scenario: Compact explore is deterministic and bounded
  Given a graph whose relevant neighborhood exceeds the compact profile
  When the same explore query runs twice
  Then both JSON results are byte-identical, within the compact limit, and carry the same truncation diagnostics

Scenario: Explore composes ranked evidence rather than a seed-only shell
  Given a query matching source paths, identifiers, relationships, flows, and reverse dependents
  When explore builds a compact result with sufficient budget
  Then it returns ranked seeds, verified excerpts, evidence-preserving edges, path spines, alternatives, and impact entries in deterministic order

Scenario: Stale source is not mixed into graph context
  Given a selected node whose source hash differs from the graph metadata
  When frozen explore renders its source context
  Then the node remains stale graph evidence while its source excerpt is omitted with a typed diagnostic

Scenario: Flow preserves path quality and uncertainty
  Given multiple routes with different hop counts, confidence, and bounded candidates
  When Atlas computes from-to and through flows
  Then it returns deterministic shortest, highest-confidence, and alternative candidate paths with complete edge evidence

Scenario: Impact reports explainable distance without sibling explosion
  Given a changed leaf and a changed container in the same graph
  When impact runs for each seed
  Then every dependent has a minimum distance and evidence path while unrelated siblings are absent from leaf impact

Scenario: Affected paths normalize safely
  Given one repository file named as relative, dot-prefixed, and absolute input
  When affected runs for each spelling
  Then all three results are byte-identical and an out-of-root spelling is rejected

Scenario: Default MCP surface remains unchanged
  Given no Atlas explore opt-in environment variable
  When MCP tools are listed
  Then `atlas_explore` is absent while explicit opt-in lists the bounded deterministic tool

Scenario: New query surfaces preserve authority and safe VCS boundaries
  Given a mismatched worktree or an invalid affected VCS mode
  When explore, flow, impact, or affected begins a query
  Then it rejects the request before returning graph facts, reading stdin, or interpreting a shell string

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track B2/B3/B4 and E1
- reference implementation: codegraph v1.3.1 explore, traversal, affected, and adaptive-budget tests, commit e552dc2
- human approval: latest-roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-explore-flow-impact.spec.md
