---
title: "codewiki"
type: external-project
project_id: codewiki
repo: rust-agents/codewiki
role: "Reference implementation for LLMWiki-style code documentation"
interfaces:
  - source-reference
protocols:
  - llmwiki-methodology
  - markdown
status: reference
source_files:
  - knowledge/requirements/req-code-live-wiki.md
  - src/spec_wiki/live.rs
external_sources:
  - rust-agents/codewiki
tags:
  - reference
  - wiki
---

# codewiki

`codewiki` is an implementation reference for repository-oriented wiki
structure and maintenance workflows. `agent-spec` adapts the methodology while
keeping its own CLI deterministic and leaving long-form interpretation to an
agent-authored skill.
