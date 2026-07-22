# Rust Atlas Query Context Compiler

`atlas context` compiles one explicit Rust graph query into bounded Agent
context. It separates candidate retrieval from response projection so callers
can tell whether missing evidence was never retrieved or was omitted to meet a
profile threshold or byte ceiling.

The command is additive. It does not replace `atlas explore`, does not mutate
the graph or KLL, and is not a default MCP tool.

## Workflow

Build the graph, select one profile, and use a frozen read for review:

```bash
agent-spec atlas build --code . --graph .agent-spec/graph
agent-spec atlas context rust_atlas::compile_context \
  --profile symbol --code . --graph .agent-spec/graph --frozen
agent-spec atlas context "entry dispatch calls" \
  --profile flow --code . --graph .agent-spec/graph --frozen
```

The query parser recognizes explicit Rust identifiers, repository-relative
paths, and the relation words `calls`, `callers`, `callees`, `references`,
`uses-type`, `implements`, and `contains`. The profile is always explicit. The
parser does not call a model or write inferred intent into the graph.

Direct execution is the default. D4 adds an explicit worker path after a daemon
has been started with non-zero query workers:

```bash
agent-spec atlas context "entry dispatch calls" \
  --profile flow --execution worker \
  --code . --graph .agent-spec/graph --frozen
```

`--fallback-direct` is valid only with worker execution and records the failed
worker attempt separately from the complete direct result. The hidden
`atlas_context` MCP tool requires `AGENT_SPEC_MCP_ATLAS_CONTEXT=1`; its
concurrent route additionally requires
`AGENT_SPEC_MCP_ATLAS_QUERY_MODE=worker`. See
[Rust Atlas Concurrent Query Serving](atlas-concurrent-query-serving.md).

## Profiles

| Profile | Candidate focus | Candidates | Paths | Source slices | Lines | Bytes | Relevance |
|---|---|---:|---:|---:|---:|---:|---:|
| `symbol` | Exact declaration, signature, callers, callees, implementation | 256 | 8 | 8 | 32 | 16000 | 300 |
| `flow` | Primary spine, alternatives, edge sites, runtime boundaries | 384 | 16 | 16 | 48 | 32000 | 250 |
| `architecture` | Modules, relationships, representative implementations | 512 | 8 | 8 | 32 | 24000 | 200 |
| `impact` | Reverse paths, dependents, unresolved frontier | 384 | 16 | 12 | 40 | 24000 | 250 |

Use `--max-bytes 1024..=1000000` or `--min-score 0..=65535` only when an
experiment needs an explicit override. The receipt records the effective
values. A byte limit is a ceiling, not a target: candidates below the relevance
threshold remain omitted even when unused bytes remain.

## Two Stages

```text
QueryIntent
  -> RetrievalCandidateSet + scores and reasons
  -> EvidencePriorityPlan
  -> relevance gate
  -> hash-verified line slices
  -> optional evidence byte pruning
  -> ContextProjection + OmissionManifest + QueryReceipt
```

Retrieval operates on one query index and gives each candidate a stable
evidence id, class, score, reason, and required flag. A defensive candidate cap
is reported as retrieval loss. Projection applies the profile threshold before
the byte ceiling. Its stable order is required evidence, profile class rank,
descending score, then evidence id.

Exact user symbols, explicit failure evidence, primary spine, runtime-boundary
sites, and unique implementations are required. If required candidates, source
spans, or serialized bytes cannot fit, the command fails with a typed
`atlas-context-required-*` error. It never converts required-evidence loss into
a partial success. Only optional off-spine sibling bodies may be reduced to a
signature skeleton.

Test, generated, and vendored source bodies have a separate admission rule.
They are projected only when the query names the repository path or symbol, or the evidence
lies on the primary flow, impact, or runtime-boundary spine. Incidental matches
remain provenance-bearing signature skeletons, and the receipt reports the
`policy_skeletonized` count.

Source content is a line slice around a graph symbol span or edge site. Atlas
normalizes the path and compares current bytes with the selected generation's
file hash first. A mismatch returns `atlas-context-stale-source`, preserves the
graph skeleton, omits source text, and sets `read_back_required`.

## Omissions And Continuation

Each omission entry records an evidence class, reason, count, highest-scored
candidate, and literal argv. Execute that argv directly; do not parse it as a
shell string. For example:

```text
agent-spec atlas context <query> --profile flow \
  --expect-graph <fingerprint> --after <evidence-id> --max-bytes 64000
```

The next process reconstructs the candidate order, verifies the graph
fingerprint, and resumes after the stable id before applying the retrieval cap.
`START` is the explicit beginning cursor. There is no process-local cursor.
Changed fingerprints and unknown evidence ids fail with
`atlas-context-graph-mismatch` or `atlas-context-cursor`.

## Receipt

`receipt.retrieval` records total, eligible, returned, cursor-prefix, hard-cap,
and coverage counts. `receipt.projection` records above-relevance, retained,
below-threshold, byte-omitted, cursor-prefix, skeleton, and retention counts.
`policy_skeletonized` accounts for incidental test/generated/vendor bodies that
remain skeletons under the source-admission rule.
The outer receipt records the exact profile and limits, serialized bytes,
truncated evidence classes, graph fingerprint, read-back requirement,
follow-up count, and `light | traversal | source-heavy | mixed` load profile.

The load profile is deterministic queue metadata consumed by D4. B5 itself
does not own workers, transport routing, or backpressure.

## Offline Regression Evidence

The E3 corpus version `2026-07-21.1` contains six B5 observations: all four
profiles, projection pressure, and stale source. The live fixture probe rebuilds
the same graph in the default test suite. At delivery, the four normal profiles
retrieved 13, 23, 12, and 6 candidates. The 8000-byte architecture case retained
3 of 12 candidates, omitted 9 optional items across two evidence classes, and
serialized to 7267 bytes. The stale case returned
`atlas-context-stale-source`, three follow-up classes, and
`read_back_required: true`.

These are regression observations, not proof of Agent productivity. D4's
worker and MCP context paths remain opt-in prototypes; default MCP surface,
profile, and worker promotion still require the E1 real-Agent A/B gate.
