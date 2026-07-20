---
title: "Knowledge Liveness Layer"
type: concept
source_files:
  - knowledge/requirements
  - src/spec_knowledge
tags:
  - kll
  - liveness
status: draft
---

# Knowledge Liveness Layer

## Role

Long-lived requirements and decisions with traceable liveness evidence.
The runtime-boundary requirement records why query hints are useful while also
forbidding those hints from becoming KLL, binding, lifecycle, or archive
authority. Passing tests and stored trace evidence, not a heuristic candidate,
establish whether the requirement remains honored.
The accepted B5 requirement similarly governs query-context priorities,
restricted-source admission, and receipt honesty. Emitted context JSON remains
derived working evidence; it cannot update requirement status or prove
liveness without the linked Contract and lifecycle evidence.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 was reviewed here; committed graph generations remain derived
evidence and do not replace KLL requirement truth.

Atlas B5 was reviewed here; its accepted KLL clause is normative while graph
projections and E3 receipts remain governed implementation evidence.
