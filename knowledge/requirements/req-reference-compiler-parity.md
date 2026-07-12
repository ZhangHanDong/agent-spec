---
kind: requirement
id: REQ-REFERENCE-COMPILER-PARITY
title: "Reference Compiler Parity Layout"
status: proposed
liveness: auto
tags: [intent-compiler, parity, compat, layout]
---

# Reference Compiler Parity Layout

## Problem

agent-spec must be able to stand in for the reference requirements compiler:
accept its `requirements.yaml` requirement trees and emit its per-requirement
four-artifact bundle so existing consumers (requirement visualization,
traceability readers) work unchanged. Compatibility is an edge projection — a
named output layout over neutral artifacts — not a shape for the core: core
schemas and the default layout stay provider-neutral, and the reference
project's vocabulary stays confined to the compat layout identifier, compat
fixtures, and the single README acknowledgement.

## Requirements

[REQ-REFERENCE-COMPILER-PARITY-COMPILE-COMMAND] `requirements compile --out <dir>` MUST emit one bundle per selected requirement: the requirement document, a draft task spec, the traceability projection, and a compilation manifest, in the default `agent-spec-v1` layout.

[REQ-REFERENCE-COMPILER-PARITY-LAYOUT-FLAG] `--layout arc-v1` MUST emit the same content under reference-compatible file names `<id>.requirements.md`, `<id>.spec.md`, `<id>.arc.traceability.json`, and `<id>.arc.compilation.json`.

[REQ-REFERENCE-COMPILER-PARITY-UNKNOWN-LAYOUT] An unknown layout identifier MUST exit non-zero with a diagnostic naming the accepted layouts.

[REQ-REFERENCE-COMPILER-PARITY-INPUT-CONFORMANCE] The YAML frontend MUST accept the reference ticketbooking-style requirement tree (FOLDER/ATOMIC nodes, dependencies, scenarios); unsupported constructs keep failing with `yaml-unsupported-construct` naming the construct.

[REQ-REFERENCE-COMPILER-PARITY-ATOMIC-WRITE] Bundle writes MUST be atomic: all artifacts are rendered and validated before the first file lands, and a failed render writes nothing.

[REQ-REFERENCE-COMPILER-PARITY-OVERWRITE] An existing bundle target MUST NOT be overwritten without `--force`; the refusal diagnostic names the colliding files.

[REQ-REFERENCE-COMPILER-PARITY-BUNDLE-DIGEST] The compilation manifest MUST record per-artifact digests plus one bundle digest over the ordered artifact digests, so external admission checks can pin the exact bundle.

[REQ-REFERENCE-COMPILER-PARITY-VOCABULARY] Core schemas and the `agent-spec-v1` layout MUST NOT carry reference-project names; a mechanical check walks `docs/intent-compiler/schemas/` and the neutral layout output for the forbidden token.

[REQ-REFERENCE-COMPILER-PARITY-FIXTURE] A parity fixture modeled on the reference requirement tree MUST hold golden bundles for both layouts, byte-stable across runs.

[REQ-REFERENCE-COMPILER-PARITY-NEGATIVE] Satisfying specs MUST include negative scenarios covering unknown layouts, overwrite refusal, and invalid input trees.

## Dependencies

- REQ-COMPILER-MACHINE-SURFACE
- REQ-PROVENANCE-RUN-HARDENING
- REQ-INTENT-COMPILER-YAML-FRONTEND
- REQ-INTENT-COMPILER-YAML-EXPORT

## Source Trace

- decision origin: openfab spec-side integration review 2026-07-12 — agent-spec replaces the reference compiler via an explicit edge layout; the core stays provider-neutral and orchestrators depend on stable surfaces one-way
- reference input shape: docs/intent-compiler/reference-validation-matrix.md (ticketbooking requirement tree rows)
- staged contract: specs/roadmap/task-reference-compiler-parity.spec.md
