spec: task
name: "Code Live Wiki"
satisfies: [REQ-CODE-LIVE-WIKI]
tags: [wiki, architecture, dogfood]
---

## Intent

Replace the previous generated wiki surface with a code live wiki tool: a deterministic Rust CLI substrate that scaffolds repo-local wiki memory, tracks article staleness through `source_files`, and derives Rust architecture metadata from Cargo while keeping a generic fallback for non-Rust repositories.

## Decisions

- The default live wiki path is `.agent-spec/wiki`, not `docs/wiki`, because this is agent working memory first and human documentation second.
- The CLI owns deterministic operations only: init, status, index, lint, metadata update, and architecture inventory rendering.
- Article interpretation remains agent-authored markdown; the CLI must not pretend to synthesize deep architecture prose from filenames.
- Rust architecture extraction uses `cargo metadata --format-version 1 --no-deps` and `serde_json`, avoiding a new dependency while borrowing symposium's Cargo-metadata model.
- The architecture inventory has a language-neutral model so future providers can add JavaScript, Python, Go, or other ecosystems without changing the wiki core.
- Reject symlinks during source, article, and architecture traversal so wiki compilation cannot escape repository boundaries.
- Remove the superseded generated `docs/wiki` commands and implementation modules; maintain only the `.agent-spec/wiki` code-live model.

## Boundaries

### Allowed Changes

- Cargo.toml
- Cargo.lock
- src/main.rs
- src/spec_verify/boundaries.rs
- src/spec_wiki/**
- specs/task-code-live-wiki.spec.md
- knowledge/requirements/req-code-live-wiki.md
- knowledge/requirements/req-autowiki-living-docs.md
- skills/agent-spec-wiki/**
- skills/agent-spec-tool-first/**
- .claude/skills/agent-spec-tool-first/**
- AGENTS.md
- README.md
- CHANGELOG.md
- docs/superpowers/plans/2026-07-08-autowiki-living-docs.md

### Forbidden

- Do not require network access.
- Do not add an LLM API dependency.
- Do not make Rust-only behavior the generic wiki baseline.
- Do not silently weaken existing lifecycle semantics.

## Out of Scope

- Full multi-language dependency providers beyond the generic fallback.
- Rendering a web UI.
- Auto-writing long-form architecture prose with an LLM from inside the CLI.

## Completion Criteria

Scenario: Live wiki scaffold
  Test: test_wiki_init_writes_live_wiki_scaffold_inventory_and_index
  Given a small Rust repository
  When `agent-spec wiki init --code . --wiki .agent-spec/wiki` runs
  Then the live wiki directories, index, architecture page, metadata, JSON inventory, and Mermaid graph are written

Scenario: Rust architecture inventory
  Test: test_wiki_inventory_reads_rust_workspace_dependencies
  Given a Cargo workspace with one member depending on another local member
  When the wiki inventory is built
  Then the inventory includes both packages and a local dependency edge

Scenario: Source-trace staleness
  Test: test_wiki_status_marks_articles_stale_when_source_files_changed
  Given a wiki article with `source_files: [src/lib.rs]`
  When `src/lib.rs` appears in the changed-file set
  Then wiki status reports that article as stale

Scenario: Live wiki lint
  Test: test_wiki_lint_requires_source_files_and_live_files
  Given a maintained wiki article without `source_files`
  When wiki lint runs
  Then it emits an error diagnostic instead of treating the article as verified

Scenario: Documentation and skill guidance
  Test: test_docs_describe_code_live_wiki_workflow
  Given the user-facing docs and agent-spec skill guidance
  When those files are inspected
  Then they describe `wiki init`, `wiki status`, `wiki inventory`, `wiki index`, `wiki lint`, and the repo-local live wiki model

Scenario: Source traversal rejects symlinks
  Test: test_discover_wiki_sources_rejects_symlinks
  Given a source path under the repository is a symbolic link
  When wiki source discovery runs
  Then the link is excluded and an Error-level `wiki-source-symlink-rejected` diagnostic is returned

Scenario: Generated wiki commands are removed
  Test: test_wiki_cli_rejects_removed_generated_wiki_commands
  Given the maintained code live wiki is the only supported wiki model
  When the CLI receives `wiki plan`, `wiki generate`, `wiki legacy-check`, `wiki export-github`, or `wiki install-ci`
  Then parsing rejects each removed command
