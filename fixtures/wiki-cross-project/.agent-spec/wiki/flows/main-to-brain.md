---
title: "Main to brain-rs context flow"
type: project-flow
flow_id: main-to-brain
projects:
  - agent-spec
  - brain-rs
kind: calls
protocols:
  - stdio
requirements:
  - REQ-CROSS-PROJECT-WIKI
specs:
  - specs/task-cross-project-wiki.spec.md
source_files:
  - Cargo.toml
external_sources:
  - /Users/example/brain-rs/src/lib.rs
tags: [data-flow, memory]
---
# Main to brain-rs context flow

## Mechanism

The main project calls brain-rs over stdio and receives structured context data.
