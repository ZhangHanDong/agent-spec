---
title: "Cross-Project Wiki"
type: concept
source_files:
  - knowledge/requirements/req-cross-project-wiki.md
  - specs/task-cross-project-wiki.spec.md
  - src/spec_wiki/project_map.rs
  - src/spec_wiki/live.rs
tags:
  - wiki
  - cross-project
  - data-flow
status: maintained
---

# Cross-Project Wiki

## Purpose

Record an important project's role and the mechanisms between projects without
treating external repositories as files owned by the current repository.

## Truth And Derivation

Project and flow Markdown articles are maintained working-memory truth. JSON and
Mermaid project maps are deterministic derived artifacts. They are intentionally
separate from Cargo-derived package/module inventory and from durable KLL truth.

## Reference Boundary

- `source_files`, `requirements`, and `specs` resolve inside the current repo.
- `external_sources` are evidence labels and are never dereferenced by default.
- A flow contains at least two distinct known projects; list order defines edge
  direction through adjacent project pairs.

## Gates

Project-map lint validates article type, stable identities, duplicate ids,
required non-empty article fields, frontmatter syntax, project membership,
canonical repo boundaries, KLL requirement ids, Task Contract level and paths,
target article directories, regular-file policy, portable JSON paths (including
diagnostics), and Mermaid escaping. Live Wiki lint/check also compare both
checked-in project-map artifacts exactly.

## Evidence

The satisfying Task Contract binds happy-path rendering, malformed graph data,
invalid trace references, artifact drift, init/check behavior, cross-CWD
inspection, portable output, repository dogfood, and documentation examples to
real Rust tests.
