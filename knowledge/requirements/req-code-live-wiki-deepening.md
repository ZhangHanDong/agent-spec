---
kind: requirement
id: REQ-CODE-LIVE-WIKI-DEEPENING
title: "Code Live Wiki Deepening"
status: accepted
liveness: auto
tags: [wiki, architecture, dogfood, traceability]
---

# Code Live Wiki Deepening

## Problem

agent-spec now has a repo-local code live wiki scaffold, but the wiki is still
thin. It can initialize, index, lint source traces, and derive basic Rust Cargo
metadata, yet it does not seed enough useful articles, does not gate stale index
or invalid source paths as one CI-ready check, does not expose Rust module-level
architecture, and does not connect wiki pages back to requirements, specs,
trace, and failure explanation.

## Requirements

[REQ-CODE-LIVE-WIKI-DEEPENING-DOGFOOD] agent-spec MUST dogfood this follow-up through a KLL requirement and a satisfying Task Contract.

[REQ-CODE-LIVE-WIKI-DEEPENING-TRACKED] agent-spec MUST allow `.agent-spec/wiki/**` to be tracked while keeping run logs, trace outputs, and temporary agent-spec state ignored.

[REQ-CODE-LIVE-WIKI-DEEPENING-LINT] `wiki lint` MUST reject article `source_files` that are absolute, escape the repo, or point outside the code root, and MUST reject broken internal Markdown links and stale `_index.md` output.

[REQ-CODE-LIVE-WIKI-DEEPENING-STATUS] `wiki status` MUST consider dirty worktree, staged, and untracked source files as local stale-article signals. It MUST NOT fail a clean checkout solely because `_meta.json` was generated before the commit that contains it existed.

[REQ-CODE-LIVE-WIKI-DEEPENING-CHECK] agent-spec MUST expose a live `wiki check` command that combines index freshness, lint, and status diagnostics for CI usage.

[REQ-CODE-LIVE-WIKI-DEEPENING-SEED] agent-spec MUST expose `wiki seed` and `wiki seed --check` to create or check focused draft wiki pages without overwriting existing maintained articles.

[REQ-CODE-LIVE-WIKI-DEEPENING-SEED-CONTENT] Seeded pages MUST cover core modules, concepts, and decisions with stable frontmatter including `title`, `type`, `source_files`, `tags`, and `status`.

[REQ-CODE-LIVE-WIKI-DEEPENING-ARCH] Rust architecture inventory MUST include module nodes, internal module edges inferred from `mod`/`pub mod` and `use crate::...`, and entrypoints.

[REQ-CODE-LIVE-WIKI-DEEPENING-DIAGRAMS] Wiki architecture output MUST expose layered Mermaid diagrams for workspace and module views, and the architecture article MUST link to those diagrams.

[REQ-CODE-LIVE-WIKI-DEEPENING-LINKS] Wiki pages MUST be associated with related KLL requirements and task specs where deterministic source-file or `satisfies: [REQ-*]` evidence exists.

[REQ-CODE-LIVE-WIKI-DEEPENING-TRACE-WIKI] Requirement trace and failure explanation output MUST include related wiki article paths when source-file evidence maps trace records to maintained wiki pages.

[REQ-CODE-LIVE-WIKI-DEEPENING-QUERY] agent-spec MUST expose `wiki query <text>` for local title/tag/source/body search.

[REQ-CODE-LIVE-WIKI-DEEPENING-INSPECT] agent-spec MUST expose `wiki inspect <path>` that lists wiki pages, related requirements, related specs, and related requirement trace records for a source or knowledge path.

[REQ-CODE-LIVE-WIKI-DEEPENING-FIXTURE] The repository MUST include a compact live wiki fixture covering init, seed, index, lint, status, and check behavior.

[REQ-CODE-LIVE-WIKI-DEEPENING-DOCS] Skills and docs MUST describe tracked live wiki usage, CI check guidance, broad-reading workflow, archive handling, and non-goals.

## Scenarios

Scenario: Track only live wiki state
  Given `.gitignore` is configured for agent-spec local state
  When git ignore rules are checked
  Then `.agent-spec/wiki/_index.md` is trackable while `.agent-spec/runs/example.json` remains ignored

Scenario: Wiki lint rejects unsafe source files and broken links
  Given a live wiki article with an absolute source file, a repo-escaping source file, and a broken internal markdown link
  When `wiki lint` runs
  Then diagnostics report unsafe source paths and broken internal links

Scenario: Wiki lint rejects stale index
  Given a live wiki article is added after `_index.md` was generated
  When `wiki lint` runs
  Then diagnostics report that the wiki index is stale

Scenario: Wiki status includes worktree changes
  Given a wiki article lists `src/lib.rs` in `source_files`
  When `src/lib.rs` is dirty or untracked
  Then `wiki status` reports that article as stale

Scenario: Wiki status supports clean checkout CI
  Given wiki metadata was generated before the current clean commit existed
  When `wiki status` runs in that clean checkout
  Then it does not report stale articles solely from historical commit diff

Scenario: Live wiki check combines gates
  Given a live wiki with stale index or lint diagnostics
  When `wiki check` runs
  Then it exits with an error and reports the diagnostics

Scenario: Wiki seed creates focused pages without overwriting maintained pages
  Given a Rust agent-spec repository and an existing maintained wiki page
  When `wiki seed` runs
  Then missing module, concept, and decision pages are created and the existing page is preserved

Scenario: Wiki seed check reports drift without writing
  Given seedable wiki pages are missing
  When `wiki seed --check` runs
  Then it reports missing seed pages and does not write them

Scenario: Rust architecture inventory includes modules and internal edges
  Given Rust files with `mod`, `pub mod`, and `use crate::...`
  When `wiki inventory --format json` runs
  Then the inventory includes module nodes, module edges, and entrypoints

Scenario: Architecture article links layered diagrams
  Given `wiki init` runs for a Rust repository
  When the architecture article and architecture directory are inspected
  Then workspace and module Mermaid diagrams are present and linked

Scenario: Wiki query searches local articles
  Given live wiki articles with titles, tags, source files, and body text
  When `wiki query lifecycle` runs
  Then matching articles are returned with their source files

Scenario: Wiki inspect maps a source path to wiki, requirements, specs, and trace
  Given a source file is covered by a wiki article and a task spec satisfying a requirement
  When `wiki inspect src/spec_wiki/live.rs` runs
  Then output lists matching wiki pages, related requirements, related specs, and related requirement trace records

Scenario: Requirements trace records link back to wiki articles
  Given a requirement trace record has code targets covered by wiki source_files
  When requirements trace output is enriched
  Then the trace record lists related wiki article paths

Scenario: Failure explanation suggests wiki articles
  Given a non-pass requirement trace record has code targets covered by wiki source_files
  When requirements explain-failure output is enriched
  Then the non-pass record lists suggested wiki article paths to read first

Scenario: Live wiki fixture covers init seed index lint status check
  Given the compact wiki fixture contains a tracked `.agent-spec/wiki`
  When fixture tests inspect init, seed, index, lint, status, and check outputs
  Then the fixture proves those live wiki workflows remain mechanically covered

Scenario: Docs describe the deepened live wiki workflow
  Given README, AGENTS, and wiki skill guidance
  When documentation tests inspect them
  Then they describe tracked wiki usage, `wiki seed`, `wiki check`, `wiki query`, `wiki inspect`, archive handling, and non-goals

## Dependencies

- REQ-CODE-LIVE-WIKI

## Source Trace

- follow-up plan: Codex session attachment `pasted-text-1.txt` (2026-07-09)
- existing live wiki requirement: knowledge/requirements/req-code-live-wiki.md

## Open Questions

None.
