---
title: "Deterministic CLI"
type: decision
source_files:
  - specs/task-code-live-wiki.spec.md
  - specs/task-code-live-wiki-deepening.spec.md
  - src/spec_wiki
tags:
  - deterministic
  - non-goal
status: draft
---

# Deterministic CLI

## Role

The CLI performs deterministic local analysis only. It can scaffold wiki files,
index frontmatter, lint links/source paths, inspect requirement/spec relations,
and emit Rust architecture inventory, but it does not call an LLM or network
service to write long-form wiki prose.

Agents may author or update prose after reading source, but those edits remain
reviewable markdown under `.agent-spec/wiki`. This preserves the same boundary
as KLL: durable truth must still be captured in `knowledge/`, executable
contracts in `specs/`, and published explanations in `docs/`.

Non-goals: built-in LLM long-form generation, hosted web UI, cloud sync, and
replacement of the KLL requirements/decision layer.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.
