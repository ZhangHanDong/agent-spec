# rust-atlas

An incrementally invalidated project graph for Rust code, built for AI agents:
query structure instead of grepping text.

- **syn baseline** (stable toolchain): module tree, symbol nodes (file, span,
  visibility, signature), `contains`/`impls-trait`/`impl-for` edges
- **Optional SCIP overlay**: resolved cross-file `references` edges from a
  rust-analyzer SCIP index in JSON form (`scip print --json`)
- **Per-file JSON shards + blake3 hashes**: staleness detected by hash compare;
  incremental rebuilds rewrite only dirty shards; `--frozen` reports staleness
  instead of rebuilding
- One schema for all phases: every edge carries `provenance`
  (`syn` | `scip` | `mir`); MIR overlay is a planned extension

Used by `agent-spec atlas build|tree|query|refs|impls|check` and the read-only
`agent-spec mcp` atlas tools. Unparsable files degrade to `unparsed`
diagnostics; extraction is read-only and performs no network or LLM calls.
