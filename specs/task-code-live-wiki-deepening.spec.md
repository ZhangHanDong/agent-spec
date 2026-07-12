spec: task
name: "Code Live Wiki Deepening"
tags: [wiki, architecture, dogfood, traceability]
satisfies: [REQ-CODE-LIVE-WIKI-DEEPENING]
depends: [task-code-live-wiki]
---

## Intent

Deepen the repo-local code live wiki from a working scaffold into a practical
agent working-memory layer. The implementation must keep the CLI deterministic,
continue treating `.agent-spec/wiki` as maintained agent memory, add stronger
lint/check/status gates, seed useful pages without overwriting human-maintained
content, expose Rust module-level architecture, and connect wiki pages back to
requirements, specs, and source paths.

## Decisions

- Keep `.agent-spec/wiki/**` trackable while other `.agent-spec` runtime state remains ignored.
- Add live `wiki check` rather than reusing the hidden legacy generated-wiki `check` semantics.
- Add `wiki seed` and `wiki seed --check`; seed must write only missing pages and must not overwrite existing maintained pages.
- Seed pages must include stable frontmatter fields: `title`, `type`, `source_files`, `tags`, and `status`.
- Extend Rust architecture inventory in the existing `ArchitectureInventory` JSON model with modules, module edges, and entrypoints.
- Generate layered Mermaid diagrams under `.agent-spec/wiki/architecture/`.
- Keep all analysis deterministic and local; do not add LLM calls, network calls, a web UI, or dynamic external provider execution.
- `wiki query` and `wiki inspect` are local search/trace helpers over wiki articles, KLL requirements, specs, and requirement trace records.

## Boundaries

### Allowed Changes
- .gitignore
- .agent-spec/wiki/**
- src/main.rs
- src/spec_wiki/**
- knowledge/requirements/req-code-live-wiki-deepening.md
- specs/task-code-live-wiki-deepening.spec.md
- skills/agent-spec-wiki/**
- skills/agent-spec-tool-first/**
- .claude/skills/agent-spec-tool-first/**
- README.md
- AGENTS.md
- CHANGELOG.md
- docs/intent-compiler/**
- fixtures/wiki-mini/**

### Forbidden
- Do not add network access or LLM calls to CLI code.
- Do not overwrite existing maintained wiki articles during seed.
- Do not make `.agent-spec/runs`, `.agent-spec/trace`, or temporary files trackable.
- Do not delete existing wiki content as a way to make checks pass.
- Do not replace KLL truth or human-facing docs with wiki pages.

## Out of Scope

- Web UI rendering.
- Dynamic third-party provider execution.
- Full language-specific providers beyond the existing Rust provider and generic fallback.
- Automatic long-form LLM-generated prose inside the CLI.

## Completion Criteria

Scenario: Track only live wiki state
  Test: test_gitignore_tracks_only_live_wiki_state
  Given `.gitignore` is configured for agent-spec local state
  When git ignore rules are checked
  Then `.agent-spec/wiki/_index.md` is trackable while `.agent-spec/runs/example.json` remains ignored

Scenario: Wiki lint rejects unsafe source files and broken links
  Test: test_wiki_lint_rejects_unsafe_source_files_and_broken_links
  Given a live wiki article with an absolute source file, a repo-escaping source file, and a broken internal markdown link
  When `lint_live_wiki` runs
  Then diagnostics report unsafe source paths and broken internal links

Scenario: Wiki lint rejects stale index
  Test: test_wiki_lint_reports_stale_index
  Given a live wiki article is added after `_index.md` was generated
  When `lint_live_wiki` runs
  Then diagnostics report that `_index.md` is stale

Scenario: Wiki status includes worktree changes
  Test: test_wiki_status_includes_dirty_staged_and_untracked_changes
  Given a wiki article lists `src/lib.rs` in `source_files`
  When dirty, staged, and untracked changed files are supplied
  Then status reports that article as stale

Scenario: Wiki status supports clean checkout CI
  Test: test_wiki_status_clean_checkout_does_not_diff_against_historical_meta_commit
  Given wiki metadata was generated before the current clean commit existed
  When `wiki_status` runs in that clean checkout
  Then it does not report stale articles solely from historical commit diff

Scenario: Live wiki check combines gates
  Test: test_wiki_live_check_combines_index_lint_and_status
  Given a live wiki with stale index or lint diagnostics
  When live wiki check runs
  Then it returns failing diagnostics

Scenario: Wiki seed creates focused pages without overwriting maintained pages
  Test: test_wiki_seed_writes_missing_pages_without_overwriting_existing
  Given a Rust agent-spec repository and an existing maintained wiki page
  When `seed_live_wiki` runs
  Then missing module, concept, and decision pages are created and the existing page is preserved

Scenario: Wiki seed check reports drift without writing
  Test: test_wiki_seed_check_reports_missing_pages_without_writing
  Given seedable wiki pages are missing
  When `seed_live_wiki_check` runs
  Then it reports missing seed pages and does not write them

Scenario: Rust architecture inventory includes modules and internal edges
  Test: test_wiki_inventory_extracts_rust_modules_edges_and_entrypoints
  Given Rust files with `mod`, `pub mod`, and `use crate::...`
  When the wiki inventory is built
  Then the inventory includes module nodes, module edges, and entrypoints

Scenario: Architecture article links layered diagrams
  Test: test_wiki_init_writes_layered_architecture_diagrams
  Given `wiki init` runs for a Rust repository
  When the architecture article and architecture directory are inspected
  Then workspace and module Mermaid diagrams are present and linked

Scenario: Wiki query searches local articles
  Test: test_wiki_query_searches_title_tags_sources_and_body
  Given live wiki articles with titles, tags, source files, and body text
  When `query_live_wiki` searches for lifecycle
  Then matching articles are returned with their source files

Scenario: Wiki inspect maps a source path to wiki, requirements, specs, and trace
  Test: test_wiki_inspect_maps_source_to_articles_requirements_and_specs
  Given a source file is covered by a wiki article and a task spec satisfying a requirement
  When `inspect_live_wiki_path` runs for that source file
  Then output lists matching wiki pages, related requirements, related specs, and related requirement trace records

Scenario: Requirements trace records link back to wiki articles
  Test: test_requirements_trace_records_include_related_wiki_articles
  Given a requirement trace record has code targets covered by wiki source_files
  When wiki article links are attached to the trace output
  Then the trace record lists related wiki article paths

Scenario: Failure explanation suggests wiki articles
  Test: test_requirements_explain_failure_suggests_related_wiki_articles
  Given a non-pass requirement trace record has code targets covered by wiki source_files
  When failure explanation output is enriched
  Then the non-pass record lists suggested wiki article paths to read first

Scenario: Live wiki fixture covers init seed index lint status check
  Test: test_live_wiki_fixture_covers_init_seed_index_lint_status_check
  Given the compact wiki fixture contains a tracked `.agent-spec/wiki`
  When fixture tests inspect init, seed, index, lint, status, and check outputs
  Then the fixture proves those live wiki workflows remain mechanically covered

Scenario: Docs describe the deepened live wiki workflow
  Test: test_docs_describe_deepened_live_wiki_workflow
  Given README, AGENTS, and wiki skill guidance
  When documentation tests inspect them
  Then they describe tracked wiki usage, `wiki seed`, `wiki check`, `wiki query`, `wiki inspect`, archive handling, and non-goals
