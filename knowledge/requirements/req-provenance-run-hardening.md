---
kind: requirement
id: REQ-PROVENANCE-RUN-HARDENING
title: "Provenance Run Hardening"
status: accepted
liveness: auto
tags: [intent-compiler, provenance, determinism, replay]
---

# Provenance Run Hardening

## Problem

Compilation provenance manifests today cover YAML import/export and record the
tool name and crate version. Standing in for a reference compiler inside a
certification pipeline demands more: the exact compiler build, the effective
command configuration, provenance for every artifact-emitting compile command,
and a one-command replay that proves a recorded compilation still reproduces
byte-identical outputs. The manifest stays a record of deterministic facts —
it carries no approval or authority semantics.

## Requirements

[REQ-PROVENANCE-RUN-HARDENING-BUILD-IDENTITY] Manifests MUST record the compiler build identity: crate version plus the build git commit embedded at compile time, with a documented `unknown` fallback when the build environment provides none.

[REQ-PROVENANCE-RUN-HARDENING-CONFIG] Manifests MUST serialize the effective command configuration (subcommand, flags, input paths) that produced the outputs.

[REQ-PROVENANCE-RUN-HARDENING-COVERAGE] Every `requirements` command that writes an `--out` artifact MUST accept `--provenance <file>` and emit a manifest with input and output digests.

[REQ-PROVENANCE-RUN-HARDENING-REPLAY] `requirements verify-run --manifest <file>` MUST re-run the recorded compilation against a temporary target, byte-compare every output, and exit non-zero naming each drifted file.

[REQ-PROVENANCE-RUN-HARDENING-SCHEMA-V2] New fields land in `compilation-provenance-v2.schema.json`; existing v1 manifests MUST continue to verify unchanged.

[REQ-PROVENANCE-RUN-HARDENING-NO-ORCHESTRATOR-FIELDS] The v2 schema MUST NOT define approval, authority, actor, or policy fields; external systems reference manifest digests from their own records.

[REQ-PROVENANCE-RUN-HARDENING-PURITY] Emitting or verifying provenance MUST leave every knowledge document byte-identical.

[REQ-PROVENANCE-RUN-HARDENING-NEGATIVE] Satisfying specs MUST include negative scenarios covering output drift, a missing manifest file, and a tampered output digest.

## Dependencies

- REQ-INTENT-COMPILER-PROVENANCE

## Source Trace

- decision origin: orchestrator-neutral compiler integration review 2026-07-12 (governing decision: ADR-001) — determinism claims become executable: build identity + configuration + replay
- reference review evidence: reference-architecture review finding "no replayable compilation provenance" (schema, build digest, configuration, replay result)
- staged contract: specs/roadmap/task-provenance-run-hardening.spec.md
