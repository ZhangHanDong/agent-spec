# KLL P1 — Knowledge & Liveness Layer (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a P1 knowledge layer to agent-spec that answers "is this decision still guarded by the code?" — typed `decision` artifacts, a `satisfies:` edge from specs to decisions, a derived liveness roll-up, a `trace` command, a gate exit code, and an `init --workspace` scaffold.

**Architecture:** A new `crate::spec_knowledge` module inside the existing single binary crate (no workspace). Decisions are a *separate* document type from specs (own frontmatter + sections), parsed by a hand-written parser mirroring `spec_parser/meta.rs`. Liveness is **derived, never stored**: it reuses `SpecGateway::verify` to get current spec verdicts and rolls them up with a deterministic precedence ladder. `init --workspace` extends the existing `cmd_init` to lay down the canonical directory tree.

**Tech Stack:** Rust 2024, single binary crate. Deps already present: `clap` (derive), `serde`/`serde_json`, `thiserror`. **No `serde_yaml`** — frontmatter is hand-parsed line-by-line (follow `spec_parser/meta.rs`). **Clippy gate:** `unwrap_used = "deny"` and `expect_used = "deny"` in non-test code — use `?`, `ok_or`, `match`; only `#[cfg(test)]` modules may `#[allow(clippy::unwrap_used)]`.

**Spec:** `docs/superpowers/specs/2026-06-23-knowledge-liveness-layer-design.md` (§6 artifact model, §6.0 id resolution, §7 satisfies + liveness ladder, §11 scaffold, §13 P1 scope).

---

## File Structure

New module `src/spec_knowledge/` (each file one responsibility):

| File | Responsibility |
|------|----------------|
| `src/spec_knowledge/mod.rs` | Module declarations + public re-exports |
| `src/spec_knowledge/model.rs` | `KnowledgeKind`, `DecisionStatus`, `LivenessDeclared`, `KnowledgeMeta`, `DecisionDoc`, `KSection`, `Liveness` types |
| `src/spec_knowledge/parser.rs` | `parse_decision_str` / `parse_decision` + `resolve_decision_id` (§6.0) |
| `src/spec_knowledge/lint.rs` | `lint_decision` — required sections + forcing functions |
| `src/spec_knowledge/index.rs` | `build_satisfies_index` — scan specs, map decision id → spec paths |
| `src/spec_knowledge/liveness.rs` | `spec_rollup` + `decision_liveness` (the precedence ladder) |
| `src/spec_knowledge/trace.rs` | `TraceReport` + `build_trace` + `format_trace` |
| `src/spec_knowledge/scaffold.rs` | `scaffold_workspace` (directory tree + templates) |

Modified existing files:

| File | Change |
|------|--------|
| `src/main.rs:12` | add `mod spec_knowledge;` |
| `src/main.rs` `Commands` enum | add `Trace { .. }`; add `--workspace` flag to `Init` |
| `src/main.rs` `run()` + handlers | dispatch `Trace`; thread `--workspace` into `cmd_init` |
| `src/spec_core/ast.rs` `SpecMeta` | add `satisfies: Vec<String>` (additive, serde default) |
| `src/spec_parser/meta.rs` `parse_meta` | parse `satisfies: [ID, ID]` array |

---

## Conventions for every task

- **TDD:** write the failing test first, run it red, implement minimal, run it green, commit.
- Tests are **inline** `#[cfg(test)] mod tests { ... }` at the bottom of each file (no `tests/` dir). Start the test module with `#![allow(clippy::unwrap_used)]` via `#[cfg(test)] #[allow(clippy::unwrap_used)] mod tests`.
- Run a single test: `cargo test spec_knowledge::<file>::tests::<name> -- --nocapture`.
- Run the module: `cargo test spec_knowledge::`.
- Never `unwrap`/`expect` in non-test code.
- Commit messages: `feat(kll): <what>`. End the body with the Co-Authored-By trailer the repo uses (check `git log -1` for the exact line).

---

## Task 1: Create the `spec_knowledge` module skeleton

**Files:**
- Create: `src/spec_knowledge/mod.rs`
- Create: `src/spec_knowledge/model.rs`
- Modify: `src/main.rs:12` (add `mod spec_knowledge;` after `mod vcs;`)

- [ ] **Step 1: Write the failing test** in `src/spec_knowledge/model.rs`

```rust
//! Knowledge-layer data model (KLL P1): decisions, liveness states.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnowledgeKind {
    Decision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Proposed,
    Accepted,
    Superseded,
    Deprecated,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LivenessDeclared {
    Auto,
    Na,
}

impl Default for LivenessDeclared {
    fn default() -> Self {
        LivenessDeclared::Auto
    }
}

/// Derived liveness state (never stored; §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Liveness {
    Honored,
    Violated,
    Unproven,
    Na,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeMeta {
    pub kind: KnowledgeKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DecisionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub liveness: LivenessDeclared,
}

/// One `## Heading` block and its raw body text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KSection {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionDoc {
    pub meta: KnowledgeMeta,
    pub sections: Vec<KSection>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

impl DecisionDoc {
    /// Find a section by case-insensitive heading match.
    pub fn section(&self, heading: &str) -> Option<&KSection> {
        self.sections
            .iter()
            .find(|s| s.heading.eq_ignore_ascii_case(heading))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_section_lookup_is_case_insensitive() {
        let doc = DecisionDoc {
            meta: KnowledgeMeta {
                kind: KnowledgeKind::Decision,
                id: "ADR-001".into(),
                status: Some(DecisionStatus::Accepted),
                supersedes: None,
                liveness: LivenessDeclared::Auto,
            },
            sections: vec![KSection {
                heading: "Context".into(),
                body: "x".into(),
            }],
            source_path: PathBuf::new(),
        };
        assert!(doc.section("context").is_some());
        assert!(doc.section("Decision").is_none());
    }
}
```

- [ ] **Step 2: Create `src/spec_knowledge/mod.rs`**

```rust
//! Knowledge & Liveness Layer (KLL). P1: decisions, satisfies edge, liveness.

pub mod model;

pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeKind, KnowledgeMeta, Liveness, LivenessDeclared,
};
```

- [ ] **Step 3: Wire the module** — add to `src/main.rs` right after `mod vcs;` (line 12):

```rust
mod spec_knowledge;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test spec_knowledge::model::tests::test_section_lookup_is_case_insensitive -- --nocapture`
Expected: PASS. Also run `cargo build` to confirm the module is wired without warnings.

- [ ] **Step 5: Commit**

```bash
git add src/spec_knowledge/ src/main.rs
git commit -m "feat(kll): add spec_knowledge module skeleton and decision model"
```

---

## Task 2: Decision frontmatter parser + id resolution (§6.0)

**Files:**
- Create: `src/spec_knowledge/parser.rs`
- Modify: `src/spec_knowledge/mod.rs` (add `pub mod parser;` + re-exports)

Mirror the hand-written style of `src/spec_parser/meta.rs:4-102` (split on `---`, key:value lines, strip quotes). **Do not** add `serde_yaml`.

- [ ] **Step 1: Write the failing tests** in `src/spec_knowledge/parser.rs`

```rust
//! Hand-written decision parser (mirrors spec_parser/meta.rs). No serde_yaml.

use crate::spec_knowledge::model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeKind, KnowledgeMeta, LivenessDeclared,
};
use std::path::Path;

/// Resolve a decision id (§6.0): frontmatter `id:` is canonical; else the
/// filename prefix `<letters>-<digits>` normalized to UPPERCASE. Returns the
/// normalized id, or `None` when neither source yields one.
pub fn resolve_decision_id(frontmatter_id: Option<&str>, path: &Path) -> Option<String> {
    if let Some(id) = frontmatter_id {
        let id = id.trim();
        if !id.is_empty() {
            return Some(id.to_ascii_uppercase());
        }
    }
    let stem = path.file_stem()?.to_str()?;
    // take leading <letters>-<digits>
    let mut parts = stem.splitn(3, '-');
    let letters = parts.next()?;
    let digits = parts.next()?;
    if !letters.is_empty()
        && letters.chars().all(|c| c.is_ascii_alphabetic())
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
    {
        Some(format!("{}-{}", letters.to_ascii_uppercase(), digits))
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_id_from_frontmatter_is_uppercased() {
        let p = PathBuf::from("knowledge/decisions/adr-001-soft-delete.md");
        assert_eq!(
            resolve_decision_id(Some("adr-001"), &p),
            Some("ADR-001".into())
        );
    }

    #[test]
    fn test_id_falls_back_to_filename_prefix() {
        let p = PathBuf::from("knowledge/decisions/adr-007-x.md");
        assert_eq!(resolve_decision_id(None, &p), Some("ADR-007".into()));
    }

    #[test]
    fn test_id_none_when_no_prefix_and_no_frontmatter() {
        let p = PathBuf::from("knowledge/decisions/notes.md");
        assert_eq!(resolve_decision_id(None, &p), None);
    }

    #[test]
    fn test_parse_decision_minimal() {
        let input = "---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n\n## Context\n\nWhy.\n\n## Decision\n\nDo X.\n\n## Consequences\n\nGood, because A. Bad, because B.\n";
        let doc = parse_decision_str(input, Path::new("adr-001-x.md")).unwrap();
        assert_eq!(doc.meta.id, "ADR-001");
        assert_eq!(doc.meta.status, Some(DecisionStatus::Accepted));
        assert_eq!(doc.meta.liveness, LivenessDeclared::Auto);
        assert!(doc.section("Context").is_some());
        assert!(doc.section("Decision").is_some());
        assert!(doc.section("Consequences").is_some());
    }

    #[test]
    fn test_parse_liveness_na() {
        let input = "---\nkind: decision\nid: ADR-009\nliveness: n/a\n---\n\n## Context\n\nLicense.\n\n## Decision\n\nMIT.\n\n## Consequences\n\nGood. Bad.\n";
        let doc = parse_decision_str(input, Path::new("adr-009.md")).unwrap();
        assert_eq!(doc.meta.liveness, LivenessDeclared::Na);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test spec_knowledge::parser::tests -- --nocapture`
Expected: FAIL — `parse_decision_str` not found.

- [ ] **Step 3: Implement the parser** (add above the `#[cfg(test)]` block)

```rust
/// Parse a decision document from a string. `path` is used for id fallback.
pub fn parse_decision_str(input: &str, path: &Path) -> Result<DecisionDoc, String> {
    let lines: Vec<&str> = input.lines().collect();
    let sep = lines
        .iter()
        .position(|l| l.trim() == "---")
        .ok_or_else(|| "missing front-matter separator '---'".to_string())?;
    // front-matter is between the first `---` and the next `---`
    let rest = &lines[sep + 1..];
    let close = rest
        .iter()
        .position(|l| l.trim() == "---")
        .ok_or_else(|| "missing closing front-matter '---'".to_string())?;
    let meta_lines = &rest[..close];
    let body_lines = &rest[close + 1..];

    let meta = parse_decision_meta(meta_lines, path)?;
    let sections = parse_sections(body_lines);
    Ok(DecisionDoc {
        meta,
        sections,
        source_path: path.to_path_buf(),
    })
}

/// Parse a decision document from disk.
pub fn parse_decision(path: &Path) -> Result<DecisionDoc, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_decision_str(&content, path)
}

fn parse_decision_meta(lines: &[&str], path: &Path) -> Result<KnowledgeMeta, String> {
    let mut id_field: Option<String> = None;
    let mut status: Option<DecisionStatus> = None;
    let mut supersedes: Option<String> = None;
    let mut liveness = LivenessDeclared::Auto;
    let mut kind = KnowledgeKind::Decision;

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, val)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim().trim_matches('"').trim();
        match key {
            "kind" => {
                if val != "decision" {
                    return Err(format!("unsupported knowledge kind '{val}' (P1: decision)"));
                }
                kind = KnowledgeKind::Decision;
            }
            "id" => id_field = Some(val.to_string()),
            "status" => {
                status = Some(match val.to_ascii_lowercase().as_str() {
                    "proposed" => DecisionStatus::Proposed,
                    "accepted" => DecisionStatus::Accepted,
                    "superseded" => DecisionStatus::Superseded,
                    "deprecated" => DecisionStatus::Deprecated,
                    "rejected" => DecisionStatus::Rejected,
                    other => return Err(format!("unknown status '{other}'")),
                });
            }
            "supersedes" => supersedes = Some(val.to_ascii_uppercase()),
            "liveness" => {
                liveness = match val.to_ascii_lowercase().as_str() {
                    "auto" => LivenessDeclared::Auto,
                    "n/a" | "na" => LivenessDeclared::Na,
                    other => return Err(format!("unknown liveness '{other}'")),
                };
            }
            _ => {} // unknown keys ignored (forward-compat), like spec meta
        }
    }

    let id = resolve_decision_id(id_field.as_deref(), path)
        .ok_or_else(|| "decision has no resolvable id (frontmatter id: or <letters>-<digits> filename)".to_string())?;

    Ok(KnowledgeMeta {
        kind,
        id,
        status,
        supersedes,
        liveness,
    })
}

/// Split body into `## Heading` sections (level-2 only for P1).
fn parse_sections(lines: &[&str]) -> Vec<KSection> {
    let mut sections: Vec<KSection> = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    for line in lines {
        if let Some(h) = line.strip_prefix("## ") {
            if let Some((heading, body)) = current.take() {
                sections.push(KSection {
                    heading,
                    body: body.join("\n").trim().to_string(),
                });
            }
            current = Some((h.trim().to_string(), Vec::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push((*line).to_string());
        }
    }
    if let Some((heading, body)) = current.take() {
        sections.push(KSection {
            heading,
            body: body.join("\n").trim().to_string(),
        });
    }
    sections
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test spec_knowledge::parser::tests -- --nocapture`
Expected: PASS (all 5).

- [ ] **Step 5: Re-export + commit** — in `src/spec_knowledge/mod.rs` add:

```rust
pub mod parser;
pub use parser::{parse_decision, parse_decision_str, resolve_decision_id};
```

```bash
cargo test spec_knowledge:: && git add src/spec_knowledge/
git commit -m "feat(kll): hand-written decision parser and id resolution"
```

---

## Task 3: Decision lint — required sections + forcing functions (§6.1, §9)

**Files:**
- Create: `src/spec_knowledge/lint.rs`
- Modify: `src/spec_knowledge/mod.rs`

Reuse `crate::spec_core::lint::{LintDiagnostic, Severity}` and `Span` for output shape (a planner-facing knowledge linter; not registered in the spec `LintPipeline` because decisions are not `SpecDocument`s).

- [ ] **Step 1: Write the failing tests** in `src/spec_knowledge/lint.rs`

```rust
//! Decision-artifact lint: required sections + forcing functions (§6.1, §9).

use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::model::{DecisionDoc, DecisionStatus};

const REQUIRED: [&str; 3] = ["Context", "Decision", "Consequences"];

/// Lint a single decision. Returns diagnostics (possibly empty).
pub fn lint_decision(doc: &DecisionDoc) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();
    let span = Span::default();

    // Required sections present.
    for req in REQUIRED {
        if doc.section(req).is_none() {
            out.push(LintDiagnostic {
                rule: "decision-required-section".into(),
                severity: Severity::Error,
                message: format!("decision is missing required `## {req}` section"),
                span: span.clone(),
                suggestion: Some(format!("add a `## {req}` section")),
            });
        }
    }

    // Forcing function: Accepted decisions MUST have non-empty Alternatives Considered.
    if doc.meta.status == Some(DecisionStatus::Accepted) {
        match doc.section("Alternatives Considered") {
            None => out.push(diag_error(
                "decision-accepted-needs-alternatives",
                "Accepted decision must document `## Alternatives Considered`",
                &span,
            )),
            Some(s) if s.body.trim().is_empty() => out.push(diag_error(
                "decision-accepted-needs-alternatives",
                "`## Alternatives Considered` is empty",
                &span,
            )),
            _ => {}
        }
    }

    // Forcing function: Consequences must name both a positive and a negative.
    if let Some(c) = doc.section("Consequences") {
        let body = c.body.to_ascii_lowercase();
        let has_pos = body.contains("good") || body.contains("positive") || body.contains("好处");
        let has_neg = body.contains("bad") || body.contains("negative") || body.contains("代价");
        if !(has_pos && has_neg) {
            out.push(LintDiagnostic {
                rule: "decision-consequences-both-sides".into(),
                severity: Severity::Warning,
                message: "Consequences should name both a positive and a negative outcome".into(),
                span: span.clone(),
                suggestion: Some("use 'Good, because …' and 'Bad, because …'".into()),
            });
        }
    }

    out
}

fn diag_error(rule: &str, msg: &str, span: &Span) -> LintDiagnostic {
    LintDiagnostic {
        rule: rule.into(),
        severity: Severity::Error,
        message: msg.into(),
        span: span.clone(),
        suggestion: None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::parser::parse_decision_str;
    use std::path::Path;

    fn parse(input: &str) -> DecisionDoc {
        parse_decision_str(input, Path::new("adr-001-x.md")).unwrap()
    }

    #[test]
    fn test_clean_accepted_decision_has_no_errors() {
        let doc = parse("---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood, because A. Bad, because B.\n## Alternatives Considered\nOption X — rejected because Y.\n");
        let errs: Vec<_> = lint_decision(&doc)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn test_missing_required_section_is_error() {
        let doc = parse("---\nkind: decision\nid: ADR-002\n---\n## Context\nc\n## Decision\nd\n");
        let rules: Vec<_> = lint_decision(&doc).iter().map(|d| d.rule.clone()).collect();
        assert!(rules.contains(&"decision-required-section".to_string()));
    }

    #[test]
    fn test_accepted_without_alternatives_is_error() {
        let doc = parse("---\nkind: decision\nid: ADR-003\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood. Bad.\n");
        let rules: Vec<_> = lint_decision(&doc).iter().map(|d| d.rule.clone()).collect();
        assert!(rules.contains(&"decision-accepted-needs-alternatives".to_string()));
    }
}
```

> **Note:** confirm `Span` is exported from `crate::spec_core` and `Span::default()` exists. If `Span` is not `Default`/`Clone`, replace `Span::default()` with the smallest constructor the codebase uses (check `src/spec_core/ast.rs` for the `Span` definition) and adjust `.clone()` accordingly.

- [ ] **Step 2: Run tests — verify fail**, then **Step 3: code already above**, then **Step 4: verify pass**

Run: `cargo test spec_knowledge::lint::tests -- --nocapture` → PASS.

- [ ] **Step 5: Re-export + commit**

`mod.rs`: `pub mod lint; pub use lint::lint_decision;`

```bash
cargo test spec_knowledge:: && git add src/spec_knowledge/
git commit -m "feat(kll): decision lint with required-section and forcing-function rules"
```

---

## Task 4: Add `satisfies:` edge to spec frontmatter

**Files:**
- Modify: `src/spec_core/ast.rs` (`SpecMeta`, around lines 24-41)
- Modify: `src/spec_parser/meta.rs` (`parse_meta`, around lines 4-102)

- [ ] **Step 1: Write the failing test** at the bottom of `src/spec_parser/meta.rs` (inside the existing `#[cfg(test)] mod tests`)

```rust
#[test]
fn test_parse_satisfies_array() {
    let lines = vec![
        "spec: task",
        r#"name: "X""#,
        "satisfies: [ADR-001, REQ-002]",
    ];
    let meta = parse_meta(&lines).unwrap();
    assert_eq!(meta.satisfies, vec!["ADR-001".to_string(), "REQ-002".to_string()]);
}

#[test]
fn test_satisfies_defaults_empty() {
    let lines = vec!["spec: task", r#"name: "X""#];
    let meta = parse_meta(&lines).unwrap();
    assert!(meta.satisfies.is_empty());
}
```

- [ ] **Step 2: Run — verify fail** (`meta.satisfies` does not exist).

Run: `cargo test spec_parser::meta::tests::test_parse_satisfies_array -- --nocapture` → FAIL (no field `satisfies`).

- [ ] **Step 3: Add the field** to `SpecMeta` in `src/spec_core/ast.rs` (after `capability`):

```rust
    /// Decision/requirement ids this spec satisfies (KLL §7). Normalized to
    /// UPPERCASE. Additive; empty for specs that declare none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub satisfies: Vec<String>,
```

Update **all 9** `SpecMeta { .. }` struct-literal sites to add `satisfies: Vec::new()` (most are in `#[cfg(test)]` modules but must still compile for `cargo test`): `src/spec_gateway/plan.rs` (×3), `src/spec_verify/complexity.rs`, `src/spec_verify/boundaries.rs`, `src/spec_verify/ai_verifier.rs`, `src/spec_verify/mod.rs` (×2). Find them with `rg "SpecMeta \{" src/`. Then parse the field in `parse_meta` (`src/spec_parser/meta.rs`). The file **inlines** array splitting (no `parse_array` helper) — see `src/spec_parser/meta.rs:54-71` for `tags`/`depends`; mirror that exact inline form:

```rust
            "satisfies" => {
                meta.satisfies = val
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_ascii_uppercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
```

> Ensure `satisfies` is set when the `parse_meta` `SpecMeta` builder runs (`meta.rs:88`); default `Vec::new()` at every other site.

- [ ] **Step 4: Run — verify pass**

Run: `cargo test spec_parser::meta:: -- --nocapture` → PASS. Then `cargo test` (full) to ensure no `SpecMeta` construction site was missed.

- [ ] **Step 5: Commit**

```bash
git add src/spec_core/ast.rs src/spec_parser/meta.rs
git commit -m "feat(kll): parse satisfies edge on spec frontmatter"
```

---

## Task 5: Build the satisfies reverse index

**Files:**
- Create: `src/spec_knowledge/index.rs`
- Modify: `src/spec_knowledge/mod.rs`

- [ ] **Step 1: Write the failing test** in `src/spec_knowledge/index.rs`

```rust
//! Reverse index: decision id -> spec files that declare `satisfies:` it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Map of UPPERCASE decision id -> spec paths satisfying it.
pub type SatisfiesIndex = BTreeMap<String, Vec<PathBuf>>;

/// Scan `specs_dir` recursively for `*.spec.md` / `*.spec`, parse each, and
/// index its `satisfies:` ids. Unparseable specs are skipped (best-effort).
pub fn build_satisfies_index(specs_dir: &Path) -> SatisfiesIndex {
    let mut index: SatisfiesIndex = BTreeMap::new();
    for path in spec_files(specs_dir) {
        let Ok(doc) = crate::spec_parser::parse_spec(&path) else {
            continue;
        };
        for id in &doc.meta.satisfies {
            index.entry(id.clone()).or_default().push(path.clone());
        }
    }
    index
}

fn spec_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect(&p, out);
        } else if is_spec_file(&p) {
            out.push(p);
        }
    }
}

fn is_spec_file(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    name.ends_with(".spec.md") || name.ends_with(".spec")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_index_maps_decision_to_specs() {
        let dir = std::env::temp_dir().join(format!("kll-idx-{}", std::process::id()));
        let specs = dir.join("specs");
        std::fs::create_dir_all(&specs).unwrap();
        std::fs::write(
            specs.join("task-a.spec.md"),
            "---\nspec: task\nname: \"A\"\nsatisfies: [ADR-001]\n---\n## Intent\nx\n",
        )
        .unwrap();
        std::fs::write(
            specs.join("task-b.spec.md"),
            "---\nspec: task\nname: \"B\"\n---\n## Intent\nx\n",
        )
        .unwrap();

        let idx = build_satisfies_index(&specs);
        assert_eq!(idx.get("ADR-001").map(|v| v.len()), Some(1));
        assert!(idx.get("ADR-999").is_none());

        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 2-4:** Run → fail (function not found) → the implementation is already in Step 1 → run → PASS.

Run: `cargo test spec_knowledge::index::tests -- --nocapture`

> **Note:** if `is_spec_file` trips the `unwrap_used` lint via `unwrap_or_default`, it is fine (`unwrap_or_default` is not `unwrap`). Confirm `cargo clippy` is clean.

- [ ] **Step 5: Re-export + commit**

`mod.rs`: `pub mod index; pub use index::{build_satisfies_index, SatisfiesIndex};`

```bash
cargo test spec_knowledge:: && git add src/spec_knowledge/
git commit -m "feat(kll): build satisfies reverse index over specs"
```

---

## Task 6: Liveness roll-up engine — the precedence ladder (§7)

**Files:**
- Create: `src/spec_knowledge/liveness.rs`
- Modify: `src/spec_knowledge/mod.rs`

The ladder is the most load-bearing logic. Keep it a **pure function** over verdicts so it is exhaustively unit-testable; the I/O (running verify) is a thin separate function.

- [ ] **Step 1: Write the failing tests** in `src/spec_knowledge/liveness.rs`

```rust
//! Derived liveness (§7). Never stored; computed from current spec verdicts.

use crate::spec_core::{Verdict, VerificationSummary};
use crate::spec_knowledge::model::{Liveness, LivenessDeclared};

/// Roll a single spec's verification summary into one representative verdict:
/// Fail if anything failed; Pass only if every scenario passed; otherwise a
/// not-yet-proven verdict (Skip stands in for skip/uncertain/pending/empty).
pub fn spec_rollup(summary: &VerificationSummary) -> Verdict {
    if summary.failed > 0 {
        Verdict::Fail
    } else if summary.total == 0
        || summary.skipped > 0
        || summary.uncertain > 0
        || summary.pending_review > 0
    {
        Verdict::Skip
    } else {
        Verdict::Pass
    }
}

/// Precedence ladder (§7), total and mutually exclusive:
/// 1. declared `n/a`            -> Na
/// 2. any satisfying spec Fail  -> Violated
/// 3. none, or any not-Pass     -> Unproven
/// 4. all Pass                  -> Honored
pub fn decision_liveness(declared: LivenessDeclared, spec_verdicts: &[Verdict]) -> Liveness {
    if declared == LivenessDeclared::Na {
        return Liveness::Na;
    }
    if spec_verdicts.iter().any(|v| *v == Verdict::Fail) {
        return Liveness::Violated;
    }
    if spec_verdicts.is_empty() || spec_verdicts.iter().any(|v| *v != Verdict::Pass) {
        return Liveness::Unproven;
    }
    Liveness::Honored
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn summary(total: usize, passed: usize, failed: usize, skipped: usize) -> VerificationSummary {
        VerificationSummary {
            total,
            passed,
            failed,
            skipped,
            uncertain: 0,
            pending_review: 0,
        }
    }

    #[test]
    fn test_spec_rollup_fail_dominates() {
        assert_eq!(spec_rollup(&summary(3, 2, 1, 0)), Verdict::Fail);
    }

    #[test]
    fn test_spec_rollup_all_pass() {
        assert_eq!(spec_rollup(&summary(2, 2, 0, 0)), Verdict::Pass);
    }

    #[test]
    fn test_spec_rollup_skip_is_not_pass() {
        assert_eq!(spec_rollup(&summary(2, 1, 0, 1)), Verdict::Skip);
        assert_eq!(spec_rollup(&summary(0, 0, 0, 0)), Verdict::Skip);
    }

    #[test]
    fn test_liveness_na_short_circuits() {
        assert_eq!(
            decision_liveness(LivenessDeclared::Na, &[Verdict::Fail]),
            Liveness::Na
        );
    }

    #[test]
    fn test_liveness_violated_on_any_fail() {
        assert_eq!(
            decision_liveness(LivenessDeclared::Auto, &[Verdict::Pass, Verdict::Fail]),
            Liveness::Violated
        );
    }

    #[test]
    fn test_liveness_unproven_when_empty_or_not_all_pass() {
        assert_eq!(decision_liveness(LivenessDeclared::Auto, &[]), Liveness::Unproven);
        assert_eq!(
            decision_liveness(LivenessDeclared::Auto, &[Verdict::Pass, Verdict::Skip]),
            Liveness::Unproven
        );
    }

    #[test]
    fn test_liveness_honored_when_all_pass() {
        assert_eq!(
            decision_liveness(LivenessDeclared::Auto, &[Verdict::Pass, Verdict::Pass]),
            Liveness::Honored
        );
    }
}
```

- [ ] **Step 2-4:** Run → fail → implementation is in Step 1 → run → PASS (8 tests).

Run: `cargo test spec_knowledge::liveness::tests -- --nocapture`

> **Note:** `Verdict` and `VerificationSummary` are re-exported at `crate::spec_core::` (the `verify` submodule is private — `src/spec_core/mod.rs`). Field names match `src/spec_core/verify.rs:131-139` exactly (`total/passed/failed/skipped/uncertain/pending_review`).

- [ ] **Step 5: Re-export + commit**

`mod.rs`: `pub mod liveness; pub use liveness::{decision_liveness, spec_rollup};`

```bash
cargo test spec_knowledge:: && git add src/spec_knowledge/
git commit -m "feat(kll): deterministic liveness precedence ladder"
```

---

## Task 7: `trace` report builder (compute liveness for a decision)

**Files:**
- Create: `src/spec_knowledge/trace.rs`
- Modify: `src/spec_knowledge/mod.rs`

Ties Tasks 2/5/6 together: load the decision, find satisfying specs, run `SpecGateway::verify` on each, roll up, format. This is the engine behind the `trace` CLI command (Task 8).

- [ ] **Step 1: Write the report types + builder + formatter** in `src/spec_knowledge/trace.rs`

```rust
//! `trace <decision-id>` report: satisfying specs, their verdicts, liveness.

use crate::spec_core::Verdict;
use crate::spec_knowledge::index::SatisfiesIndex;
use crate::spec_knowledge::liveness::{decision_liveness, spec_rollup};
use crate::spec_knowledge::model::{DecisionDoc, Liveness, LivenessDeclared};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SpecVerdict {
    pub spec: PathBuf,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceReport {
    pub decision_id: String,
    pub declared: LivenessDeclared,
    pub specs: Vec<SpecVerdict>,
    pub liveness: Liveness,
}

/// Build a trace report. `verify_fn` runs verification for one spec path and
/// returns its rolled-up verdict — injected so the builder is unit-testable
/// without invoking cargo test.
pub fn build_trace<F>(
    decision: &DecisionDoc,
    index: &SatisfiesIndex,
    mut verify_fn: F,
) -> TraceReport
where
    F: FnMut(&Path) -> Verdict,
{
    let specs: Vec<SpecVerdict> = index
        .get(&decision.meta.id)
        .map(|paths| {
            paths
                .iter()
                .map(|p| SpecVerdict {
                    spec: p.clone(),
                    verdict: verify_fn(p),
                })
                .collect()
        })
        .unwrap_or_default();

    let verdicts: Vec<Verdict> = specs.iter().map(|s| s.verdict).collect();
    let liveness = decision_liveness(decision.meta.liveness, &verdicts);

    TraceReport {
        decision_id: decision.meta.id.clone(),
        declared: decision.meta.liveness,
        specs,
        liveness,
    }
}

/// Default verify function used by the CLI: run the gateway and roll up.
pub fn verify_spec_rollup(spec_path: &Path, code_path: &Path) -> Verdict {
    match crate::spec_gateway::SpecGateway::load(spec_path) {
        Ok(gw) => match gw.verify(code_path) {
            Ok(report) => spec_rollup(&report.summary),
            Err(_) => Verdict::Uncertain,
        },
        Err(_) => Verdict::Uncertain,
    }
}

pub fn format_trace_text(r: &TraceReport) -> String {
    let mut s = format!("decision {}  liveness={:?}\n", r.decision_id, r.liveness);
    if r.specs.is_empty() {
        s.push_str("  (no spec satisfies this decision)\n");
    }
    for sv in &r.specs {
        s.push_str(&format!("  [{:?}] {}\n", sv.verdict, sv.spec.display()));
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::model::{DecisionStatus, KnowledgeKind, KnowledgeMeta};
    use std::collections::BTreeMap;

    fn decision(id: &str, declared: LivenessDeclared) -> DecisionDoc {
        DecisionDoc {
            meta: KnowledgeMeta {
                kind: KnowledgeKind::Decision,
                id: id.into(),
                status: Some(DecisionStatus::Accepted),
                supersedes: None,
                liveness: declared,
            },
            sections: vec![],
            source_path: PathBuf::new(),
        }
    }

    #[test]
    fn test_trace_honored_when_all_specs_pass() {
        let mut idx: SatisfiesIndex = BTreeMap::new();
        idx.insert("ADR-001".into(), vec![PathBuf::from("specs/a.spec.md")]);
        let r = build_trace(&decision("ADR-001", LivenessDeclared::Auto), &idx, |_| Verdict::Pass);
        assert_eq!(r.liveness, Liveness::Honored);
    }

    #[test]
    fn test_trace_unproven_when_no_satisfying_spec() {
        let idx: SatisfiesIndex = BTreeMap::new();
        let r = build_trace(&decision("ADR-002", LivenessDeclared::Auto), &idx, |_| Verdict::Pass);
        assert_eq!(r.liveness, Liveness::Unproven);
        assert!(r.specs.is_empty());
    }

    #[test]
    fn test_trace_violated_when_a_spec_fails() {
        let mut idx: SatisfiesIndex = BTreeMap::new();
        idx.insert("ADR-003".into(), vec![PathBuf::from("specs/a.spec.md")]);
        let r = build_trace(&decision("ADR-003", LivenessDeclared::Auto), &idx, |_| Verdict::Fail);
        assert_eq!(r.liveness, Liveness::Violated);
    }
}
```

- [ ] **Step 2-4:** Run → fail → code already above → run → PASS.

Run: `cargo test spec_knowledge::trace::tests -- --nocapture`

> **Note:** confirm `SpecGateway` is reachable as `crate::spec_gateway::SpecGateway` (per `spec_gateway/mod.rs` re-export). Adjust the path if it is `crate::spec_gateway::lifecycle::SpecGateway`.

- [ ] **Step 5: Re-export + commit**

`mod.rs`: `pub mod trace; pub use trace::{build_trace, format_trace_text, verify_spec_rollup, TraceReport};`

```bash
cargo test spec_knowledge:: && git add src/spec_knowledge/
git commit -m "feat(kll): trace report builder with injectable verify"
```

---

## Task 8: `trace` CLI command + gate exit code

**Files:**
- Modify: `src/main.rs` (`Commands` enum, `run()` dispatch, new handler `cmd_trace`)

P1 gate policy (built-in defaults; config parsing deferred to P2): `--gate` makes a `violated` decision exit non-zero (error); `unproven` prints a warning but exits 0 (gate-green-day-one); `honored`/`n/a` exit 0.

- [ ] **Step 1: Add the `Trace` variant** to the `Commands` enum (`src/main.rs:37-328`)

```rust
    /// Trace a decision to the specs that satisfy it and report liveness.
    Trace {
        /// Decision id (e.g. ADR-001), case-insensitive.
        id: String,
        /// Knowledge root (decisions live under <knowledge>/decisions).
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        /// Specs root.
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        /// Code directory to verify against.
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Output format: text | json.
        #[arg(long, default_value = "text")]
        format: String,
        /// Exit non-zero when the decision is violated.
        #[arg(long)]
        gate: bool,
    },
```

- [ ] **Step 2: Add the dispatch arm** in `run()` (`src/main.rs:342-459`)

```rust
        Commands::Trace { id, knowledge, specs, code, format, gate } => {
            cmd_trace(&id, &knowledge, &specs, &code, &format, gate)
        }
```

- [ ] **Step 3: Implement `cmd_trace`** (near the other `cmd_*` handlers)

```rust
fn cmd_trace(
    id: &str,
    knowledge: &Path,
    specs: &Path,
    code: &Path,
    format: &str,
    gate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::spec_knowledge::model::Liveness;

    let target = id.to_ascii_uppercase();

    // Find the decision file under <knowledge>/decisions whose resolved id matches.
    let decisions_dir = knowledge.join("decisions");
    let decision = find_decision(&decisions_dir, &target)?;

    let index = crate::spec_knowledge::build_satisfies_index(specs);
    let report = crate::spec_knowledge::build_trace(&decision, &index, |spec_path| {
        crate::spec_knowledge::trace::verify_spec_rollup(spec_path, code)
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => print!("{}", crate::spec_knowledge::format_trace_text(&report)),
    }

    if gate {
        match report.liveness {
            Liveness::Violated => {
                eprintln!("gate: decision {} is VIOLATED", report.decision_id);
                std::process::exit(2);
            }
            Liveness::Unproven => {
                eprintln!("gate (warning): decision {} is UNPROVEN", report.decision_id);
            }
            Liveness::Honored | Liveness::Na => {}
        }
    }
    Ok(())
}

fn find_decision(
    dir: &Path,
    target_id: &str,
) -> Result<crate::spec_knowledge::DecisionDoc, Box<dyn std::error::Error>> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if !(name.ends_with(".md") || name.ends_with(".spec")) {
            continue;
        }
        if let Ok(doc) = crate::spec_knowledge::parse_decision(&p) {
            if doc.meta.id == target_id {
                return Ok(doc);
            }
        }
    }
    Err(format!("no decision with id {target_id} in {}", dir.display()).into())
}
```

- [ ] **Step 4: Build + manual smoke test**

```bash
cargo build
# Create a tiny fixture
mkdir -p /tmp/kll/knowledge/decisions /tmp/kll/specs
printf -- '---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood. Bad.\n## Alternatives Considered\nX.\n' > /tmp/kll/knowledge/decisions/adr-001-x.md
printf -- '---\nspec: task\nname: "A"\nsatisfies: [ADR-001]\n---\n## Intent\nx\n' > /tmp/kll/specs/task-a.spec.md
cargo run -- trace ADR-001 --knowledge /tmp/kll/knowledge --specs /tmp/kll/specs --code /tmp/kll --gate
```

Expected: prints `decision ADR-001  liveness=Unproven` (the spec has no bound tests → `Skip` rollup → `Unproven`), gate prints an UNPROVEN warning, exit code 0. `cargo run -- trace ADR-001 ... --format json` prints the JSON report.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs && git commit -m "feat(kll): trace CLI command with gate exit code"
```

---

## Task 9: `init --workspace` scaffold

**Files:**
- Modify: `src/main.rs` (`Init` variant gets `--workspace`; dispatch + handler)
- Create: `src/spec_knowledge/scaffold.rs`
- Modify: `src/spec_knowledge/mod.rs`

Idempotent: create only what is missing; never overwrite. Lays down the canonical tree (§11) — at least `knowledge/decisions/` (README + `adr-template.md`), placeholder `knowledge/guidance/`, `knowledge/context/`, `knowledge/standards/canon/artifact-types.md`, and `.agent-spec/config.yaml`.

- [ ] **Step 1: Write the failing test** in `src/spec_knowledge/scaffold.rs`

```rust
//! `init --workspace` scaffold (§11). Idempotent: create-if-missing only.

use std::io;
use std::path::Path;

/// Files created by the workspace scaffold, relative to root.
const FILES: &[(&str, &str)] = &[
    ("knowledge/decisions/README.md", DECISIONS_README),
    ("knowledge/decisions/adr-template.md", ADR_TEMPLATE),
    ("knowledge/guidance/README.md", GUIDANCE_README),
    ("knowledge/context/README.md", CONTEXT_README),
    ("knowledge/standards/canon/artifact-types.md", ARTIFACT_TYPES),
    (".agent-spec/config.yaml", CONFIG_YAML),
];

/// Create the canonical workspace tree under `root`. Returns the list of
/// paths actually created (skips existing files).
pub fn scaffold_workspace(root: &Path) -> io::Result<Vec<String>> {
    let mut created = Vec::new();
    for (rel, contents) in FILES {
        let path = root.join(rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
        created.push((*rel).to_string());
    }
    Ok(created)
}

const DECISIONS_README: &str = "# Decisions\n\nMADR-style decision records. One decision per file, `NNNNN-slug.md`.\nWhen NOT to use: routine implementation choices with no real trade-off — leave those in code/comments.\n";
const ADR_TEMPLATE: &str = "---\nkind: decision\nid: ADR-NNN\nstatus: Proposed\n---\n\n## Context\n\n## Decision\n\n## Consequences\n\nGood, because …\nBad, because …\n\n## Alternatives Considered\n";
const GUIDANCE_README: &str = "# Guidance\n\n[P2] Agent-facing guidance + skill designation (typed, governance tier). Empty in P1.\n";
const CONTEXT_README: &str = "# Context (free-form)\n\nEscape hatch: arbitrary agent-context. Served read-only, NOT linted, no schema.\n";
const ARTIFACT_TYPES: &str = "# Artifact types (canon)\n\nDecision (P1): required `## Context · ## Decision · ## Consequences`; recommended `## Status · ## Category · ## Alternatives Considered`; `## Supersedes`.\nThis canon documents the schema the lint enforces. It is exempt from artifact lint.\n";
const CONFIG_YAML: &str = "paths:\n  knowledge: knowledge\n  specs: specs\nliveness:\n  gate:\n    violated: error\n    unproven: warning\n";

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_scaffold_is_idempotent() {
        let root = std::env::temp_dir().join(format!("kll-scaffold-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();

        let first = scaffold_workspace(&root).unwrap();
        assert!(first.iter().any(|p| p == "knowledge/decisions/adr-template.md"));
        assert!(root.join(".agent-spec/config.yaml").exists());

        // Second run creates nothing.
        let second = scaffold_workspace(&root).unwrap();
        assert!(second.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }
}
```

- [ ] **Step 2-4:** Run → fail → code above → run → PASS.

Run: `cargo test spec_knowledge::scaffold::tests -- --nocapture`

- [ ] **Step 5a: Wire `--workspace` into `init`** — add flag to the `Init` variant (`src/main.rs:161-174`):

```rust
        /// Scaffold the canonical KLL workspace tree instead of a single spec.
        #[arg(long)]
        workspace: bool,
```

Update the dispatch arm for `Init` to pass `workspace`, and at the top of `cmd_init` short-circuit:

```rust
    if workspace {
        let root = std::env::current_dir()?;
        let created = crate::spec_knowledge::scaffold::scaffold_workspace(&root)?;
        if created.is_empty() {
            println!("workspace already scaffolded (nothing to do)");
        } else {
            for p in created {
                println!("created {p}");
            }
        }
        return Ok(());
    }
```

(Add `workspace: bool` **only to `cmd_init`** at `src/main.rs:2197` and short-circuit there *before* it delegates to `cmd_init_at`; leave `cmd_init_at` at `src/main.rs:2207` untouched — otherwise it gets an unused param. Update the `Init` dispatch arm in `run()` to pass `workspace` into `cmd_init`.)

- [ ] **Step 5b: Re-export, build, smoke test, commit**

`mod.rs`: `pub mod scaffold; pub use scaffold::scaffold_workspace;`

```bash
cargo build
cd /tmp && rm -rf kll-ws && mkdir kll-ws && cd kll-ws && cargo run --manifest-path <repo>/Cargo.toml -- init --workspace && find . -type f
```

Expected: prints `created knowledge/decisions/...` etc.; second run prints "already scaffolded".

```bash
git add src/main.rs src/spec_knowledge/ && git commit -m "feat(kll): init --workspace scaffold (idempotent)"
```

---

## Final verification (maps to design §15 P1 acceptance)

- [ ] `cargo test` — all green; `cargo clippy --all-targets` — no `unwrap_used`/`expect_used` violations.
- [ ] `agent-spec init --workspace` idempotently generates the tree (Task 9).
- [ ] A `decision` artifact parses and lints (required sections + forcing functions) (Tasks 2-3).
- [ ] A spec's `satisfies:` is parsed; the reverse index maps decision → specs (Tasks 4-5).
- [ ] `agent-spec trace <id>` prints satisfying specs + verdicts + liveness; `--gate` exits non-zero on `violated`, warns on `unproven` (Tasks 6-8).
- [ ] No existing spec's verdict changed (run the pre-existing test suite; additive only).

## Deferred to P2/P3 (explicitly NOT in this plan)

`requirement`/`guidance`/`proposal` kinds; MCP server; `.agent-spec/config.yaml` *parsing* (P1 uses built-in gate defaults; the file is scaffolded but not read); EARS/BCP-14/29148 lint; supersession-integrity lint; SARIF output; deeper `guard`/`lifecycle` integration; scenario/rule-level `Satisfies`.
