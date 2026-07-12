# Requirements Intake And Work Units Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic requirements intake and work-unit pipeline that turns PRD/issue material into `knowledge/requirements/*.md`, validates those requirement artifacts, generates executable work units, and drafts Task Contracts with `satisfies: [REQ-*]` links.

**Architecture:** Keep agent-spec contract-first. The new layer imports explicitly marked PRD/issue requirement blocks into KLL requirement artifacts, derives a requirement graph and work-unit set from KLL docs, and optionally renders `.spec.md` drafts. Existing `lint-knowledge`, `lifecycle`, and `trace` remain the verification and liveness gates.

**Tech Stack:** Rust 2024, existing hand-written parsers, `serde`/`serde_json`, `clap` subcommands, current KLL modules under `src/spec_knowledge/`, current CLI in `src/main.rs`.

## Global Constraints

- No LLM or network dependency in the first implementation.
- Do not copy reference-project Python code; borrow only the product model and deterministic concepts.
- Preserve existing `knowledge/requirements/*.md` compatibility: `kind: requirement`, `id: REQ-NNN`, `## Problem`, `## Requirements`.
- Add explicit KLL requirement `title:` support in `KnowledgeMeta`; do not infer product titles from lossy filename slugs.
- Preserve the current hand-written parser style; do not add `serde_yaml`.
- Do not weaken existing KLL gates: malformed knowledge must surface as errors.
- Output ordering must be deterministic: sort by normalized requirement id and path.
- Generated spec drafts must be reviewable artifacts, not auto-accepted contracts.
- `Open Questions` with real content blocks executable work-unit generation for that requirement.
- `satisfies:` is the trace edge from generated Task Contracts back to KLL requirements.
- Generated draft specs contain pending `Test:` selectors and are expected to fail `lifecycle` until a human replaces those selectors with real tests.
- `--check` flags compare generated content against on-disk files; checking only file existence is not sufficient.

---

## Reference Project

Reference repository:

```text
<reference-project>
```

License:

```text
MIT License, <reference-project>/LICENSE
```

Borrow these concepts:

- Requirements are source artifacts, not loose prompt context.
  - Reference: `README.md`, sections "Why the reference project" and "What the reference project does".
- Structured requirement model with ids, dependencies, scenarios, and optional visual references.
  - Reference: `example/ticketbooking-demo/requirements.yaml`.
- Deterministic processing queue from requirement nodes to executable units.
  - Reference: `src/agent/agent_workflow.py`, `_build_processing_tasks`.
- Non-leaf work classification: full work only when a grouping node has scenarios; otherwise shell or skip.
  - Reference: `src/agent/workflow_phase_utils.py`, `classify_non_leaf_work`.
- Traceability model: requirement -> interface/test/status evidence.
  - Reference: `src/agent/agent-runtime/traceability.py`.

Do not borrow these parts into agent-spec:

- the reference project's app compiler loop that generates full runnable applications.
- the reference project's agent-specific interface designer, test generator, and TDD developer agents.
- the reference project's web/android template assumptions.
- the reference project's Python runtime structure.

agent-spec adaptation:

```text
PRD/issue blocks
  -> knowledge/requirements/*.md
  -> requirement graph
  -> work_units.json
  -> specs/task-*.spec.md drafts
  -> lifecycle/trace/KLL liveness
```

---

## Target File Structure

- Create: `src/spec_knowledge/intake.rs`
  - Parses deterministic `agent-spec:requirement` blocks from PRD/issue Markdown.
  - Renders those blocks as KLL requirement Markdown files.

- Modify: `src/spec_knowledge/model.rs`
  - Adds optional `title` metadata to `KnowledgeMeta`.

- Modify: `src/spec_knowledge/parser.rs`
  - Parses frontmatter `title:` into `KnowledgeMeta.title`.

- Create: `src/spec_knowledge/requirement_graph.rs`
  - Converts parsed KLL requirement docs into a normalized graph.
  - Extracts dependencies, child requirement ids, scenarios, source trace, and open questions from sections.
  - Validates ids, dangling dependencies, cycles, executable readiness, and duplicate requirement ids.

- Create: `src/spec_knowledge/work_units.rs`
  - Converts a requirement graph into executable work units.
  - Classifies units as `leaf_full`, `parent_scenario`, `grouping_only`, or `blocked_questions`.

- Create: `src/spec_knowledge/draft_specs.rs`
  - Renders work units into reviewable task `.spec.md` drafts.
  - Writes `satisfies: [REQ-*]`, BDD scenarios, source trace, and open questions.

- Modify: `src/spec_knowledge/mod.rs`
  - Re-export new modules and public functions.

- Modify: `src/spec_knowledge/scaffold.rs`
  - Update `knowledge/requirements/README.md` and `req-template.md` with the richer consumable format.

- Modify: `src/main.rs`
  - Add a nested `requirements` command with `import`, `graph`, `work-units`, and `draft-specs` subcommands.

- Modify: `README.md`
  - Document the full intake-to-contract pipeline and show command examples.

- Create: `knowledge/requirements/req-kll-work-units.md`
  - Source requirement artifact satisfied by the task spec in this plan.

- Create: `specs/task-requirements-intake-work-units.spec.md`
  - Task Contract for this feature, with scenarios bound to Rust tests.

---

## Data Contracts

### PRD/Issue Import Block

The deterministic import format is an HTML comment block. It is intentionally explicit so raw PRD prose is not silently reinterpreted.

```md
<!-- agent-spec:requirement id=REQ-101 title="User Login" tags=auth,web source=issue:#123 -->
## Problem

Users with existing accounts need to authenticate without creating duplicate sessions.

## Requirements

[REQ-101] The authentication service MUST create a login session when valid credentials are submitted.
[REQ-101A] The authentication service MUST NOT create a login session when credentials are invalid.

## Scenarios

Scenario: Valid login
  Given the visitor has a valid persisted account
  When the visitor submits the correct username or email and password
  Then the system establishes a login session and shows the authenticated state

Scenario: Invalid login
  Given the visitor enters an unknown account or wrong password
  When the visitor submits the login form
  Then the system shows authentication failure feedback and does not create a session

## Dependencies

- REQ-100

## Source Trace

- issue:#123

## Open Questions

None.
<!-- /agent-spec:requirement -->
```

### KLL Requirement Artifact

The importer renders one Markdown file per block:

```md
---
kind: requirement
id: REQ-101
title: "User Login"
liveness: auto
tags: [auth, web]
---

## Problem

Users with existing accounts need to authenticate without creating duplicate sessions.

## Requirements

[REQ-101] The authentication service MUST create a login session when valid credentials are submitted.
[REQ-101A] The authentication service MUST NOT create a login session when credentials are invalid.

## Scenarios

Scenario: Valid login
  Given the visitor has a valid persisted account
  When the visitor submits the correct username or email and password
  Then the system establishes a login session and shows the authenticated state

Scenario: Invalid login
  Given the visitor enters an unknown account or wrong password
  When the visitor submits the login form
  Then the system shows authentication failure feedback and does not create a session

## Dependencies

- REQ-100

## Source Trace

- issue:#123

## Open Questions

None.
```

### Work Unit JSON

The work-unit command writes deterministic JSON:

```json
{
  "version": 1,
  "units": [
    {
      "id": "WU-REQ-101",
      "requirement_id": "REQ-101",
      "title": "User Login",
      "source_path": "knowledge/requirements/req-101-user-login.md",
      "mode": "leaf_full",
      "status": "ready",
      "depends_on": ["REQ-100"],
      "satisfies": ["REQ-101"],
      "scenario_count": 2,
      "blocked_by": []
    }
  ],
  "diagnostics": []
}
```

---

### Task 1: Add The Agent-Spec Task Contract For This Feature

**Files:**
- Create: `knowledge/requirements/req-kll-work-units.md`
- Create: `specs/task-requirements-intake-work-units.spec.md`

**Interfaces:**
- Consumes: existing agent-spec DSL and `satisfies:` frontmatter.
- Produces: source KLL requirement and executable contract for the implementation tasks below.

- [ ] **Step 1: Write the source requirement artifact**

Create `knowledge/requirements/req-kll-work-units.md` with this content:

```md
---
kind: requirement
id: REQ-KLL-WORK-UNITS
title: "Requirements Intake And Work Units"
liveness: auto
tags: [kll, requirements, work-units]
---

## Problem

agent-spec can verify Task Contracts and trace specs to KLL decisions or requirements, but it does not yet provide a deterministic path from PRD or issue material into long-lived requirement artifacts, executable work units, and reviewable Task Contract drafts.

## Requirements

[REQ-KLL-WORK-UNITS] agent-spec MUST import explicitly marked PRD or issue requirement blocks into `knowledge/requirements/*.md` artifacts.
[REQ-KLL-WORK-UNITS-GRAPH] agent-spec MUST validate KLL requirement artifacts as a dependency graph before generating executable work units.
[REQ-KLL-WORK-UNITS-DRAFTS] agent-spec MUST generate reviewable Task Contract drafts with `satisfies: [REQ-*]` links for ready work units.

## Scenarios

Scenario: Import a marked requirement block
  Given a PRD or issue file contains an explicit `agent-spec:requirement` block
  When the requirements import command runs
  Then agent-spec writes a KLL requirement artifact preserving the id, title, requirements, scenarios, dependencies, source trace, and open questions

Scenario: Generate a work unit from a ready requirement
  Given a KLL requirement has scenarios and no blocking open questions
  When the work-unit command runs
  Then agent-spec emits a ready work unit linked to the source requirement id

Scenario: Draft a Task Contract from a ready work unit
  Given a ready work unit exists for a KLL requirement
  When the draft-spec command runs
  Then agent-spec writes a reviewable `.spec.md` draft with `satisfies: [REQ-*]`

## Dependencies

None.

## Source Trace

- product decision: drawer_agent_spec_default_91c5ee38
- reference project: <reference-project>

## Open Questions

None.
```

- [ ] **Step 2: Write the task spec**

Create `specs/task-requirements-intake-work-units.spec.md` with this content:

```md
spec: task
name: "Requirements Intake And Work Units"
tags: [kll, requirements, work-units]
satisfies: [REQ-KLL-WORK-UNITS]
---

## Intent

Build a deterministic pipeline that imports marked PRD/issue requirement blocks into `knowledge/requirements/*.md`, validates those artifacts as a graph, generates executable work units, and drafts reviewable Task Contracts linked by `satisfies: [REQ-*]`.

## Decisions

- Use explicit `<!-- agent-spec:requirement ... -->` blocks for deterministic PRD/issue intake.
- Keep raw unmarked PRD prose outside automated import.
- Reuse KLL `kind: requirement` artifacts as the long-lived source of truth.
- Generate work units from KLL requirement docs, not directly from PRD source files.
- Generate `.spec.md` drafts only; do not mark generated drafts as verified or accepted.
- Block executable work-unit generation when `## Open Questions` contains content other than `None.`.

## Boundaries

### Allowed Changes
- src/main.rs
- src/spec_knowledge/**
- knowledge/requirements/req-kll-work-units.md
- README.md
- specs/task-requirements-intake-work-units.spec.md

### Forbidden
- Do not add network or LLM dependencies.
- Do not add serde_yaml.
- Do not copy Python code from the reference project.
- Do not change existing verification verdict semantics.

## Completion Criteria

Scenario: Import marked PRD requirement blocks into KLL requirement files
  Test: test_requirements_import_parses_block_and_renders_artifact
  Given a Markdown source containing one `agent-spec:requirement` block
  When the import parser processes the source
  Then it returns one requirement artifact with frontmatter `kind: requirement`, the declared id, title-derived filename, source trace, requirements, scenarios, dependencies, and open questions preserved

Scenario: Reject malformed requirement import blocks
  Test: test_requirements_import_rejects_missing_id
  Given a Markdown source containing an `agent-spec:requirement` block without an id
  When the import parser processes the source
  Then it returns an error explaining that `id` is required

Scenario: Build a graph from KLL requirements
  Test: test_requirement_graph_extracts_dependencies_scenarios_and_open_questions
  Given two KLL requirement docs where one depends on the other
  When the requirement graph builder reads the knowledge directory
  Then it produces two nodes, records the dependency edge, parses scenarios, and records no dangling-dependency diagnostic

Scenario: Detect invalid requirement graph edges
  Test: test_requirement_graph_reports_dangling_dependency_and_cycle
  Given KLL requirement docs with one missing dependency and one dependency cycle
  When the requirement graph validator runs
  Then it reports both a dangling dependency diagnostic and a cycle diagnostic

Scenario: Generate work units only for executable requirements
  Test: test_work_units_skip_grouping_and_block_open_questions
  Given one requirement with scenarios, one grouping requirement with child ids but no scenarios, and one requirement with open questions
  When work units are generated
  Then the scenario-backed requirement is `ready`, the grouping requirement is `grouping_only`, and the open-question requirement is `blocked_questions`

Scenario: Draft task specs link back to requirements
  Test: test_draft_specs_render_satisfies_and_bdd_scenarios
  Given a ready work unit for `REQ-101`
  When the draft-spec renderer runs
  Then the draft spec includes `satisfies: [REQ-101]`, an Intent from `Problem`, and BDD scenarios from `## Scenarios`
```

- [ ] **Step 3: Run spec lint**

Run:

```bash
cargo run --quiet -- lint specs/task-requirements-intake-work-units.spec.md --min-score 0.7
cargo run --quiet -- lint-knowledge --knowledge knowledge --gate
```

Expected:

```text
Score: 0.70 or higher
knowledge gate exits 0
```

- [ ] **Step 4: Commit the contract**

Run:

```bash
git add knowledge/requirements/req-kll-work-units.md specs/task-requirements-intake-work-units.spec.md
git commit -m "spec: add requirements intake work units contract"
```

Expected:

```text
[feat/knowledge-liveness-layer ...] spec: add requirements intake work units contract
```

---

### Task 2: Add Deterministic PRD/Issue Requirement Block Intake

**Files:**
- Create: `src/spec_knowledge/intake.rs`
- Modify: `src/spec_knowledge/model.rs`
- Modify: `src/spec_knowledge/parser.rs`
- Modify: `src/spec_knowledge/mod.rs`

**Interfaces:**
- Consumes: raw Markdown source text containing `agent-spec:requirement` blocks.
- Produces:
  - `RequirementImportBlock`
  - `parse_requirement_blocks(input: &str, source_name: &str) -> Result<Vec<RequirementImportBlock>, RequirementImportError>`
  - `KnowledgeMeta.title: Option<String>`
  - `render_requirement_artifact(block: &RequirementImportBlock) -> String`
  - `requirement_artifact_filename(block: &RequirementImportBlock) -> String`

- [ ] **Step 1: Write failing KLL title metadata tests**

Add this test inside the existing parser tests in `src/spec_knowledge/parser.rs`:

```rust
#[test]
fn test_parse_requirement_title_metadata() {
    let input = "---\nkind: requirement\nid: REQ-101\ntitle: \"User Login\"\n---\n\n## Problem\np\n## Requirements\n[REQ-101] The system MUST log users in.\n";
    let doc = parse_requirement_str(input, Path::new("req-101-user-login.md")).unwrap();
    assert_eq!(doc.meta.title.as_deref(), Some("User Login"));
}
```

- [ ] **Step 2: Run metadata test to verify it fails**

Run:

```bash
cargo test --quiet test_parse_requirement_title_metadata
```

Expected:

```text
error[E0609]: no field `title` on type `KnowledgeMeta`
```

- [ ] **Step 3: Add `title` to KLL metadata**

Modify `src/spec_knowledge/model.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeMeta {
    pub kind: KnowledgeKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DecisionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub liveness: LivenessDeclared,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}
```

Update all `KnowledgeMeta { ... }` test literals in the repository to set:

```rust
title: None,
```

Modify `src/spec_knowledge/parser.rs`:

```rust
let mut title: Option<String> = None;
```

Handle the key in `parse_knowledge_meta`:

```rust
"title" => title = Some(val.to_string()),
```

Return it:

```rust
Ok(KnowledgeMeta {
    kind,
    id,
    title,
    status,
    supersedes,
    liveness,
    tags,
})
```

- [ ] **Step 4: Run metadata test**

Run:

```bash
cargo test --quiet test_parse_requirement_title_metadata
```

Expected:

```text
1 passed
```

- [ ] **Step 5: Write failing intake parser tests**

Add tests inside `src/spec_knowledge/intake.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_requirements_import_parses_block_and_renders_artifact() {
        let input = r#"Intro.
<!-- agent-spec:requirement id=REQ-101 title="User Login" tags=auth,web source=issue:#123 -->
## Problem

Users with existing accounts need to authenticate.

## Requirements

[REQ-101] The authentication service MUST create a login session when valid credentials are submitted.

## Scenarios

Scenario: Valid login
  Given the visitor has a valid persisted account
  When the visitor submits valid credentials
  Then the system establishes a login session

## Dependencies

- REQ-100

## Source Trace

- issue:#123

## Open Questions

None.
<!-- /agent-spec:requirement -->
"#;

        let blocks = parse_requirement_blocks(input, "issue-123.md").unwrap();
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.id, "REQ-101");
        assert_eq!(block.title, "User Login");
        assert_eq!(block.tags, vec!["auth", "web"]);
        assert_eq!(block.source, Some("issue:#123".to_string()));
        assert!(block.body.contains("## Requirements"));

        let rendered = render_requirement_artifact(block);
        assert!(rendered.contains("kind: requirement"));
        assert!(rendered.contains("id: REQ-101"));
        assert!(rendered.contains("title: \"User Login\""));
        assert!(rendered.contains("tags: [auth, web]"));
        assert!(rendered.contains("[REQ-101] The authentication service MUST create a login session"));
        assert!(
            rendered.find("## Source Trace").unwrap() < rendered.find("## Open Questions").unwrap(),
            "Source Trace must appear before Open Questions"
        );
        assert_eq!(requirement_artifact_filename(block), "req-101-user-login.md");
    }

    #[test]
    fn test_requirements_import_rejects_missing_id() {
        let input = r#"<!-- agent-spec:requirement title="User Login" -->
## Problem

p

## Requirements

[REQ-101] The service MUST authenticate users.
<!-- /agent-spec:requirement -->"#;

        let err = parse_requirement_blocks(input, "bad.md").unwrap_err();
        assert!(err.to_string().contains("id is required"));
    }
}
```

- [ ] **Step 6: Run intake tests to verify they fail**

Run:

```bash
cargo test --quiet spec_knowledge::intake
```

Expected:

```text
error[E0432] or error[E0425]
```

- [ ] **Step 7: Implement the intake model and parser**

Create `src/spec_knowledge/intake.rs` with:

```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementImportBlock {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub body: String,
    pub source_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementImportError {
    pub message: String,
}

impl fmt::Display for RequirementImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RequirementImportError {}

pub fn parse_requirement_blocks(
    input: &str,
    source_name: &str,
) -> Result<Vec<RequirementImportBlock>, RequirementImportError> {
    let mut blocks = Vec::new();
    let mut rest = input;
    loop {
        let Some(start) = rest.find("<!-- agent-spec:requirement") else {
            break;
        };
        rest = &rest[start..];
        let Some(header_end) = rest.find("-->") else {
            return Err(err("requirement block opening marker is not closed"));
        };
        let header = &rest["<!-- agent-spec:requirement".len()..header_end];
        let after_header = &rest[header_end + "-->".len()..];
        let Some(close_start) = after_header.find("<!-- /agent-spec:requirement -->") else {
            return Err(err("requirement block closing marker is missing"));
        };
        let body = after_header[..close_start].trim().to_string();
        let attrs = parse_attrs(header);
        let id = required_attr(&attrs, "id")?.to_ascii_uppercase();
        let title = required_attr(&attrs, "title")?.to_string();
        let tags = attrs
            .iter()
            .find(|(k, _)| k == "tags")
            .map(|(_, v)| {
                v.split(',')
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let source = attrs
            .iter()
            .find(|(k, _)| k == "source")
            .map(|(_, v)| v.to_string());
        blocks.push(RequirementImportBlock {
            id,
            title,
            tags,
            source,
            body,
            source_name: source_name.to_string(),
        });
        rest = &after_header[close_start + "<!-- /agent-spec:requirement -->".len()..];
    }
    Ok(blocks)
}

fn err(message: &str) -> RequirementImportError {
    RequirementImportError {
        message: message.to_string(),
    }
}

fn required_attr<'a>(
    attrs: &'a [(String, String)],
    key: &str,
) -> Result<&'a str, RequirementImportError> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| err(&format!("{key} is required")))
}

fn parse_attrs(input: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if key_start == i {
            break;
        }
        let key = input[key_start..i].trim().to_string();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            break;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            let value = input[value_start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
            value
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            input[value_start..i].trim().to_string()
        };
        attrs.push((key, value));
    }
    attrs
}

pub fn render_requirement_artifact(block: &RequirementImportBlock) -> String {
    let tags = if block.tags.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", block.tags.join(", "))
    };
    let mut body = block.body.trim().to_string();
    if block.source.is_some() && !body.contains("## Source Trace") {
        let source_trace = format!(
            "\n## Source Trace\n\n- {}\n",
            block.source.as_deref().unwrap_or_default()
        );
        if let Some(open_questions_pos) = body.find("## Open Questions") {
            body.insert_str(open_questions_pos, &source_trace);
        } else {
            body.push_str(&source_trace);
        }
    }
    format!(
        "---\nkind: requirement\nid: {}\ntitle: \"{}\"\nliveness: auto\ntags: {}\n---\n\n{}\n",
        block.id,
        escape_title(&block.title),
        tags,
        body
    )
}

pub fn requirement_artifact_filename(block: &RequirementImportBlock) -> String {
    format!(
        "{}-{}.md",
        block.id.to_ascii_lowercase(),
        slugify(&block.title)
    )
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn escape_title(input: &str) -> String {
    input.replace('"', "\\\"")
}
```

- [ ] **Step 8: Re-export the module**

Modify `src/spec_knowledge/mod.rs`:

```rust
pub mod intake;

pub use intake::{
    RequirementImportBlock, RequirementImportError, parse_requirement_blocks,
    render_requirement_artifact, requirement_artifact_filename,
};
```

- [ ] **Step 9: Run parser tests**

Run:

```bash
cargo test --quiet spec_knowledge::intake
```

Expected:

```text
2 passed
```

- [ ] **Step 10: Commit**

Run:

```bash
git add src/spec_knowledge/intake.rs src/spec_knowledge/model.rs src/spec_knowledge/parser.rs src/spec_knowledge/mod.rs
git commit -m "feat: parse requirement intake blocks"
```

Expected:

```text
[feat/knowledge-liveness-layer ...] feat: parse requirement intake blocks
```

---

### Task 3: Build The Requirement Graph From KLL Requirement Docs

**Files:**
- Create: `src/spec_knowledge/requirement_graph.rs`
- Modify: `src/spec_knowledge/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeDoc` values from `collect_knowledge_checked`.
- Produces:
  - `RequirementGraph`
  - `RequirementNode`
  - `RequirementScenario`
  - `RequirementStep`
  - `RequirementGraphDiagnostic`
  - `build_requirement_graph(knowledge_dir: &Path) -> RequirementGraph`
  - `validate_requirement_graph(graph: &RequirementGraph) -> Vec<RequirementGraphDiagnostic>`

- [ ] **Step 1: Write failing graph tests**

Add tests inside `src/spec_knowledge/requirement_graph.rs`:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("agent-spec-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("requirements")).unwrap();
        dir
    }

    #[test]
    fn test_requirement_graph_extracts_dependencies_scenarios_and_open_questions() {
        let dir = temp_dir("req-graph-ok");
        fs::write(
            dir.join("requirements/req-100-session.md"),
            "---\nkind: requirement\nid: REQ-100\ntitle: \"Session Persistence\"\n---\n## Problem\nSession exists.\n## Requirements\n[REQ-100] The session service MUST persist sessions.\n## Scenarios\nScenario: Persist session\n  Given valid session data\n  When the service stores the session\n  Then the session is available for later reads\n## Open Questions\nNone.\n",
        )
        .unwrap();
        fs::write(
            dir.join("requirements/req-101-login.md"),
            "---\nkind: requirement\nid: REQ-101\ntitle: \"User Login\"\ntags: [auth]\n---\n## Problem\nLogin.\n## Requirements\n[REQ-101] The authentication service MUST create a login session.\n## Dependencies\n- REQ-100\n## Scenarios\nScenario: Valid login\n  Given a valid account\n  When valid credentials are submitted\n  Then a login session is created\n## Open Questions\nNone.\n",
        )
        .unwrap();

        let graph = build_requirement_graph(&dir);
        assert!(graph.parse_errors.is_empty());
        assert_eq!(graph.nodes.len(), 2);
        let login = graph.node("REQ-101").unwrap();
        assert_eq!(login.title, "User Login");
        assert_eq!(login.dependencies, vec!["REQ-100"]);
        assert_eq!(login.tags, vec!["auth"]);
        assert_eq!(login.scenarios.len(), 1);
        assert!(login.open_questions.is_empty());
        assert!(validate_requirement_graph(&graph).is_empty());
    }

    #[test]
    fn test_requirement_graph_reports_dangling_dependency_and_cycle() {
        let dir = temp_dir("req-graph-bad");
        fs::write(
            dir.join("requirements/req-201-a.md"),
            "---\nkind: requirement\nid: REQ-201\n---\n## Problem\nA.\n## Requirements\n[REQ-201] The service MUST do A.\n## Dependencies\n- REQ-202\n- REQ-999\n",
        )
        .unwrap();
        fs::write(
            dir.join("requirements/req-202-b.md"),
            "---\nkind: requirement\nid: REQ-202\n---\n## Problem\nB.\n## Requirements\n[REQ-202] The service MUST do B.\n## Dependencies\n- REQ-201\n",
        )
        .unwrap();

        let graph = build_requirement_graph(&dir);
        let diagnostics = validate_requirement_graph(&graph);
        assert!(diagnostics.iter().any(|d| d.code == "dangling-dependency"));
        assert!(diagnostics.iter().any(|d| d.code == "dependency-cycle"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --quiet spec_knowledge::requirement_graph
```

Expected:

```text
error[E0432] or error[E0425]
```

- [ ] **Step 3: Implement graph types and section extraction**

Create `src/spec_knowledge/requirement_graph.rs` with these public types:

```rust
use crate::spec_knowledge::{
    KnowledgeKind, KnowledgeParseError, collect_knowledge_checked, extract_requirements,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementGraph {
    pub nodes: Vec<RequirementNode>,
    pub diagnostics: Vec<RequirementGraphDiagnostic>,
    pub parse_errors: Vec<KnowledgeParseErrorView>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementNode {
    pub id: String,
    pub title: String,
    pub source_path: PathBuf,
    pub problem: String,
    pub clauses: Vec<RequirementClauseView>,
    pub dependencies: Vec<String>,
    pub children: Vec<String>,
    pub scenarios: Vec<RequirementScenario>,
    pub source_trace: Vec<String>,
    pub open_questions: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementClauseView {
    pub id: Option<String>,
    pub keyword: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementScenario {
    pub name: String,
    pub steps: Vec<RequirementStep>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementStep {
    pub keyword: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementGraphDiagnostic {
    pub code: String,
    pub severity: String,
    pub requirement_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KnowledgeParseErrorView {
    pub path: PathBuf,
    pub message: String,
}
```

Implement `build_requirement_graph` so it:

- Calls `collect_knowledge_checked(knowledge_dir)`.
- Keeps only `KnowledgeKind::Requirement`.
- Converts each doc into `RequirementNode`.
- Sets `RequirementNode.title` from `doc.meta.title`; if absent, falls back to `doc.meta.id` and emits a `missing-title` warning diagnostic. Do not derive product titles from filename slugs.
- Uses `doc.section("Problem")`.
- Uses existing `extract_requirements(doc)` for clauses.
- Converts `RequirementClause.keyword` from `Option<NormativeKeyword>` into `Option<String>` with explicit mapping: `Must -> "MUST"`, `MustNot -> "MUST NOT"`, `Should -> "SHOULD"`, `ShouldNot -> "SHOULD NOT"`, `May -> "MAY"`.
- Extracts ids from `## Dependencies` and `## Child Requirements`.
- Parses `## Scenarios` blocks beginning with `Scenario:` or `场景:`.
- Parses steps beginning with `Given`, `When`, `Then`, `And`, `But`, `假设`, `当`, `那么`, `并且`, `但是`.
- Treats `Open Questions` body as empty when it is blank, `None.`, `None`, `无`, or `无。`.

- [ ] **Step 4: Implement graph validation**

Implement `validate_requirement_graph` so it reports:

```text
duplicate-requirement-id: same id appears in more than one file
dangling-dependency: a dependency id is not present in the graph
dangling-child: a child requirement id is not present in the graph
dependency-cycle: dependencies contain a cycle
missing-title: a requirement artifact has no frontmatter title
missing-scenarios: a leaf requirement has no scenarios and no open questions
blocked-open-questions: open questions prevent executable unit generation
```

Severity mapping is part of the contract:

```text
error: duplicate-requirement-id, dangling-dependency, dangling-child, dependency-cycle
warning: missing-title, missing-scenarios, blocked-open-questions
```

The validator must sort diagnostics by `(requirement_id, code, message)` before returning.

- [ ] **Step 5: Re-export graph APIs**

Modify `src/spec_knowledge/mod.rs`:

```rust
pub mod requirement_graph;

pub use requirement_graph::{
    KnowledgeParseErrorView, RequirementClauseView, RequirementGraph,
    RequirementGraphDiagnostic, RequirementNode, RequirementScenario, RequirementStep,
    build_requirement_graph, validate_requirement_graph,
};
```

- [ ] **Step 6: Run graph tests**

Run:

```bash
cargo test --quiet spec_knowledge::requirement_graph
```

Expected:

```text
2 passed
```

- [ ] **Step 7: Commit**

Run:

```bash
git add src/spec_knowledge/requirement_graph.rs src/spec_knowledge/mod.rs
git commit -m "feat: build requirement graph from KLL docs"
```

Expected:

```text
[feat/knowledge-liveness-layer ...] feat: build requirement graph from KLL docs
```

---

### Task 4: Generate Executable Work Units From The Requirement Graph

**Files:**
- Create: `src/spec_knowledge/work_units.rs`
- Modify: `src/spec_knowledge/mod.rs`

**Interfaces:**
- Consumes: `RequirementGraph`.
- Produces:
  - `WorkUnitSet`
  - `WorkUnit`
  - `WorkUnitMode`
  - `WorkUnitStatus`
  - `build_work_units(graph: &RequirementGraph) -> WorkUnitSet`

- [ ] **Step 1: Write failing work-unit tests**

Add tests inside `src/spec_knowledge/work_units.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_knowledge::{
        RequirementClauseView, RequirementGraph, RequirementNode, RequirementScenario,
        RequirementStep,
    };
    use std::path::PathBuf;

    fn node(id: &str, scenarios: usize, children: Vec<&str>, open_questions: Vec<&str>) -> RequirementNode {
        RequirementNode {
            id: id.to_string(),
            title: format!("{id} title"),
            source_path: PathBuf::from(format!("knowledge/requirements/{}.md", id.to_ascii_lowercase())),
            problem: format!("{id} problem"),
            clauses: vec![RequirementClauseView {
                id: Some(id.to_string()),
                keyword: Some("MUST".to_string()),
                text: format!("The system MUST satisfy {id}."),
            }],
            dependencies: Vec::new(),
            children: children.into_iter().map(str::to_string).collect(),
            scenarios: (0..scenarios)
                .map(|idx| RequirementScenario {
                    name: format!("Scenario {idx}"),
                    steps: vec![
                        RequirementStep { keyword: "Given".into(), content: "a precondition".into() },
                        RequirementStep { keyword: "When".into(), content: "an action happens".into() },
                        RequirementStep { keyword: "Then".into(), content: "an outcome is visible".into() },
                    ],
                })
                .collect(),
            source_trace: Vec::new(),
            open_questions: open_questions.into_iter().map(str::to_string).collect(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn test_work_units_skip_grouping_and_block_open_questions() {
        let graph = RequirementGraph {
            nodes: vec![
                node("REQ-101", 1, vec![], vec![]),
                node("REQ-200", 0, vec!["REQ-201"], vec![]),
                node("REQ-300", 1, vec![], vec!["Should this support SSO?"]),
                node("REQ-400", 0, vec![], vec![]),
            ],
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };

        let set = build_work_units(&graph);
        assert_eq!(set.units.len(), 4);
        assert_eq!(set.unit("REQ-101").unwrap().mode, WorkUnitMode::LeafFull);
        assert_eq!(set.unit("REQ-101").unwrap().status, WorkUnitStatus::Ready);
        assert_eq!(set.unit("REQ-200").unwrap().mode, WorkUnitMode::GroupingOnly);
        assert_eq!(set.unit("REQ-300").unwrap().mode, WorkUnitMode::BlockedQuestions);
        assert_eq!(set.unit("REQ-300").unwrap().status, WorkUnitStatus::Blocked);
        assert_eq!(set.unit("REQ-400").unwrap().mode, WorkUnitMode::MissingScenarios);
        assert_eq!(set.unit("REQ-400").unwrap().status, WorkUnitStatus::Blocked);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --quiet spec_knowledge::work_units
```

Expected:

```text
error[E0432] or error[E0425]
```

- [ ] **Step 3: Implement work-unit types and classification**

Create `src/spec_knowledge/work_units.rs` with:

```rust
use crate::spec_knowledge::{RequirementGraph, RequirementGraphDiagnostic, RequirementNode};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkUnitSet {
    pub version: u32,
    pub units: Vec<WorkUnit>,
    pub diagnostics: Vec<RequirementGraphDiagnostic>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkUnit {
    pub id: String,
    pub requirement_id: String,
    pub title: String,
    pub source_path: PathBuf,
    pub mode: WorkUnitMode,
    pub status: WorkUnitStatus,
    pub depends_on: Vec<String>,
    pub satisfies: Vec<String>,
    pub scenario_count: usize,
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitMode {
    LeafFull,
    ParentScenario,
    GroupingOnly,
    BlockedQuestions,
    MissingScenarios,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitStatus {
    Ready,
    Informational,
    Blocked,
}

impl WorkUnitSet {
    pub fn unit(&self, requirement_id: &str) -> Option<&WorkUnit> {
        self.units.iter().find(|unit| unit.requirement_id == requirement_id)
    }
}

pub fn build_work_units(graph: &RequirementGraph) -> WorkUnitSet {
    let mut units: Vec<WorkUnit> = graph.nodes.iter().map(unit_from_node).collect();
    units.sort_by(|a, b| a.requirement_id.cmp(&b.requirement_id));
    WorkUnitSet {
        version: 1,
        units,
        diagnostics: graph.diagnostics.clone(),
    }
}

fn unit_from_node(node: &RequirementNode) -> WorkUnit {
    let has_questions = !node.open_questions.is_empty();
    let has_scenarios = !node.scenarios.is_empty();
    let has_children = !node.children.is_empty();
    let mode = if has_questions {
        WorkUnitMode::BlockedQuestions
    } else if has_scenarios && has_children {
        WorkUnitMode::ParentScenario
    } else if has_scenarios {
        WorkUnitMode::LeafFull
    } else if has_children {
        WorkUnitMode::GroupingOnly
    } else {
        WorkUnitMode::MissingScenarios
    };
    let status = match mode {
        WorkUnitMode::BlockedQuestions => WorkUnitStatus::Blocked,
        WorkUnitMode::MissingScenarios => WorkUnitStatus::Blocked,
        WorkUnitMode::GroupingOnly => WorkUnitStatus::Informational,
        WorkUnitMode::LeafFull | WorkUnitMode::ParentScenario => WorkUnitStatus::Ready,
    };
    WorkUnit {
        id: format!("WU-{}", node.id),
        requirement_id: node.id.clone(),
        title: node.title.clone(),
        source_path: node.source_path.clone(),
        mode,
        status,
        depends_on: node.dependencies.clone(),
        satisfies: vec![node.id.clone()],
        scenario_count: node.scenarios.len(),
        blocked_by: node.open_questions.clone(),
    }
}
```

- [ ] **Step 4: Re-export work-unit APIs**

Modify `src/spec_knowledge/mod.rs`:

```rust
pub mod work_units;

pub use work_units::{
    WorkUnit, WorkUnitMode, WorkUnitSet, WorkUnitStatus, build_work_units,
};
```

- [ ] **Step 5: Run work-unit tests**

Run:

```bash
cargo test --quiet spec_knowledge::work_units
```

Expected:

```text
1 passed
```

- [ ] **Step 6: Commit**

Run:

```bash
git add src/spec_knowledge/work_units.rs src/spec_knowledge/mod.rs
git commit -m "feat: generate work units from requirements"
```

Expected:

```text
[feat/knowledge-liveness-layer ...] feat: generate work units from requirements
```

---

### Task 5: Render Draft Task Specs From Ready Work Units

**Files:**
- Create: `src/spec_knowledge/draft_specs.rs`
- Modify: `src/spec_knowledge/mod.rs`

**Interfaces:**
- Consumes: `RequirementNode` and `WorkUnit`.
- Produces:
  - `DraftSpec`
  - `render_draft_spec(node: &RequirementNode, unit: &WorkUnit) -> Option<DraftSpec>`
  - `draft_spec_filename(unit: &WorkUnit) -> String`

- [ ] **Step 1: Write failing draft renderer test**

Add tests inside `src/spec_knowledge/draft_specs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_knowledge::{
        RequirementClauseView, RequirementNode, RequirementScenario, RequirementStep, WorkUnit,
        WorkUnitMode, WorkUnitStatus,
    };
    use std::path::PathBuf;

    #[test]
    fn test_draft_specs_render_satisfies_and_bdd_scenarios() {
        let node = RequirementNode {
            id: "REQ-101".into(),
            title: "User Login".into(),
            source_path: PathBuf::from("knowledge/requirements/req-101-user-login.md"),
            problem: "Users need to log in with existing accounts.".into(),
            clauses: vec![RequirementClauseView {
                id: Some("REQ-101".into()),
                keyword: Some("MUST".into()),
                text: "The authentication service MUST create a login session.".into(),
            }],
            dependencies: vec!["REQ-100".into()],
            children: Vec::new(),
            scenarios: vec![RequirementScenario {
                name: "Valid login".into(),
                steps: vec![
                    RequirementStep { keyword: "Given".into(), content: "the visitor has a valid persisted account".into() },
                    RequirementStep { keyword: "When".into(), content: "the visitor submits valid credentials".into() },
                    RequirementStep { keyword: "Then".into(), content: "the system establishes a login session".into() },
                ],
            }],
            source_trace: vec!["issue:#123".into()],
            open_questions: Vec::new(),
            tags: vec!["auth".into()],
        };
        let unit = WorkUnit {
            id: "WU-REQ-101".into(),
            requirement_id: "REQ-101".into(),
            title: "User Login".into(),
            source_path: node.source_path.clone(),
            mode: WorkUnitMode::LeafFull,
            status: WorkUnitStatus::Ready,
            depends_on: vec!["REQ-100".into()],
            satisfies: vec!["REQ-101".into()],
            scenario_count: 1,
            blocked_by: Vec::new(),
        };

        let draft = render_draft_spec(&node, &unit).unwrap();
        assert_eq!(draft.filename, "task-req-101-user-login.spec.md");
        assert!(draft.content.contains("spec: task"));
        assert!(draft.content.contains("satisfies: [REQ-101]"));
        assert!(draft.content.contains("## Intent"));
        assert!(draft.content.contains("Users need to log in with existing accounts."));
        assert!(draft.content.contains("Scenario: Valid login"));
        assert!(draft.content.contains("Test: pending_req_101_valid_login"));
        assert!(draft.content.contains("Given the visitor has a valid persisted account"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --quiet spec_knowledge::draft_specs
```

Expected:

```text
error[E0432] or error[E0425]
```

- [ ] **Step 3: Implement the draft renderer**

Create `src/spec_knowledge/draft_specs.rs` with:

```rust
use crate::spec_knowledge::{RequirementNode, WorkUnit, WorkUnitStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSpec {
    pub filename: String,
    pub content: String,
}

pub fn render_draft_spec(node: &RequirementNode, unit: &WorkUnit) -> Option<DraftSpec> {
    if unit.status != WorkUnitStatus::Ready {
        return None;
    }
    let filename = draft_spec_filename(unit);
    let mut content = String::new();
    content.push_str("spec: task\n");
    content.push_str(&format!("name: \"{}\"\n", escape_title(&node.title)));
    content.push_str("tags: [requirements, generated-draft]\n");
    if !unit.depends_on.is_empty() {
        content.push_str(&format!("depends: [{}]\n", unit.depends_on.join(", ")));
    }
    content.push_str(&format!("satisfies: [{}]\n", unit.satisfies.join(", ")));
    content.push_str("---\n\n");
    content.push_str("## Intent\n\n");
    content.push_str(node.problem.trim());
    content.push_str("\n\n## Decisions\n\n");
    content.push_str("- Generated draft from KLL requirement artifact; human review must confirm boundaries and test selectors before implementation.\n");
    for clause in &node.clauses {
        content.push_str("- ");
        content.push_str(&clause.text);
        content.push('\n');
    }
    content.push_str("\n## Boundaries\n\n");
    content.push_str("### Allowed Changes\n");
    content.push_str("- src/**\n");
    content.push_str("- tests/**\n\n");
    content.push_str("### Forbidden\n");
    content.push_str("- Do not weaken or remove the source requirement clauses.\n");
    content.push_str("- Do not mark this generated draft complete until each `Test:` selector names a real test.\n\n");
    content.push_str("## Completion Criteria\n\n");
    for scenario in &node.scenarios {
        content.push_str(&format!("Scenario: {}\n", scenario.name));
        content.push_str(&format!(
            "  Test: {}\n",
            pending_test_name(&node.id, &scenario.name)
        ));
        for step in &scenario.steps {
            content.push_str(&format!("  {} {}\n", step.keyword, step.content));
        }
        content.push('\n');
    }
    if !node.source_trace.is_empty() {
        content.push_str("## Questions\n\n");
        content.push_str("- Confirm concrete file boundaries before implementation.\n");
        content.push_str("- Replace pending test selectors with real test names before lifecycle verification.\n");
    }
    Some(DraftSpec { filename, content })
}

pub fn draft_spec_filename(unit: &WorkUnit) -> String {
    format!(
        "task-{}-{}.spec.md",
        unit.requirement_id.to_ascii_lowercase(),
        slugify(&unit.title)
    )
}

fn pending_test_name(requirement_id: &str, scenario_name: &str) -> String {
    format!(
        "pending_{}_{}",
        requirement_id.to_ascii_lowercase().replace('-', "_"),
        slugify(scenario_name).replace('-', "_")
    )
}

fn escape_title(input: &str) -> String {
    input.replace('"', "\\\"")
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
```

- [ ] **Step 4: Re-export draft APIs**

Modify `src/spec_knowledge/mod.rs`:

```rust
pub mod draft_specs;

pub use draft_specs::{DraftSpec, draft_spec_filename, render_draft_spec};
```

- [ ] **Step 5: Run draft tests**

Run:

```bash
cargo test --quiet spec_knowledge::draft_specs
```

Expected:

```text
1 passed
```

- [ ] **Step 6: Commit**

Run:

```bash
git add src/spec_knowledge/draft_specs.rs src/spec_knowledge/mod.rs
git commit -m "feat: render draft specs from work units"
```

Expected:

```text
[feat/knowledge-liveness-layer ...] feat: render draft specs from work units
```

---

### Task 6: Add Requirements CLI Subcommands

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Produces CLI:
  - `agent-spec requirements import --from <file> --out knowledge/requirements`
  - `agent-spec requirements graph --knowledge knowledge --format json --gate`
  - `agent-spec requirements work-units --knowledge knowledge --out .agent-spec/work_units.json`
  - `agent-spec requirements draft-specs --knowledge knowledge --out specs/generated --check`

- [ ] **Step 1: Write failing CLI tests in `src/main.rs`**

Add tests under the existing `#[cfg(test)] mod tests` in `src/main.rs`:

```rust
#[test]
fn test_requirements_import_command_writes_artifact() {
    let dir = make_temp_dir("requirements-import-cli");
    let source = dir.join("issue.md");
    let out = dir.join("knowledge/requirements");
    fs::write(
        &source,
        "<!-- agent-spec:requirement id=REQ-101 title=\"User Login\" tags=auth source=issue:#123 -->\n## Problem\nLogin.\n\n## Requirements\n\n[REQ-101] The authentication service MUST create a login session.\n\n## Scenarios\n\nScenario: Valid login\n  Given a valid account\n  When valid credentials are submitted\n  Then a session is created\n\n## Open Questions\n\nNone.\n<!-- /agent-spec:requirement -->\n",
    )
    .unwrap();

    cmd_requirements_import(&source, &out, false).unwrap();
    let artifact = out.join("req-101-user-login.md");
    assert!(artifact.exists());
    let body = fs::read_to_string(artifact).unwrap();
    assert!(body.contains("kind: requirement"));
    assert!(body.contains("id: REQ-101"));
    assert!(body.contains("title: \"User Login\""));
}

#[test]
fn test_requirements_work_units_command_writes_json() {
    let dir = make_temp_dir("requirements-work-units-cli");
    let knowledge = dir.join("knowledge");
    fs::create_dir_all(knowledge.join("requirements")).unwrap();
    fs::write(
        knowledge.join("requirements/req-101-login.md"),
        "---\nkind: requirement\nid: REQ-101\ntitle: \"User Login\"\n---\n## Problem\nLogin.\n## Requirements\n[REQ-101] The authentication service MUST create a login session.\n## Scenarios\nScenario: Valid login\n  Given a valid account\n  When valid credentials are submitted\n  Then a session is created\n## Open Questions\nNone.\n",
    )
    .unwrap();
    let out = dir.join(".agent-spec/work_units.json");

    cmd_requirements_work_units(&knowledge, Some(&out), "json").unwrap();
    let json = fs::read_to_string(out).unwrap();
    assert!(json.contains("\"requirement_id\":\"REQ-101\"") || json.contains("\"requirement_id\": \"REQ-101\""));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --quiet requirements_import_command requirements_work_units_command
```

Expected:

```text
error[E0425]
```

- [ ] **Step 3: Add nested Clap commands**

This is the first nested subcommand family in `src/main.rs`; keep help text style consistent with existing flat commands: one concise imperative sentence per command, lowercase flag names, and no hidden side effects.

Modify the CLI definitions in `src/main.rs`:

```rust
#[derive(Subcommand)]
enum RequirementCommands {
    /// Import marked PRD/issue blocks into knowledge/requirements/*.md
    Import {
        #[arg(long)]
        from: PathBuf,
        #[arg(long, default_value = "knowledge/requirements")]
        out: PathBuf,
        #[arg(long)]
        check: bool,
    },
    /// Validate and print the requirement graph
    Graph {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long)]
        gate: bool,
    },
    /// Generate work_units.json from KLL requirements
    WorkUnits {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Render reviewable task spec drafts from ready work units
    DraftSpecs {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "specs/generated")]
        out: PathBuf,
        #[arg(long)]
        check: bool,
    },
}
```

Add this variant to `Commands`:

```rust
/// Import, validate, plan, and draft from KLL requirements
Requirements {
    #[command(subcommand)]
    action: RequirementCommands,
},
```

- [ ] **Step 4: Add command dispatch**

Add to the `match cli.command` arm:

```rust
Commands::Requirements { action } => cmd_requirements(action),
```

Add helper functions:

```rust
fn cmd_requirements(action: RequirementCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        RequirementCommands::Import { from, out, check } => {
            cmd_requirements_import(&from, &out, check)
        }
        RequirementCommands::Graph {
            knowledge,
            format,
            gate,
        } => cmd_requirements_graph(&knowledge, &format, gate),
        RequirementCommands::WorkUnits {
            knowledge,
            out,
            format,
        } => cmd_requirements_work_units(&knowledge, out.as_deref(), &format),
        RequirementCommands::DraftSpecs {
            knowledge,
            out,
            check,
        } => cmd_requirements_draft_specs(&knowledge, &out, check),
    }
}
```

- [ ] **Step 5: Implement command behavior**

Add command functions near the KLL command section:

```rust
fn cmd_requirements_import(
    from: &Path,
    out: &Path,
    check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let input = std::fs::read_to_string(from)?;
    let source_name = from.display().to_string();
    let blocks = crate::spec_knowledge::parse_requirement_blocks(&input, &source_name)?;
    if blocks.is_empty() {
        return Err(format!("no agent-spec requirement blocks found in {}", from.display()).into());
    }
    let mut rendered = Vec::new();
    for block in &blocks {
        let filename = crate::spec_knowledge::requirement_artifact_filename(block);
        let path = out.join(filename);
        let content = crate::spec_knowledge::render_requirement_artifact(block);
        rendered.push((path, content));
    }
    if check {
        for (path, content) in rendered {
            let actual = std::fs::read_to_string(&path).unwrap_or_default();
            if actual != content {
                return Err(format!("generated requirement artifact drifted: {}", path.display()).into());
            }
        }
        return Ok(());
    }
    std::fs::create_dir_all(out)?;
    for (path, content) in rendered {
        std::fs::write(path, content)?;
    }
    Ok(())
}

fn cmd_requirements_graph(
    knowledge: &Path,
    format: &str,
    gate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    graph
        .diagnostics
        .extend(crate::spec_knowledge::validate_requirement_graph(&graph));
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&graph)?),
        _ => print_requirement_graph_text(&graph),
    }
    if gate && graph.diagnostics.iter().any(|d| d.severity == "error") {
        return Err("requirement graph gate failed".into());
    }
    Ok(())
}

fn cmd_requirements_work_units(
    knowledge: &Path,
    out: Option<&Path>,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    graph
        .diagnostics
        .extend(crate::spec_knowledge::validate_requirement_graph(&graph));
    let units = crate::spec_knowledge::build_work_units(&graph);
    let body = match format {
        "text" => format_work_units_text(&units),
        _ => serde_json::to_string_pretty(&units)?,
    };
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
    } else {
        print!("{body}");
    }
    Ok(())
}
```

Implement `cmd_requirements_draft_specs`, `print_requirement_graph_text`, and `format_work_units_text` in the same section. `cmd_requirements_draft_specs` must:

- Build graph and work units.
- Skip units whose status is not `Ready`.
- Find each matching `RequirementNode`.
- Render with `render_draft_spec`.
- In `--check`, return an error if an expected draft file is missing or differs.
- Without `--check`, write files under `out`.
- Match `requirements import --check` semantics: compare generated content exactly, not just file existence.

- [ ] **Step 6: Run CLI tests**

Run:

```bash
cargo test --quiet requirements_import_command requirements_work_units_command
```

Expected:

```text
2 passed
```

- [ ] **Step 7: Run command smoke tests manually**

Create `/tmp/agent-spec-prd.md` with the example import block from this plan, then run:

```bash
cargo run --quiet -- requirements import --from /tmp/agent-spec-prd.md --out /tmp/agent-spec-reqs/knowledge/requirements
cargo run --quiet -- requirements graph --knowledge /tmp/agent-spec-reqs/knowledge --format json --gate
cargo run --quiet -- requirements work-units --knowledge /tmp/agent-spec-reqs/knowledge --out /tmp/agent-spec-reqs/.agent-spec/work_units.json
cargo run --quiet -- requirements draft-specs --knowledge /tmp/agent-spec-reqs/knowledge --out /tmp/agent-spec-reqs/specs/generated
```

Expected:

```text
All commands exit 0.
/tmp/agent-spec-reqs/knowledge/requirements/req-101-user-login.md exists.
/tmp/agent-spec-reqs/.agent-spec/work_units.json exists.
/tmp/agent-spec-reqs/specs/generated/task-req-101-user-login.spec.md exists.
```

- [ ] **Step 8: Commit**

Run:

```bash
git add src/main.rs
git commit -m "feat: add requirements workflow commands"
```

Expected:

```text
[feat/knowledge-liveness-layer ...] feat: add requirements workflow commands
```

---

### Task 7: Update KLL Scaffold And User Documentation

**Files:**
- Modify: `src/spec_knowledge/scaffold.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: the new import block and KLL requirement format.
- Produces: documented, scaffolded usage path.

- [ ] **Step 1: Update requirement scaffold template**

Modify `REQ_TEMPLATE` in `src/spec_knowledge/scaffold.rs` to:

```rust
const REQ_TEMPLATE: &str = "---\nkind: requirement\nid: REQ-NNN\ntitle: \"Requirement Title\"\nliveness: auto\ntags: []\n---\n\n## Problem\n\nDescribe the user or system problem this requirement solves.\n\n## Requirements\n\n[REQ-NNN] The system MUST produce an observable response.\n\n## Scenarios\n\nScenario: Main behavior\n  Given a concrete starting state\n  When a concrete action occurs\n  Then a concrete observable outcome occurs\n\n## Dependencies\n\nNone.\n\n## Source Trace\n\n- issue:#NNN\n\n## Open Questions\n\nNone.\n";
```

Modify `REQUIREMENTS_README` to describe:

- one artifact per stable requirement or grouping requirement,
- `title:` as the canonical human-readable title used by graph/work-unit/spec draft generation,
- `## Requirements` as the normative source,
- `## Scenarios` as the work-unit/spec drafting source,
- `## Dependencies` as ordering edges,
- `## Open Questions` as a generation blocker.

- [ ] **Step 2: Add README section**

Add a README section named `## Requirements Intake And Work Units` after the KLL command table. Include this command path:

```bash
agent-spec requirements import --from docs/prd.md --out knowledge/requirements
agent-spec lint-knowledge --knowledge knowledge --gate
agent-spec requirements graph --knowledge knowledge --format json --gate
agent-spec requirements work-units --knowledge knowledge --out .agent-spec/work_units.json
agent-spec requirements draft-specs --knowledge knowledge --out specs/generated
# Draft specs contain pending Test selectors; this lifecycle command is expected to fail until a human replaces them with real tests.
agent-spec lifecycle specs/generated/task-req-101-user-login.spec.md --code .
agent-spec trace REQ-101 --knowledge knowledge --specs specs --code .
```

Document this warning immediately under the command block:

```md
Generated drafts are review artifacts. Their `Test:` selectors start with `pending_...`; `agent-spec lifecycle` reports those nonexistent selectors as `fail`. Replace each pending selector with a real test name before treating the draft as executable or using `trace` as acceptance evidence.
```

Document that PRD/issue import requires explicit blocks:

```md
<!-- agent-spec:requirement id=REQ-101 title="User Login" tags=auth,web source=issue:#123 -->
## Problem

Users with existing accounts need to authenticate.

## Requirements

[REQ-101] The authentication service MUST create a login session when valid credentials are submitted.

## Scenarios

Scenario: Valid login
  Given the visitor has a valid persisted account
  When the visitor submits valid credentials
  Then the system establishes a login session

## Open Questions

None.
<!-- /agent-spec:requirement -->
```

- [ ] **Step 3: Run formatting and docs checks**

Run:

```bash
cargo fmt --check
cargo test --quiet spec_knowledge::scaffold
```

Expected:

```text
cargo fmt --check exits 0.
scaffold tests pass.
```

- [ ] **Step 4: Commit**

Run:

```bash
git add src/spec_knowledge/scaffold.rs README.md
git commit -m "docs: document requirements intake workflow"
```

Expected:

```text
[feat/knowledge-liveness-layer ...] docs: document requirements intake workflow
```

---

### Task 8: End-To-End Verification And Contract Acceptance

**Files:**
- Modify only if earlier tasks reveal a defect in implementation or docs.

**Interfaces:**
- Consumes: all earlier tasks.
- Produces: verified feature branch state.

- [ ] **Step 1: Run Rust formatting**

Run:

```bash
cargo fmt --check
```

Expected:

```text
No output and exit code 0.
```

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test --quiet spec_knowledge::intake
cargo test --quiet test_parse_requirement_title_metadata
cargo test --quiet spec_knowledge::requirement_graph
cargo test --quiet spec_knowledge::work_units
cargo test --quiet spec_knowledge::draft_specs
cargo test --quiet requirements_import_command requirements_work_units_command
```

Expected:

```text
Every command exits 0.
```

- [ ] **Step 3: Run full test suite**

Run:

```bash
cargo test --quiet
```

Expected:

```text
All tests pass.
```

- [ ] **Step 4: Run clippy**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected:

```text
No warnings and exit code 0.
```

- [ ] **Step 5: Run agent-spec lifecycle for the new contract**

Run:

```bash
cargo run --quiet -- lifecycle specs/task-requirements-intake-work-units.spec.md --code . --change-scope none --format json
```

Expected:

```json
{
  "summary": {
    "failed": 0,
    "skipped": 0,
    "uncertain": 0
  }
}
```

- [ ] **Step 6: Run repo guard**

Run:

```bash
cargo run --quiet -- guard --spec-dir specs --code . --change-scope none
```

Expected:

```text
All specs pass or existing unrelated planned specs remain unchanged in status.
```

- [ ] **Step 7: Generate acceptance summary**

Run:

```bash
cargo run --quiet -- explain specs/task-requirements-intake-work-units.spec.md --code . --format markdown
```

Expected:

```text
Markdown explains the contract and shows all scenarios passing.
```

- [ ] **Step 8: Commit verification fixes**

If verification required edits, commit them:

```bash
git add src/main.rs src/spec_knowledge README.md knowledge/requirements/req-kll-work-units.md specs/task-requirements-intake-work-units.spec.md
git commit -m "test: verify requirements workflow"
```

Expected:

```text
Commit is created only if verification edits were needed.
```

---

## Self-Review

Spec coverage:

- PRD/issue to `knowledge/requirements/*.md`: Task 2 and Task 6 `requirements import`.
- KLL `title:` metadata support: Task 2.
- Source requirement for the new feature: Task 1 creates `REQ-KLL-WORK-UNITS`.
- Required KLL format and scaffold: Task 7.
- Requirement graph validation: Task 3 and Task 6 `requirements graph`.
- Work-unit generation: Task 4 and Task 6 `requirements work-units`.
- Draft Task Contracts with `satisfies`: Task 5 and Task 6 `requirements draft-specs`.
- Existing lifecycle/trace handoff: Task 7 docs and Task 8 verification.
- reference project included: `Reference Project` section.

Quality checks:

- No LLM dependency is introduced.
- No Python code from the reference project is copied.
- Parse and graph errors are surfaced through commands and tests.
- Open questions block executable unit status.
- Generated specs remain drafts with pending test selectors, and docs state that their lifecycle is expected to fail until selectors are replaced.
- `requirements import --check` and `requirements draft-specs --check` both compare exact generated content.

Type consistency:

- `RequirementNode.id` feeds `WorkUnit.requirement_id`.
- `KnowledgeMeta.title` feeds `RequirementNode.title`, which feeds `WorkUnit.title` and draft spec names.
- `WorkUnit.satisfies` feeds draft spec frontmatter `satisfies`.
- `RequirementScenario.steps` feeds draft BDD steps.
- `RequirementGraph.diagnostics` feeds graph and work-unit command outputs.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-07-requirements-intake-work-units.md`. Two execution options:

1. Subagent-Driven (recommended) - dispatch a fresh subagent per task, review between tasks, fast iteration.

2. Inline Execution - execute tasks in this session using executing-plans, batch execution with checkpoints.
