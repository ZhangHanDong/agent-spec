---
name: agent-spec-wiki
description: Use when initializing, checking, enriching, or reviewing agent-spec code live wiki pages, source_files trace, or Rust architecture inventory.
---

# Agent-Spec Code Live Wiki Workflow

Use this skill for repo-local code live wiki maintenance and review.

## Rules

- Treat `.agent-spec/wiki/` as agent working memory, not KLL truth.
- Put durable truth in `knowledge/`, executable contracts in `specs/`, and
  reader-authored docs in `docs/`.
- Keep `.agent-spec/wiki/**` as tracked live wiki memory when the project wants it in git;
  keep `.agent-spec/runs`, `.agent-spec/trace`, temp files, and runtime state
  ignored.
- Preserve `source_files` frontmatter in every maintained wiki article.
- Before broad source reading, run `wiki status` and prefer fresh wiki articles
  when they cover the needed area.
- Use `wiki query <text>` before opening many files, and use `wiki inspect
  <path>` when you need the wiki pages, KLL requirements, and task specs related
  to a source or knowledge path.
- After code or architecture changes, update affected articles, run `wiki index`,
  then run `wiki lint` or `wiki check`.
- Archive old wiki material into `learnings/` or `archive/` summaries with
  source links instead of deleting it abruptly.
- Non-goals: no built-in LLM long-form wiki generation, no web UI, and no
  replacement for KLL requirements or decisions.
- Rust architecture inventory is deterministic Cargo metadata; prose remains
  agent-authored and must cite source files.
- Reject symlinks in source, article, and architecture traversal. The CLI has no
  generated `docs/wiki` compatibility mode; `.agent-spec/wiki` is the single
  maintained wiki model.

## Workflow

```bash
agent-spec wiki init --code . --wiki .agent-spec/wiki
agent-spec wiki seed --code . --wiki .agent-spec/wiki
agent-spec wiki seed --code . --wiki .agent-spec/wiki --check
agent-spec wiki status --code . --wiki .agent-spec/wiki
agent-spec wiki query "intent compiler" --wiki .agent-spec/wiki
agent-spec wiki inspect src/spec_wiki/live.rs --code . --wiki .agent-spec/wiki
agent-spec wiki inventory --code . --format json
agent-spec wiki inventory --code . --format mermaid
agent-spec wiki project-map --code . --wiki .agent-spec/wiki --format json --out .agent-spec/wiki/architecture/project-map.json
agent-spec wiki project-map --code . --wiki .agent-spec/wiki --format mermaid --out .agent-spec/wiki/architecture/project-map.mmd
agent-spec wiki inspect-project brain-rs --code . --wiki .agent-spec/wiki --format text
agent-spec wiki index --wiki .agent-spec/wiki
agent-spec wiki lint --code . --wiki .agent-spec/wiki
agent-spec wiki check --code . --wiki .agent-spec/wiki
agent-spec wiki meta update --code . --wiki .agent-spec/wiki
bash scripts/docs-lint.sh
```

## Article Format

```markdown
---
title: "Parser"
type: module
source_files:
  - src/spec_parser/parser.rs
tags:
  - parser
---

# Parser
```

## Architecture

For Rust projects, `wiki inventory` writes a Rust architecture inventory model
from Cargo metadata and can render a Mermaid graph. For non-Rust projects, the
generic inventory still supports code live wiki status, index, lint, and
metadata.

Generated architecture files:

- `.agent-spec/wiki/architecture/inventory.json`
- `.agent-spec/wiki/architecture/workspace.mmd`
- `.agent-spec/wiki/architecture/modules.mmd`

The wiki compounds project understanding over time; it is not regenerated as a
large batch of shallow file summaries.

## Cross-Project Wiki

Use project articles for important dependent projects and flow articles for
working mechanisms or data flow between projects.
Both article types must be regular Markdown files. Do not use symlinks: project
map discovery and `wiki init --check` reject them explicitly.
Every field shown below is required and non-empty. Treat malformed lines,
duplicate keys, or missing fields as article errors; do not generate a partial
project map from them.

- Project articles: `.agent-spec/wiki/projects/*.md`
- Flow articles: `.agent-spec/wiki/flows/*.md`
- `source_files`: repo-local files that drive stale checks
- `external_sources`: outside paths, URLs, or repo ids recorded as evidence
  labels; agent-spec performs no external repository scan by default

Project article example:

```md
---
title: "brain-rs"
type: external-project
project_id: brain-rs
repo: rust-agents/brain-rs
role: "Context provider"
interfaces: [cli, json]
protocols: [stdio]
status: active
source_files:
  - src/integration/brain.rs
external_sources:
  - https://example.invalid/rust-agents/brain-rs
---
# brain-rs
```

Flow article example:

```md
---
title: "agent-spec to brain-rs context flow"
type: project-flow
flow_id: agent-spec-to-brain
projects:
  - agent-spec
  - brain-rs
kind: calls
protocols: [stdio, json]
requirements:
  - REQ-CROSS-PROJECT-WIKI
specs:
  - specs/task-cross-project-wiki.spec.md
source_files:
  - src/integration/brain.rs
external_sources:
  - https://example.invalid/rust-agents/brain-rs/src/lib.rs
---
# agent-spec to brain-rs context flow
```

The `projects` list is ordered; each adjacent pair becomes a directed edge.
`requirements` and `specs` resolve inside the current repository. Put paths,
URLs, and repository identifiers from outside the current repository in
`external_sources` only.

After editing project or flow articles, run:

```bash
agent-spec wiki project-map --code . --wiki .agent-spec/wiki --format json --out .agent-spec/wiki/architecture/project-map.json
agent-spec wiki project-map --code . --wiki .agent-spec/wiki --format mermaid --out .agent-spec/wiki/architecture/project-map.mmd
agent-spec wiki index --wiki .agent-spec/wiki
agent-spec wiki check --code . --wiki .agent-spec/wiki
```

The project-map JSON and Mermaid output are derived artifacts. The maintained
truth remains the project articles and flow articles.
`wiki lint` and `wiki check` require both derived artifacts to exist and match
the maintained project and flow articles exactly.
