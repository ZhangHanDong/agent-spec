# External Code Graph Provider Kit

The F1 provider kit is a producer-facing Rust SDK and conformance gate for tools
that project code intelligence into agent-spec's provider-neutral Code Graph IR.
It does not add a non-Rust provider by itself. Rust Atlas remains the first
production provider; SCIP, tree-sitter, and language-specific adapters remain F2.

## Authority Boundary

An external provider publishes derived code evidence. It cannot write KLL,
requirements, Task Contracts, lifecycle verdicts, or documentation authority.
The host validates a complete response before publishing it. A stale, partial,
wrong-worktree, malformed, cancelled, timed-out, or oversized response never
becomes fresh evidence and never replaces the last valid artifact.

The reusable package is `agent-spec-code-graph-provider` in
`crates/code-graph-provider`. It has no dependency on `rust-atlas` or the
agent-spec binary.

The first published SDK version is `0.1.0`:

```toml
[dependencies]
agent-spec-code-graph-provider = "0.1.0"
```

Provider protocol compatibility is governed by the manifest IR range and wire
schema ids. A Cargo version match does not allow a provider to bypass those
runtime gates.

## Artifact Contracts

| Artifact | Schema id | Owner |
|---|---|---|
| Provider manifest | `agent-spec/code-graph-provider/manifest-v1` | Provider package |
| Project registration | `agent-spec/code-graph-provider/registration-v1` | Consuming project |
| Process request | `agent-spec/code-graph-provider/request-v1` | Host |
| Extraction payload | `agent-spec/code-graph-provider/extraction-payload-v1` | Extractor |
| Extraction artifact | `agent-spec/code-graph-provider/extraction-artifact-v1` | Host |
| Enrichment payload | `agent-spec/code-graph-provider/enrichment-payload-v1` | Enricher |
| Enrichment artifact | `agent-spec/code-graph-provider/enrichment-artifact-v1` | Host |
| Conformance fixture | `agent-spec/code-graph-provider/conformance-fixture-v1` | Test corpus |
| Conformance receipt | `agent-spec/code-graph-provider/conformance-receipt-v1` | Host |

All deserialized wire structs reject unknown fields. Schema ids and the declared
IR range are compatibility gates, not descriptive labels.

## Manifest And Registration

The manifest declares provider identity and behavior:

- lowercase provider id, version, and language id;
- `extractor` or `semantic-enricher` role;
- an IR range that includes version 1;
- role-compatible capabilities;
- `stdio-json-v1` startup;
- repository-relative freshness input patterns;
- timeout, stdout, stderr, and diagnostic limits;
- deterministic and no-daemon support.

Extractors require `nodes` and `containment`; `basic-references` is optional.
Enrichers declare `semantic-edges`, `query-hints`, or both. Capabilities cannot
cross roles.

The project registration is separate and defaults to disabled when `enabled`
is omitted. Enabling it supplies one executable, literal argv, and an optional
cwd. The adapter calls `std::process::Command` directly. It does not join argv
into a shell command, discover an installer, download a runtime, contact a
network endpoint, or connect to an implicit daemon.

```json
{
  "schema": "agent-spec/code-graph-provider/registration-v1",
  "provider_id": "example-extractor",
  "enabled": true,
  "executable": "/opt/example/bin/code-graph",
  "args": ["--project-config", ".example/config.json"],
  "cwd": "."
}
```

## Process Protocol

For each explicit invocation, the host writes one compact JSON request followed
by a newline to stdin. The provider writes exactly one JSON payload to stdout.
Human diagnostics belong on stderr and are bounded independently. A nonzero
exit is `provider-process`; malformed JSON is `provider-response`; an unsupported
payload schema is `provider-schema`.

The host polls the child without a daemon. Caller cancellation, manifest timeout,
or either output limit kills and reaps the child. Only a zero exit followed by
strict projection can reach publication.

## Extraction Projection

Extractor node ids must be unique and start with `<provider-id>:`. Source paths
must be repository-relative, use `/`, and contain no empty, `.` or `..`
components. Nodes and edges carry extractor id/version, evidence text, and
`exact`, `candidate`, or `heuristic` confidence. Containment and basic-reference
edges are checked against declared capabilities.

The host canonicalizes freshness inputs, affected paths, nodes, edges, and
diagnostics. It then derives a BLAKE3 graph fingerprint over IR version and the
canonical payload. Providers do not choose the published fingerprint.

Fresh responses have no affected paths. Stale and partial responses require
affected paths and diagnostics. Every response must match the request's
worktree id. These rules prevent a provider from turning incomplete or borrowed
evidence into an authoritative-looking graph.

## Semantic Enrichment

An enrichment payload has no node or KLL field. It names the base graph
fingerprint and can add only semantic edges or query hints. Every item requires
extractor id/version, evidence, and confidence. The host derives a separate
enrichment fingerprint, so an enricher cannot silently replace extractor facts.

## Commands

Validate a manifest and optional project registration without starting it:

```bash
agent-spec atlas provider validate \
  --manifest path/to/manifest.json \
  --registration .agent-spec/providers/example.json
```

Run the checked-in protocol conformance fixture:

```bash
agent-spec atlas provider conformance \
  --manifest fixtures/code-graph-provider/basic/manifest.json \
  --registration fixtures/code-graph-provider/basic/registration.json \
  --fixture fixtures/code-graph-provider/basic/conformance.json \
  --code . \
  --scratch .agent-spec/provider-conformance \
  --out .agent-spec/provider-conformance/receipt.json
```

`validate` does not execute a provider. `conformance` is explicit and local.
When `--out` is absent, each command writes strict JSON to stdout. With `--out`,
stdout stays empty and the result is atomically replaced. A blocked conformance
receipt is written before the command exits nonzero.

## Conformance Matrix

The receipt always records these checks:

| Check | Required behavior |
|---|---|
| `stable-id` | Published ids equal the fixture's provider-scoped ids |
| `deterministic-repeat` | Reordered but identical facts produce the same graph fingerprint |
| `partial-parse` | Partial paths and diagnostics remain visible |
| `stale-worktree` | Stale remains stale and a wrong worktree is rejected |
| `unknown-schema` | Unsupported payload schemas fail closed |
| `bounded-output` | Oversized stdout or stderr stops the child |
| `cancellation` | Caller cancellation stops and reaps the child |
| `atomic-publish` | Invalid output leaves the prior artifact byte-identical |

The checked-in shell fixture only exercises this transport and validation
contract. It is not a parser and is not evidence that agent-spec supports a
second language. An F2 provider must pass this matrix and add real-repository
quality evidence before adoption.

## Stable Diagnostics

The main failure families are `provider-manifest-*`, `provider-registration`,
`provider-disabled`, `provider-request`, `provider-schema`, `provider-identity`,
`provider-worktree-mismatch`, `provider-path`, `provider-node-id`,
`provider-evidence`, `provider-freshness`, `provider-output-limit`,
`provider-timeout`, `provider-cancelled`, `provider-response`, and
`provider-publish`.

These diagnostics are part of the adapter boundary. Consumers should branch on
the code, retain the message as evidence, and never reinterpret an error as a
fresh empty graph.
