# AutoWiki Living Docs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an agent-spec wiki feature that generates, checks, versions, and exports a living repository wiki from code, KLL artifacts, specs, lifecycle evidence, and documentation standards.

**Architecture:** Keep the CLI deterministic and local-first. The core wiki pipeline builds a `WikiPlan` from repository sources, renders structured Markdown pages with explicit source trace and run metadata, checks generated pages for drift/staleness/link integrity, and optionally exports a flattened GitHub Wiki tree. AI prose drafting, visual screenshots, and push-to-remote behavior stay outside the deterministic core and enter only through reviewed files or explicit manifests.

**Tech Stack:** Rust 2024, existing `clap`, `serde`, `serde_json`, hand-written file scanners, current `src/spec_knowledge/` requirement/spec/trace modules, current docs lint script, Markdown output, no new network dependency, no LLM calls in CLI code, no `serde_yaml`.

## Global Constraints

- Do not add network calls, LLM calls, cloud sync, GitHub API calls, or remote pushes to the core wiki CLI.
- Do not add `serde_yaml`; use existing frontmatter and Markdown parsing patterns.
- Every generated wiki page must carry deterministic metadata: page id, source paths, source fingerprints, generated commit or VCS reference, dirty state, generator version, and timestamp.
- Generated wiki pages must distinguish evidence-backed facts from prose summaries. If a page cannot cite a source file, requirement id, spec path, trace record, or command output, emit a diagnostic instead of inventing content.
- `docs/wiki/` is generated output. Human-authored docs remain under existing `docs/` paths outside `docs/wiki/`.
- `knowledge/` remains the machine-consumable truth layer. Wiki generation may summarize KLL artifacts but must not become a second source of truth.
- `specs/` remains the executable contract layer. Wiki pages may link specs and scenarios but must not rewrite spec contents.
- Default wiki generation must be deterministic across repeated runs on the same worktree state.
- Default wiki checks must fail on drift, missing source paths, stale fingerprints, malformed generated frontmatter, missing required pages, broken internal links, or missing source trace sections.
- External tools remain optional. `bash scripts/docs-lint.sh` checks generated Markdown quality after wiki generation; built-in Chinese docs lint still always runs.
- Visual screenshots are accepted only through a deterministic manifest that points to existing image files. The wiki CLI must not launch browsers, apps, or terminals.
- GitHub Wiki export must flatten page paths, rewrite internal links, and generate `Home.md` and `_Sidebar.md`, but it must not clone, replace, or push `{repo}.wiki.git`.
- CI installation must generate check workflows by default. Any workflow that commits or pushes generated wiki output must be an explicit future proposal, not hidden in this implementation.
- Archive output and historical trace records are valid sources for wiki history pages, but archived specs are not active liveness guards.
- New functionality must dogfood agent-spec's own repository before relying on fixtures.
- New public commands must be documented in README, AGENTS, tool-first skills, and command references.
- Final verification must include `cargo fmt --check`, `cargo test --quiet`, `cargo clippy --all-targets -- -D warnings`, `lint-knowledge --gate`, `requirements plan --gate`, `wiki plan --gate`, `wiki generate --check`, `wiki export-github --check`, docs lint, and lifecycle on the self-hosting wiki task spec.

---

## External Reference Summary

Factory AutoWiki provides the product reference:

- AutoWiki reads a codebase and generates a browsable wiki covering architecture, modules, APIs, and conventions, then keeps it current on push.
- One generation produces Markdown that can render locally, in Factory's app, and in GitHub Wiki.
- The generation flow is codebase analysis, page generation, optional visual screenshots, upload, and GitHub sync.
- Refresh is installed through a CI workflow that runs on pushes to the default branch.
- GitHub Wiki sync flattens hierarchy, rewrites internal links, creates `_Sidebar.md` and `Home.md`, and replaces the wiki tree.
- Each generation is versioned with commit hash, branch, dirty state, tool version, and timestamp.
- Cloud sync can be disabled by enterprise policy.

agent-spec adaptation:

```text
code + tests + docs + knowledge + specs + lifecycle traces
  -> WikiPlan
  -> WikiPageSet
  -> docs/wiki/*.md + docs/wiki/search-index.json + docs/wiki/wiki-tree.json
  -> wiki check + docs lint + KLL/spec gates
  -> optional flattened GitHub Wiki export
```

AutoWiki parity deliberately excluded from this implementation:

- No cloud viewer.
- No cloud sync.
- No automatic remote push.
- No headless app launch from the wiki CLI.
- No model-generated prose inside CLI code.

## Target File Structure

- Create: `knowledge/requirements/req-autowiki-living-docs.md`
  - KLL requirement for the wiki feature.

- Create: `specs/task-autowiki-living-docs.spec.md`
  - Self-hosting Task Contract for implementation.

- Create: `src/spec_wiki/mod.rs`
  - Public facade and re-exports for wiki modules.

- Create: `src/spec_wiki/model.rs`
  - Serializable IR: `WikiPlan`, `WikiPage`, `WikiSource`, `WikiDiagnostic`, `WikiRunMetadata`, `WikiTree`, `WikiSearchIndex`, `WikiVisualAssetManifest`.

- Create: `src/spec_wiki/sources.rs`
  - Deterministic source discovery for code, docs, KLL artifacts, specs, trace ledgers, archive summaries, and cargo metadata files.

- Create: `src/spec_wiki/plan.rs`
  - Builds required page plan, page dependencies, source fingerprints, and stale diagnostics.

- Create: `src/spec_wiki/render.rs`
  - Renders Markdown pages, `wiki-tree.json`, `search-index.json`, and run metadata.

- Create: `src/spec_wiki/check.rs`
  - Checks generated output for drift, stale metadata, missing sources, malformed frontmatter, required pages, and internal link validity.

- Create: `src/spec_wiki/github.rs`
  - Flattens wiki paths, rewrites internal links, renders `Home.md`, and renders `_Sidebar.md`.

- Create: `src/spec_wiki/ci.rs`
  - Generates CI check workflows for GitHub Actions and GitLab CI without network or push behavior.

- Create: `src/spec_wiki/assets.rs`
  - Parses a deterministic visual asset manifest and attaches existing screenshots to pages.

- Modify: `src/main.rs`
  - Add `wiki` subcommands and tests.

- Modify: `src/spec_knowledge/mod.rs`
  - No direct dependency on wiki modules; only add helpers if a task proves they belong in KLL.

- Modify: `Cargo.toml`
  - No new dependency expected. Change only if implementation shows an unavoidable standard crate gap.

- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CHANGELOG.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `skills/agent-spec-tool-first/references/commands.md`
- Modify: `.claude/skills/agent-spec-tool-first/SKILL.md`
- Modify: `.claude/skills/agent-spec-tool-first/references/commands.md`
  - Document wiki workflow and command semantics.

- Create: `skills/agent-spec-wiki/SKILL.md`
  - Agent skill for reviewed prose enrichment and visual asset manifest authoring.

- Create: `fixtures/wiki-mini/`
  - Small Rust fixture demonstrating generated wiki pages, KLL/spec trace links, visual asset manifest handling, and GitHub Wiki export.

---

### Task 1: Self-Hosting KLL Requirement And Task Contract

**Files:**
- Create: `knowledge/requirements/req-autowiki-living-docs.md`
- Create: `specs/task-autowiki-living-docs.spec.md`

**Interfaces:**
- Consumes: Existing KLL requirement schema and task spec schema.
- Produces: Requirement id `REQ-AUTOWIKI-LIVING-DOCS`; task spec satisfying that id.

- [ ] **Step 1: Write the KLL requirement**

Create `knowledge/requirements/req-autowiki-living-docs.md`:

```md
---
kind: requirement
id: REQ-AUTOWIKI-LIVING-DOCS
title: "AutoWiki Living Docs"
liveness: auto
tags: [docs, wiki, kll, trace]
---

# AutoWiki Living Docs

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
```

- [ ] **Step 2: Write the task spec**

Create `specs/task-autowiki-living-docs.spec.md`:

```spec
spec: task
name: "AutoWiki Living Docs"
tags: [docs, wiki, kll, trace]
satisfies: [REQ-AUTOWIKI-LIVING-DOCS]
depends: [task-requirements-compiler-plan-dag]
---

## Intent

Add a deterministic living wiki pipeline to agent-spec. It should summarize
repository structure, KLL requirements, specs, trace evidence, docs standards,
and verification state as Markdown while preserving source trace and stale
output checks.

## Decisions

- Add `src/spec_wiki/` as the wiki module family.
- Add `agent-spec wiki plan`.
- Add `agent-spec wiki generate`.
- Add `agent-spec wiki check`.
- Add `agent-spec wiki export-github`.
- Add `agent-spec wiki install-ci`.
- Keep AI prose drafting outside CLI code.
- Accept screenshots only through a checked asset manifest.
- Do not push to GitHub Wiki from the CLI.

## Boundaries

### Allowed Changes
- src/**
- README.md
- AGENTS.md
- CHANGELOG.md
- docs/superpowers/plans/**
- knowledge/requirements/**
- specs/task-autowiki-living-docs.spec.md
- skills/agent-spec-tool-first/**
- .claude/skills/agent-spec-tool-first/**
- skills/agent-spec-wiki/**
- fixtures/wiki-mini/**

### Forbidden
- Do not add network calls.
- Do not add LLM calls.
- Do not add `serde_yaml`.
- Do not push to a remote repository.
- Do not treat generated wiki Markdown as KLL truth.

## Completion Criteria

Scenario: Wiki plan exposes required pages and source trace
  Test: test_wiki_plan_json_contains_pages_sources_and_metadata
  Given a fixture repository with code, KLL requirements, specs, docs, and trace ledgers
  When `cmd_wiki_plan` renders JSON
  Then the output contains required pages, source paths, fingerprints, metadata, and no gate-blocking diagnostics

Scenario: Wiki generation renders deterministic local Markdown
  Test: test_wiki_generate_writes_markdown_tree_search_and_metadata
  Given a fixture wiki plan
  When `cmd_wiki_generate` writes `docs/wiki`
  Then the output contains Markdown pages, `wiki-tree.json`, `search-index.json`, and run metadata

Scenario: Wiki check fails on stale generated output
  Test: test_wiki_check_fails_on_stale_source_fingerprint
  Given a generated wiki page with a stale source fingerprint
  When `cmd_wiki_check` validates it
  Then it returns a `wiki-stale-source` diagnostic

Scenario: GitHub Wiki export rewrites paths and links
  Test: test_wiki_export_github_flattens_tree_and_rewrites_links
  Given nested generated wiki pages with internal links
  When `cmd_wiki_export_github` renders an export tree
  Then it contains flattened filenames, rewritten internal links, `Home.md`, and `_Sidebar.md`

Scenario: CI workflow is generated as a check gate
  Test: test_wiki_install_ci_writes_github_check_workflow
  Given GitHub provider options
  When `cmd_wiki_install_ci` renders workflow content
  Then the workflow runs wiki generate/check and docs lint without secrets or remote push

Scenario: Visual assets are manifest-driven
  Test: test_wiki_visual_asset_manifest_embeds_existing_images_only
  Given a visual asset manifest with one existing image and one missing image
  When wiki generation reads the manifest
  Then the existing image is embedded and the missing image emits a diagnostic

Scenario: Wiki workflow is documented
  Test: test_docs_describe_autowiki_living_docs_workflow
  Given README, AGENTS, tool-first skills, and command references
  When documentation tests inspect their content
  Then they mention wiki plan, wiki generate, wiki check, wiki export-github, wiki install-ci, source trace, GitHub Wiki export, visual asset manifest, and local-first operation
```

- [ ] **Step 3: Verify the new KLL/spec pair is wired**

Run:

```bash
cargo run --quiet -- lint-knowledge --knowledge knowledge --gate
cargo run --quiet -- requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected:

```text
lint-knowledge exits 0 with no errors.
requirements plan exits 0 and includes REQ-AUTOWIKI-LIVING-DOCS coverage by specs/task-autowiki-living-docs.spec.md.
```

- [ ] **Step 4: Commit**

```bash
git add knowledge/requirements/req-autowiki-living-docs.md specs/task-autowiki-living-docs.spec.md
git commit -m "spec: add autowiki living docs contract"
```

Expected:

```text
Commit succeeds.
```

---

### Task 2: Add Wiki IR And Source Discovery

**Files:**
- Create: `src/spec_wiki/mod.rs`
- Create: `src/spec_wiki/model.rs`
- Create: `src/spec_wiki/sources.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: repository root paths and existing KLL/spec/trace/archive files.
- Produces: `WikiPlan`, `WikiPage`, `WikiSource`, `WikiRunMetadata`, `WikiDiagnostic`, `discover_wiki_sources(root: &Path, opts: &WikiSourceOptions) -> WikiSourceSet`.

- [ ] **Step 1: Add failing model and source discovery tests**

Add a test module in `src/spec_wiki/sources.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_wiki_sources_sorts_and_classifies_inputs() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-wiki-sources-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::create_dir_all(dir.join(".agent-spec/trace")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
        fs::write(dir.join("knowledge/requirements/req-add.md"), "---\nkind: requirement\nid: REQ-ADD\ntitle: \"Add\"\nliveness: auto\n---\n## Problem\nAdd.\n").unwrap();
        fs::write(dir.join("specs/task-add.spec.md"), "spec: task\nname: \"Add\"\nsatisfies: [REQ-ADD]\n---\n## Intent\nAdd.\n").unwrap();
        fs::write(dir.join(".agent-spec/trace/run.json"), "{\"version\":1,\"records\":[],\"diagnostics\":[]}").unwrap();

        let opts = WikiSourceOptions::default();
        let set = discover_wiki_sources(&dir, &opts);

        assert!(set.sources.iter().any(|s| s.kind == WikiSourceKind::Code));
        assert!(set.sources.iter().any(|s| s.kind == WikiSourceKind::Knowledge));
        assert!(set.sources.iter().any(|s| s.kind == WikiSourceKind::Spec));
        assert!(set.sources.iter().any(|s| s.kind == WikiSourceKind::Trace));
        assert!(set.sources.windows(2).all(|w| w[0].path <= w[1].path));

        let _ = fs::remove_dir_all(dir);
    }
}
```

Run:

```bash
cargo test test_discover_wiki_sources_sorts_and_classifies_inputs --quiet
```

Expected:

```text
Compilation fails because src/spec_wiki/sources.rs and the source types do not exist.
```

- [ ] **Step 2: Create `src/spec_wiki/model.rs`**

Add:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiPlan {
    pub version: u32,
    pub metadata: WikiRunMetadata,
    pub pages: Vec<WikiPage>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiRunMetadata {
    pub generator: String,
    pub generator_version: String,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub dirty: Option<bool>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiPage {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub kind: WikiPageKind,
    pub sources: Vec<WikiSourceRef>,
    pub sections: Vec<WikiSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum WikiPageKind {
    Home,
    Architecture,
    Requirements,
    Module,
    Api,
    Decisions,
    Testing,
    Conventions,
    History,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSection {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSourceSet {
    pub sources: Vec<WikiSource>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSource {
    pub kind: WikiSourceKind,
    pub path: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSourceRef {
    pub kind: WikiSourceKind,
    pub path: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum WikiSourceKind {
    Code,
    Cargo,
    Documentation,
    Knowledge,
    Spec,
    Trace,
    Archive,
    Asset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiDiagnostic {
    pub code: String,
    pub severity: String,
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiSourceOptions {
    pub include_archives: bool,
}

impl Default for WikiSourceOptions {
    fn default() -> Self {
        Self {
            include_archives: true,
        }
    }
}
```

- [ ] **Step 3: Create `src/spec_wiki/mod.rs`**

Add:

```rust
pub mod model;
pub mod sources;

pub use model::{
    WikiDiagnostic, WikiPage, WikiPageKind, WikiPlan, WikiRunMetadata, WikiSection, WikiSource,
    WikiSourceKind, WikiSourceOptions, WikiSourceRef, WikiSourceSet,
};
pub use sources::discover_wiki_sources;
```

- [ ] **Step 4: Implement deterministic source discovery**

Create `src/spec_wiki/sources.rs`:

```rust
use crate::spec_wiki::{
    WikiDiagnostic, WikiSource, WikiSourceKind, WikiSourceOptions, WikiSourceSet,
};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub fn discover_wiki_sources(root: &Path, opts: &WikiSourceOptions) -> WikiSourceSet {
    let mut sources = Vec::new();
    let mut diagnostics = Vec::new();
    collect_sources(root, root, opts, &mut sources, &mut diagnostics);
    sources.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.kind.cmp(&b.kind)));
    sources.dedup_by(|a, b| a.path == b.path && a.kind == b.kind);
    WikiSourceSet {
        sources,
        diagnostics,
    }
}

fn collect_sources(
    root: &Path,
    dir: &Path,
    opts: &WikiSourceOptions,
    sources: &mut Vec<WikiSource>,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        diagnostics.push(WikiDiagnostic {
            code: "wiki-source-read-error".into(),
            severity: "warning".into(),
            path: Some(dir.to_path_buf()),
            message: "directory could not be read during wiki source discovery".into(),
        });
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if should_skip(root, &path, opts) {
            continue;
        }
        if path.is_dir() {
            collect_sources(root, &path, opts, sources, diagnostics);
            continue;
        }
        if let Some(kind) = classify_source(root, &path, opts) {
            sources.push(WikiSource {
                kind,
                fingerprint: fingerprint_file(&path),
                path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
            });
        }
    }
}

fn should_skip(root: &Path, path: &Path, opts: &WikiSourceOptions) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let Some(first) = rel.components().next().and_then(|c| c.as_os_str().to_str()) else {
        return false;
    };
    matches!(first, "target" | ".git")
        || (!opts.include_archives && rel.components().any(|c| c.as_os_str() == "archive"))
}

fn classify_source(root: &Path, path: &Path, _opts: &WikiSourceOptions) -> Option<WikiSourceKind> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let text = rel.to_string_lossy();
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if text.starts_with("src/") && name.ends_with(".rs") {
        return Some(WikiSourceKind::Code);
    }
    if matches!(name, "Cargo.toml" | "Cargo.lock") {
        return Some(WikiSourceKind::Cargo);
    }
    if text.starts_with("knowledge/") && name.ends_with(".md") {
        return Some(WikiSourceKind::Knowledge);
    }
    if text.starts_with("specs/") && (name.ends_with(".spec") || name.ends_with(".spec.md")) {
        return Some(WikiSourceKind::Spec);
    }
    if text.starts_with(".agent-spec/trace/") && name.ends_with(".json") {
        return Some(WikiSourceKind::Trace);
    }
    if text.starts_with("docs/") && name.ends_with(".md") && !text.starts_with("docs/wiki/") {
        return Some(WikiSourceKind::Documentation);
    }
    if text.starts_with(".agent-spec/archive/") {
        return Some(WikiSourceKind::Archive);
    }
    None
}

fn fingerprint_file(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
```

- [ ] **Step 5: Wire `src/main.rs` module declaration**

Add near other module declarations:

```rust
mod spec_wiki;
```

- [ ] **Step 6: Run the source discovery test**

```bash
cargo test test_discover_wiki_sources_sorts_and_classifies_inputs --quiet
```

Expected:

```text
Test passes.
```

- [ ] **Step 7: Commit**

```bash
git add src/spec_wiki src/main.rs
git commit -m "feat: add wiki source discovery"
```

Expected:

```text
Commit succeeds.
```

---

### Task 3: Build Wiki Plan Pages

**Files:**
- Create: `src/spec_wiki/plan.rs`
- Modify: `src/spec_wiki/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `WikiSourceSet`, `WikiRunMetadata`.
- Produces: `build_wiki_plan(root: &Path, opts: &WikiPlanOptions) -> WikiPlan`.

- [ ] **Step 1: Add failing plan test**

Create a test in `src/spec_wiki/plan.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_wiki_plan_contains_required_pages_sources_and_metadata() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-wiki-plan-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
        fs::write(dir.join("knowledge/requirements/req-add.md"), "---\nkind: requirement\nid: REQ-ADD\ntitle: \"Add\"\nliveness: auto\n---\n## Problem\nAdd.\n").unwrap();
        fs::write(dir.join("specs/task-add.spec.md"), "spec: task\nname: \"Add\"\nsatisfies: [REQ-ADD]\n---\n## Intent\nAdd.\n").unwrap();

        let plan = build_wiki_plan(&dir, &WikiPlanOptions::default());

        assert_eq!(plan.version, 1);
        assert!(plan.pages.iter().any(|p| p.id == "WIKI-HOME"));
        assert!(plan.pages.iter().any(|p| p.id == "WIKI-ARCHITECTURE"));
        assert!(plan.pages.iter().any(|p| p.id == "WIKI-REQUIREMENTS"));
        assert!(plan.pages.iter().any(|p| p.id == "WIKI-TESTING"));
        assert!(plan.pages.iter().all(|p| !p.sources.is_empty()));
        assert_eq!(plan.metadata.generator, "agent-spec wiki");

        let _ = fs::remove_dir_all(dir);
    }
}
```

Run:

```bash
cargo test test_wiki_plan_contains_required_pages_sources_and_metadata --quiet
```

Expected:

```text
Compilation fails because build_wiki_plan does not exist.
```

- [ ] **Step 2: Extend model with plan options**

Add to `src/spec_wiki/model.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPlanOptions {
    pub include_archives: bool,
    pub timestamp: u64,
}

impl Default for WikiPlanOptions {
    fn default() -> Self {
        Self {
            include_archives: true,
            timestamp: 0,
        }
    }
}
```

- [ ] **Step 3: Implement `build_wiki_plan`**

Create `src/spec_wiki/plan.rs`:

```rust
use crate::spec_wiki::{
    discover_wiki_sources, WikiPage, WikiPageKind, WikiPlan, WikiPlanOptions, WikiRunMetadata,
    WikiSection, WikiSourceKind, WikiSourceOptions, WikiSourceRef,
};
use std::path::{Path, PathBuf};

pub fn build_wiki_plan(root: &Path, opts: &WikiPlanOptions) -> WikiPlan {
    let source_set = discover_wiki_sources(
        root,
        &WikiSourceOptions {
            include_archives: opts.include_archives,
        },
    );
    let refs = source_set
        .sources
        .iter()
        .map(|source| WikiSourceRef {
            kind: source.kind.clone(),
            path: source.path.clone(),
            fingerprint: source.fingerprint.clone(),
        })
        .collect::<Vec<_>>();

    let mut pages = vec![
        page(
            "WIKI-HOME",
            "Home",
            "Home.md",
            WikiPageKind::Home,
            refs.clone(),
            "Repository map generated from code, KLL, specs, docs, and trace evidence.",
        ),
        page(
            "WIKI-ARCHITECTURE",
            "Architecture",
            "architecture.md",
            WikiPageKind::Architecture,
            filter_refs(&refs, &[WikiSourceKind::Code, WikiSourceKind::Cargo]),
            "Architecture overview derived from source modules and cargo metadata.",
        ),
        page(
            "WIKI-REQUIREMENTS",
            "Requirements Trace",
            "requirements.md",
            WikiPageKind::Requirements,
            filter_refs(&refs, &[WikiSourceKind::Knowledge, WikiSourceKind::Spec, WikiSourceKind::Trace]),
            "Requirement, spec, scenario, test, and trace evidence index.",
        ),
        page(
            "WIKI-TESTING",
            "Testing And Verification",
            "testing.md",
            WikiPageKind::Testing,
            filter_refs(&refs, &[WikiSourceKind::Spec, WikiSourceKind::Trace]),
            "Lifecycle, test selector, trace, and verification summary.",
        ),
        page(
            "WIKI-CONVENTIONS",
            "Conventions",
            "conventions.md",
            WikiPageKind::Conventions,
            filter_refs(&refs, &[WikiSourceKind::Documentation, WikiSourceKind::Knowledge]),
            "Documentation and project convention index.",
        ),
        page(
            "WIKI-HISTORY",
            "History",
            "history.md",
            WikiPageKind::History,
            filter_refs(&refs, &[WikiSourceKind::Archive, WikiSourceKind::Trace]),
            "Archive and trace history index.",
        ),
    ];

    for source in refs.iter().filter(|source| source.kind == WikiSourceKind::Code) {
        let stem = source
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");
        pages.push(page(
            &format!("WIKI-MODULE-{}", normalize_id(stem)),
            &format!("Module {}", stem),
            &format!("modules/{stem}.md"),
            WikiPageKind::Module,
            vec![source.clone()],
            "Module page generated from source file evidence.",
        ));
    }

    pages.sort_by(|a, b| a.path.cmp(&b.path));

    WikiPlan {
        version: 1,
        metadata: WikiRunMetadata {
            generator: "agent-spec wiki".into(),
            generator_version: env!("CARGO_PKG_VERSION").into(),
            commit: None,
            branch: None,
            dirty: None,
            timestamp: opts.timestamp,
        },
        pages,
        diagnostics: source_set.diagnostics,
    }
}

fn page(
    id: &str,
    title: &str,
    path: &str,
    kind: WikiPageKind,
    sources: Vec<WikiSourceRef>,
    body: &str,
) -> WikiPage {
    WikiPage {
        id: id.into(),
        title: title.into(),
        path: PathBuf::from(path),
        kind,
        sources,
        sections: vec![WikiSection {
            heading: "Summary".into(),
            body: body.into(),
        }],
    }
}

fn filter_refs(refs: &[WikiSourceRef], kinds: &[WikiSourceKind]) -> Vec<WikiSourceRef> {
    let mut out = refs
        .iter()
        .filter(|source| kinds.iter().any(|kind| kind == &source.kind))
        .cloned()
        .collect::<Vec<_>>();
    if out.is_empty() {
        out = refs.iter().take(1).cloned().collect();
    }
    out
}

fn normalize_id(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_uppercase() } else { '-' })
        .collect()
}
```

- [ ] **Step 4: Re-export plan APIs**

Update `src/spec_wiki/mod.rs`:

```rust
pub mod model;
pub mod plan;
pub mod sources;

pub use model::{
    WikiDiagnostic, WikiPage, WikiPageKind, WikiPlan, WikiPlanOptions, WikiRunMetadata,
    WikiSection, WikiSource, WikiSourceKind, WikiSourceOptions, WikiSourceRef, WikiSourceSet,
};
pub use plan::build_wiki_plan;
pub use sources::discover_wiki_sources;
```

- [ ] **Step 5: Add `wiki plan` CLI shape test**

In `src/main.rs` tests, add:

```rust
#[test]
fn test_wiki_plan_cli_parses_nested_subcommand() {
    let cli = super::Cli::parse_from([
        "agent-spec",
        "wiki",
        "plan",
        "--code",
        ".",
        "--format",
        "json",
        "--gate",
    ]);

    match cli.command {
        super::Commands::Wiki {
            action:
                super::WikiCommands::Plan {
                    code,
                    format,
                    gate,
                },
        } => {
            assert_eq!(code, PathBuf::from("."));
            assert_eq!(format, "json");
            assert!(gate);
        }
        _ => panic!("expected wiki plan command"),
    }
}
```

- [ ] **Step 6: Add `WikiCommands` and `cmd_wiki_plan`**

In `src/main.rs`, add a `Wiki` command variant:

```rust
Wiki {
    #[command(subcommand)]
    action: WikiCommands,
},
```

Add:

```rust
#[derive(Subcommand)]
enum WikiCommands {
    Plan {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long)]
        gate: bool,
    },
}
```

Add dispatch:

```rust
Commands::Wiki { action } => cmd_wiki(action),
```

Add handlers:

```rust
fn cmd_wiki(action: WikiCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        WikiCommands::Plan { code, format, gate } => cmd_wiki_plan(&code, &format, gate),
    }
}

fn cmd_wiki_plan(code: &Path, format: &str, gate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let plan = crate::spec_wiki::build_wiki_plan(code, &crate::spec_wiki::WikiPlanOptions::default());
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&plan)?),
        _ => {
            println!(
                "wiki plan: {} pages, {} diagnostics",
                plan.pages.len(),
                plan.diagnostics.len()
            );
            for page in &plan.pages {
                println!("- {} -> {}", page.id, page.path.display());
            }
        }
    }
    if gate && plan.diagnostics.iter().any(|diag| diag.severity == "error") {
        return Err("wiki plan gate failed".into());
    }
    Ok(())
}
```

- [ ] **Step 7: Run plan tests**

```bash
cargo test test_wiki_plan_contains_required_pages_sources_and_metadata --quiet
cargo test test_wiki_plan_cli_parses_nested_subcommand --quiet
```

Expected:

```text
Both tests pass.
```

- [ ] **Step 8: Commit**

```bash
git add src/spec_wiki src/main.rs
git commit -m "feat: add wiki plan command"
```

Expected:

```text
Commit succeeds.
```

---

### Task 4: Render Local Markdown Wiki

**Files:**
- Create: `src/spec_wiki/render.rs`
- Modify: `src/spec_wiki/model.rs`
- Modify: `src/spec_wiki/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `WikiPlan`.
- Produces: `render_wiki_page(page: &WikiPage, metadata: &WikiRunMetadata) -> String`, `write_wiki_output(plan: &WikiPlan, out: &Path) -> Result<WikiWriteReport, WikiError>`.

- [ ] **Step 1: Add failing render tests**

Create tests in `src/spec_wiki/render.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_wiki::{
        WikiPage, WikiPageKind, WikiRunMetadata, WikiSection, WikiSourceKind, WikiSourceRef,
    };
    use std::path::PathBuf;

    fn sample_page() -> WikiPage {
        WikiPage {
            id: "WIKI-HOME".into(),
            title: "Home".into(),
            path: PathBuf::from("Home.md"),
            kind: WikiPageKind::Home,
            sources: vec![WikiSourceRef {
                kind: WikiSourceKind::Code,
                path: PathBuf::from("src/lib.rs"),
                fingerprint: "abc123".into(),
            }],
            sections: vec![WikiSection {
                heading: "Summary".into(),
                body: "Repository overview.".into(),
            }],
        }
    }

    #[test]
    fn test_render_wiki_page_includes_frontmatter_and_source_trace() {
        let metadata = WikiRunMetadata {
            generator: "agent-spec wiki".into(),
            generator_version: "0.4.0".into(),
            commit: Some("abc123".into()),
            branch: Some("main".into()),
            dirty: Some(false),
            timestamp: 42,
        };
        let markdown = render_wiki_page(&sample_page(), &metadata);
        assert!(markdown.contains("kind: wiki-page"));
        assert!(markdown.contains("id: WIKI-HOME"));
        assert!(markdown.contains("generated_commit: abc123"));
        assert!(markdown.contains("## Source Trace"));
        assert!(markdown.contains("src/lib.rs"));
        assert!(markdown.contains("abc123"));
    }
}
```

Run:

```bash
cargo test test_render_wiki_page_includes_frontmatter_and_source_trace --quiet
```

Expected:

```text
Compilation fails because render_wiki_page does not exist.
```

- [ ] **Step 2: Extend model with write report**

Add to `src/spec_wiki/model.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiWriteReport {
    pub pages_written: usize,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiTree {
    pub version: u32,
    pub pages: Vec<WikiTreePage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiTreePage {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSearchIndex {
    pub version: u32,
    pub pages: Vec<WikiSearchPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSearchPage {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub text: String,
}
```

- [ ] **Step 3: Implement renderer**

Create `src/spec_wiki/render.rs`:

```rust
use crate::spec_wiki::{
    WikiPage, WikiPlan, WikiRunMetadata, WikiSearchIndex, WikiSearchPage, WikiTree, WikiTreePage,
    WikiWriteReport,
};
use std::path::{Path, PathBuf};

pub fn render_wiki_page(page: &WikiPage, metadata: &WikiRunMetadata) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("kind: wiki-page\n");
    out.push_str(&format!("id: {}\n", page.id));
    out.push_str(&format!("title: \"{}\"\n", escape_title(&page.title)));
    out.push_str(&format!("generated_by: \"{}\"\n", metadata.generator));
    out.push_str(&format!("generator_version: \"{}\"\n", metadata.generator_version));
    if let Some(commit) = &metadata.commit {
        out.push_str(&format!("generated_commit: {commit}\n"));
    }
    if let Some(branch) = &metadata.branch {
        out.push_str(&format!("generated_branch: {branch}\n"));
    }
    if let Some(dirty) = metadata.dirty {
        out.push_str(&format!("generated_dirty: {dirty}\n"));
    }
    out.push_str(&format!("generated_timestamp: {}\n", metadata.timestamp));
    out.push_str("sources:\n");
    for source in &page.sources {
        out.push_str(&format!(
            "  - path: \"{}\"\n    kind: {:?}\n    fingerprint: \"{}\"\n",
            source.path.display(),
            source.kind,
            source.fingerprint
        ));
    }
    out.push_str("---\n\n");
    out.push_str(&format!("# {}\n\n", page.title));
    for section in &page.sections {
        out.push_str(&format!("## {}\n\n{}\n\n", section.heading, section.body));
    }
    out.push_str("## Source Trace\n\n");
    for source in &page.sources {
        out.push_str(&format!(
            "- `{:?}` `{}` `{}`\n",
            source.kind,
            source.path.display(),
            source.fingerprint
        ));
    }
    out
}

pub fn write_wiki_output(
    plan: &WikiPlan,
    out_dir: &Path,
) -> Result<WikiWriteReport, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for page in &plan.pages {
        let path = out_dir.join(&page.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, render_wiki_page(page, &plan.metadata))?;
        files.push(path);
    }
    let tree_path = out_dir.join("wiki-tree.json");
    std::fs::write(&tree_path, serde_json::to_string_pretty(&render_tree(plan))?)?;
    files.push(tree_path);

    let index_path = out_dir.join("search-index.json");
    std::fs::write(&index_path, serde_json::to_string_pretty(&render_search_index(plan))?)?;
    files.push(index_path);

    let metadata_path = out_dir.join("run-metadata.json");
    std::fs::write(&metadata_path, serde_json::to_string_pretty(&plan.metadata)?)?;
    files.push(metadata_path);

    files.sort();
    Ok(WikiWriteReport {
        pages_written: plan.pages.len(),
        files,
    })
}

pub fn render_tree(plan: &WikiPlan) -> WikiTree {
    WikiTree {
        version: 1,
        pages: plan
            .pages
            .iter()
            .map(|page| WikiTreePage {
                id: page.id.clone(),
                title: page.title.clone(),
                path: page.path.clone(),
            })
            .collect(),
    }
}

pub fn render_search_index(plan: &WikiPlan) -> WikiSearchIndex {
    WikiSearchIndex {
        version: 1,
        pages: plan
            .pages
            .iter()
            .map(|page| WikiSearchPage {
                id: page.id.clone(),
                title: page.title.clone(),
                path: page.path.clone(),
                text: page
                    .sections
                    .iter()
                    .map(|section| format!("{} {}", section.heading, section.body))
                    .collect::<Vec<_>>()
                    .join("\n"),
            })
            .collect(),
    }
}

fn escape_title(title: &str) -> String {
    title.replace('\\', "\\\\").replace('"', "\\\"")
}
```

- [ ] **Step 4: Re-export render APIs**

Update `src/spec_wiki/mod.rs`:

```rust
pub mod render;
pub use render::{render_search_index, render_tree, render_wiki_page, write_wiki_output};
```

Also add the new model exports:

```rust
WikiSearchIndex, WikiSearchPage, WikiTree, WikiTreePage, WikiWriteReport,
```

- [ ] **Step 5: Add `wiki generate` CLI**

Extend `WikiCommands`:

```rust
Generate {
    #[arg(long, default_value = ".")]
    code: PathBuf,
    #[arg(long, default_value = "docs/wiki")]
    out: PathBuf,
    #[arg(long)]
    check: bool,
    #[arg(long, default_value = "text")]
    format: String,
},
```

Dispatch:

```rust
WikiCommands::Generate { code, out, check, format } => {
    cmd_wiki_generate(&code, &out, check, &format)
}
```

Add handler:

```rust
fn cmd_wiki_generate(
    code: &Path,
    out: &Path,
    check: bool,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = crate::spec_wiki::build_wiki_plan(code, &crate::spec_wiki::WikiPlanOptions::default());
    if check {
        let temp = std::env::temp_dir().join(format!("agent-spec-wiki-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let report = crate::spec_wiki::write_wiki_output(&plan, &temp)?;
        if out.exists() {
            compare_generated_trees(&temp, out)?;
        }
        let _ = std::fs::remove_dir_all(&temp);
        if format == "json" {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("wiki generate check: {} pages", report.pages_written);
        }
        return Ok(());
    }
    let report = crate::spec_wiki::write_wiki_output(&plan, out)?;
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("wiki generated: {} pages", report.pages_written);
    }
    Ok(())
}

fn compare_generated_trees(expected: &Path, actual: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let expected_files = collect_files(expected);
    for expected_file in expected_files {
        let rel = expected_file.strip_prefix(expected).unwrap_or(&expected_file);
        let actual_file = actual.join(rel);
        let expected_content = std::fs::read_to_string(&expected_file)?;
        let actual_content = std::fs::read_to_string(&actual_file).unwrap_or_default();
        if expected_content != actual_content {
            return Err(format!("wiki generated output drifted: {}", actual_file.display()).into());
        }
    }
    Ok(())
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_files(&path));
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
```

- [ ] **Step 6: Run render and generate tests**

```bash
cargo test test_render_wiki_page_includes_frontmatter_and_source_trace --quiet
cargo run --quiet -- wiki generate --code . --out docs/wiki --format json
cargo run --quiet -- wiki generate --code . --out docs/wiki --check
```

Expected:

```text
The render test passes.
wiki generate writes docs/wiki pages.
wiki generate --check exits 0 immediately after generation.
```

- [ ] **Step 7: Commit**

```bash
git add src/spec_wiki src/main.rs docs/wiki
git commit -m "feat: render local wiki markdown"
```

Expected:

```text
Commit succeeds.
```

---

### Task 5: Add Wiki Check For Drift, Staleness, And Links

**Files:**
- Create: `src/spec_wiki/check.rs`
- Modify: `src/spec_wiki/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: generated `docs/wiki` output and current repository root.
- Produces: `check_wiki_output(root: &Path, wiki_dir: &Path) -> WikiCheckReport`.

- [ ] **Step 1: Add failing check tests**

Create tests in `src/spec_wiki/check.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_check_wiki_output_reports_stale_source_fingerprint() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-wiki-check-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("docs/wiki")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn value() -> i32 { 1 }\n").unwrap();
        fs::write(
            dir.join("docs/wiki/Home.md"),
            "---\nkind: wiki-page\nid: WIKI-HOME\ntitle: \"Home\"\nsources:\n  - path: \"src/lib.rs\"\n    kind: Code\n    fingerprint: \"stale\"\n---\n\n# Home\n\n[Architecture](architecture.md)\n",
        )
        .unwrap();
        fs::write(dir.join("docs/wiki/architecture.md"), "---\nkind: wiki-page\nid: WIKI-ARCHITECTURE\ntitle: \"Architecture\"\nsources:\n  - path: \"src/lib.rs\"\n    kind: Code\n    fingerprint: \"stale\"\n---\n\n# Architecture\n").unwrap();

        let report = check_wiki_output(&dir, &dir.join("docs/wiki"));

        assert!(report
            .diagnostics
            .iter()
            .any(|diag| diag.code == "wiki-stale-source"));

        let _ = fs::remove_dir_all(dir);
    }
}
```

Run:

```bash
cargo test test_check_wiki_output_reports_stale_source_fingerprint --quiet
```

Expected:

```text
Compilation fails because check_wiki_output does not exist.
```

- [ ] **Step 2: Extend model with check report**

Add to `src/spec_wiki/model.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiCheckReport {
    pub passed: bool,
    pub diagnostics: Vec<WikiDiagnostic>,
}
```

- [ ] **Step 3: Implement check module**

Create `src/spec_wiki/check.rs`:

```rust
use crate::spec_wiki::{WikiCheckReport, WikiDiagnostic};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub fn check_wiki_output(root: &Path, wiki_dir: &Path) -> WikiCheckReport {
    let mut diagnostics = Vec::new();
    if !wiki_dir.exists() {
        diagnostics.push(WikiDiagnostic {
            code: "wiki-output-missing".into(),
            severity: "error".into(),
            path: Some(wiki_dir.to_path_buf()),
            message: "wiki output directory does not exist".into(),
        });
    }

    for file in markdown_files(wiki_dir) {
        check_frontmatter(root, wiki_dir, &file, &mut diagnostics);
        check_internal_links(wiki_dir, &file, &mut diagnostics);
    }

    for required in ["Home.md", "architecture.md", "requirements.md", "testing.md"] {
        let path = wiki_dir.join(required);
        if !path.exists() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-required-page-missing".into(),
                severity: "error".into(),
                path: Some(path),
                message: format!("required generated wiki page `{required}` is missing"),
            });
        }
    }

    WikiCheckReport {
        passed: !diagnostics.iter().any(|diag| diag.severity == "error"),
        diagnostics,
    }
}

fn check_frontmatter(
    root: &Path,
    _wiki_dir: &Path,
    file: &Path,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let content = fs::read_to_string(file).unwrap_or_default();
    if !content.starts_with("---\n") || !content.contains("kind: wiki-page") {
        diagnostics.push(WikiDiagnostic {
            code: "wiki-frontmatter-missing".into(),
            severity: "error".into(),
            path: Some(file.to_path_buf()),
            message: "generated wiki page is missing wiki-page frontmatter".into(),
        });
        return;
    }
    let mut current_source: Option<PathBuf> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(raw) = trimmed.strip_prefix("- path: ") {
            current_source = Some(PathBuf::from(raw.trim_matches('"')));
        }
        if let Some(raw) = trimmed.strip_prefix("fingerprint: ") {
            if let Some(source) = current_source.take() {
                let recorded = raw.trim_matches('"');
                let actual_path = root.join(&source);
                if !actual_path.exists() {
                    diagnostics.push(WikiDiagnostic {
                        code: "wiki-source-missing".into(),
                        severity: "error".into(),
                        path: Some(file.to_path_buf()),
                        message: format!("source path `{}` does not exist", source.display()),
                    });
                } else {
                    let actual = fingerprint_file(&actual_path);
                    if actual != recorded {
                        diagnostics.push(WikiDiagnostic {
                            code: "wiki-stale-source".into(),
                            severity: "error".into(),
                            path: Some(file.to_path_buf()),
                            message: format!("source `{}` fingerprint changed", source.display()),
                        });
                    }
                }
            }
        }
    }
}

fn check_internal_links(wiki_dir: &Path, file: &Path, diagnostics: &mut Vec<WikiDiagnostic>) {
    let content = fs::read_to_string(file).unwrap_or_default();
    for link in extract_markdown_links(&content) {
        if link.starts_with("http://") || link.starts_with("https://") || link.starts_with('#') {
            continue;
        }
        let target = file.parent().unwrap_or(wiki_dir).join(link);
        if !target.exists() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-internal-link-broken".into(),
                severity: "error".into(),
                path: Some(file.to_path_buf()),
                message: "generated wiki page contains a broken internal link".into(),
            });
        }
    }
}

fn extract_markdown_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    for part in content.split("](").skip(1) {
        if let Some(end) = part.find(')') {
            links.push(part[..end].to_string());
        }
    }
    links
}

fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_markdown_files(dir, &mut out);
    out.sort();
    out
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

fn fingerprint_file(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
```

- [ ] **Step 4: Re-export check APIs**

Update `src/spec_wiki/mod.rs`:

```rust
pub mod check;
pub use check::check_wiki_output;
```

Also export `WikiCheckReport`.

- [ ] **Step 5: Add `wiki check` CLI**

Extend `WikiCommands`:

```rust
Check {
    #[arg(long, default_value = ".")]
    code: PathBuf,
    #[arg(long, default_value = "docs/wiki")]
    out: PathBuf,
    #[arg(long, default_value = "text")]
    format: String,
},
```

Add handler:

```rust
fn cmd_wiki_check(
    code: &Path,
    out: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = crate::spec_wiki::check_wiki_output(code, out);
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "wiki check: {}, {} diagnostics",
            if report.passed { "passed" } else { "failed" },
            report.diagnostics.len()
        );
        for diag in &report.diagnostics {
            println!("[{}] {} - {}", diag.severity, diag.code, diag.message);
        }
    }
    if !report.passed {
        return Err("wiki check failed".into());
    }
    Ok(())
}
```

- [ ] **Step 6: Run check tests and command**

```bash
cargo test test_check_wiki_output_reports_stale_source_fingerprint --quiet
cargo run --quiet -- wiki check --code . --out docs/wiki
```

Expected:

```text
The stale-source test passes.
The wiki check command exits 0 after a fresh wiki generate run.
```

- [ ] **Step 7: Commit**

```bash
git add src/spec_wiki src/main.rs
git commit -m "feat: add wiki check gate"
```

Expected:

```text
Commit succeeds.
```

---

### Task 6: Add GitHub Wiki Export

**Files:**
- Create: `src/spec_wiki/github.rs`
- Modify: `src/spec_wiki/model.rs`
- Modify: `src/spec_wiki/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: generated local wiki directory.
- Produces: flattened GitHub Wiki export tree with rewritten links, `Home.md`, `_Sidebar.md`.

- [ ] **Step 1: Add failing export test**

Create tests in `src/spec_wiki/github.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_github_wiki_export_flattens_paths_and_rewrites_links() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-github-wiki-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("docs/wiki/modules")).unwrap();
        fs::write(
            dir.join("docs/wiki/Home.md"),
            "# Home\n\n[Module](modules/lib.md)\n",
        )
        .unwrap();
        fs::write(dir.join("docs/wiki/modules/lib.md"), "# Lib\n").unwrap();

        let report = export_github_wiki(&dir.join("docs/wiki"), &dir.join("wiki-export")).unwrap();

        assert!(dir.join("wiki-export/Home.md").exists());
        assert!(dir.join("wiki-export/modules--lib.md").exists());
        assert!(dir.join("wiki-export/_Sidebar.md").exists());
        let home = fs::read_to_string(dir.join("wiki-export/Home.md")).unwrap();
        assert!(home.contains("(modules--lib.md)"));
        assert_eq!(report.files_written.len(), 3);

        let _ = fs::remove_dir_all(dir);
    }
}
```

Run:

```bash
cargo test test_github_wiki_export_flattens_paths_and_rewrites_links --quiet
```

Expected:

```text
Compilation fails because export_github_wiki does not exist.
```

- [ ] **Step 2: Extend model with export report**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiExportReport {
    pub files_written: Vec<PathBuf>,
}
```

- [ ] **Step 3: Implement GitHub export**

Create `src/spec_wiki/github.rs`:

```rust
use crate::spec_wiki::WikiExportReport;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn export_github_wiki(
    wiki_dir: &Path,
    out_dir: &Path,
) -> Result<WikiExportReport, Box<dyn std::error::Error>> {
    let _ = fs::remove_dir_all(out_dir);
    fs::create_dir_all(out_dir)?;

    let files = markdown_files(wiki_dir);
    let mut mapping = BTreeMap::new();
    for file in &files {
        let rel = file.strip_prefix(wiki_dir).unwrap_or(file);
        mapping.insert(rel.to_path_buf(), flatten_path(rel));
    }

    let mut written = Vec::new();
    for file in &files {
        let rel = file.strip_prefix(wiki_dir).unwrap_or(file);
        let Some(flat) = mapping.get(rel) else {
            continue;
        };
        let content = fs::read_to_string(file)?;
        let rewritten = rewrite_links(&content, &mapping);
        let target = out_dir.join(flat);
        fs::write(&target, rewritten)?;
        written.push(target);
    }

    let sidebar = render_sidebar(&mapping);
    let sidebar_path = out_dir.join("_Sidebar.md");
    fs::write(&sidebar_path, sidebar)?;
    written.push(sidebar_path);

    if !out_dir.join("Home.md").exists() {
        let home_path = out_dir.join("Home.md");
        fs::write(&home_path, "# Home\n\nSee [_Sidebar](_Sidebar.md).\n")?;
        written.push(home_path);
    }

    written.sort();
    Ok(WikiExportReport {
        files_written: written,
    })
}

fn flatten_path(path: &Path) -> PathBuf {
    if path == Path::new("Home.md") {
        return PathBuf::from("Home.md");
    }
    let mut parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return PathBuf::from("Home.md");
    }
    let last = parts.pop().unwrap_or_else(|| "Home.md".into());
    if parts.is_empty() {
        return PathBuf::from(last);
    }
    PathBuf::from(format!("{}--{}", parts.join("--"), last))
}

fn rewrite_links(content: &str, mapping: &BTreeMap<PathBuf, PathBuf>) -> String {
    let mut out = content.to_string();
    for (source, target) in mapping {
        let source_text = source.to_string_lossy();
        let target_text = target.to_string_lossy();
        out = out.replace(&format!("]({source_text})"), &format!("]({target_text})"));
    }
    out
}

fn render_sidebar(mapping: &BTreeMap<PathBuf, PathBuf>) -> String {
    let mut out = String::from("# Sidebar\n\n");
    for (source, target) in mapping {
        let title = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Page")
            .replace('-', " ");
        out.push_str(&format!("- [{}]({})\n", title, target.display()));
    }
    out
}

fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
}
```

- [ ] **Step 4: Re-export GitHub APIs**

Update `src/spec_wiki/mod.rs`:

```rust
pub mod github;
pub use github::export_github_wiki;
```

Also export `WikiExportReport`.

- [ ] **Step 5: Add `wiki export-github` CLI**

Extend `WikiCommands`:

```rust
ExportGithub {
    #[arg(long, default_value = "docs/wiki")]
    wiki: PathBuf,
    #[arg(long, default_value = ".agent-spec/wiki-github")]
    out: PathBuf,
    #[arg(long)]
    check: bool,
    #[arg(long, default_value = "text")]
    format: String,
},
```

Add handler:

```rust
fn cmd_wiki_export_github(
    wiki: &Path,
    out: &Path,
    check: bool,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp;
    let target = if check {
        temp = std::env::temp_dir().join(format!("agent-spec-wiki-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        &temp
    } else {
        out
    };
    let report = crate::spec_wiki::export_github_wiki(wiki, target)?;
    if check && out.exists() {
        compare_generated_trees(target, out)?;
        let _ = std::fs::remove_dir_all(target);
    }
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("github wiki export: {} files", report.files_written.len());
    }
    Ok(())
}
```

- [ ] **Step 6: Run export tests and command**

```bash
cargo test test_github_wiki_export_flattens_paths_and_rewrites_links --quiet
cargo run --quiet -- wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github --format json
cargo run --quiet -- wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github --check
```

Expected:

```text
The export test passes.
GitHub export writes flattened files and check exits 0 immediately after export.
```

- [ ] **Step 7: Commit**

```bash
git add src/spec_wiki src/main.rs .agent-spec/wiki-github
git commit -m "feat: export generated wiki for github wiki"
```

Expected:

```text
Commit succeeds.
```

---

### Task 7: Add Visual Asset Manifest Support

**Files:**
- Create: `src/spec_wiki/assets.rs`
- Modify: `src/spec_wiki/model.rs`
- Modify: `src/spec_wiki/plan.rs`
- Modify: `src/spec_wiki/render.rs`
- Modify: `src/spec_wiki/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `.agent-spec/wiki-assets.json`.
- Produces: validated asset entries attached to pages and rendered as Markdown image links.

- [ ] **Step 1: Add failing asset manifest test**

Create tests in `src/spec_wiki/assets.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_visual_asset_manifest_accepts_existing_images_only() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-wiki-assets-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("docs/assets")).unwrap();
        fs::write(dir.join("docs/assets/home.png"), "png").unwrap();
        fs::write(
            dir.join("assets.json"),
            "[{\"page_id\":\"WIKI-HOME\",\"path\":\"docs/assets/home.png\",\"caption\":\"Home screen\"},{\"page_id\":\"WIKI-HOME\",\"path\":\"docs/assets/missing.png\",\"caption\":\"Missing\"}]",
        )
        .unwrap();

        let report = read_visual_asset_manifest(&dir, &dir.join("assets.json")).unwrap();

        assert_eq!(report.assets.len(), 1);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "wiki-asset-missing");

        let _ = fs::remove_dir_all(dir);
    }
}
```

Run:

```bash
cargo test test_visual_asset_manifest_accepts_existing_images_only --quiet
```

Expected:

```text
Compilation fails because read_visual_asset_manifest does not exist.
```

- [ ] **Step 2: Extend model**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiVisualAsset {
    pub page_id: String,
    pub path: PathBuf,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiVisualAssetReport {
    pub assets: Vec<WikiVisualAsset>,
    pub diagnostics: Vec<WikiDiagnostic>,
}
```

- [ ] **Step 3: Implement asset manifest reader**

Create `src/spec_wiki/assets.rs`:

```rust
use crate::spec_wiki::{WikiDiagnostic, WikiVisualAsset, WikiVisualAssetReport};
use std::path::Path;

pub fn read_visual_asset_manifest(
    root: &Path,
    manifest: &Path,
) -> Result<WikiVisualAssetReport, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(manifest)?;
    let mut assets: Vec<WikiVisualAsset> = serde_json::from_str(&content)?;
    let mut diagnostics = Vec::new();
    assets.retain(|asset| {
        let exists = root.join(&asset.path).exists();
        if !exists {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-asset-missing".into(),
                severity: "warning".into(),
                path: Some(asset.path.clone()),
                message: format!("visual asset `{}` does not exist", asset.path.display()),
            });
        }
        exists
    });
    assets.sort_by(|a, b| {
        a.page_id
            .cmp(&b.page_id)
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(WikiVisualAssetReport {
        assets,
        diagnostics,
    })
}
```

- [ ] **Step 4: Attach assets to pages**

Add to `WikiPage` in `model.rs`:

```rust
pub assets: Vec<WikiVisualAsset>,
```

Update every `WikiPage` construction to pass `assets: Vec::new()` first. Then add a helper in `plan.rs`:

```rust
pub fn attach_visual_assets(plan: &mut WikiPlan, assets: Vec<crate::spec_wiki::WikiVisualAsset>) {
    for asset in assets {
        if let Some(page) = plan.pages.iter_mut().find(|page| page.id == asset.page_id) {
            page.assets.push(asset);
        } else {
            plan.diagnostics.push(crate::spec_wiki::WikiDiagnostic {
                code: "wiki-asset-page-missing".into(),
                severity: "warning".into(),
                path: Some(asset.path),
                message: "visual asset references a page id that is not in the wiki plan".into(),
            });
        }
    }
}
```

- [ ] **Step 5: Render images in pages**

In `render_wiki_page`, before `## Source Trace`, add:

```rust
if !page.assets.is_empty() {
    out.push_str("## Visual Evidence\n\n");
    for asset in &page.assets {
        out.push_str(&format!(
            "![{}]({})\n\n",
            asset.caption,
            asset.path.display()
        ));
    }
}
```

Also include assets in source trace:

```rust
for asset in &page.assets {
    out.push_str(&format!(
        "- `Asset` `{}` `{}`\n",
        asset.path.display(),
        asset.caption
    ));
}
```

- [ ] **Step 6: Add `--assets` to wiki generate**

Extend `WikiCommands::Generate`:

```rust
#[arg(long)]
assets: Option<PathBuf>,
```

In `cmd_wiki_generate`, after plan creation:

```rust
let mut plan = crate::spec_wiki::build_wiki_plan(code, &crate::spec_wiki::WikiPlanOptions::default());
if let Some(asset_manifest) = assets {
    let asset_report = crate::spec_wiki::read_visual_asset_manifest(code, &asset_manifest)?;
    plan.diagnostics.extend(asset_report.diagnostics);
    crate::spec_wiki::attach_visual_assets(&mut plan, asset_report.assets);
}
```

- [ ] **Step 7: Run asset tests**

```bash
cargo test test_visual_asset_manifest_accepts_existing_images_only --quiet
cargo test test_wiki_visual_asset_manifest_embeds_existing_images_only --quiet
```

Expected:

```text
Both tests pass.
```

- [ ] **Step 8: Commit**

```bash
git add src/spec_wiki src/main.rs specs/task-autowiki-living-docs.spec.md
git commit -m "feat: support wiki visual asset manifests"
```

Expected:

```text
Commit succeeds.
```

---

### Task 8: Add CI Workflow Generation

**Files:**
- Create: `src/spec_wiki/ci.rs`
- Modify: `src/spec_wiki/model.rs`
- Modify: `src/spec_wiki/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: provider `github|gitlab`, branch name, output path.
- Produces: deterministic workflow content that runs wiki generation/checks and docs lint.

- [ ] **Step 1: Add failing CI test**

Create tests in `src/spec_wiki/ci.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_github_wiki_check_workflow_has_no_secrets_or_push() {
        let workflow = render_wiki_ci_workflow(WikiCiProvider::Github, "main");
        assert!(workflow.contains("agent-spec wiki generate"));
        assert!(workflow.contains("agent-spec wiki check"));
        assert!(workflow.contains("bash scripts/docs-lint.sh"));
        assert!(!workflow.contains("secrets."));
        assert!(!workflow.contains("git push"));
    }
}
```

Run:

```bash
cargo test test_render_github_wiki_check_workflow_has_no_secrets_or_push --quiet
```

Expected:

```text
Compilation fails because render_wiki_ci_workflow does not exist.
```

- [ ] **Step 2: Add CI provider model**

Add:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WikiCiProvider {
    Github,
    Gitlab,
}
```

- [ ] **Step 3: Implement CI renderer**

Create `src/spec_wiki/ci.rs`:

```rust
use crate::spec_wiki::WikiCiProvider;

pub fn render_wiki_ci_workflow(provider: WikiCiProvider, branch: &str) -> String {
    match provider {
        WikiCiProvider::Github => format!(
            "name: agent-spec Wiki Check\n\non:\n  push:\n    branches: [{branch}]\n  pull_request:\n\njobs:\n  wiki-check:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: dtolnay/rust-toolchain@stable\n      - name: Build agent-spec\n        run: cargo build --quiet\n      - name: Generate wiki\n        run: cargo run --quiet -- wiki generate --code . --out docs/wiki --check\n      - name: Check wiki\n        run: cargo run --quiet -- wiki check --code . --out docs/wiki\n      - name: Documentation lint\n        run: bash scripts/docs-lint.sh\n"
        ),
        WikiCiProvider::Gitlab => format!(
            "agent-spec-wiki-check:\n  image: rust:latest\n  rules:\n    - if: '$CI_COMMIT_BRANCH == \"{branch}\"'\n    - if: '$CI_PIPELINE_SOURCE == \"merge_request_event\"'\n  script:\n    - cargo build --quiet\n    - cargo run --quiet -- wiki generate --code . --out docs/wiki --check\n    - cargo run --quiet -- wiki check --code . --out docs/wiki\n    - bash scripts/docs-lint.sh\n"
        ),
    }
}
```

- [ ] **Step 4: Re-export CI APIs**

Update `src/spec_wiki/mod.rs`:

```rust
pub mod ci;
pub use ci::render_wiki_ci_workflow;
```

Export `WikiCiProvider`.

- [ ] **Step 5: Add `wiki install-ci` CLI**

Extend `WikiCommands`:

```rust
InstallCi {
    #[arg(long, default_value = "github")]
    provider: String,
    #[arg(long, default_value = "main")]
    branch: String,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    check: bool,
},
```

Add handler:

```rust
fn cmd_wiki_install_ci(
    provider: &str,
    branch: &str,
    out: Option<&Path>,
    check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let provider = match provider {
        "github" => crate::spec_wiki::WikiCiProvider::Github,
        "gitlab" => crate::spec_wiki::WikiCiProvider::Gitlab,
        other => return Err(format!("unsupported wiki CI provider `{other}`").into()),
    };
    let content = crate::spec_wiki::render_wiki_ci_workflow(provider, branch);
    let path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| match provider {
            crate::spec_wiki::WikiCiProvider::Github => {
                PathBuf::from(".github/workflows/agent-spec-wiki-check.yml")
            }
            crate::spec_wiki::WikiCiProvider::Gitlab => PathBuf::from(".gitlab-ci-agent-spec-wiki.yml"),
        });
    if check {
        let actual = std::fs::read_to_string(&path).unwrap_or_default();
        if actual != content {
            return Err(format!("wiki CI workflow drifted: {}", path.display()).into());
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}
```

- [ ] **Step 6: Run CI tests**

```bash
cargo test test_render_github_wiki_check_workflow_has_no_secrets_or_push --quiet
cargo run --quiet -- wiki install-ci --provider github --branch main --out .github/workflows/agent-spec-wiki-check.yml
cargo run --quiet -- wiki install-ci --provider github --branch main --out .github/workflows/agent-spec-wiki-check.yml --check
```

Expected:

```text
The test passes.
The generated workflow contains only check commands and check mode exits 0.
```

- [ ] **Step 7: Commit**

```bash
git add src/spec_wiki src/main.rs .github/workflows/agent-spec-wiki-check.yml
git commit -m "feat: add wiki ci check workflow generation"
```

Expected:

```text
Commit succeeds.
```

---

### Task 9: Add Documentation, Skill, And Fixture

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CHANGELOG.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `skills/agent-spec-tool-first/references/commands.md`
- Modify: `.claude/skills/agent-spec-tool-first/SKILL.md`
- Modify: `.claude/skills/agent-spec-tool-first/references/commands.md`
- Create: `skills/agent-spec-wiki/SKILL.md`
- Create: `fixtures/wiki-mini/Cargo.toml`
- Create: `fixtures/wiki-mini/src/lib.rs`
- Create: `fixtures/wiki-mini/knowledge/requirements/req-counter.md`
- Create: `fixtures/wiki-mini/specs/task-counter.spec.md`
- Create: `fixtures/wiki-mini/docs/overview.md`

**Interfaces:**
- Consumes: implemented wiki commands.
- Produces: user-facing docs, agent-facing skill, and compact fixture.

- [ ] **Step 1: Add docs content test**

In `src/main.rs` tests, add:

```rust
#[test]
fn test_docs_describe_autowiki_living_docs_workflow() {
    let readme = include_str!("../README.md");
    let agents = include_str!("../AGENTS.md");
    let skill = include_str!("../skills/agent-spec-tool-first/SKILL.md");
    let commands = include_str!("../skills/agent-spec-tool-first/references/commands.md");
    let wiki_skill = include_str!("../skills/agent-spec-wiki/SKILL.md");

    for content in [readme, agents, skill, commands, wiki_skill] {
        assert!(content.contains("wiki plan"));
        assert!(content.contains("wiki generate"));
        assert!(content.contains("wiki check"));
        assert!(content.contains("wiki export-github"));
        assert!(content.contains("wiki install-ci"));
        assert!(content.contains("source trace"));
        assert!(content.contains("GitHub Wiki export"));
        assert!(content.contains("visual asset manifest"));
        assert!(content.contains("local-first"));
    }
}
```

Do not run this test until `skills/agent-spec-wiki/SKILL.md` exists.

- [ ] **Step 2: Add README and AGENTS wiki section**

Add to README and AGENTS:

```md
### Living Wiki

agent-spec can generate a local-first living wiki from code, KLL artifacts,
Task Contracts, docs, archive summaries, and lifecycle trace evidence.

```bash
agent-spec wiki plan --code . --format json --gate
agent-spec wiki generate --code . --out docs/wiki
agent-spec wiki check --code . --out docs/wiki
agent-spec wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github
agent-spec wiki install-ci --provider github --branch main
```

Generated wiki pages are not KLL truth. They are source-traced summaries with
frontmatter, source fingerprints, run metadata, `wiki-tree.json`, and
`search-index.json`. Use `wiki check` to reject stale pages and broken internal
links. Use GitHub Wiki export when a team wants to read the generated docs in
GitHub's wiki tab; agent-spec writes the flattened tree but does not push it.

Screenshots and UI/TUI captures enter through a reviewed visual asset manifest:

```bash
agent-spec wiki generate --assets .agent-spec/wiki-assets.json
```
```

- [ ] **Step 3: Add tool-first command reference**

Add to `skills/agent-spec-tool-first/references/commands.md`:

```md
## wiki

```bash
agent-spec wiki plan --code . --format json --gate
agent-spec wiki generate --code . --out docs/wiki [--assets .agent-spec/wiki-assets.json] [--check]
agent-spec wiki check --code . --out docs/wiki
agent-spec wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github [--check]
agent-spec wiki install-ci --provider github --branch main [--check]
```

`wiki plan` builds the deterministic page/source/source-fingerprint IR.
`wiki generate` writes local Markdown, `wiki-tree.json`, `search-index.json`,
and run metadata. `wiki check` rejects stale pages, missing sources, malformed
generated frontmatter, missing required pages, and broken internal links.
`wiki export-github` flattens hierarchy, rewrites internal links, and writes
`Home.md` plus `_Sidebar.md`; it does not push. `wiki install-ci` writes a check
workflow that runs wiki generation/check and docs lint on push or pull request.
```

- [ ] **Step 4: Add `skills/agent-spec-wiki/SKILL.md`**

Create:

```md
---
name: agent-spec-wiki
description: Use when generating, checking, enriching, or reviewing agent-spec living wiki pages, GitHub Wiki exports, or visual asset manifests.
---

# Agent-Spec Living Wiki Workflow

Use this skill for local-first wiki generation and review.

## Rules

- Treat `docs/wiki/` as generated output.
- Do not edit generated wiki pages by hand.
- Put durable truth in `knowledge/`, executable contracts in `specs/`, and
  reader-authored docs outside `docs/wiki/`.
- Use AI only to propose prose improvements against an existing `WikiPlan`.
- Preserve source trace and fingerprints in every generated page.
- Screenshots must be listed in `.agent-spec/wiki-assets.json` before they are
  embedded.

## Workflow

```bash
agent-spec wiki plan --code . --format json --gate
agent-spec wiki generate --code . --out docs/wiki
agent-spec wiki check --code . --out docs/wiki
bash scripts/docs-lint.sh
```

For GitHub Wiki export:

```bash
agent-spec wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github
```

For CI setup:

```bash
agent-spec wiki install-ci --provider github --branch main
```

## Visual Asset Manifest

Use JSON:

```json
[
  {
    "page_id": "WIKI-HOME",
    "path": "docs/assets/home.png",
    "caption": "Home screen"
  }
]
```

Then run:

```bash
agent-spec wiki generate --code . --out docs/wiki --assets .agent-spec/wiki-assets.json
```
```

- [ ] **Step 5: Create wiki fixture**

Create `fixtures/wiki-mini/Cargo.toml`:

```toml
[package]
name = "wiki-mini"
version = "0.1.0"
edition = "2024"

[dependencies]
```

Create `fixtures/wiki-mini/src/lib.rs`:

```rust
#[derive(Debug, Default)]
pub struct Counter {
    value: i32,
}

impl Counter {
    pub fn increment(&mut self) {
        self.value += 1;
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}
```

Create `fixtures/wiki-mini/knowledge/requirements/req-counter.md`:

```md
---
kind: requirement
id: REQ-COUNTER
title: "Counter"
liveness: auto
---

## Problem

The fixture needs one small requirement for wiki trace demonstration.

## Requirements

[REQ-COUNTER] The counter MUST increment by one.

## Scenarios

Scenario: Increment counter
  Given a new counter
  When it is incremented
  Then its value is 1

## Source Trace

- fixture

## Open Questions

None.
```

Create `fixtures/wiki-mini/specs/task-counter.spec.md`:

```spec
spec: task
name: "Counter"
satisfies: [REQ-COUNTER]
---

## Intent

Provide a small counter for wiki fixture generation.

## Completion Criteria

Scenario: Increment counter
  Test: counter_increment_adds_one
  Given a new counter
  When it is incremented
  Then its value is 1
```

Create `fixtures/wiki-mini/docs/overview.md`:

```md
# Wiki Mini Overview

This fixture demonstrates agent-spec living wiki generation.
```

- [ ] **Step 6: Run docs and fixture checks**

```bash
cargo test test_docs_describe_autowiki_living_docs_workflow --quiet
cargo run --quiet -- wiki plan --code fixtures/wiki-mini --format json --gate
cargo run --quiet -- wiki generate --code fixtures/wiki-mini --out fixtures/wiki-mini/docs/wiki
cargo run --quiet -- wiki check --code fixtures/wiki-mini --out fixtures/wiki-mini/docs/wiki
cargo run --quiet -- wiki export-github --wiki fixtures/wiki-mini/docs/wiki --out fixtures/wiki-mini/.agent-spec/wiki-github
```

Expected:

```text
The docs test passes.
Fixture wiki plan exits 0.
Fixture wiki generate/check/export all exit 0.
```

- [ ] **Step 7: Commit**

```bash
git add README.md AGENTS.md CHANGELOG.md skills .claude/skills fixtures/wiki-mini src/main.rs
git commit -m "docs: document living wiki workflow"
```

Expected:

```text
Commit succeeds.
```

---

### Task 10: Final Dogfood, Gates, And Acceptance

**Files:**
- Modify: `docs/wiki/**`
- Modify: `.agent-spec/wiki-github/**`
- Modify: `.agent-spec/trace/**`

**Interfaces:**
- Consumes: all new wiki commands and existing KLL/spec gates.
- Produces: verified self-hosting wiki output and requirement trace evidence.

- [ ] **Step 1: Generate and check this repository's wiki**

Run:

```bash
cargo run --quiet -- wiki plan --code . --format json --gate
cargo run --quiet -- wiki generate --code . --out docs/wiki
cargo run --quiet -- wiki check --code . --out docs/wiki
cargo run --quiet -- wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github
cargo run --quiet -- wiki export-github --wiki docs/wiki --out .agent-spec/wiki-github --check
```

Expected:

```text
All commands exit 0.
docs/wiki contains Home.md, architecture.md, requirements.md, testing.md, conventions.md, history.md, wiki-tree.json, search-index.json, and run-metadata.json.
.agent-spec/wiki-github contains Home.md and _Sidebar.md.
```

- [ ] **Step 2: Run documentation gates**

Run:

```bash
bash scripts/docs-lint.sh
```

Expected:

```text
Built-in Chinese docs lint passes.
Installed external tools run or warn as unavailable.
The command exits 0 unless an installed tool reports findings.
```

- [ ] **Step 3: Run KLL and requirements gates**

Run:

```bash
cargo run --quiet -- lint-knowledge --knowledge knowledge --gate
cargo run --quiet -- requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected:

```text
Both commands exit 0.
requirements plan includes REQ-AUTOWIKI-LIVING-DOCS covered by specs/task-autowiki-living-docs.spec.md.
```

- [ ] **Step 4: Run Rust gates**

Run:

```bash
cargo fmt --check
cargo test --quiet
cargo clippy --all-targets -- -D warnings
```

Expected:

```text
All commands exit 0.
```

- [ ] **Step 5: Run lifecycle for the wiki task contract**

Run:

```bash
cargo run --quiet -- lifecycle specs/task-autowiki-living-docs.spec.md --code . --format json --change-scope worktree --run-log-dir .agent-spec/runs
```

Expected:

```text
The lifecycle report has `"passed": true`.
Every scenario in specs/task-autowiki-living-docs.spec.md has verdict `pass`.
The lifecycle run writes requirement trace evidence for REQ-AUTOWIKI-LIVING-DOCS.
```

- [ ] **Step 6: Replay wiki requirement evidence**

Run:

```bash
cargo run --quiet -- requirements replay REQ-AUTOWIKI-LIVING-DOCS --trace-dir .agent-spec/runs/.agent-spec/trace --format text
cargo run --quiet -- requirements explain-failure REQ-AUTOWIKI-LIVING-DOCS --trace-dir .agent-spec/runs/.agent-spec/trace --format json
cargo run --quiet -- requirements trace-graph REQ-AUTOWIKI-LIVING-DOCS --trace-dir .agent-spec/runs/.agent-spec/trace --format mermaid
```

Expected:

```text
Replay shows the latest evidence chain.
Explain-failure has no non-pass records.
Trace graph contains requirement, work unit, spec, scenario, test, code target, worktree or VCS nodes.
```

- [ ] **Step 7: Commit final generated and evidence artifacts**

```bash
git add docs/wiki .agent-spec/wiki-github .agent-spec/runs/.agent-spec/trace
git commit -m "docs: dogfood living wiki generation"
```

Expected:

```text
Commit succeeds.
```

---

## Self-Review Checklist

- Spec coverage: Tasks 1-10 cover KLL requirement, task spec, IR, source discovery, page planning, Markdown generation, check gate, GitHub export, visual asset manifests, CI workflow generation, docs, fixture, and final dogfood.
- Placeholder scan: This plan contains no unresolved placeholder instructions.
- Type consistency: `WikiPlan`, `WikiPage`, `WikiSource`, `WikiDiagnostic`, `WikiRunMetadata`, `WikiCheckReport`, `WikiExportReport`, `WikiVisualAsset`, and `WikiCiProvider` are introduced before use.
- Boundary check: Core CLI remains deterministic, local-first, and model-free.
- AutoWiki adaptation check: Codebase analysis, page generation, optional visual assets, GitHub Wiki export, auto-refresh workflow, and version metadata are all covered; cloud viewer/sync/push are explicit non-goals.

## Execution Handoff

Plan complete. Implement task-by-task with either:

1. **Subagent-Driven (recommended)**: dispatch a fresh subagent per task, review between tasks, and keep commits small.
2. **Inline Execution**: execute tasks in this session with checkpoints after each task.
