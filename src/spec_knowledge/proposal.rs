//! Proposal artifacts (KLL P3, §6.3). An independent governance type: MADR-
//! shaped, `liveness` always `n/a` (never enters the code gate), and a
//! `## Produces` edge linking to the decisions/requirements it spawned — closing
//! the LEP→ADR handoff that Lore left as an open question.

use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::model::{KnowledgeDoc, LivenessDeclared};

const REQUIRED: [&str; 3] = ["Context", "Decision", "Consequences"];

/// Ids a proposal declares it `## Produces` (heading `## Produces` with a body
/// list, or the inline form `## Produces: ADR-001`). UPPERCASE, de-duplicated.
pub fn produces(doc: &KnowledgeDoc) -> Vec<String> {
    let mut out = Vec::new();
    for section in &doc.sections {
        if !section.heading.to_ascii_lowercase().starts_with("produces") {
            continue;
        }
        // Scan both the heading (inline form) and the body (list form).
        for src in [section.heading.as_str(), section.body.as_str()] {
            for tok in src.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
                if is_id_token(tok) {
                    let up = tok.to_ascii_uppercase();
                    if !out.contains(&up) {
                        out.push(up);
                    }
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

/// Lint a proposal: required MADR sections, the `liveness: n/a` forcing
/// function, and a non-empty `## Produces` edge.
pub fn lint_proposal(doc: &KnowledgeDoc) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();

    for req in REQUIRED {
        if doc.section(req).is_none() {
            out.push(diag(
                "proposal-required-section",
                Severity::Error,
                format!("proposal is missing required `## {req}` section"),
            ));
        }
    }

    if doc.meta.liveness != LivenessDeclared::Na {
        out.push(diag(
            "proposal-liveness-na",
            Severity::Warning,
            "proposal should declare `liveness: n/a` (it never enters the code gate)".into(),
        ));
    }

    if produces(doc).is_empty() {
        out.push(diag(
            "proposal-no-produces",
            Severity::Info,
            "proposal declares no `## Produces:` edge to a decision/requirement".into(),
        ));
    }

    out
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::parser::parse_knowledge_str;
    use std::path::Path;

    fn parse(input: &str) -> KnowledgeDoc {
        parse_knowledge_str(input, Path::new("lep-001-x.md")).unwrap()
    }

    #[test]
    fn test_produces_inline_and_list() {
        let inline = parse(
            "---\nkind: proposal\nid: LEP-001\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Produces: ADR-007\n",
        );
        assert_eq!(produces(&inline), vec!["ADR-007".to_string()]);

        let list = parse(
            "---\nkind: proposal\nid: LEP-002\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Produces\n- ADR-008\n- REQ-009\n",
        );
        assert_eq!(
            produces(&list),
            vec!["ADR-008".to_string(), "REQ-009".to_string()]
        );
    }

    #[test]
    fn test_clean_proposal_has_no_errors() {
        let doc = parse(
            "---\nkind: proposal\nid: LEP-001\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Produces: ADR-007\n",
        );
        let errs: Vec<_> = lint_proposal(&doc)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn test_proposal_missing_section_and_bad_liveness() {
        let doc =
            parse("---\nkind: proposal\nid: LEP-003\n---\n## Context\nc\n## Produces: ADR-001\n");
        let rules: Vec<_> = lint_proposal(&doc).iter().map(|d| d.rule.clone()).collect();
        assert!(rules.contains(&"proposal-required-section".to_string()));
        assert!(rules.contains(&"proposal-liveness-na".to_string()));
    }
}
