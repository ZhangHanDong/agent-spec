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

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.
