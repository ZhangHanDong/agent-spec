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
                span,
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
                span,
            )),
            Some(s) if s.body.trim().is_empty() => out.push(diag_error(
                "decision-accepted-needs-alternatives",
                "`## Alternatives Considered` is empty",
                span,
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
                span,
                suggestion: Some("use 'Good, because …' and 'Bad, because …'".into()),
            });
        }
    }

    out
}

fn diag_error(rule: &str, msg: &str, span: Span) -> LintDiagnostic {
    LintDiagnostic {
        rule: rule.into(),
        severity: Severity::Error,
        message: msg.into(),
        span,
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
        let doc = parse(
            "---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood, because A. Bad, because B.\n## Alternatives Considered\nOption X — rejected because Y.\n",
        );
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
        let doc = parse(
            "---\nkind: decision\nid: ADR-003\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood. Bad.\n",
        );
        let rules: Vec<_> = lint_decision(&doc).iter().map(|d| d.rule.clone()).collect();
        assert!(rules.contains(&"decision-accepted-needs-alternatives".to_string()));
    }
}
