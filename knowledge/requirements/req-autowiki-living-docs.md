---
kind: requirement
id: REQ-AUTOWIKI-LIVING-DOCS
title: "AutoWiki Living Docs"
status: superseded
liveness: auto
tags: [docs, wiki, kll, trace]
---

# AutoWiki Living Docs

> Superseded by `REQ-CODE-LIVE-WIKI` and
> `specs/task-code-live-wiki.spec.md`. The replacement keeps deterministic
> local tooling but changes the model from generated `docs/wiki` output to a
> maintained repo-local code live wiki under `.agent-spec/wiki`.

## Problem

agent-spec has KLL, specs, lifecycle evidence, docs lint, and requirement trace
records, but it does not yet produce a browsable living wiki that summarizes a
repository while preserving source trace and freshness evidence.

## Requirements

[REQ-AUTOWIKI-LIVING-DOCS-PLAN] agent-spec MUST build a deterministic wiki plan from code, docs, KLL artifacts, specs, archive summaries, and trace ledgers.

[REQ-AUTOWIKI-LIVING-DOCS-GENERATE] agent-spec MUST render local Markdown wiki pages with source trace, source fingerprints, run metadata, page tree, and search index.

[REQ-AUTOWIKI-LIVING-DOCS-CHECK] agent-spec MUST detect generated wiki drift, stale source fingerprints, missing sources, malformed generated frontmatter, missing required pages, and broken internal links.

[REQ-AUTOWIKI-LIVING-DOCS-GITHUB] agent-spec MUST export a flattened GitHub Wiki tree with rewritten internal links, `Home.md`, and `_Sidebar.md` without pushing to a remote.

[REQ-AUTOWIKI-LIVING-DOCS-CI] agent-spec MUST generate CI check workflows that refresh-check the wiki on default branch pushes without requiring cloud sync or secret keys.

[REQ-AUTOWIKI-LIVING-DOCS-ASSETS] agent-spec MUST support visual screenshots only through a deterministic local asset manifest.

[REQ-AUTOWIKI-LIVING-DOCS-DOCS] agent-spec MUST document wiki commands, generated output ownership, source trace semantics, and CI usage in README, AGENTS, and tool-first skills.

## Scenarios

Scenario: Wiki plan exposes required pages and source trace
  Given a repository with KLL requirements, specs, docs, code modules, and trace ledgers
  When the operator runs `agent-spec wiki plan --format json --gate`
  Then the JSON contains required wiki pages, source paths, fingerprints, run metadata, and no gate-blocking diagnostics

Scenario: Wiki generation renders deterministic local Markdown
  Given a wiki plan for a repository
  When the operator runs `agent-spec wiki generate --out docs/wiki --check`
  Then the generated pages contain source trace frontmatter, `wiki-tree.json`, `search-index.json`, and run metadata

Scenario: Wiki check fails on stale generated output
  Given a generated wiki page whose recorded source fingerprint no longer matches the source file
  When the operator runs `agent-spec wiki check --out docs/wiki`
  Then the command fails with a `wiki-stale-source` diagnostic

Scenario: GitHub Wiki export rewrites paths and links
  Given generated wiki pages with nested paths and internal links
  When the operator runs `agent-spec wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github --check`
  Then the export contains flattened filenames, rewritten internal links, `Home.md`, and `_Sidebar.md`

Scenario: Visual assets are manifest-driven
  Given a visual asset manifest pointing to an existing screenshot file
  When the operator runs `agent-spec wiki generate --assets .agent-spec/wiki-assets.json`
  Then the target page embeds the screenshot and records the asset path in source trace

Scenario: Wiki feature is documented
  Given README, AGENTS, tool-first skills, and command references
  When documentation tests inspect their content
  Then they mention wiki plan, wiki generate, wiki check, wiki export-github, wiki install-ci, source trace, GitHub Wiki export, visual asset manifest, and local-first operation

## Dependencies

- REQ-REQUIREMENTS-COMPILER-PLAN-DAG

## Source Trace

- Factory AutoWiki product page: https://factory.ai/product/autowiki
- Factory AutoWiki generate docs: https://docs.factory.ai/cli/features/wiki/generate
- Factory AutoWiki refresh docs: https://docs.factory.ai/cli/features/wiki/auto-refresh
- Factory AutoWiki overview docs: https://docs.factory.ai/cli/features/wiki/overview
- Current agent-spec intent compiler plan: docs/superpowers/plans/2026-07-08-requirements-compiler-plan-dag.md

## Open Questions

None.
