---
title: "Spec Verify"
type: module
source_files:
  - src/spec_verify
tags:
  - verification
  - lifecycle
status: draft
---

# Spec Verify

## Role

Mechanical and inferential scenario verification.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 was reviewed here; symbol verification uses the pinned generation and
retains the existing stale/worktree fail-closed behavior.

## Boundary path expressions

`BoundariesVerifier` no longer owns path recognition. It calls
`spec_core::boundary_paths::collect_boundary_patterns` (Allow entries are
paths by definition; Deny/General only when the whole entry is a single path
token) and `path_matches_pattern`; the same functions back
`spec_gateway::plan::collect_allowed_patterns` and the MCP `spec_allows_path`
tool. Trailing ` — note` / ` # note` outside backticks are stripped;
parentheses are not. Verifier still runs only with an explicit change set and
stays silent when a spec declares no path boundary at all.

## Package membership

`TestVerifier` fetches `cargo metadata --no-deps` once per run when any
binding names a `Package:`; a non-member package short-circuits to
`uncertain` with a reason naming the workspace root and members, without
running cargo test. Metadata failure falls back to the legacy path.
