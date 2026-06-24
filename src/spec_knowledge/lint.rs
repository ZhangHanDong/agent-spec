//! Knowledge-artifact lint: required sections + forcing functions (§6.1, §6.2, §9).

use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::model::{DecisionDoc, DecisionStatus, KnowledgeDoc};
use crate::spec_knowledge::requirement::{extract_requirements, normative_token_count};

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

fn diag(rule: &str, severity: Severity, msg: String, suggestion: Option<&str>) -> LintDiagnostic {
    LintDiagnostic {
        rule: rule.into(),
        severity,
        message: msg,
        span: Span::default(),
        suggestion: suggestion.map(|s| s.to_string()),
    }
}

const REQUIREMENT_REQUIRED: [&str; 2] = ["Problem", "Requirements"];

/// Lint a requirement artifact (§6.2): required sections plus the toggleable
/// quality rules (BCP-14 keywords, ISO/IEC/IEEE 29148 single-statement, EARS
/// shape). Quality rules emit at Warning/Info so the gate stays green day one.
pub fn lint_requirement(doc: &KnowledgeDoc) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();

    for req in REQUIREMENT_REQUIRED {
        if doc.section(req).is_none() {
            out.push(diag(
                "requirement-required-section",
                Severity::Error,
                format!("requirement is missing required `## {req}` section"),
                Some("add the section"),
            ));
        }
    }

    let clauses = extract_requirements(doc);
    if doc.section("Requirements").is_some() && clauses.is_empty() {
        out.push(diag(
            "requirement-empty",
            Severity::Error,
            "`## Requirements` declares no clauses".into(),
            Some("add `[REQ-NNN] … MUST/SHOULD/MAY …` lines"),
        ));
    }

    for (i, clause) in clauses.iter().enumerate() {
        let n = i + 1;
        // BCP-14: every normative clause should carry a keyword.
        if clause.keyword.is_none() {
            out.push(diag(
                "requirement-bcp14-keyword",
                Severity::Warning,
                format!("requirement clause {n} has no BCP-14 keyword (MUST/SHOULD/MAY)"),
                Some("use an RFC 2119/8174 keyword in UPPERCASE"),
            ));
        }
        // ISO/IEC/IEEE 29148: one requirement per statement.
        if normative_token_count(&clause.text) > 1 {
            out.push(diag(
                "requirement-single-statement",
                Severity::Warning,
                format!("requirement clause {n} bundles multiple normative statements"),
                Some("split into atomic `[REQ-NNN]` clauses (ISO/IEC/IEEE 29148)"),
            ));
        }
        // Traceability: recommend an explicit id.
        if clause.id.is_none() {
            out.push(diag(
                "requirement-needs-id",
                Severity::Info,
                format!("requirement clause {n} has no `[REQ-NNN]` id"),
                Some("prefix the clause with `[REQ-NNN]` for traceability"),
            ));
        }
        // EARS: a normative clause needs a subject before the keyword.
        if clause.keyword.is_some() && subject_before_keyword(&clause.text).is_empty() {
            out.push(diag(
                "requirement-ears-subject",
                Severity::Warning,
                format!("requirement clause {n} has no subject before its keyword"),
                Some("use an EARS shape, e.g. `The <system> MUST <response>`"),
            ));
        }
    }

    out
}

const GUIDANCE_REQUIRED: [&str; 2] = ["Scope", "Instructions"];

/// Lint a guidance artifact (§6.4): required sections, plus a forcing function
/// that guidance never enters the code gate (`liveness` must be `n/a`).
pub fn lint_guidance(doc: &KnowledgeDoc) -> Vec<LintDiagnostic> {
    use crate::spec_knowledge::model::LivenessDeclared;
    let mut out = Vec::new();

    for req in GUIDANCE_REQUIRED {
        if doc.section(req).is_none() {
            out.push(diag(
                "guidance-required-section",
                Severity::Error,
                format!("guidance is missing required `## {req}` section"),
                Some("add the section"),
            ));
        }
    }

    if doc.meta.liveness != LivenessDeclared::Na {
        out.push(diag(
            "guidance-liveness-na",
            Severity::Warning,
            "guidance should declare `liveness: n/a` (it never enters the code gate)".into(),
            Some("set `liveness: n/a` in the front-matter"),
        ));
    }

    out
}

/// The text preceding the first BCP-14 keyword (the EARS "subject"), trimmed.
fn subject_before_keyword(text: &str) -> String {
    const NEEDLES: &[&str] = &[
        "MUST",
        "SHALL",
        "SHOULD",
        "MAY",
        "REQUIRED",
        "RECOMMENDED",
        "OPTIONAL",
    ];
    let first = NEEDLES.iter().filter_map(|n| text.find(n)).min();
    match first {
        Some(pos) => text[..pos].trim().to_string(),
        None => text.trim().to_string(),
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

    // ---- requirement lint (§6.2) ----

    fn parse_req(input: &str) -> KnowledgeDoc {
        crate::spec_knowledge::parser::parse_requirement_str(input, Path::new("req-001-x.md"))
            .unwrap()
    }

    #[test]
    fn test_clean_requirement_has_no_errors() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-001\n---\n## Problem\np\n## Requirements\n[REQ-001] The API MUST return 429 on rate limit.\n",
        );
        let errs: Vec<_> = lint_requirement(&doc)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn test_requirement_missing_problem_is_error() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-002\n---\n## Requirements\n[REQ-002] The API MUST do x.\n",
        );
        let rules: Vec<_> = lint_requirement(&doc)
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"requirement-required-section".to_string()));
    }

    #[test]
    fn test_requirement_compound_and_no_keyword_warn() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-003\n---\n## Problem\np\n## Requirements\n[REQ-003] The API MUST log and the client MUST retry.\nthe cache is warmed at boot.\n",
        );
        let rules: Vec<_> = lint_requirement(&doc)
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"requirement-single-statement".to_string()));
        assert!(rules.contains(&"requirement-bcp14-keyword".to_string()));
    }

    // ---- guidance lint (§6.4) ----

    fn parse_guidance(input: &str) -> KnowledgeDoc {
        crate::spec_knowledge::parser::parse_knowledge_str(input, Path::new("g-001-x.md")).unwrap()
    }

    #[test]
    fn test_clean_guidance_has_no_errors() {
        let doc = parse_guidance(
            "---\nkind: guidance\nid: G-001\nliveness: n/a\n---\n## Scope\nrust modules\n## Instructions\nprefer ? over unwrap\n",
        );
        let errs: Vec<_> = lint_guidance(&doc)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn test_guidance_missing_section_and_bad_liveness() {
        let doc = parse_guidance("---\nkind: guidance\nid: G-002\n---\n## Scope\ns\n");
        let rules: Vec<_> = lint_guidance(&doc).iter().map(|d| d.rule.clone()).collect();
        assert!(rules.contains(&"guidance-required-section".to_string()));
        assert!(rules.contains(&"guidance-liveness-na".to_string()));
    }
}
