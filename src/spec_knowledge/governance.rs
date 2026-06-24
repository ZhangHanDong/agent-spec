//! Corpus-level governance lint (§9): id-conflict, supersession integrity, and
//! stale references across the whole knowledge set. Per-doc section/forcing-
//! function rules live in `lint`; this module needs every doc at once.
//!
//! Self-referential exemption (§9) is handled by the collectors that feed this:
//! they scan only the typed kind directories, never `standards/**` or README.

use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::lint::{lint_decision, lint_guidance, lint_requirement};
use crate::spec_knowledge::model::{DecisionStatus, KnowledgeDoc, KnowledgeKind};
use crate::spec_knowledge::parser::parse_knowledge;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Collect every typed knowledge doc under `knowledge_dir` (the four kind
/// directories), skipping README files and unparseable docs. Sorted by id.
/// `standards/**` is never scanned — that is the §9 self-referential exemption.
pub fn collect_knowledge(knowledge_dir: &Path) -> Vec<KnowledgeDoc> {
    const KINDS: [KnowledgeKind; 4] = [
        KnowledgeKind::Decision,
        KnowledgeKind::Requirement,
        KnowledgeKind::Guidance,
        KnowledgeKind::Proposal,
    ];
    let mut files = Vec::new();
    for kind in KINDS {
        collect_md(&knowledge_dir.join(kind.dir()), &mut files);
    }
    files.sort();
    let mut docs: Vec<KnowledgeDoc> = files
        .iter()
        .filter_map(|p| parse_knowledge(p).ok())
        .collect();
    docs.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
    docs
}

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_md(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            // README and `*-template.md` are scaffolding, not artifacts.
            if name != "README.md" && !name.ends_with("-template.md") {
                out.push(p);
            }
        }
    }
}

/// Per-document lint dispatched by kind.
pub fn lint_doc(doc: &KnowledgeDoc) -> Vec<LintDiagnostic> {
    match doc.meta.kind {
        KnowledgeKind::Decision => lint_decision(doc),
        KnowledgeKind::Requirement => lint_requirement(doc),
        KnowledgeKind::Guidance => lint_guidance(doc),
        KnowledgeKind::Proposal => crate::spec_knowledge::proposal::lint_proposal(doc),
    }
}

fn diag(rule: &str, severity: Severity, msg: String) -> LintDiagnostic {
    LintDiagnostic {
        rule: rule.into(),
        severity,
        message: msg,
        span: Span::default(),
        suggestion: None,
    }
}

/// Lint the whole corpus: id conflicts, supersession integrity, stale refs.
pub fn lint_corpus(docs: &[KnowledgeDoc]) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();

    // id -> docs holding it (for conflict detection).
    let mut by_id: BTreeMap<&str, Vec<&KnowledgeDoc>> = BTreeMap::new();
    for d in docs {
        by_id.entry(d.meta.id.as_str()).or_default().push(d);
    }

    // §6.0 conflict: two files resolving to the same id is an error.
    for (id, holders) in &by_id {
        if holders.len() > 1 {
            out.push(diag(
                "knowledge-id-conflict",
                Severity::Error,
                format!("id {id} is declared by {} files", holders.len()),
            ));
        }
    }

    let superseded: std::collections::BTreeSet<&str> = docs
        .iter()
        .filter(|d| d.meta.status == Some(DecisionStatus::Superseded))
        .map(|d| d.meta.id.as_str())
        .collect();

    for d in docs {
        // Supersession integrity: the `supersedes` target must exist and be marked.
        if let Some(target) = &d.meta.supersedes {
            match by_id.get(target.as_str()) {
                None => out.push(diag(
                    "supersession-dangling",
                    Severity::Error,
                    format!("{} supersedes {target}, which does not exist", d.meta.id),
                )),
                Some(holders) => {
                    let marked = holders
                        .iter()
                        .any(|h| h.meta.status == Some(DecisionStatus::Superseded));
                    if !marked {
                        out.push(diag(
                            "supersession-target-not-marked",
                            Severity::Warning,
                            format!(
                                "{} supersedes {target}, but {target} is not marked `status: superseded`",
                                d.meta.id
                            ),
                        ));
                    }
                }
            }
        }

        // Stale reference: a doc body should not point at a superseded id
        // (unless it is the very doc that supersedes it).
        let own_target = d.meta.supersedes.as_deref();
        for refid in referenced_ids(d) {
            if superseded.contains(refid.as_str()) && Some(refid.as_str()) != own_target {
                out.push(diag(
                    "references-superseded",
                    Severity::Warning,
                    format!("{} references superseded id {refid}", d.meta.id),
                ));
            }
        }

        // Produces integrity: a proposal's produced ids should exist (§6.3).
        if d.meta.kind == KnowledgeKind::Proposal {
            for produced in crate::spec_knowledge::proposal::produces(d) {
                if !by_id.contains_key(produced.as_str()) {
                    out.push(diag(
                        "produces-dangling",
                        Severity::Warning,
                        format!("{} produces {produced}, which does not exist", d.meta.id),
                    ));
                }
            }
        }
    }

    out
}

/// Scan a doc's section bodies for `LETTERS-DIGITS` id tokens (e.g. ADR-001),
/// excluding the doc's own id. De-duplicated, in first-seen order.
fn referenced_ids(doc: &KnowledgeDoc) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for section in &doc.sections {
        for token in section
            .body
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        {
            if is_id_token(token) {
                let up = token.to_ascii_uppercase();
                if up != doc.meta.id && seen.insert(up.clone()) {
                    out.push(up);
                }
            }
        }
    }
    out
}

fn is_id_token(t: &str) -> bool {
    let Some((letters, digits)) = t.split_once('-') else {
        return false;
    };
    !letters.is_empty()
        && letters.chars().all(|c| c.is_ascii_alphabetic())
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::parser::parse_knowledge_str;
    use std::path::Path;

    fn parse(input: &str, name: &str) -> KnowledgeDoc {
        parse_knowledge_str(input, Path::new(name)).unwrap()
    }

    #[test]
    fn test_id_conflict_is_error() {
        let a = parse(
            "---\nkind: decision\nid: ADR-001\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "a.md",
        );
        let b = parse(
            "---\nkind: decision\nid: ADR-001\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "b.md",
        );
        let rules: Vec<_> = lint_corpus(&[a, b])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"knowledge-id-conflict".to_string()));
    }

    #[test]
    fn test_dangling_and_unmarked_supersession() {
        // ADR-002 supersedes ADR-001 which exists but is NOT marked superseded.
        let old = parse(
            "---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-001.md",
        );
        let new = parse(
            "---\nkind: decision\nid: ADR-002\nstatus: accepted\nsupersedes: ADR-001\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-002.md",
        );
        let rules: Vec<_> = lint_corpus(&[old, new])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"supersession-target-not-marked".to_string()));

        // Dangling: supersedes a non-existent id.
        let lone = parse(
            "---\nkind: decision\nid: ADR-003\nsupersedes: ADR-099\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-003.md",
        );
        let rules: Vec<_> = lint_corpus(&[lone])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"supersession-dangling".to_string()));
    }

    #[test]
    fn test_produces_dangling_warns() {
        let prop = parse(
            "---\nkind: proposal\nid: LEP-001\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Produces: ADR-404\n",
            "lep-001.md",
        );
        let rules: Vec<_> = lint_corpus(&[prop])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"produces-dangling".to_string()));
    }

    #[test]
    fn test_references_superseded_warns() {
        let dead = parse(
            "---\nkind: decision\nid: ADR-001\nstatus: superseded\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-001.md",
        );
        let cites = parse(
            "---\nkind: decision\nid: ADR-005\n---\n## Context\nas decided in ADR-001\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-005.md",
        );
        let rules: Vec<_> = lint_corpus(&[dead, cites])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"references-superseded".to_string()));
    }
}
