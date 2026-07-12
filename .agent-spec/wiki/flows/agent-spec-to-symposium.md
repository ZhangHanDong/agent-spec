---
title: "agent-spec adapts symposium metadata model"
type: project-flow
flow_id: agent-spec-to-symposium
projects:
  - agent-spec
  - symposium
kind: adapts-metadata-from
protocols:
  - cargo-metadata-json
requirements:
  - REQ-CODE-LIVE-WIKI
specs:
  - specs/task-code-live-wiki.spec.md
source_files:
  - knowledge/requirements/req-code-live-wiki.md
  - specs/task-code-live-wiki.spec.md
  - src/spec_wiki/architecture.rs
external_sources:
  - consult/symposium
tags:
  - architecture
  - rust
---

# agent-spec adapts symposium metadata model

## Mechanism

The Rust provider invokes Cargo metadata, converts workspace packages and local
dependencies into a language-neutral inventory, and renders deterministic JSON
and Mermaid artifacts. The provider boundary allows other language ecosystems
to supply equivalent metadata later.

## Data Flow

`Cargo.toml` and Cargo metadata output feed the Rust provider. The provider
normalizes packages, targets, modules, entrypoints, and dependency edges into
the wiki architecture inventory consumed by agents and lint/check workflows.
