spec: task
name: "Cross-Project Wiki"
tags: [wiki, architecture, data-flow, cross-project]
satisfies: [REQ-CROSS-PROJECT-WIKI]
depends: [task-code-live-wiki-deepening]
---

## Intent

Build a deterministic cross-project wiki so agents can inspect important
dependent projects, their mechanisms, and data flows without treating external
repositories as current-project source files.

## Decisions

- Add `src/spec_wiki/project_map.rs` for project-map parsing, validation, and rendering.
- Use `.agent-spec/wiki/projects/*.md` for project articles.
- Use `.agent-spec/wiki/flows/*.md` for cross-project mechanism/data-flow articles.
- Keep `source_files` repo-local and introduce `external_sources` as non-dereferenced labels.
- Add `wiki project-map` and `wiki inspect-project`.
- Treat flow `requirements` and `specs` as current-repository references; use `external_sources` for outside evidence.
- Require exact checked-in project-map JSON and Mermaid artifacts in live wiki lint/check.
- Pass the code root explicitly to project inspection and require `--out` with `project-map --check`.
- Require flow `specs` to declare task-level Task Contracts, not merely parseable specs.
- Require every project and flow article field in the KLL schema to be present and non-empty, and reject malformed or duplicate frontmatter entries.
- Report project/flow directory enumeration failures and reject symlinked maintained articles.
- Require `wiki init --check` to validate the maintained directories in the target wiki before creating its temporary comparison wiki.
- Dogfood the project map with agent-spec's real `codewiki` and `symposium` relationships.
- Do not add network calls, LLM calls, or external repository scans.

## Boundaries

### Allowed Changes
- src/spec_wiki/**
- src/main.rs
- README.md
- AGENTS.md
- CHANGELOG.md
- skills/agent-spec-wiki/SKILL.md
- knowledge/requirements/req-cross-project-wiki.md
- specs/task-cross-project-wiki.spec.md
- fixtures/wiki-cross-project/**
- .agent-spec/wiki/**
- docs/superpowers/plans/2026-07-09-cross-project-wiki.md

### Forbidden
- Do not add dependencies.
- Do not read external project files by default.
- Do not reinterpret `external_sources` as repo-local `source_files`.
- Do not make project map facts durable KLL truth.

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — Flat task scenarios map directly to the KLL clauses; capability promotion is out of scope. -->

Scenario: Project map renders external projects and flows
  Test:
    Filter: test_project_map_builds_projects_flows_edges_and_external_sources
    Level: integration
  Given wiki project and flow articles for `agent-spec` and `brain-rs`
  When `build_project_map` runs
  Then the map contains both projects, one flow, one edge, protocols, requirements, specs, and external source labels

Scenario: Project map lint rejects broken references
  Test: test_project_map_reports_unknown_flow_project
  Given a flow article references a project id with no project article
  When `build_project_map` runs
  Then diagnostics include `wiki-project-flow-unknown-project`

Scenario: Project and flow article contracts reject malformed identity and source data
  Test: test_project_map_reports_contract_diagnostics
  Given malformed articles, duplicate ids, a repeated project member, and a missing repo-local source
  When `build_project_map` runs
  Then diagnostics identify every malformed article, identity violation, membership violation, and missing source

Scenario: Project and flow article schemas reject incomplete or malformed frontmatter
  Test:
    Filter: test_project_map_rejects_incomplete_and_malformed_articles
    Level: integration
  Given project or flow frontmatter has a malformed line or omits required non-empty fields
  When `build_project_map` runs
  Then diagnostics identify each malformed line or missing field and exclude the incomplete articles from the map

Scenario: Flow trace references resolve inside the current repository
  Test:
    Filter: test_project_map_reports_invalid_requirement_and_spec_references
    Level: integration
  Given a flow references an unknown requirement id, a missing spec path, and a parseable project-level spec
  When `build_project_map` runs
  Then diagnostics include unknown-requirement, missing-spec, and invalid-Task-Contract errors without reading `external_sources`

Scenario: Wiki lint includes project-map diagnostics
  Test: test_wiki_lint_reports_project_map_diagnostics
  Given a live wiki has a broken project flow
  When `lint_live_wiki` runs
  Then diagnostics include project-map diagnostics

Scenario: Wiki lint rejects missing or drifted project-map artifacts
  Test: test_wiki_lint_reports_project_map_artifact_drift
  Given maintained articles differ from missing or stale JSON and Mermaid artifacts
  When `lint_live_wiki` runs
  Then diagnostics identify each missing or drifted project-map artifact

Scenario: Wiki init check preserves maintained cross-project articles
  Test: test_wiki_init_check_preserves_maintained_project_articles
  Given a target wiki contains maintained project and flow articles with current derived artifacts
  When `wiki init --check` runs
  Then the check passes against the non-empty project map

Scenario: Wiki init check rejects missing maintained directories
  Test:
    Filter: test_wiki_init_check_rejects_missing_maintained_directories
    Level: integration
  Given the target wiki is missing `projects/` or `flows/`
  When `wiki init --check` runs
  Then the command returns an error before creating the temporary comparison wiki

Scenario: Wiki init check rejects project-map diagnostics
  Test:
    Filter: test_wiki_init_check_rejects_project_map_diagnostics
    Level: integration
  Given maintained project or flow articles produce error diagnostics
  When `wiki init --check` runs against matching derived artifacts
  Then the command returns an error

Scenario: Wiki project-map command parses all output fields
  Test: test_wiki_project_map_cli_parses_nested_subcommand
  Given CLI arguments for `wiki project-map`
  When Clap parses them
  Then the command contains code, wiki, format, out, and check fields

Scenario: Wiki project-map command writes and checks a concrete artifact
  Test:
    Filter: test_wiki_project_map_command_writes_and_checks_artifact
    Level: integration
  Given a valid project map and an output path
  When the command writes and then checks the artifact
  Then exact content passes and drifted content fails

Scenario: Wiki project-map check requires an output path
  Test:
    Filter: test_wiki_project_map_check_requires_out
    Level: unit
  Given CLI arguments contain `wiki project-map --check` without `--out`
  When Clap parses them
  Then parsing fails

Scenario: Wiki project-map check rejects error diagnostics
  Test:
    Filter: test_wiki_project_map_check_rejects_error_diagnostics
    Level: integration
  Given an output artifact exactly matches a project map containing error diagnostics
  When the command checks the artifact
  Then the command returns an error

Scenario: Inspect project reports related flows
  Test:
    Filter: test_wiki_inspect_project_reports_related_flows
    Level: integration
  Given a known project id appears in one project article and one flow article
  When `inspect_wiki_project` runs
  Then the report contains the project, related flow, protocols, requirements, specs, and external source labels

Scenario: Inspect project validates sources against the explicit code root
  Test: test_wiki_inspect_project_uses_explicit_code_root
  Given the repository root differs from the process current directory
  When `inspect_wiki_project` receives that root
  Then repo-local source diagnostics are empty

Scenario: Project-map output is portable and Mermaid-safe
  Test:
    Filter: test_project_map_renders_safe_mermaid_and_portable_json
    Level: unit
  Given repo-relative source, article, and diagnostic paths plus a Mermaid edge label containing delimiters
  When JSON and Mermaid output render
  Then every JSON path uses slash separators and Mermaid output remains syntactically escaped

Scenario: Project map reports article directory enumeration failures
  Test:
    Filter: test_project_map_reports_article_enumeration_failures
    Level: integration
  Given a project or flow article directory is missing or unreadable, or one of its entries cannot be enumerated
  When `build_project_map` runs
  Then an error diagnostic identifies the unreadable directory or entry instead of silently returning an empty collection

Scenario: agent-spec dogfoods real cross-project relationships
  Test:
    Filter: test_agent_spec_wiki_tracks_project_map_artifacts
    Level: integration
  Given agent-spec adapted wiki methodology from `codewiki` and Cargo metadata modeling from `symposium`
  When the checked-in project map is built
  Then it contains all three projects, both directed relationships, requirement and Task Contract links, local source evidence, exact checked-in JSON and Mermaid artifacts, and no diagnostics

Scenario: Documentation explains cross-project wiki authoring
  Test: test_docs_describe_cross_project_wiki_authoring
  Given README, AGENTS, and the wiki skill
  When documentation tests inspect them
  Then they include complete frontmatter examples and describe regular Markdown files, project articles, flow articles, `source_files`, `external_sources`, project-map JSON, Mermaid output, and no external repository scan by default
