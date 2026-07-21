# Release Process

agent-spec publishes three Cargo packages from one workspace. Release commits
must keep their dependency versions, changelog, user documentation, and
`Cargo.lock` consistent.

## Version Policy

- `agent-spec` follows stable semantic versioning for the CLI and documented
  machine contracts.
- `rust-atlas` is independently versioned. While it is below 1.0, graph schema
  or public API changes require a minor version bump.
- `agent-spec-code-graph-provider` is independently versioned. Its Cargo API,
  wire schema ids, and provider IR range are separate compatibility gates.
- Atlas graphs, indexes, bindings, and receipts are derived artifacts. Release
  notes must state whether an upgrade requires `agent-spec atlas build --full`.

## Release Gate

From the release PR worktree, run:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo package -p agent-spec-code-graph-provider --allow-dirty
cargo package -p rust-atlas --allow-dirty
bash scripts/docs-lint.sh
cargo run -q -- lint-knowledge --knowledge knowledge --gate
cargo run -q -- requirements graph --knowledge knowledge --format json --gate
cargo run -q -- wiki check --code . --wiki .agent-spec/wiki
```

Run the active release Task Contracts with `agent-spec lifecycle`. Review the
package file lists so fixtures, runtime state, build output, and secrets are not
included. Use `--allow-dirty` only in the release PR worktree; the merge commit
and tagged checkout must be clean.

## Publish Order

Publish dependencies before the root binary:

1. `cargo publish -p agent-spec-code-graph-provider`
2. Wait until version resolution succeeds from crates.io.
3. `cargo publish -p rust-atlas`
4. Wait until version resolution succeeds from crates.io.
5. `cargo package -p agent-spec`
6. `cargo publish -p agent-spec`

The root package verification can require the newly published dependency
versions. Do not use `--no-verify` to bypass that ordering.

After publication, create and push the signed `v<agent-spec-version>` tag,
publish GitHub release notes from `CHANGELOG.md`, and verify the crates.io and
docs.rs pages. Tags identify the workspace release; the changelog records the
independent crate versions included in it.
