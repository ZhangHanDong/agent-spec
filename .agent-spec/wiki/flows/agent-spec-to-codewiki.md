---
title: "agent-spec adapts codewiki methodology"
type: project-flow
flow_id: agent-spec-to-codewiki
projects:
  - agent-spec
  - codewiki
kind: adapts-methodology-from
protocols:
  - source-review
  - markdown
requirements:
  - REQ-CODE-LIVE-WIKI
specs:
  - specs/task-code-live-wiki.spec.md
source_files:
  - knowledge/requirements/req-code-live-wiki.md
  - src/spec_wiki/live.rs
  - skills/agent-spec-wiki/SKILL.md
external_sources:
  - rust-agents/codewiki
tags:
  - methodology
  - wiki
---

# agent-spec adapts codewiki methodology

## Mechanism

The code live wiki borrows repository-oriented documentation structure and
maintenance ideas from `codewiki`. The deterministic Rust CLI owns discovery,
inventory, staleness, lint, and derived artifacts; the wiki skill owns agent
interpretation and maintained prose.

## Boundary

`rust-agents/codewiki` is an evidence label only. Current behavior is guarded by
the local requirement, Task Contract, implementation, and tests listed above.
