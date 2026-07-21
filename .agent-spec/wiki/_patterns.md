---
title: "Patterns"
type: patterns
source_files:
  - Cargo.lock
  - Cargo.toml
  - build.rs
  - crates/code-graph-provider/Cargo.toml
  - crates/code-graph-provider/src/lib.rs
  - crates/rust-atlas/Cargo.toml
  - crates/rust-atlas/src/context.rs
  - crates/rust-atlas/src/lib.rs
  - src/atlas_agent_eval.rs
  - src/main.rs
---
# Patterns

Capture cross-cutting implementation patterns here as they become durable.

Reviewed for Atlas D2: the generation, input-plan, and incremental modules
follow deterministic JSON and typed-failure patterns. Batch phases stream
shards, while source, graph, and overlay bytes are checked before publication;
post-commit queue cleanup remains warning-only and recoverable.

Reviewed for Atlas D3: persisted watermarks, typed runtime states, advisory
leases, bounded retries, and strict loopback protocol parsing reuse those
deterministic JSON and fail-closed patterns.

Reviewed for Atlas B5: retrieval and projection are separate deterministic
stages. Stable evidence ids and graph-pinned continuation argv explain
retrieval, relevance, policy, and byte loss without hidden cursor state.

Reviewed for Atlas E1: experiment, plan, receipt, and gate are separate strict
schemas. Structural completeness precedes correctness, and correctness
precedes baseline-MAD efficiency comparison.

Reviewed for Atlas F1: provider manifest, registration, process response,
projection artifact, and conformance receipt stay separate. Host validation and
atomic publication precede any use as derived code evidence.
