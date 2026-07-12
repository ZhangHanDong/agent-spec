---
kind: requirement
id: REQ-CROSS-PROJECT-WIKI
title: "Cross-Project Wiki"
status: accepted
liveness: auto
tags: [wiki, architecture, data-flow, cross-project]
---

# Cross-Project Wiki

## Problem

The code live wiki records the current repository's modules, requirements,
specs, and trace evidence, but important systems often depend on other
first-class projects. Agents need a deterministic way to see those dependent
projects, the mechanisms between them, and the data flow evidence without
treating external repositories as part of the current source tree.

## Requirements

[REQ-CROSS-PROJECT-WIKI-PROJECTS] The wiki MUST support maintained project articles under `.agent-spec/wiki/projects/` with stable project ids, repo labels, roles, interfaces, protocols, status, repo-local source files, and external source labels.

[REQ-CROSS-PROJECT-WIKI-FLOWS] The wiki MUST support maintained flow articles under `.agent-spec/wiki/flows/` that connect two or more project ids and record kind, protocols, requirements, specs, repo-local source files, and external source labels.

[REQ-CROSS-PROJECT-WIKI-MAP] The CLI MUST build deterministic project-map JSON and Mermaid graph output from project and flow articles.

[REQ-CROSS-PROJECT-WIKI-LINT] `wiki lint` and `wiki check` MUST report duplicate project ids, malformed project ids, unknown flow project refs, flows with fewer than two projects, and missing repo-local source files as diagnostics.

[REQ-CROSS-PROJECT-WIKI-INSPECT] The CLI MUST expose `wiki inspect-project <project-id>` to list the project article, related flows, protocols, requirements, specs, and external source labels.

[REQ-CROSS-PROJECT-WIKI-DOCS] README, AGENTS, and the agent-spec wiki skill MUST document the difference between repo-local `source_files` and non-dereferenced `external_sources`.

[REQ-CROSS-PROJECT-WIKI-FLOW-IDENTITY] Flow ids MUST be stable, lowercase kebab-case, and unique; every flow MUST reference at least two distinct known project ids; malformed project and flow articles MUST produce diagnostics instead of being ignored.

[REQ-CROSS-PROJECT-WIKI-ARTICLE-SCHEMA] Project articles MUST provide non-empty `title`, `repo`, `role`, `interfaces`, `protocols`, `status`, `source_files`, and `external_sources`; flow articles MUST provide non-empty `title`, `projects`, `kind`, `protocols`, `requirements`, `specs`, `source_files`, and `external_sources`. Invalid frontmatter syntax and duplicate keys MUST produce diagnostics, and incomplete articles MUST NOT enter the project map.

[REQ-CROSS-PROJECT-WIKI-REFERENCES] Flow `requirements` MUST resolve to current-repository KLL requirement ids and flow `specs` MUST be repo-relative, remain inside the code root, exist, parse successfully, and declare task-level Task Contracts. Outside-repository evidence MUST use `external_sources` instead.

[REQ-CROSS-PROJECT-WIKI-ARTIFACT-GATE] `wiki lint` and `wiki check` MUST reject missing or drifted `architecture/project-map.json` and `architecture/project-map.mmd`; `wiki init --check` MUST derive them from the maintained articles in the target wiki.

[REQ-CROSS-PROJECT-WIKI-CLI-ROOT] `wiki inspect-project` MUST validate repo-local sources against an explicit code root, and `wiki project-map --check` MUST require an output path and fail when the project map contains error diagnostics.

[REQ-CROSS-PROJECT-WIKI-PORTABLE] Every project-map JSON path, including diagnostic paths, MUST use slash separators and Mermaid output MUST escape user-authored labels so output remains deterministic and syntactically valid.

[REQ-CROSS-PROJECT-WIKI-DISCOVERY] Project and flow article directories MUST report enumeration failures instead of becoming silently empty, maintained project/flow articles MUST be regular Markdown files rather than symlinks, and `wiki init --check` MUST fail when either maintained directory is missing from the target wiki.

[REQ-CROSS-PROJECT-WIKI-DOGFOOD] The agent-spec repository MUST maintain a non-empty project map that records its actual methodology and metadata-model relationships with `codewiki` and `symposium`, with requirements, Task Contracts, and local source evidence.

## Scenarios

Scenario: Project map renders external projects and flows
  Given wiki project and flow articles for `agent-spec` and `brain-rs`
  When the project map builder runs
  Then the JSON contains both projects, one flow, one derived edge, protocols, requirements, specs, and external source labels

Scenario: Project map lint rejects broken references
  Given a flow article references `missing-project`
  When live wiki lint runs
  Then diagnostics include `wiki-project-flow-unknown-project`

Scenario: Project map lint covers article and graph identity errors
  Given malformed articles, duplicate ids, a repeated project member, and a missing repo-local source
  When live wiki lint runs
  Then diagnostics identify every malformed article, identity violation, membership violation, and missing source

Scenario: Project and flow article schemas are complete
  Given an article has invalid frontmatter syntax or omits a required non-empty field
  When the project map builder runs
  Then diagnostics identify the malformed line or missing field and the article is excluded from the map

Scenario: Flow trace references resolve inside the current repository
  Given a flow references an unknown requirement id, a missing spec path, or a parseable non-task spec
  When live wiki lint runs
  Then diagnostics identify the unknown requirement and invalid Task Contract without reading `external_sources`

Scenario: Project map artifacts are required and current
  Given project or flow articles differ from the checked-in project-map artifacts
  When `wiki lint` or `wiki check` runs
  Then diagnostics identify each missing or drifted JSON and Mermaid artifact

Scenario: Init check preserves maintained articles
  Given a target wiki contains maintained project and flow articles with current derived artifacts
  When `wiki init --check` runs
  Then the check passes without replacing the maintained articles with an empty project map

Scenario: Init check rejects invalid maintained articles
  Given maintained project or flow articles produce error diagnostics
  When `wiki init --check` runs against matching derived artifacts
  Then the command exits with an error

Scenario: Init check rejects missing maintained directories
  Given the target wiki is missing `projects/` or `flows/`
  When `wiki init --check` runs
  Then the command exits with an error instead of recreating the directory only in its temporary wiki

Scenario: Project inspection uses an explicit code root
  Given the process current directory differs from the repository root
  When `wiki inspect-project brain-rs --code <root>` runs
  Then repo-local source validation uses `<root>` and reports no false missing-source diagnostics

Scenario: Project map check has a concrete artifact target
  Given `wiki project-map --check` has no `--out` path
  When command arguments are parsed
  Then argument parsing fails instead of printing an unchecked map

Scenario: Project map check rejects diagnostic errors
  Given the output artifact exactly matches a project map that contains error diagnostics
  When `wiki project-map --check --out <path>` runs
  Then the command exits with an error

Scenario: Project map output is portable and safe
  Given repo-relative source, article, and diagnostic paths plus user-authored Mermaid edge labels
  When JSON and Mermaid project maps render
  Then every path uses slash separators and Mermaid delimiters are escaped

Scenario: Article discovery failures are visible
  Given a project or flow article directory is missing or unreadable, an entry cannot be enumerated, or an article is symlinked
  When the project map builder runs
  Then an error diagnostic identifies the directory or article instead of silently omitting it

Scenario: agent-spec dogfoods real cross-project relationships
  Given agent-spec adapted wiki methodology from `codewiki` and Cargo metadata modeling from `symposium`
  When the checked-in project map is built
  Then it contains all three projects, both directed relationships, requirement and Task Contract links, local source evidence, exact derived artifacts, and no diagnostics

Scenario: Inspect project reports related flows
  Given a known project id appears in one project article and one flow article
  When `wiki inspect-project brain-rs --format json` runs
  Then the output contains the project article, related flow, protocols, requirements, specs, and external source labels

Scenario: Documentation explains source boundaries
  Given README, AGENTS, and the wiki skill
  When documentation tests inspect them
  Then they include complete project and flow frontmatter examples and describe `source_files`, `external_sources`, project-map JSON, Mermaid output, and no external repository scan by default

## Dependencies

- REQ-CODE-LIVE-WIKI-DEEPENING

## Source Trace

- Implementation plan: docs/superpowers/plans/2026-07-09-cross-project-wiki.md
- Builds on code live wiki requirement: knowledge/requirements/req-code-live-wiki-deepening.md
