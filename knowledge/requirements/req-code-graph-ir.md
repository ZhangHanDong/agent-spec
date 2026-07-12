---
kind: requirement
id: REQ-CODE-GRAPH-IR
title: "Provider-Neutral Code Graph IR and Code Bindings"
status: proposed
liveness: auto
tags: [intent-compiler, code-graph, bindings, atlas]
---

# Provider-Neutral Code Graph IR and Code Bindings

## Problem

Rust Atlas produces a Rust-specific project graph, but the Intent-Code Linker
must consume program facts without binding the Requirement IR to one language
or provider. The target architecture requires a provider-neutral Code Graph IR
consumer contract and a typed code-binding artifact so work units can map to
code targets that lifecycle, trace, and impact analysis consume uniformly.

## Requirements

[REQ-CODE-GRAPH-IR-CONSUMER] A provider-neutral consumer contract MUST define the node, edge, provenance, capability, and staleness facts the linker reads, independent of any provider's storage format.

[REQ-CODE-GRAPH-IR-PROVIDER-LABEL] Every consumed fact MUST carry its provider identity and capability labels.

[REQ-CODE-GRAPH-IR-BINDINGS-SCHEMA] A typed code-binding artifact (`.agent-spec/code-bindings.json`) MUST bind requirement id, work unit id, provider, and target nodes (id, kind, file, provenance, graph fingerprint), documented by a versioned JSON schema.

[REQ-CODE-GRAPH-IR-STALENESS] A stale graph MUST block definitive binding rather than silently serving old symbol ownership.

[REQ-CODE-GRAPH-IR-KLL-FROZEN] Bindings are derived working data; the Requirement IR MUST remain byte-identical through binding generation.

[REQ-CODE-GRAPH-IR-NEGATIVE] Satisfying specs MUST include negative scenarios covering stale-graph blocking and unknown provider rejection.

## Dependencies

- REQ-RUST-ATLAS

## Source Trace

- target architecture: docs/intent-compiler/architecture.md (Code Grounding And Intent-Code Linking; delivery boundary 2)
- staged contract: specs/roadmap/task-code-graph-ir-bindings.spec.md
