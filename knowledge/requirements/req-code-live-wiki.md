---
kind: requirement
id: REQ-CODE-LIVE-WIKI
title: "Code Live Wiki"
status: accepted
liveness: auto
tags: [wiki, architecture, dogfood]
---

# Code Live Wiki

## Problem

The previous wiki feature could generate many source-trace pages, but it did not
behave like a maintained code live wiki: agents still lacked a repo-local
working-memory layer that reports stale articles, keeps an index, and derives
architecture inventory from Rust project metadata while remaining useful for
non-Rust repositories.

## Requirements

[REQ-CODE-LIVE-WIKI-LIVE] agent-spec MUST provide a repo-local code live wiki that treats raw source files as the source of truth and maintained wiki articles as agent-readable working memory.

[REQ-CODE-LIVE-WIKI-STRUCTURE] The wiki MUST scaffold `_index.md`, `_architecture.md`, `_patterns.md`, `_log.md`, `_meta.json`, `architecture/`, `modules/`, `concepts/`, `decisions/`, `learnings/`, and `queries/`.

[REQ-CODE-LIVE-WIKI-TRACE] Every maintained markdown article MUST declare `source_files` frontmatter so stale articles can be detected from code changes.

[REQ-CODE-LIVE-WIKI-RUST] Rust projects MUST expose a deterministic architecture inventory and Mermaid dependency diagram derived from Cargo workspace metadata when `Cargo.toml` is present.

[REQ-CODE-LIVE-WIKI-GENERIC] Non-Rust projects MUST still get generic source inventory, article linting, index generation, metadata, and stale-page status.

[REQ-CODE-LIVE-WIKI-DOGFOOD] This feature MUST be developed behind an agent-spec Task Contract that satisfies this requirement and binds mechanical tests to each observable behavior.

[REQ-CODE-LIVE-WIKI-SYMLINK] Wiki source, article, and architecture inventory traversal MUST reject symbolic links instead of following them across repository boundaries.

[REQ-CODE-LIVE-WIKI-SINGLE-MODEL] The CLI MUST expose only the maintained `.agent-spec/wiki` code-live model and MUST NOT retain the superseded generated `docs/wiki` command path.

## Scenarios

Scenario: Live wiki scaffold
  Given a repository with Rust source files
  When the operator runs `agent-spec wiki init --code . --wiki .agent-spec/wiki`
  Then `.agent-spec/wiki/_index.md`, `_architecture.md`, `_meta.json`, `architecture/inventory.json`, and `architecture/workspace.mmd` exist

Scenario: Rust architecture inventory
  Given a Cargo workspace with a local path dependency
  When the operator runs `agent-spec wiki inventory --format json`
  Then the JSON includes workspace packages and a local dependency edge

Scenario: Stale article status
  Given a wiki article whose frontmatter lists `source_files: [src/lib.rs]`
  When `src/lib.rs` changes after the recorded wiki metadata commit
  Then `agent-spec wiki status` reports that article as stale

Scenario: Live wiki lint
  Given a maintained wiki article without `source_files`
  When the operator runs `agent-spec wiki lint`
  Then the command fails with a `wiki-source-files-missing` diagnostic

Scenario: Wiki source traversal rejects symlinks
  Given a wiki-discoverable source path is a symbolic link
  When source discovery runs
  Then the linked content is not inventoried and an Error-level symlink diagnostic is emitted

Scenario: Superseded generated wiki commands are unavailable
  Given the code live wiki replaced the generated docs wiki model
  When the CLI parses a removed wiki plan, generate, legacy-check, export-github, or install-ci command
  Then command parsing fails

## Source Trace

- LLMWiki methodology: https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f
- Rust CodeWiki reference implementation: /Users/zhangalex/Work/Projects/FW/rust-agents/codewiki
- Rust Cargo metadata pattern reference: /Users/zhangalex/Work/Projects/consult/symposium
- Supersedes the generated AutoWiki plan: knowledge/requirements/req-autowiki-living-docs.md
