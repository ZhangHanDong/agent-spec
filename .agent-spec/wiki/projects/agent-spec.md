---
title: "agent-spec"
type: external-project
project_id: agent-spec
repo: .
role: "Specification compiler and code live wiki host"
interfaces:
  - cli
  - filesystem
protocols:
  - agent-spec-cli
  - markdown
status: active
source_files:
  - Cargo.toml
  - src/spec_wiki
external_sources:
  - rust-agents/agent-spec
tags:
  - primary
  - specification
  - wiki
---

# agent-spec

`agent-spec` is the primary repository. It compiles requirements into Task
Contracts and traceable work, and hosts the deterministic code live wiki.

The project map records selected design inputs from important external projects
without scanning those repositories or treating them as local source truth.
