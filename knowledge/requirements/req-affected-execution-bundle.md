---
kind: requirement
id: REQ-AFFECTED-EXECUTION-BUNDLE
title: "Affected Execution Bundle"
status: accepted
liveness: auto
tags: [atlas, intent-compiler, quality]
---

# Affected Execution Bundle

## Problem

An intent-aware impact report identifies what may be affected but does not yet give an
implementation Agent a deterministic, justified set of early checks, acceptance gates,
tests, project guidance, and required skills.

## Requirements

[REQ-AFFECTED-EXECUTION-BUNDLE-INPUT] The bundle MUST consume an intent-impact report and requirement risk without recomputing provider impact.

[REQ-AFFECTED-EXECUTION-BUNDLE-QUALITY] Fast checks and acceptance gates MUST come from typed quality-provider roles and requirement risk, with a reason for every selection.

[REQ-AFFECTED-EXECUTION-BUNDLE-POLICY] Risk A MUST require lifecycle, trace, targeted tests, and adversarial review; risk B MUST require lifecycle and trace; risk C MUST require lifecycle only.

[REQ-AFFECTED-EXECUTION-BUNDLE-PROVIDERS] Every selected quality provider MUST preserve executable, argv, cwd, timeout, and output-limit configuration without shell interpolation.

[REQ-AFFECTED-EXECUTION-BUNDLE-TESTS] Authoritative tests MUST come only from explicit Task Contract selectors; heuristic candidates MAY be shown separately and MUST NOT become acceptance evidence.

[REQ-AFFECTED-EXECUTION-BUNDLE-GUIDANCE] Guidance and required skills MUST be scoped to affected paths; unresolved skills MUST remain typed gaps.

[REQ-AFFECTED-EXECUTION-BUNDLE-RECEIPTS] Immutable skill receipts MUST remain separate from lifecycle, test, and quality evidence.

[REQ-AFFECTED-EXECUTION-BUNDLE-DETERMINISM] Identical inputs MUST produce byte-identical bundle JSON and stable selection reasons.

## Dependencies

- REQ-INTENT-AWARE-AFFECTED
- REQ-QUALITY-PLANNING

## Scenarios

Scenario: Risk selects justified checks and gates
  Given an intent-impact report linked to a risk-classed contract
  When the affected execution bundle is built
  Then every fast check and acceptance gate names its source role and selection reason

Scenario: Risk policy changes the executable bundle
  Given otherwise identical risk A, B, and C contracts
  When affected execution bundles are built
  Then required evidence, selected providers, fast checks, and acceptance gates differ according to the declared risk policy

Scenario: Explicit tests remain authoritative
  Given explicit selectors and heuristic candidates
  When tests are selected
  Then only explicit selectors appear under authoritative tests

Scenario: Missing provider or selector remains blocking context
  Given provider or selector gaps in the intent-impact report
  When the bundle is built
  Then those gaps remain present and no replacement evidence is fabricated

Scenario: Skills are scoped and receipts stay non-evidence
  Given path-scoped guidance naming one installed and one missing skill
  When the bundle is built
  Then both skills are explained while only the installed skill has a receipt and neither receipt counts as acceptance evidence

## Source Trace

- canonical roadmap: docs/atlas-roadmap.md, Track C2
- prerequisite: knowledge/requirements/req-intent-aware-affected.md
- human approval: implement the latest Atlas roadmap, 2026-07-20

## Open Questions

None.
