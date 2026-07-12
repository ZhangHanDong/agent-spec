---
kind: requirement
id: REQ-INTENT-COMPILER-PROVENANCE
title: "Intent Compiler Compilation Provenance"
status: accepted
liveness: auto
tags: [intent-compiler, provenance, digest]
---

# Intent Compiler Compilation Provenance

## Problem

The YAML frontend and exporter transform requirements deterministically, but
nothing records what was compiled: which input bytes, by which tool version,
producing which output bytes, and whether the transformation reproduces. The
enterprise pipeline requires a compilation provenance artifact binding these
facts so a verifier can later prove an artifact chain without re-trusting the
producer.

## Requirements

[REQ-INTENT-COMPILER-PROVENANCE-OPT-IN] `requirements import` and `requirements export` MUST accept an optional `--provenance <path>` flag naming a `.json` manifest target.

[REQ-INTENT-COMPILER-PROVENANCE-JSON-ONLY] A provenance target without a `.json` extension MUST be a diagnostic.

[REQ-INTENT-COMPILER-PROVENANCE-INPUT] The manifest MUST record the compilation direction, the input path, and the input's blake3 digest.

[REQ-INTENT-COMPILER-PROVENANCE-OUTPUTS] The manifest MUST record every output path with its blake3 digest.

[REQ-INTENT-COMPILER-PROVENANCE-TOOL] The manifest MUST record the tool name, tool version, and dialect schema version.

[REQ-INTENT-COMPILER-PROVENANCE-REPRODUCIBLE] The manifest MUST record a reproducibility result computed by re-rendering the transformation and comparing digests.

[REQ-INTENT-COMPILER-PROVENANCE-VERIFY] A verification function MUST recompute digests from the manifest's paths and report any drifted input or output.

[REQ-INTENT-COMPILER-PROVENANCE-SCHEMA] The manifest shape MUST be documented by a versioned JSON schema under `docs/intent-compiler/schemas/`.

[REQ-INTENT-COMPILER-PROVENANCE-NEGATIVE] Satisfying specs MUST include negative scenarios covering non-JSON targets and drift detection.

## Dependencies

- REQ-INTENT-COMPILER-YAML-FRONTEND
- REQ-INTENT-COMPILER-YAML-EXPORT

## Source Trace

- enterprise requirement: ARC-style compilation provenance in the factory roadmap (input digest, tool digest, output digests, reproducibility)
- decision origin: intent-compiler review 2026-07-12 after reading the enterprise roadmap revision
- contract: specs/task-intent-compiler-provenance-manifest.spec.md
