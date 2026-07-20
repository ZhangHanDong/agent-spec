---
kind: requirement
id: REQ-ATLAS-RUNTIME-BOUNDARY-HINTS
title: "Rust Atlas Runtime Boundary Hints"
status: accepted
liveness: auto
tags: [atlas, flow, runtime, boundary, query-hint]
---

# Rust Atlas Runtime Boundary Hints

## Problem

Rust Atlas can prove static and bounded-candidate paths, but a flow may stop at an async spawn,
channel, callback registry, reflection site, or framework route whose continuation is selected at
runtime. Reporting only `no-path` hides the useful boundary; inventing a graph edge would poison
impact, lifecycle, and KLL evidence with an unproved relationship.

## Requirements

[REQ-ATLAS-RUNTIME-BOUNDARY-QUERY-WHEN] Runtime-boundary detection MUST run only for a static flow that has no complete path.

[REQ-ATLAS-RUNTIME-BOUNDARY-QUERY-ONLY] Runtime-boundary detection MUST emit query hints without modifying graph shards, query indexes, graph fingerprints, or deterministic impact.

[REQ-ATLAS-RUNTIME-BOUNDARY-AST] The Rust detector MUST use the parsed `syn` AST of source-backed function bodies.

[REQ-ATLAS-RUNTIME-BOUNDARY-AST-TEXT] The Rust detector MUST NOT match comments, string contents, or arbitrary text with a regex-only source scan.

[REQ-ATLAS-RUNTIME-BOUNDARY-AST-OWNER] A detected site MUST belong to the unique function AST selected for its graph node.

[REQ-ATLAS-RUNTIME-BOUNDARY-AST-AMBIGUOUS] An ambiguous function AST selection MUST NOT produce a hint.

[REQ-ATLAS-RUNTIME-BOUNDARY-AST-SIGNATURE] Function AST selection MUST use one whitespace canonicalization for every signature input.

[REQ-ATLAS-RUNTIME-BOUNDARY-PATH] A scanned source path MUST remain within the canonical code root.

[REQ-ATLAS-RUNTIME-BOUNDARY-HASH] A scanned source file MUST have a current blake3 hash equal to its `Meta.files` value.

[REQ-ATLAS-RUNTIME-BOUNDARY-FRESHNESS] Stale, missing, escaped, non-UTF-8, or unparseable source MUST NOT produce a hint.

[REQ-ATLAS-RUNTIME-BOUNDARY-STALE-FRONTIER] A source node whose current hash differs from `Meta.files` MUST NOT contribute descendants to the runtime-boundary scan frontier.

[REQ-ATLAS-RUNTIME-BOUNDARY-SEMANTIC-FRONTIER] A SCIP or MIR edge MUST expand the runtime-boundary scan frontier only while its corresponding authority layer is fresh.

[REQ-ATLAS-RUNTIME-BOUNDARY-MECHANISMS] The first detector version MUST distinguish these mechanisms: async task spawn; channel send; callback registry; reflection; framework route.

[REQ-ATLAS-RUNTIME-BOUNDARY-SERVICE-HANDLER] Framework-route detection MUST preserve the handler continuation from both `route(path, handler)` and one-argument `service(handler)` forms.

[REQ-ATLAS-RUNTIME-BOUNDARY-RECEIVER-ROLE] A receiver role MUST match a whole or underscore-delimited AST identifier.

[REQ-ATLAS-RUNTIME-BOUNDARY-RECEIVER-CHAIN] A receiver role MUST originate from the callable or access chain that produces the method receiver.

[REQ-ATLAS-RUNTIME-BOUNDARY-RECEIVER-LITERAL] Literal contents MUST NOT contribute receiver roles.

[REQ-ATLAS-RUNTIME-BOUNDARY-SITE] Each hint MUST retain the exact source site, normalized expression form, optional static key, plus unresolved candidate text.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-INDEX] Candidate continuations MUST resolve through the existing query index.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-CONTEXT] Candidate lookup MUST prefer exact source-module matches.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-FALLBACK] Global suffix fallback MUST run only when no contextual exact match exists.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-CRATE] Candidate lookup MUST canonicalize `crate` paths from the source package.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-SELF-MODULE] Candidate lookup MUST canonicalize `self` paths from the source module.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-SUPER-MODULE] Candidate lookup MUST canonicalize `super` paths from the source module.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-TRAIT-MODULE] Candidate lookup in a default trait method MUST canonicalize lowercase `self` and `super` paths from the trait declaration module.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-TRAIT-SELF] Candidate lookup in a default trait method MUST resolve `<Self as Trait>::member` against the indexed trait declaration rather than requiring an implementation self type.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-SELF-TYPE] Candidate lookup MUST canonicalize `Self` paths from the source implementation.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-QUALIFIED] Candidate lookup MUST resolve a qualified-self continuation against its complete `<self type as trait>::member` identity.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-GENERICS] Candidate lookup MUST retain generic arguments in every qualified-self path component.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-GENERIC-TYPE] A generic reflection candidate MUST resolve to its indexed type declaration without changing the preserved candidate text.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-REFLECTION-NAMESPACE] A reflection candidate MUST resolve only to declarations in the Rust type namespace.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-CALLABLE-NAMESPACE] An async-task, callback-registry, or framework-route candidate MUST resolve only to indexed function declarations.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-INHERENT] A callable path in `crate::Type::method`, `self::Type::method`, or source-relative `Type::method` form MUST resolve to the matching indexed inherent-implementation method.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-ORDER] Candidate continuations MUST remain canonically sorted plus unique.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-LIMIT] A hint MUST contain at most 16 resolved candidate nodes.

[REQ-ATLAS-RUNTIME-BOUNDARY-CANDIDATE-OVERFLOW] Candidate overflow MUST return no arbitrary candidate prefix as a complete set.

[REQ-ATLAS-RUNTIME-BOUNDARY-NODE-LIMIT] A query MUST scan at most 8 function nodes in source-first static-reachability order.

[REQ-ATLAS-RUNTIME-BOUNDARY-BYTE-LIMIT] A query MUST scan at most 200000 source bytes.

[REQ-ATLAS-RUNTIME-BOUNDARY-SOURCE-CACHE] Frontier construction and AST scanning MUST share one hash-validated per-file source cache so node and byte limits apply before additional source reads.

[REQ-ATLAS-RUNTIME-BOUNDARY-HINT-LIMIT] A query MUST emit at most 4 hints.

[REQ-ATLAS-RUNTIME-BOUNDARY-LIMITS] Limit exhaustion MUST be explicit without creating a partial candidate edge.

[REQ-ATLAS-RUNTIME-BOUNDARY-OUTPUT] Flow output MUST mark every runtime boundary as `query-hint` with heuristic confidence.

[REQ-ATLAS-RUNTIME-BOUNDARY-DIAGNOSTIC] Flow output MUST add an `atlas-flow-runtime-boundary` diagnostic that states the static path ends at runtime dispatch.

[REQ-ATLAS-RUNTIME-BOUNDARY-INERT] A found, ambiguous-endpoint, unknown-endpoint, or traversal-truncated flow MUST NOT scan source or emit runtime-boundary hints.

[REQ-ATLAS-RUNTIME-BOUNDARY-REGRESSION] The E3 query-quality corpus MUST score the expected path, candidate, evidence, plus exact live-flow diagnostic on an offline fixture with a fresh resolved helper edge.

[REQ-ATLAS-RUNTIME-BOUNDARY-NON-AUTHORITY] Runtime-boundary hints plus their candidate continuations MUST NOT satisfy requirement bindings, lifecycle symbols, archive evidence, or deterministic affected paths.

[REQ-ATLAS-RUNTIME-BOUNDARY-NEGATIVE] Satisfying specs MUST cover these cases: comments or strings; literals inside receiver expressions; unrelated role-named call arguments; receiver suffix lookalikes; unrelated calls; same-line sibling functions; one-argument service handlers; competing same-name module candidates; same-name type and function declarations; relative candidate paths; inherent associated-method paths; default-trait module paths; default-trait qualified `Self`; generic qualified-self candidate paths; generic reflection types; qualified signatures; stale source frontier; stale semantic edge frontier; connected flow suppression; pre-read node and byte budgets; bounded candidates; bounded scan output; byte-stable graph state around a hinted query.

[REQ-ATLAS-RUNTIME-BOUNDARY-WIKI] The tracked Atlas wiki MUST label runtime candidates as query hints.

[REQ-ATLAS-RUNTIME-BOUNDARY-WIKI-NON-EVIDENCE] The tracked Atlas wiki MUST NOT classify runtime candidates as graph facts or lifecycle evidence.

## Dependencies

- REQ-ATLAS-EXPLORE-FLOW-IMPACT
- REQ-ATLAS-DYNAMIC-DISPATCH
- REQ-ATLAS-QUERY-QUALITY-REGRESSION

## Scenarios

Scenario: Disconnected flow names the runtime boundary without inventing an edge
  Given a fresh Rust function registers a statically named callback but the graph has no complete path to it
  When Atlas evaluates the flow
  Then the JSON output contains the callback-registry site and candidate with `authority: query-hint`
  And the persisted graph files remain byte-identical

Scenario: Connected flow remains authoritative and quiet
  Given the graph already contains a complete path between the requested endpoints
  When Atlas evaluates the flow
  Then the flow remains found and contains no runtime-boundary hint

Scenario: Fresh semantic helper edges preserve the caller boundary
  Given fresh SCIP resolves a static call from the source function to a registration helper but not its runtime continuation
  When Atlas evaluates a disconnected flow from that source to the registered callback
  Then Atlas scans the source before descending and returns its callback-registry hint

Scenario: Same-line functions retain separate runtime bodies
  Given a dispatch function and an unrelated source function share one source line
  When Atlas evaluates a disconnected flow from the unrelated function
  Then Atlas emits no runtime boundary from the sibling dispatch body

Scenario: Qualified signatures retain their function body
  Given a graph function signature contains a `crate` qualified parameter type
  When Atlas selects the fresh source body for a disconnected flow
  Then the flow JSON contains one callback-registry runtime boundary

Scenario: Receiver suffix lookalikes remain inert
  Given unrelated receiver names `ctx` and `syllabus` call methods named `send` and `register`
  When the Rust detector scans their function body
  Then the scanner contract test exits zero only when the hint list is empty

Scenario: One-argument service registration preserves its continuation
  Given a route receiver invokes `service(get(handler))`
  When the Rust detector scans the method call
  Then the scanner contract test exits zero only when `candidate_texts` equals `["handler"]`

Scenario: Rust-relative and qualified-self candidate paths resolve in source context
  Given indexed candidate nodes include generic qualified-self handlers, competing trait implementations exist, and continuation text uses every supported relative form
  When Atlas resolves the preserved candidate text through its query index
  Then the resolver contract test exits zero only when the node id list equals `module,parent,qualified,root,self` and `candidates_truncated` is false

Scenario: Generic reflection candidates resolve to their type declaration
  Given reflection preserves candidate text `Message<u8>` and the index contains `Message`
  When Atlas resolves the candidate text
  Then the resolver contract test exits zero only when the candidate id equals `message` and truncation is false

Scenario: Reflection candidate lookup respects the Rust type namespace
  Given a module legally declares a type and a function with the same `Message` symbol
  When Atlas resolves reflection candidate text `Message`
  Then the resolver contract test exits zero only when the candidate id list contains the type declaration
  And the same-symbol function is rejected before the candidate limit is applied

Scenario: Inherent associated callbacks resolve through implementation symbols
  Given a built graph indexes `crate::Handler::callback` as an inherent implementation method
  When Atlas evaluates a disconnected flow from its callback registration
  Then the flow contract test exits zero only when the candidate is the indexed inherent method

Scenario: Suffix fallback does not override a source-module exact match
  Given the source module and a sibling module both declare `handler`
  When Atlas resolves bare candidate text `handler`
  Then the resolver contract test exits zero only when the candidate id list equals `local`
  And the sibling suffix candidate is rejected

Scenario: Stale source cannot become a runtime hint
  Given a graph-backed function body has changed since the graph was built
  When frozen flow cannot find a complete static path
  Then Atlas preserves stale authority diagnostics and emits no hint from the changed body

Scenario: Stale source cannot expose a fresh descendant hint
  Given a changed source function retains an obsolete graph edge to an unchanged helper
  When Atlas builds the runtime-boundary scan frontier
  Then neither the stale source nor the helper contributes a runtime hint

Scenario: Stale semantic edges cannot expand the scan frontier
  Given source files remain fresh but the SCIP index fingerprint no longer matches
  When frozen flow evaluates a persisted helper edge from that SCIP layer
  Then Atlas reports stale semantic authority and emits no helper runtime hint

Scenario: Default trait candidates use the declaration module
  Given a default trait method registers `self::handler` from a nested module
  When Atlas resolves its runtime candidate
  Then the candidate is the module function rather than a member under the trait namespace

Scenario: Default trait qualified Self resolves to the trait declaration
  Given a default trait method registers `<Self as Runner>::handler`
  When Atlas resolves its runtime candidate
  Then the candidate is the indexed `Runner::handler` trait member

Scenario: Runtime candidates remain bounded
  Given one runtime boundary has more than 16 symbols from suffix fallback
  When Atlas resolves candidate text through the query index
  Then the JSON output contains an empty resolved candidate set and `candidates_truncated: true`

Scenario: Scan limits bound frontier source reads
  Given a reachable frontier exceeds eight functions or its next source file exceeds the remaining 200000-byte budget
  When Atlas constructs the runtime-boundary frontier
  Then the frontier contract test exits zero only when it returns eight nodes, no oversized-file cache entry, and truncation

Scenario: Wiki does not promote runtime hints
  Given tracked Atlas architecture and authority articles
  When the contract test scans their exact authority terms
  Then the contract test exits non-zero if any page omits `query-hint`, `graph`, `lifecycle`, or explicit negation

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track A4.1
- reference implementation: codegraph v1.3.1, `src/mcp/dynamic-boundaries.ts`, `src/mcp/tools.ts`, and `__tests__/dynamic-boundaries.test.ts`, commit e552dc2
- human approval: latest-roadmap implementation goal, 2026-07-20
- contract: specs/task-atlas-runtime-boundary-hints.spec.md
