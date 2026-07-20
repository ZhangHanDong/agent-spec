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

`atlas impact` and `atlas affected` can identify code that deserves review,
but they do not create Contract test selectors. Scenario-to-test obligations
remain explicit in the Contract and are verified by lifecycle.
Likewise, `flow.runtime_boundaries` may guide source inspection but cannot
satisfy a Contract symbol, scenario, or test selector because it carries
query-hint authority rather than graph provenance.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 was reviewed here; generation and resource controls remain execution
mechanics governed by explicit Task Contract scenarios.

Atlas D3 was reviewed here; pending or degraded runtime state does not satisfy
a scenario, and no-daemon lifecycle verification remains supported.
