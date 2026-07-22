# agent-spec-code-graph-provider

Rust SDK and conformance harness for projecting external code-intelligence
tools into agent-spec's provider-neutral Code Graph IR.

The crate defines strict provider manifests, opt-in project registrations,
separate extraction and semantic-enrichment payloads, freshness/worktree
validation, host-derived BLAKE3 fingerprints, bounded child-process execution,
cancellation, and atomic publication. It does not include a parser, language
runtime, installer, daemon, or orchestration system.

```toml
[dependencies]
agent-spec-code-graph-provider = "0.1.0"
```

Providers should first run `agent-spec atlas provider validate`, then pass
`agent-spec atlas provider conformance`. Passing the transport matrix does not
by itself establish production support for a language.

See the
[Provider Kit guide](https://github.com/ZhangHanDong/agent-spec/blob/main/docs/code-graph-provider-kit.md)
for artifact schemas, authority boundaries, and the eight conformance checks.
