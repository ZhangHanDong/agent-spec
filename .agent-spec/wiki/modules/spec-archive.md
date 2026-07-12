---
title: "Spec Archive"
type: module
source_files:
  - src/spec_archive.rs
tags:
  - archive
  - contracts
status: draft
---

# Spec Archive

## Role

Archival summary and completed-spec compression. Only specs tagged `done` or
`completed` are candidates.

## Evidence Binding

Archive planning reads lifecycle run logs, but a matching display name is not
enough. Passing evidence must match both the canonical current spec path and
the current content fingerprint. This prevents a stale run for an older
contract body from authorizing archival.

## Mutation Safety

Application rejects plans with Error diagnostics, missing sources, existing or
duplicate targets, and identical source/target paths. Every entry is
preflighted before the first rename. If a rename later fails, already moved
entries are rolled back. The CLI writes the archive summary through a temporary
file and publishes it only after the moves succeed.

## Maintenance

Update this page when evidence identity or archive mutation semantics change.
