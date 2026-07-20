# Atlas Query Context Compiler Design

**Status:** Approved by the reviewed `docs/atlas-roadmap.md` B5 contract

**Requirement:** `REQ-ATLAS-QUERY-CONTEXT-COMPILER`

**Task Contract:** `specs/task-atlas-query-context-compiler.spec.md`

## Purpose

B5 separates finding graph evidence from packaging that evidence for an Agent. The graph and its
immutable generation remain authoritative. The context compiler is a deterministic, bounded view
whose receipt makes both retrieval loss and projection loss visible.

## Pipeline

```text
explicit query + profile
  -> QueryIntent
  -> RetrievalCandidateSet + scoring reasons
  -> EvidencePriorityPlan
  -> relevance gate
  -> hash-verified source-span projection
  -> byte-ceiling pruning of optional evidence
  -> ContextProjection + OmissionManifest + QueryReceipt
```

`parse_query_intent` recognizes Rust identifiers, normalized repository paths and a fixed relation
vocabulary. The profile is a typed CLI/API value, not inferred from prose. Unrecognized terms may
be reported for honesty but never become graph facts or hidden ranking instructions.

## Retrieval

Retrieval operates against one reader-leased `QueryIndex` generation. It creates stable candidate
ids from evidence kind plus canonical node/edge/path identity. Candidate scores have explicit
reasons such as `exact-symbol`, `exact-path`, `primary-spine`, `boundary-site`,
`unique-implementation`, `adjacent-structure` and `off-spine-sibling`. A defensive hard cap is
allowed, but its eligible and returned counts belong to the retrieval receipt. The byte budget is
not consulted during this stage.

The four profiles use the same candidate supergraph with profile-specific class priorities:

- `symbol`: named declarations, signatures, location and direct caller/callee summaries;
- `flow`: primary path, alternative paths, edge sites and runtime-boundary hints;
- `architecture`: modules, containment/relationship summaries and representative implementations;
- `impact`: reverse paths, unresolved frontier, test-shaped nodes and binding/test-obligation gaps
  when the consumer supplies them.

## Projection

Projection first discards candidates below the profile threshold. It then orders retained evidence
by required status, class priority, descending score and canonical evidence id. Required evidence
includes exact user-named symbols, primary spine, unique implementation, runtime-boundary or edge
site spans and explicit failure evidence. Optional off-spine sibling bodies may become signature
skeletons when one representative implementation remains.

Source content is a line slice around a graph node span or edge site. Before reading it, the compiler
normalizes the repository-relative path and compares current bytes with the selected generation's
recorded file hash. Mismatch yields a typed stale diagnostic; it never returns unverified source.
The byte budget is a ceiling. Optional evidence is removed in reverse priority until serialized
output fits. If required evidence alone does not fit, projection returns a typed error.

## Omission And Continuation

Omissions are grouped by evidence class and reason (`below-relevance`, `retrieval-cap`,
`byte-ceiling`, `after-cursor` or `source-unavailable`). Each entry carries the count, highest score
candidate and an argv continuation:

```text
agent-spec atlas context <query> --profile <profile>
  --after <stable-evidence-id> --expect-graph <graph-fingerprint>
```

The next process reconstructs and re-sorts candidates from the immutable graph, validates the
fingerprint and resumes after the evidence id. No server-side cursor exists. A missing cursor id or
changed fingerprint is a typed failure.

## Receipt And D4 Load Profile

`QueryReceipt` contains separate `retrieval` and `projection` sections. Retrieval records eligible,
returned and hard-cap omitted candidates. Projection records above-threshold, retained, skeletonized
and omitted candidates. The common section records profile, exact limits, serialized bytes,
truncated classes, graph fingerprint, source read-back requirement and executable follow-ups.

A deterministic load profile (`light`, `traversal`, `source-heavy`, or `mixed`) is derived from
retrieval expansions, path count and source slices. D4 may use this as queue metadata, but B5 does
not add a worker pool or alter transport behavior.

## Compatibility And Promotion

`atlas context` is additive. Existing `explore-v1`, primitive queries and MCP discovery remain
unchanged. B5 results enter the E3 fixed corpus immediately. Making the context compiler a default
MCP tool or changing default profiles requires the separate E1 A/B gate.

