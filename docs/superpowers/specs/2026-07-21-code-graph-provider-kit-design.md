# Code Graph Provider Adapter Kit Design

**Status:** Approved by the reviewed `docs/atlas-roadmap.md` F1 contract

**Requirement:** `REQ-CODE-GRAPH-PROVIDER-KIT`

**Task Contract:** `specs/task-code-graph-provider-kit.spec.md`

## Purpose

F1 turns the existing provider-neutral consumer types into a producer-facing adapter contract.
It does not add a second language. It makes later providers independently implementable and
testable without coupling agent-spec to their parser, runtime, installer, or orchestration system.

## Package Boundary

`agent-spec-code-graph-provider` is a standalone workspace library. It owns wire types, strict
validation, canonical projection, process limits, atomic publication, and the conformance harness.
It does not depend on `rust-atlas` and does not import KLL models. The agent-spec binary may call
the library for explicit validation and conformance commands, while the existing Rust Atlas adapter
continues to satisfy the consumer contract directly.

## Manifest And Registration

The provider manifest describes identity and compatibility, not installation:

- provider id/version and one language id;
- extractor or semantic-enricher role;
- supported IR schema range and role-compatible capabilities;
- `stdio-json-v1` process startup protocol;
- freshness input declarations;
- timeout, stdout, stderr, and diagnostic limits;
- deterministic and no-daemon support declarations.

A separate project registration controls execution. It defaults to disabled and, when enabled,
contains one executable, literal argv, and optional cwd. The host never joins these fields into a
shell string. No provider is selected by language inference or PATH discovery.

## Separate Producer Schemas

Extraction payloads can contain provider-scoped nodes, containment edges, basic references,
freshness facts, and diagnostics. Semantic enrichment payloads cannot contain nodes. They identify
the base graph fingerprint and can only add edges or query hints carrying extractor id/version,
evidence, and confidence. Neither schema has a KLL, requirement, decision, or documentation field.

The host validates and canonicalizes payloads before publication. Node ids are stable and begin
with the provider id. Paths are repository-relative forward-slash paths without absolute roots,
backslashes, or dot segments. Collections are sorted and duplicate-free. The host derives the graph
fingerprint from canonical facts plus provider, worktree, schema, and freshness identity.

## Freshness And Failure

Every request and response names a worktree identity. A mismatch is an error. Fresh responses have
no stale paths; stale responses name stale paths; partial responses retain affected paths and a
diagnostic. Validation never rewrites stale or partial evidence to fresh.

The process runner has three independent stop conditions: timeout, caller cancellation, and bounded
stdout/stderr. It kills and reaps the child before returning a normalized diagnostic. A response is
published only after process success, strict deserialization, role validation, freshness validation,
canonical projection, and fingerprinting. Publication uses a temporary file in the destination
directory followed by rename, so any earlier failure leaves the old artifact byte-identical.

## Conformance

The checked-in fixture implements the local `stdio-json-v1` test protocol and exposes cases for a
fresh graph, deterministic repeat, partial parse, stale state, wrong worktree, unknown schema,
oversized output, and cancellation. The harness records every check in a strict receipt. It also
pre-seeds an artifact and proves failed cases cannot replace it.

The fixture proves the adapter contract, not language support or production accuracy. F2 providers
must run the same harness and add their own real repository quality corpus before adoption.
