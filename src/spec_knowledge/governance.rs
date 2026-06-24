//! Corpus-level governance lint (§9): id-conflict, supersession integrity, and
//! stale references across the whole knowledge set. Per-doc section/forcing-
//! function rules live in `lint`; this module needs every doc at once.
//!
//! Self-referential exemption (§9) is handled by the collectors that feed this:
//! they scan only the typed kind directories, never `standards/**` or README.

use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::lint::{lint_decision, lint_guidance, lint_requirement};
use crate::spec_knowledge::model::{DecisionStatus, KnowledgeDoc, KnowledgeKind};
use std::collections::BTreeMap;

/// Per-document lint dispatched by kind. Proposals reuse the decision rules
/// (same MADR shape); their dedicated `Produces`-edge checks are added in P3.
pub fn lint_doc(doc: &KnowledgeDoc) -> Vec<LintDiagnostic> {
    match doc.meta.kind {
        KnowledgeKind::Decision => lint_decision(doc),
        KnowledgeKind::Requirement => lint_requirement(doc),
        KnowledgeKind::Guidance => lint_guidance(doc),
        KnowledgeKind::Proposal => lint_decision(doc),
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
