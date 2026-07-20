---
title: "Task Contract"
type: concept
source_files:
  - README.md
  - AGENTS.md
  - skills/agent-spec-tool-first/SKILL.md
tags:
  - contract
  - workflow
status: draft
---

# Task Contract

## Role

Human-authored task contract that defines intent, decisions, boundaries, and completion criteria.

## Atlas Symbols

When a contract declares Atlas symbols, the graph is derived input to lifecycle
verification. Build the graph first, inspect `atlas status` for worktree and
layer freshness, and rebuild after schema or query-index diagnostics. A stale
available semantic layer or borrowed graph cannot support a definitive symbol
verdict.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.
