---
title: "Wiki Working Memory"
type: concept
source_files:
  - skills/agent-spec-wiki/SKILL.md
  - .agent-spec/wiki/_index.md
tags:
  - wiki
  - working-memory
status: maintained
---

# Wiki Working Memory

## Role

Maintained agent-readable wiki pages backed by `source_files`, not durable KLL
truth. Module and concept pages preserve reusable local understanding; project
and flow pages preserve cross-project roles, protocols, mechanisms, and data
flow. Architecture JSON and Mermaid files are derived views checked against
those maintained pages.

The generated index is the entry point for current coverage, including the
Atlas architecture, its explore/flow/impact source ownership, the authority
concept, and the derived-authority decision. Updating an article requires
regenerating that index and refreshing metadata before the wiki check can
confirm source trace freshness.
The Atlas pages also distinguish runtime-boundary working context from durable
KLL and graph authority, so an agent can use the hints without promoting them
to facts.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.
