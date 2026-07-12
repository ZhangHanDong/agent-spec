---
kind: requirement
id: REQ-INTENT-COMPILER-YAML-EXPORT
title: "Intent Compiler YAML Export"
status: proposed
liveness: auto
tags: [intent-compiler, yaml, export, projection]
---

# Intent Compiler YAML Export

## Problem

Human-confirmed requirements are hand-owned canonical IR under
`knowledge/requirements/`. External reference-style tooling consumes
requirement trees as `requirements.yaml`. The intent compiler can import that
dialect but cannot produce it, so confirmed requirements cannot be projected
back out to YAML-world without hand-maintained duplicates that drift. The
compiler needs a deterministic exporter: confirmed IR in, compatible YAML
projection out, with an explicit account of anything the dialect cannot carry.

## Requirements

[REQ-INTENT-COMPILER-YAML-EXPORT-COMMAND] `agent-spec requirements export` MUST render requirement documents to a `.yaml`/`.yml` target selected by file extension.

[REQ-INTENT-COMPILER-YAML-EXPORT-SCOPE] Export MUST cover `kind: requirement` documents with status `proposed` or `accepted`; superseded, deprecated, and rejected documents MUST be excluded with a diagnostic.

[REQ-INTENT-COMPILER-YAML-EXPORT-FILTER] An `--id` filter MUST restrict export to the named requirement ids.

[REQ-INTENT-COMPILER-YAML-EXPORT-MAP-FOLDER] Each requirement document MUST map to one top-level FOLDER node carrying its id, title, status, and Problem text as `description`.

[REQ-INTENT-COMPILER-YAML-EXPORT-MAP-LEAF] Each `[REQ-<DOC>-<SUFFIX>]` clause MUST map to one ATOMIC leaf whose id is the lowercased suffix and whose `statement` is the clause text.

[REQ-INTENT-COMPILER-YAML-EXPORT-MAP-TITLE] Leaf titles MUST be synthesized deterministically from the clause suffix.

[REQ-INTENT-COMPILER-YAML-EXPORT-MAP-DOC-LEVEL] Document-level `## Dependencies` entries and `## Scenarios` blocks MUST export as FOLDER-level `dependencies` and `scenarios`.

[REQ-INTENT-COMPILER-YAML-EXPORT-DIALECT-SYMMETRY] The YAML frontend MUST accept FOLDER-level `dependencies` and `scenarios` and render them to the same document sections the exporter reads.

[REQ-INTENT-COMPILER-YAML-EXPORT-FIXPOINT] Exporting, importing the exported file, and exporting again MUST produce byte-identical YAML.

[REQ-INTENT-COMPILER-YAML-EXPORT-UNSUPPORTED] A clause id that does not extend its document id, or scalar content the dialect cannot carry, MUST fail the export with a diagnostic; no file is written.

[REQ-INTENT-COMPILER-YAML-EXPORT-LOSSY] Content the dialect cannot express (Source Trace entries, tags, lint acknowledgements, multi-paragraph formatting) MUST be reported in a lossiness diagnostic rather than dropped silently.

[REQ-INTENT-COMPILER-YAML-EXPORT-DERIVED] The exported file is a derived projection: export MUST overwrite the target deterministically, and a `--check` mode MUST exit non-zero when the target drifts from a fresh render.

[REQ-INTENT-COMPILER-YAML-EXPORT-IR-FROZEN] Export MUST NOT modify any knowledge document or change the Requirement IR schema.

[REQ-INTENT-COMPILER-YAML-EXPORT-NEGATIVE] Satisfying specs MUST include negative scenarios covering nonconforming clause ids, inexpressible scalars, excluded statuses, and `--check` drift.

## Dependencies

- REQ-INTENT-COMPILER-YAML-FRONTEND

## Source Trace

- decision origin: intent-compiler flow review 2026-07-12 — confirmed requirements are hand-owned canonical IR; YAML is a projection (import side entrance shipped, export direction staged here)
- dogfood evidence motivating the export direction: docs/intent-compiler/dogfood-rust-atlas.md
- staged contract: specs/roadmap/task-intent-compiler-yaml-export.spec.md
