---
title: "Knowledge Versus Docs"
type: decision
source_files:
  - skills/agent-spec-wiki/SKILL.md
  - AGENTS.md
tags:
  - knowledge
  - docs
  - wiki
status: draft
---

# Knowledge Versus Docs

## Role

Durable truth belongs in knowledge/, executable contracts in specs/, human docs in docs/, and agent working memory in .agent-spec/wiki/.

Atlas graph shards, query indexes, and code bindings are derived working data.
Their freshness and authority gates affect verification, but they do not replace
accepted KLL requirements or decisions.

Reader-facing Atlas commands and budgets belong in README and agent guidance;
the accepted requirement and Task Contract remain the normative statement of
why those query surfaces and honesty boundaries exist.
The wiki may summarize runtime-boundary behavior for agent navigation, while
the KLL requirement owns its normative limits and the dedicated docs page owns
the user-facing command contract.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 was reviewed here; the reader guide remains in `docs/`, the normative
requirement remains in `knowledge/`, and generated graph data remains derived.
