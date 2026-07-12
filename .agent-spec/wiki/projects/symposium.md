---
title: "symposium"
type: external-project
project_id: symposium
repo: consult/symposium
role: "Reference for Rust workspace metadata and dependency extraction"
interfaces:
  - cargo-metadata
protocols:
  - cargo-metadata-json
status: reference
source_files:
  - knowledge/requirements/req-code-live-wiki.md
  - specs/task-code-live-wiki.spec.md
  - src/spec_wiki/architecture.rs
external_sources:
  - consult/symposium
tags:
  - reference
  - rust
  - architecture
---

# symposium

`symposium` provides the reference pattern for deriving Rust workspace and
local dependency structure from Cargo metadata. `agent-spec` adapts that model
behind its language-neutral architecture inventory.
