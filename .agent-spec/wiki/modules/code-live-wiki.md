---
title: "Code Live Wiki"
type: module
source_files:
  - src/spec_wiki/live.rs
  - src/spec_wiki/architecture.rs
  - src/spec_wiki/model.rs
  - src/main.rs
  - knowledge/requirements/req-code-live-wiki.md
  - knowledge/requirements/req-code-live-wiki-deepening.md
  - specs/task-code-live-wiki.spec.md
  - specs/task-code-live-wiki-deepening.spec.md
  - skills/agent-spec-wiki/SKILL.md
tags:
  - wiki
  - architecture
  - dogfood
---

# Code Live Wiki

## Role

The code live wiki is repo-local agent working memory under `.agent-spec/wiki`.
It is not the durable truth layer. Durable requirements and decisions belong in
`knowledge/`, executable contracts belong in `specs/`, and human-facing prose
belongs in `docs/`.

The CLI owns deterministic wiki operations:

- `wiki init` creates the live wiki skeleton, architecture inventory, Mermaid
  graph, metadata, and index.
- `wiki seed` creates focused module, concept, and decision drafts without
  overwriting maintained pages.
- `wiki status` compares dirty, staged, and untracked current worktree files
  against article `source_files`.
- `wiki query` searches article titles, tags, source files, and bodies before an
  agent opens a broad source area.
- `wiki inspect <path>` maps a source or knowledge path to related wiki pages,
  KLL requirements, and task specs.
- `wiki inventory` renders Rust Cargo metadata plus module and dependency
  edges as JSON or Mermaid.
- `wiki index` rebuilds `_index.md` from article frontmatter.
- `wiki lint` checks required files, safe source trace, internal links, and
  index freshness.
- `wiki check` combines index freshness, lint, and current worktree stale
  status. In CI clean checkouts it is the tracked wiki structure gate.
- `wiki meta update` records the current repository metadata.

The superseded generated `docs/wiki` implementation and its hidden plan,
generate, legacy-check, export, and CI commands are removed. The maintained
`.agent-spec/wiki` model is the only wiki surface.

## Data Model

Maintained articles are Markdown files with frontmatter. The important field is
`source_files`: every page that explains code must list the files it depends on.
`wiki status` and `wiki lint` use that field as the maintenance boundary.
Source, article, spec, and architecture walkers reject symlinks rather than
following them outside repository boundaries. In a dirty worktree, an article
is current when its own file is updated alongside a changed listed source; a
source-only change remains stale and blocks `wiki check`.

The architecture inventory is language-neutral in shape, but the Rust provider
is currently the only structured provider. It uses `cargo metadata
--format-version 1 --no-deps`, extracts workspace packages, targets, dependency
edges, source files, `mod`/`pub mod` declarations, `use crate::...` edges, and
entrypoints, then writes:

- `.agent-spec/wiki/architecture/inventory.json`
- `.agent-spec/wiki/architecture/workspace.mmd`
- `.agent-spec/wiki/architecture/modules.mmd`

Non-Rust repositories fall back to generic file inventory so wiki status,
indexing, metadata, and linting still work.

## Operating Pattern

Before reading a broad part of the repository, run:

```bash
agent-spec wiki status --code . --wiki .agent-spec/wiki
agent-spec wiki query "topic" --wiki .agent-spec/wiki
```

If a fresh article covers the target area, read it first. If no article exists,
read the source files and write a focused article under `modules/`, `concepts/`,
`decisions/`, `learnings/`, or `queries/`.

After changing code or wiki pages, run:

```bash
agent-spec wiki index --wiki .agent-spec/wiki
agent-spec wiki check --code . --wiki .agent-spec/wiki
```

Do not rely on `wiki init` to write deep prose. It intentionally creates a
small deterministic scaffold; agents add durable understanding over time.
When Atlas adds a query-only capability such as runtime-boundary hints, update
both its architecture and authority articles so the wiki records the mechanism
without confusing heuristic working context with persisted graph facts.

Old wiki content should be summarized into `learnings/` or `archive/` pages with
source links instead of being deleted abruptly. Non-goals: no built-in LLM
long-form generation, no web UI, and no replacement for KLL requirements or
decisions.
