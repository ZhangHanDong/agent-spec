---
title: "Spec Lint"
type: module
source_files:
  - src/spec_lint
tags:
  - lint
  - quality
status: draft
---

# Spec Lint

## Role

Spec quality analysis and contract smell detection.

## Heuristics worth knowing

- `precedence-fallback-coverage` treats any `->` chain in Decisions/Constraints
  as an ordering claim. Rust return arrows are excluded only when the arrow is
  inside a backtick code span with exactly one `->`, the left side ends with
  `)` or `|`, and the right side is a type-shaped token (`bool`,
  `Option<..>`, `ClauseCoverage`). `memory() -> disk` still counts as a chain
  because `disk` is not a type. See `strip_signature_arrows` /
  `is_rust_signature_span` in `linters.rs`.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.
