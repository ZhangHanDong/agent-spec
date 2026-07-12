---
title: "Spec Wiki"
type: module
source_files:
  - src/spec_wiki
tags:
  - wiki
  - architecture
status: maintained
---

# Spec Wiki

## Role

Repo-local code live wiki, architecture inventory, source trace checks, and
deterministic cross-project architecture maps.

## Cross-Project Model

Maintained project articles live in `projects/`; maintained mechanism and data
flow articles live in `flows/`. `project_id` and `flow_id` are stable unique
kebab-case identities. A flow's ordered `projects` list derives adjacent,
directed graph edges.

`source_files` are current-repository paths and participate in source boundary
and stale checks. `requirements` resolve to current KLL ids, and `specs` resolve
to parseable repo-relative Task Contracts. Outside paths, URLs, and repository
identifiers remain opaque labels under `external_sources`.

## Derived Artifacts

`architecture/project-map.json` is the machine-readable graph and
`architecture/project-map.mmd` is its Mermaid view. `wiki lint` and `wiki check`
rebuild the expected content and reject missing or drifted artifacts.

## Inspection

`wiki inspect-project <id> --code <root>` returns the project, related flows,
protocols, requirements, specs, and external evidence labels. The explicit code
root keeps repo-local validation independent of the process current directory.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.
