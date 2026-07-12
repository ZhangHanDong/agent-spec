---
kind: requirement
id: REQ-COMPILER-MACHINE-SURFACE
title: "Compiler Machine Surface"
status: proposed
liveness: auto
tags: [intent-compiler, machine-surface, json, governance]
---

# Compiler Machine Surface

## Problem

Orchestrators drive the intent compiler as a subprocess: they need governance
actions and traceability answers as stable, digest-bearing JSON, not printed
prose. Today `requirements transition` prints a text line and no single command
projects requirement → spec → test → verdict → liveness as one machine
document. The gap forces every orchestrator to parse human output or re-derive
the join. The surface must stay orchestrator-agnostic: the compiler reports
facts and digests; approval, identity, and policy binding belong to external
systems that reference those digests.

## Requirements

[REQ-COMPILER-MACHINE-SURFACE-TRANSITION-JSON] `requirements transition --format json` MUST emit one JSON object with the requirement id, prior status, new status, document path, and the document's blake3 digest after the rewrite.

[REQ-COMPILER-MACHINE-SURFACE-SUPERSEDE-JSON] `requirements supersede --format json` MUST emit both documents' paths, statuses, and blake3 digests after the atomic rewrite.

[REQ-COMPILER-MACHINE-SURFACE-FAILURE-SHAPE] A failed transition or supersession MUST exit non-zero with the diagnostic on stderr and MUST NOT print partial JSON on stdout.

[REQ-COMPILER-MACHINE-SURFACE-NO-ACTOR] The JSON output MUST NOT contain actor, identity, authority, approval, or policy fields; external systems bind approvals to the reported digests.

[REQ-COMPILER-MACHINE-SURFACE-TRACEABILITY-COMMAND] `requirements traceability <ID> --format json` MUST project one requirement's clauses, satisfying specs, scenarios, bound tests, latest verdicts, and derived liveness as a single JSON document.

[REQ-COMPILER-MACHINE-SURFACE-TRACEABILITY-SCHEMA] The projection MUST validate against `requirement-traceability-v1.schema.json` published under the `agent-spec/intent-compiler/` schema `$id` namespace.

[REQ-COMPILER-MACHINE-SURFACE-TRACEABILITY-DETERMINISM] Two runs over identical inputs MUST produce byte-identical projections; all collections carry a documented sort order.

[REQ-COMPILER-MACHINE-SURFACE-TRACEABILITY-OUT] `--out <file>` MUST write the projection to disk exactly as printed.

[REQ-COMPILER-MACHINE-SURFACE-UNKNOWN-ID] An unknown requirement id MUST exit non-zero with a diagnostic naming the id.

[REQ-COMPILER-MACHINE-SURFACE-PURITY] Neither JSON emission nor the traceability projection may mutate any knowledge document.

[REQ-COMPILER-MACHINE-SURFACE-NEGATIVE] Satisfying specs MUST include negative scenarios covering illegal transitions under `--format json`, unknown ids, and byte-identical purity.

## Dependencies

- REQ-REQUIREMENT-GOVERNANCE
- REQ-REQUIREMENT-STATUS-QUERY

## Source Trace

- decision origin: openfab spec-side integration review 2026-07-12 — compiler stays orchestrator-agnostic; approval/authority binding is external and references digests
- reference review evidence: live reference-architecture review findings (approval envelopes bind to exact artifact digests; the CLI cannot attest identity)
- staged contract: specs/roadmap/task-compiler-machine-surface.spec.md
