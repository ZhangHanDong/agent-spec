---
kind: requirement
id: REQ-INTENT-CODE-LINKER
title: "Intent-Code Linker (Atlas Slice)"
status: accepted
liveness: auto
tags: [intent-compiler, linker, atlas, lifecycle, trace]
---

# Intent-Code Linker (Atlas Slice)

## Problem

Task Contracts can name code symbols that silently stop existing: nothing
mechanical connects a contract's intent to the program facts it talks about.
The first Linker slice closes that gap for Rust: contracts declare symbols,
lifecycle validates them against a fresh Atlas graph, and trace evidence
records typed, stale-aware code targets — while the Requirement IR stays
independent of derived program facts.

## Requirements

[REQ-INTENT-CODE-LINKER-SYMBOLS-SECTION] Task Contracts MUST declare code-graph references in a `### Symbols` boundary subsection as `- <provider>: <canonical-path>` entries, fully separate from path globs.

[REQ-INTENT-CODE-LINKER-MISSING] Lifecycle verification MUST fail with `atlas-symbol-missing` when a declared symbol is absent from a fresh graph.

[REQ-INTENT-CODE-LINKER-STALE-FIRST] A stale or absent graph MUST fail with `atlas-stale` before any symbol lookup, so a lagging graph never produces false symbol-missing diagnostics.

[REQ-INTENT-CODE-LINKER-SILENT-PASS] Contracts whose declared symbols all resolve MUST produce no atlas diagnostics.

[REQ-INTENT-CODE-LINKER-OPTIONAL] Contracts without symbol declarations MUST NOT require a graph; non-Rust projects carry zero Atlas burden.

[REQ-INTENT-CODE-LINKER-TYPED-TRACE] Persisted trace evidence MUST record typed code targets — provider, node id, node kind, file, provenance, and graph fingerprint — when contract symbols resolve against a fresh graph.

[REQ-INTENT-CODE-LINKER-IR-INDEPENDENT] Requirement documents MUST stay byte-identical through symbol validation and trace persistence; derived Atlas facts never become durable KLL truth.

[REQ-INTENT-CODE-LINKER-READ-ONLY] Symbol validation and fact resolution MUST NOT mutate the graph store.

## Scenarios

Scenario: missing symbol fails lifecycle
  Given a contract declaring a symbol absent from a fresh graph
  When lifecycle validates the contract
  Then diagnostics include atlas-symbol-missing naming the symbol

Scenario: stale graph blocks with a distinct diagnostic
  Given a built graph lagging one modified source file
  When lifecycle validates a contract symbol
  Then diagnostics include atlas-stale and no atlas-symbol-missing

Scenario: typed trace targets persist
  Given a passing run whose contract symbols resolve
  When trace evidence is persisted
  Then each code target records provider, node id, kind, file, provenance, and graph fingerprint

## Dependencies

- REQ-CODE-GRAPH-IR

## Source Trace

- decision origin: target intent-compiler architecture, delivery boundary 3 (`docs/intent-compiler/architecture.md`)
- staged contract: specs/task-atlas-kll-integration.spec.md (authored pre-governance-arc; satisfies link added at kickoff)
