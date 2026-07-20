---
title: "Patterns"
type: patterns
source_files:
  - Cargo.lock
  - Cargo.toml
  - build.rs
  - crates/rust-atlas/Cargo.toml
  - crates/rust-atlas/src/lib.rs
  - src/main.rs
---
# Patterns

Capture cross-cutting implementation patterns here as they become durable.

Reviewed for Atlas D2: the new generation, input-plan, and incremental modules
follow the existing deterministic JSON and typed-failure patterns. Batch phases
stream shards, while deterministic source, graph, and overlay bytes are checked
before publication; post-commit queue cleanup remains warning-only and
recoverable.
