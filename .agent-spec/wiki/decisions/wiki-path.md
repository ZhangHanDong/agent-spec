---
title: "Wiki Path"
type: decision
source_files:
  - knowledge/requirements/req-code-live-wiki.md
  - knowledge/requirements/req-code-live-wiki-deepening.md
  - .gitignore
tags:
  - wiki
  - git
status: draft
---

# Wiki Path

## Role

`.agent-spec/wiki/**` is trackable live wiki state because it is reviewed agent
working memory. It compounds understanding across sessions without promoting
every note into durable KLL truth.

Other `.agent-spec` runtime outputs stay ignored: run logs, trace ledgers,
temporary files, generated work unit manifests, and local execution state are
tool artifacts rather than curated wiki articles.

This keeps the boundary simple:

- `knowledge/`: durable requirements, decisions, guidance, and proposals.
- `specs/`: executable Task Contracts.
- `docs/`: human-facing prose.
- `.agent-spec/wiki/`: maintained agent working memory with `source_files`.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.
