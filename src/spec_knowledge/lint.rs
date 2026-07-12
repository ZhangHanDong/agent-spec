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

    let scenarios_body = requirement_section_body(doc, "Scenarios");
    let has_scenario = scenarios_body.lines().any(|line| {
        line.trim_start().starts_with("Scenario:") || line.trim_start().starts_with("场景:")
    });

    if clauses
        .iter()
        .any(|clause| clause.keyword == Some(crate::spec_knowledge::NormativeKeyword::Must))
        && !has_scenario
    {
        out.push(diag(
            "requirement-must-needs-scenario",
            Severity::Warning,
            "MUST requirements should have at least one scenario".into(),
            Some("add a `## Scenarios` section with Given/When/Then steps"),
        ));
    }

    for then_line in scenario_then_lines(doc) {
        let lower = then_line.to_ascii_lowercase();
        let weak = [
            "works",
            "handles it",
            "is supported",
            "succeeds",
            "完成",
            "正常",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let observable = [
            "stdout",
            "stderr",
            "file",
            "status",
            "response",
            "contains",
            "returns",
            "writes",
            "persists",
            "visible",
            "error",
            "exits",
            "appears",
            "emits",
            "lists",
            "shows",
            "created",
            "available",
            "输出",
            "响应",
            "状态码",
            "文件",
            "包含",
            "返回",
            "错误",
            "出现",
            "展示",
            "创建",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        if weak || !observable {
            out.push(diag(
                "requirement-weak-then",
                Severity::Warning,
                format!("scenario Then step is not clearly observable: `{then_line}`"),
                Some("state the observable stdout/stderr/file/API/status/persisted result"),
            ));
        }
    }

    if !has_real_source_trace(doc) {
        out.push(diag(
            "requirement-source-trace-required",
            Severity::Warning,
            "requirement has no concrete `## Source Trace` entry".into(),
            Some("add the source PRD, issue, paper, interview answer, or design doc reference"),
        ));
    }

    for (i, clause) in clauses.iter().enumerate() {
        let lower = clause.text.to_ascii_lowercase();
        if lower.contains(" and ") && normative_token_count(&clause.text) >= 1 {
            out.push(diag(
                "requirement-compound-clause",
                Severity::Warning,
                format!(
                    "requirement clause {} may contain multiple obligations",
                    i + 1
                ),
                Some("split independent obligations into separate requirement ids"),
            ));
        }
        let mentions_nfr = [
            "fast",
            "performance",
            "reliable",
            "secure",
            "scalable",
            "性能",
            "可靠",
            "安全",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let has_measure = lower.chars().any(|ch| ch.is_ascii_digit())
            || ["ms", "seconds", "%", "p95", "p99", "秒", "毫秒"]
                .iter()
                .any(|needle| lower.contains(needle));
        if mentions_nfr && !has_measure {
            out.push(diag(
                "requirement-nfr-needs-measure",
                Severity::Warning,
                format!("requirement clause {} names a non-functional property without a measurable threshold", i + 1),
                Some("add a threshold and probe, such as p95 latency, max retries, or failure rate"),
            ));
        }
    }

    let requirement_text = clauses
        .iter()
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    let scenarios_lower = scenarios_body.to_ascii_lowercase();
    let needs_negative = [
        "invalid",
        "reject",
        "error",
        "permission",
        "auth",
        "delete",
        "fallback",
        "失败",
        "拒绝",
        "错误",
        "权限",
        "认证",
        "删除",
    ]
    .iter()
    .any(|needle| requirement_text.contains(needle));
    let has_negative = [
        "invalid",
        "reject",
        "error",
        "fail",
        "denied",
        "not create",
        "失败",
        "拒绝",
        "错误",
        "不创建",
    ]
    .iter()
    .any(|needle| scenarios_lower.contains(needle));
    if needs_negative && !has_negative {
        out.push(diag(
            "requirement-needs-negative-scenario",
            Severity::Warning,
            "requirement names validation, auth, delete, fallback, or error behavior without a negative scenario".into(),
            Some("add at least one scenario for the rejection or failure path"),
        ));
    }

    let scenario_words = scenario_words(doc);
    for transition in state_machine_transitions(doc) {
        if !transition_is_covered(&transition, &scenario_words) {
            out.push(diag(
                "requirement-state-machine-transition-uncovered",
                Severity::Warning,
                format!(
                    "state-machine transition has no matching scenario coverage: `{transition}`"
                ),
                Some("add a scenario that names the event and expected target state"),
            ));
        }
    }

    out
}

fn requirement_section_body(doc: &KnowledgeDoc, heading: &str) -> String {
    doc.section(heading)
        .map(|section| section.body.clone())
        .unwrap_or_default()
}

fn scenario_then_lines(doc: &KnowledgeDoc) -> Vec<String> {
    requirement_section_body(doc, "Scenarios")
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix("Then ")
                .or_else(|| line.strip_prefix("那么 "))
                .map(str::trim)
                .map(str::to_string)
        })
        .collect()
}

fn has_real_source_trace(doc: &KnowledgeDoc) -> bool {
    requirement_section_body(doc, "Source Trace")
        .lines()
        .map(|line| line.trim().trim_start_matches('-').trim())
        .any(|line| !line.is_empty() && !line.eq_ignore_ascii_case("none."))
}

fn state_machine_transitions(doc: &KnowledgeDoc) -> Vec<String> {
    requirement_section_body(doc, "State Machine")
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("On ") && line.contains("->"))
        .map(str::to_string)
        .collect()
}

fn scenario_words(doc: &KnowledgeDoc) -> Vec<String> {
    words_for_matching(&requirement_section_body(doc, "Scenarios"))
}

fn transition_is_covered(transition: &str, scenario_words: &[String]) -> bool {
    let Some((event, target)) = transition
        .trim_start_matches("On ")
        .split_once("->")
        .map(|(event, target)| (event.trim(), target.trim()))
    else {
        return false;
    };
    let event_words = words_for_matching(event);
    let target_words = words_for_matching(target);

    let event_covered = event_words
        .iter()
        .filter(|word| word.len() >= 4)
        .any(|word| scenario_words.contains(word));
    let target_covered = target_words
        .iter()
        .filter(|word| word.len() >= 4)
        .any(|word| scenario_words.contains(word));

    event_covered && target_covered
}

fn words_for_matching(input: &str) -> Vec<String> {
    let spaced = split_camel_words(input);
    spaced
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_lowercase())
        .collect()
}

fn split_camel_words(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut previous_was_lower_or_digit = false;
    for ch in input.chars() {
        if ch.is_ascii_uppercase() && previous_was_lower_or_digit {
            out.push(' ');
        }
        previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        out.push(ch);
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

    #[test]
    fn test_lint_requirement_warns_when_must_clause_has_no_scenario() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-500\ntitle: \"No Scenario\"\n---\n## Problem\nNeed behavior.\n## Requirements\n[REQ-500] The system MUST produce output.\n## Source Trace\n- issue:#500\n",
        );
        let diagnostics = lint_requirement(&doc);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.rule == "requirement-must-needs-scenario")
        );
    }

    #[test]
    fn test_lint_requirement_warns_on_weak_then_step() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-501\ntitle: \"Weak Then\"\n---\n## Problem\nNeed behavior.\n## Requirements\n[REQ-501] The system MUST produce output.\n## Scenarios\nScenario: Weak outcome\n  Given input\n  When the feature runs\n  Then it works\n## Source Trace\n- issue:#501\n",
        );
        let diagnostics = lint_requirement(&doc);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.rule == "requirement-weak-then")
        );
    }

    #[test]
    fn test_lint_requirement_warns_when_source_trace_missing() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-502\ntitle: \"No Source\"\n---\n## Problem\nNeed behavior.\n## Requirements\n[REQ-502] The system MUST produce output.\n## Scenarios\nScenario: Output\n  Given input\n  When the system runs\n  Then stdout contains \"done\"\n",
        );
        let diagnostics = lint_requirement(&doc);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.rule == "requirement-source-trace-required")
        );
    }

    #[test]
    fn test_lint_requirement_warns_on_unmeasured_nfr() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-503\ntitle: \"Fast\"\n---\n## Problem\nNeed speed.\n## Requirements\n[REQ-503] The system MUST be fast and reliable.\n## Scenarios\nScenario: Speed\n  Given input\n  When the system runs\n  Then output is visible\n## Source Trace\n- issue:#503\n",
        );
        let diagnostics = lint_requirement(&doc);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.rule == "requirement-nfr-needs-measure")
        );
    }

    #[test]
    fn test_lint_requirement_warns_on_compound_clause() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-504\ntitle: \"Compound\"\n---\n## Problem\nNeed two things.\n## Requirements\n[REQ-504] The system MUST validate input and persist output.\n## Scenarios\nScenario: Output\n  Given valid input\n  When the system runs\n  Then output is visible\n## Source Trace\n- issue:#504\n",
        );
        let diagnostics = lint_requirement(&doc);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.rule == "requirement-compound-clause")
        );
    }

    #[test]
    fn test_lint_requirement_warns_when_negative_behavior_lacks_negative_scenario() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-505\ntitle: \"Auth\"\n---\n## Problem\nNeed auth.\n## Requirements\n[REQ-505] The system MUST reject unauthorized users.\n## Scenarios\nScenario: Authorized user\n  Given an authorized user\n  When the system runs\n  Then output is visible\n## Source Trace\n- issue:#505\n",
        );
        let diagnostics = lint_requirement(&doc);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.rule == "requirement-needs-negative-scenario")
        );
    }

    #[test]
    fn test_lint_requirement_warns_on_uncovered_state_machine_transition() {
        let doc = parse_req(
            "---\nkind: requirement\nid: REQ-SM\ntitle: \"Lifecycle\"\n---\n## Problem\nNeed lifecycle correctness.\n## Requirements\n[REQ-SM] The node lifecycle MUST handle planned stop.\n## State Machine\nState: Running\n  On planned stop -> StoppingCleanly\n## Scenarios\nScenario: Start node\n  Given a stopped node\n  When the node starts\n  Then status is Running\n## Source Trace\n- issue:#sm\n",
        );
        let diagnostics = lint_requirement(&doc);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.rule == "requirement-state-machine-transition-uncovered")
        );
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
