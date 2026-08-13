use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::model::{DecisionStatus, KnowledgeDoc};
use crate::spec_knowledge::{
    KnowledgeKind, RequirementPlan, collect_knowledge_checked, lint_requirement,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Envelope schema version carried by every questions payload. Bumped when the
/// question or option shape changes so an old reader fails loudly instead of
/// silently misreading (REQ-DECISION-POINT-ENVELOPE).
pub const ENVELOPE_VERSION: u32 = 1;

/// Maximum candidates a question may carry. Matches what agent harnesses
/// render as a choice list; more than this is a drafting error, not a UI hint.
pub const MAX_OPTIONS: usize = 4;

/// Which pipeline stage asked. Lets a renderer group questions without
/// parsing ids or diagnostic codes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    Requirements,
    Knowledge,
    Verification,
}

/// One candidate answer. `value` is what a write-back applies; `label` and
/// `description` are what a human reads. Candidates are drafted from source
/// text by an agent — the CLI validates their shape and never invents them
/// (ADR-003).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionOption {
    pub label: String,
    pub description: String,
    pub value: String,
    /// Set only when the source document states a recommendation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClarificationQuestion {
    pub id: String,
    pub target_id: String,
    pub diagnostic_code: String,
    pub blocking: bool,
    pub prompt: String,
    pub source: String,
    pub kind: QuestionKind,
    #[serde(default)]
    pub multi_select: bool,
    /// Candidate answers. Empty is valid and honest: it means no candidate
    /// could be grounded, and the question stays free-form. A populated list
    /// never claims to exhaust the answer space.
    #[serde(default)]
    pub options: Vec<DecisionOption>,
    /// Evidence gathered for the thing being asked about. Populated by the
    /// verification stage, which has evidence to show; empty elsewhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// Exact scenario identity for verification questions. Keeping this
    /// separate from the human-readable prompt avoids lossy slug parsing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_name: Option<String>,
    /// A harness may fill this field and pass the answered envelope directly
    /// to `resolve-ai`; the nested field names are exactly `AiDecision`'s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<crate::spec_core::AiDecision>,
}

/// Extract decision-point questions from a knowledge document: a proposal's
/// unresolved questions (one free-form question per list item) and a
/// decision's alternatives (one choice whose candidates are the alternatives).
/// A settled decision still yields its question, marked non-blocking, so a
/// caller can show it for review without treating it as pending work.
pub fn build_knowledge_questions(doc: &KnowledgeDoc) -> Vec<ClarificationQuestion> {
    let source = doc.source_path.display().to_string();
    let mut out = Vec::new();

    if let Some(section) = doc.section("Unresolved Questions") {
        for (index, item) in list_items(&section.body).into_iter().enumerate() {
            out.push(ClarificationQuestion {
                id: format!("Q-{}-UQ-{}", doc.meta.id, index + 1),
                target_id: doc.meta.id.clone(),
                diagnostic_code: "unresolved-question".into(),
                blocking: doc.meta.status != Some(DecisionStatus::Accepted),
                prompt: item,
                source: source.clone(),
                kind: QuestionKind::Knowledge,
                multi_select: false,
                options: Vec::new(),
                evidence: Vec::new(),
                scenario_name: None,
                answer: None,
            });
        }
    }

    if let Some(section) = doc.section("Alternatives Considered") {
        let items = list_items(&section.body);
        if !items.is_empty() {
            let options = items
                .iter()
                .take(MAX_OPTIONS)
                .map(|item| {
                    let (label, description) = split_alternative(item);
                    DecisionOption {
                        label,
                        description,
                        value: item.clone(),
                        // Only a stated recommendation counts; never inferred.
                        recommended: states_recommendation(item),
                    }
                })
                .collect::<Vec<_>>();
            out.push(ClarificationQuestion {
                id: format!("Q-{}-ALT", doc.meta.id),
                target_id: doc.meta.id.clone(),
                diagnostic_code: "alternatives-considered".into(),
                blocking: doc.meta.status != Some(DecisionStatus::Accepted),
                prompt: format!(
                    "{}: which alternative does this decision take?",
                    doc.meta.id
                ),
                source,
                kind: QuestionKind::Knowledge,
                multi_select: false,
                options,
                evidence: Vec::new(),
                scenario_name: None,
                answer: None,
            });
        }
    }

    out
}

/// Markdown list items in a section body, joined across continuation lines.
fn list_items(body: &str) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            items.push(rest.trim().to_string());
        } else if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && let Some(last) = items.last_mut()
        {
            last.push(' ');
            last.push_str(trimmed);
        }
    }
    items.retain(|item| !item.is_empty() && !item.eq_ignore_ascii_case("none."));
    items
}

/// An alternative reads "<option> — <reason it was rejected>"; the head is a
/// usable label, the whole line is the description.
fn split_alternative(item: &str) -> (String, String) {
    // Separators in use across the corpus: Chinese double dash, English em
    // dash, ASCII double hyphen. Longest first so `——` is not split as `—`.
    let head = [" —— ", " — ", " -- ", "——"]
        .iter()
        .find_map(|sep| item.split_once(sep).map(|(head, _)| head))
        .unwrap_or(item)
        .trim();
    let label = head.chars().take(60).collect::<String>();
    (
        if label.is_empty() {
            item.chars().take(60).collect()
        } else {
            label
        },
        item.to_string(),
    )
}

fn states_recommendation(item: &str) -> bool {
    let lower = item.to_ascii_lowercase();
    lower.contains("recommended") || item.contains("推荐")
}

/// Turn scenarios the machine could not settle into judgment questions — the
/// inverse of `resolve-ai`, emitting what it consumes. Mechanically decided
/// pass/fail scenarios never appear: a proven verdict is not up for a vote,
/// and `resolve-ai` never applies an answer to either state. Legacy model
/// decisions retain their narrower skip-only behavior.
/// The verdict vocabulary is the one place the CLI supplies candidates — it is
/// a closed enum, not an inference from source text.
pub fn build_verification_questions(
    spec_name: &str,
    spec_path: &str,
    results: &[crate::spec_core::ScenarioResult],
) -> Vec<ClarificationQuestion> {
    use crate::spec_core::Verdict;

    let mut out = Vec::new();
    for (index, result) in results.iter().enumerate() {
        let unsettled = match result.verdict {
            Verdict::Skip => "skip",
            Verdict::Uncertain => "uncertain",
            Verdict::PendingReview => "pending-review",
            Verdict::Pass | Verdict::Fail => continue,
        };
        let evidence = result
            .evidence
            .iter()
            .map(serialize_verification_evidence)
            .collect::<Vec<_>>();
        out.push(ClarificationQuestion {
            id: verification_question_id(index, &result.scenario_name),
            target_id: spec_name.to_string(),
            diagnostic_code: unsettled.into(),
            blocking: true,
            prompt: format!(
                "scenario `{}` is {unsettled}; does it meet the contract?",
                result.scenario_name
            ),
            source: spec_path.to_string(),
            kind: QuestionKind::Verification,
            multi_select: false,
            options: verdict_options(),
            evidence,
            scenario_name: Some(result.scenario_name.clone()),
            answer: None,
        });
    }
    out
}

fn serialize_verification_evidence(evidence: &crate::spec_core::Evidence) -> String {
    let stable = match evidence {
        crate::spec_core::Evidence::TestOutput {
            test_name,
            stdout,
            passed,
            package,
            level,
            test_double,
            targets,
        } => crate::spec_core::Evidence::TestOutput {
            test_name: test_name.clone(),
            stdout: normalize_cargo_test_output(stdout),
            passed: *passed,
            package: package.clone(),
            level: level.clone(),
            test_double: test_double.clone(),
            targets: targets.clone(),
        },
        other => other.clone(),
    };
    serde_json::to_string(&stable).unwrap_or_else(|_| "<unserializable evidence>".into())
}

/// Remove Cargo orchestration noise that changes between a cold and warm
/// rerun while retaining test names, results, warnings, and compiler errors.
/// Verification answers bind this normalized representation, so cache state
/// and wall-clock timing cannot make unchanged evidence look stale.
fn normalize_cargo_test_output(stdout: &str) -> String {
    let mut normalized = String::new();
    for segment in stdout.split_inclusive('\n') {
        let (line, newline) = segment
            .strip_suffix('\n')
            .map_or((segment, ""), |line| (line, "\n"));
        let trimmed = line.trim_start();
        let orchestration = trimmed.starts_with("Compiling ")
            || trimmed.starts_with("Finished `")
            || trimmed.starts_with("Running unittests ")
            || trimmed.starts_with("Running tests/")
            || trimmed.starts_with("Running tests\\")
            || trimmed.starts_with("Running doc-tests ")
            || trimmed.starts_with("Blocking waiting for file lock");
        if orchestration {
            continue;
        }
        if trimmed.starts_with("test result:")
            && let Some(index) = line.find("; finished in ")
        {
            normalized.push_str(&line[..index]);
            normalized.push_str("; finished in <duration>");
            normalized.push_str(newline);
            continue;
        }
        normalized.push_str(line);
        normalized.push_str(newline);
    }
    normalized
}

fn verdict_options() -> Vec<DecisionOption> {
    [
        (
            "Pass",
            "The scenario meets the contract on this evidence",
            "pass",
        ),
        ("Fail", "The scenario does not meet the contract", "fail"),
        (
            "Skip",
            "Not judgeable yet — leave unsettled and gather more evidence",
            "skip",
        ),
    ]
    .into_iter()
    .map(|(label, description, value)| DecisionOption {
        label: label.into(),
        description: description.into(),
        value: value.into(),
        recommended: false,
    })
    .collect()
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(48).collect()
}

fn verification_question_id(index: usize, scenario_name: &str) -> String {
    let readable = slug(scenario_name);
    let digest = blake3::hash(scenario_name.as_bytes()).to_hex().to_string();
    let digest = &digest[..12];
    if readable.is_empty() {
        format!("Q-VERIFY-{}-{digest}", index + 1)
    } else {
        format!("Q-VERIFY-{}-{readable}-{digest}", index + 1)
    }
}

/// The questions payload as emitted and ingested: a versioned envelope around
/// the question list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionEnvelope {
    pub envelope_version: u32,
    /// Replay inputs for verification questions. Other question kinds omit it,
    /// preserving their v1 wire shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_context: Option<VerificationQuestionContext>,
    pub questions: Vec<ClarificationQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationQuestionContext {
    pub ai_mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub change_paths: Vec<PathBuf>,
}

impl QuestionEnvelope {
    pub fn new(questions: Vec<ClarificationQuestion>) -> Self {
        Self {
            envelope_version: ENVELOPE_VERSION,
            verification_context: None,
            questions,
        }
    }

    pub fn with_verification_context(mut self, context: VerificationQuestionContext) -> Self {
        self.verification_context = Some(context);
        self
    }
}

/// Validate agent-drafted candidates: bounded count, and every candidate
/// readable on its own. Returns one diagnostic per violation, each naming the
/// question id and the offending field. An empty option list is valid.
pub fn validate_envelope(questions: &[ClarificationQuestion]) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();
    let mut ids = BTreeSet::new();
    for question in questions {
        if !ids.insert(question.id.as_str()) {
            out.push(envelope_diag(
                "envelope-duplicate-question-id",
                format!("question id {} appears more than once", question.id),
                "give every question a unique stable id",
            ));
        }
        if question.options.len() > MAX_OPTIONS {
            out.push(envelope_diag(
                "envelope-too-many-options",
                format!(
                    "question {} carries {} options; at most {MAX_OPTIONS} are allowed",
                    question.id,
                    question.options.len()
                ),
                "drop or merge candidates until at most four remain; the free-form answer path covers the rest",
            ));
        }
        for (index, option) in question.options.iter().enumerate() {
            if option.label.trim().is_empty() {
                out.push(envelope_diag(
                    "envelope-option-missing-label",
                    format!(
                        "question {} option {} has an empty `label` field",
                        question.id,
                        index + 1
                    ),
                    "give every candidate a short label a human can pick by",
                ));
            }
            if option.description.trim().is_empty() {
                out.push(envelope_diag(
                    "envelope-option-missing-description",
                    format!(
                        "question {} option {} has an empty `description` field",
                        question.id,
                        index + 1
                    ),
                    "state in one sentence what choosing this candidate means",
                ));
            }
        }
    }
    out
}

/// Merge agent-drafted options atomically. Shape, identity, and stage metadata
/// are all checked before the first target question is changed.
pub fn merge_drafted_options(
    questions: &mut [ClarificationQuestion],
    drafted: &[ClarificationQuestion],
) -> Vec<LintDiagnostic> {
    let mut diagnostics = validate_envelope(drafted);
    for supplied in drafted {
        match questions.iter().find(|question| question.id == supplied.id) {
            None => diagnostics.push(envelope_diag(
                "envelope-unknown-question-id",
                format!("supplied options name unknown question id {}", supplied.id),
                "regenerate the questions envelope and draft options against its current ids",
            )),
            Some(target)
                if target.target_id != supplied.target_id || target.kind != supplied.kind =>
            {
                diagnostics.push(envelope_diag(
                    "envelope-question-metadata-mismatch",
                    format!(
                        "question {} does not match target_id/kind from the current envelope",
                        supplied.id
                    ),
                    "copy target_id and kind from the current emitted question",
                ));
            }
            Some(_) => {}
        }
    }
    if !diagnostics.is_empty() {
        return diagnostics;
    }
    for supplied in drafted {
        if let Some(target) = questions
            .iter_mut()
            .find(|question| question.id == supplied.id)
        {
            target.options.clone_from(&supplied.options);
            target.multi_select = supplied.multi_select;
        }
    }
    diagnostics
}

fn envelope_diag(rule: &str, message: String, suggestion: &str) -> LintDiagnostic {
    LintDiagnostic {
        rule: rule.into(),
        severity: Severity::Error,
        message,
        span: Span::default(),
        suggestion: Some(suggestion.into()),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ClarificationDiagnostic {
    pub target_id: String,
    pub code: String,
    pub severity: String,
    pub message: String,
    pub source: String,
}

pub fn collect_clarification_lint_diagnostics(
    knowledge_dir: &Path,
) -> Vec<ClarificationDiagnostic> {
    let collection = collect_knowledge_checked(knowledge_dir);
    let mut out = Vec::new();
    for doc in collection.docs {
        if doc.meta.kind != KnowledgeKind::Requirement {
            continue;
        }
        for diagnostic in lint_requirement(&doc) {
            if !is_question_lint(&diagnostic.rule) {
                continue;
            }
            out.push(ClarificationDiagnostic {
                target_id: doc.meta.id.clone(),
                code: diagnostic.rule,
                severity: severity_label(diagnostic.severity).into(),
                message: diagnostic.message,
                source: doc.source_path.display().to_string(),
            });
        }
    }
    out.sort_by(|a, b| {
        a.target_id
            .cmp(&b.target_id)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });
    out
}

fn is_question_lint(rule: &str) -> bool {
    matches!(
        rule,
        "requirement-weak-then"
            | "requirement-must-needs-scenario"
            | "requirement-nfr-needs-measure"
            | "requirement-source-trace-required"
            | "requirement-compound-clause"
            | "requirement-single-statement"
            | "requirement-needs-negative-scenario"
            | "requirement-state-machine-transition-uncovered"
    )
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

pub fn build_clarification_questions(
    plan: &RequirementPlan,
    lint_diagnostics: &[ClarificationDiagnostic],
) -> Vec<ClarificationQuestion> {
    let mut questions = Vec::new();
    for node in &plan.requirements {
        for (idx, question) in node.blocked_by.iter().enumerate() {
            questions.push(ClarificationQuestion {
                id: format!("Q-{}-{}", node.id, idx + 1),
                target_id: node.id.clone(),
                diagnostic_code: "blocked-open-questions".into(),
                blocking: true,
                prompt: question.clone(),
                source: node.source_path.display().to_string(),
                kind: QuestionKind::Requirements,
                multi_select: false,
                options: Vec::new(),
                evidence: Vec::new(),
                scenario_name: None,
                answer: None,
            });
        }
    }

    for diagnostic in lint_diagnostics {
        questions.push(ClarificationQuestion {
            id: format!("Q-{}-{}", diagnostic.target_id, diagnostic.code),
            target_id: diagnostic.target_id.clone(),
            diagnostic_code: diagnostic.code.clone(),
            blocking: diagnostic.severity == "error",
            prompt: diagnostic.message.clone(),
            source: diagnostic.source.clone(),
            kind: QuestionKind::Requirements,
            multi_select: false,
            options: Vec::new(),
            evidence: Vec::new(),
            scenario_name: None,
            answer: None,
        });
    }

    questions.sort_by(|a, b| {
        a.target_id
            .cmp(&b.target_id)
            .then_with(|| a.diagnostic_code.cmp(&b.diagnostic_code))
            .then_with(|| a.id.cmp(&b.id))
    });
    questions.dedup_by(|a, b| a.id == b.id);
    questions
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::{
        RequirementPlan, RequirementPlanBatch, RequirementPlanDiagnostic, RequirementPlanNode,
        RequirementPlanStatus, RequirementSpecCoverage,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn option(label: &str, description: &str) -> DecisionOption {
        DecisionOption {
            label: label.into(),
            description: description.into(),
            value: label.to_ascii_lowercase(),
            recommended: false,
        }
    }

    fn question(kind: QuestionKind, options: Vec<DecisionOption>) -> ClarificationQuestion {
        ClarificationQuestion {
            id: "Q-REQ-A-1".into(),
            target_id: "REQ-A".into(),
            diagnostic_code: "requirement-compound-clause".into(),
            blocking: false,
            prompt: "clause 1 may contain multiple obligations".into(),
            source: "knowledge/requirements/req-a.md".into(),
            kind,
            multi_select: false,
            options,
            evidence: Vec::new(),
            scenario_name: None,
            answer: None,
        }
    }

    fn knowledge_doc(input: &str, name: &str) -> KnowledgeDoc {
        crate::spec_knowledge::parse_knowledge_str(input, Path::new(name)).unwrap()
    }

    fn scenario(
        name: &str,
        verdict: crate::spec_core::Verdict,
        evidence: Vec<crate::spec_core::Evidence>,
    ) -> crate::spec_core::ScenarioResult {
        crate::spec_core::ScenarioResult {
            scenario_name: name.into(),
            verdict,
            step_results: Vec::new(),
            evidence,
            duration_ms: 0,
            provenance: None,
        }
    }

    #[test]
    fn test_knowledge_questions_extracts_unresolved_questions() {
        let doc = knowledge_doc(
            "---\nkind: proposal\nid: LEP-009\nstatus: proposed\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Unresolved Questions\n\n- First open point?\n- Second open point?\n",
            "knowledge/proposals/lep-009.md",
        );
        let questions = build_knowledge_questions(&doc);
        let unresolved: Vec<_> = questions
            .iter()
            .filter(|q| q.diagnostic_code == "unresolved-question")
            .collect();
        assert_eq!(
            unresolved.len(),
            2,
            "one question per list item: {questions:?}"
        );
        assert!(unresolved.iter().all(|q| q.kind == QuestionKind::Knowledge));
        assert!(
            unresolved
                .iter()
                .all(|q| q.source == "knowledge/proposals/lep-009.md"),
            "source points at the proposal"
        );
        assert!(unresolved[0].blocking, "an open proposal's questions block");
    }

    #[test]
    fn test_knowledge_questions_omits_unstated_recommendation() {
        let doc = knowledge_doc(
            "---\nkind: decision\nid: ADR-009\nstatus: proposed\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Alternatives Considered\n\n- Option A — rejected because X.\n- Option B — rejected because Y.\n",
            "knowledge/decisions/adr-009.md",
        );
        let questions = build_knowledge_questions(&doc);
        let alt = questions
            .iter()
            .find(|q| q.diagnostic_code == "alternatives-considered")
            .unwrap_or_else(|| panic!("alternatives must yield one choice: {questions:?}"));
        assert_eq!(alt.options.len(), 2);
        assert!(
            alt.options.iter().all(|o| !o.recommended),
            "no recommendation is stated, so none is marked"
        );
        assert_eq!(
            alt.options[0].label, "Option A",
            "label is the head, not the reason"
        );
    }

    #[test]
    fn test_knowledge_questions_marks_stated_recommendation() {
        let doc = knowledge_doc(
            "---\nkind: decision\nid: ADR-010\nstatus: proposed\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Alternatives Considered\n\n- Option A (recommended) — keeps the surface small.\n- Option B — rejected because Y.\n",
            "knowledge/decisions/adr-010.md",
        );
        let alt = build_knowledge_questions(&doc)
            .into_iter()
            .find(|q| q.diagnostic_code == "alternatives-considered")
            .unwrap_or_else(|| panic!("alternatives must yield one choice"));
        assert!(
            alt.options[0].recommended,
            "a stated recommendation is carried"
        );
        assert!(!alt.options[1].recommended);
    }

    #[test]
    fn test_knowledge_questions_empty_for_resolved_proposal() {
        let doc = knowledge_doc(
            "---\nkind: proposal\nid: LEP-011\nstatus: accepted\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Unresolved Questions\n\nNone.\n",
            "knowledge/proposals/lep-011.md",
        );
        assert!(
            build_knowledge_questions(&doc).is_empty(),
            "`None.` is not a question"
        );
    }

    #[test]
    fn test_knowledge_questions_alternatives_bounded_to_four() {
        let items = (1..=6)
            .map(|i| format!("- Option {i} — rejected because {i}."))
            .collect::<Vec<_>>()
            .join("\n");
        let doc = knowledge_doc(
            &format!(
                "---\nkind: decision\nid: ADR-012\nstatus: proposed\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Alternatives Considered\n\n{items}\n"
            ),
            "knowledge/decisions/adr-012.md",
        );
        let alt = build_knowledge_questions(&doc)
            .into_iter()
            .find(|q| q.diagnostic_code == "alternatives-considered")
            .unwrap_or_else(|| panic!("alternatives must yield one choice"));
        assert_eq!(
            alt.options.len(),
            MAX_OPTIONS,
            "six alternatives are capped at four"
        );
        assert!(
            validate_envelope(&[alt]).is_empty(),
            "a capped question validates"
        );
    }

    #[test]
    fn test_verify_emit_questions_carries_scenario_and_evidence() {
        let results = vec![scenario(
            "unsettled path",
            crate::spec_core::Verdict::Skip,
            vec![crate::spec_core::Evidence::AiAnalysis {
                model: "stub".into(),
                confidence: 0.4,
                reasoning: "no mechanical binding".into(),
                human_judgment: None,
            }],
        )];
        let questions =
            build_verification_questions("Some Spec", "specs/task-some.spec.md", &results);
        assert_eq!(questions.len(), 1);
        let q = &questions[0];
        assert_eq!(q.kind, QuestionKind::Verification);
        assert!(
            q.prompt.contains("unsettled path"),
            "carries scenario text: {}",
            q.prompt
        );
        assert!(!q.evidence.is_empty(), "carries gathered evidence");
        assert!(
            q.evidence[0].contains("no mechanical binding"),
            "{:?}",
            q.evidence
        );
        let values: Vec<_> = q.options.iter().map(|o| o.value.as_str()).collect();
        assert_eq!(
            values,
            vec!["pass", "fail", "skip"],
            "verdict vocabulary as candidates"
        );
        assert!(validate_envelope(&questions).is_empty());
    }

    #[test]
    fn test_verification_questions_normalize_cargo_rerun_noise() {
        fn output(stdout: &str) -> crate::spec_core::Evidence {
            crate::spec_core::Evidence::TestOutput {
                test_name: "manual_review".into(),
                stdout: stdout.into(),
                passed: true,
                package: None,
                level: None,
                test_double: None,
                targets: None,
            }
        }

        let cold = "   Compiling fixture v0.1.0 (/tmp/fixture)\n    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.65s\n     Running unittests src/lib.rs (target/debug/deps/fixture-a)\n\nrunning 1 test\ntest tests::manual_review ... ok\n\ntest result: ok. 1 passed; 0 failed; finished in 0.01s\n";
        let warm = "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s\n     Running unittests src/lib.rs (target/debug/deps/fixture-b)\n\nrunning 1 test\ntest tests::manual_review ... ok\n\ntest result: ok. 1 passed; 0 failed; finished in 0.00s\n";
        let first = build_verification_questions(
            "S",
            "specs/s.spec.md",
            &[scenario(
                "manual review",
                crate::spec_core::Verdict::PendingReview,
                vec![output(cold)],
            )],
        );
        let second = build_verification_questions(
            "S",
            "specs/s.spec.md",
            &[scenario(
                "manual review",
                crate::spec_core::Verdict::PendingReview,
                vec![output(warm)],
            )],
        );

        assert_eq!(first[0].evidence, second[0].evidence);
        assert!(first[0].evidence[0].contains("test tests::manual_review ... ok"));
        assert!(!first[0].evidence[0].contains("Compiling fixture"));
        assert!(!first[0].evidence[0].contains("0.65s"));
    }

    #[test]
    fn test_verify_emit_questions_skips_mechanical_verdicts() {
        let results = vec![
            scenario("proven good", crate::spec_core::Verdict::Pass, Vec::new()),
            scenario("proven bad", crate::spec_core::Verdict::Fail, Vec::new()),
            scenario(
                "unsettled",
                crate::spec_core::Verdict::Uncertain,
                Vec::new(),
            ),
        ];
        let questions =
            build_verification_questions("Some Spec", "specs/task-some.spec.md", &results);
        assert_eq!(
            questions.len(),
            1,
            "only the unsettled scenario is asked about"
        );
        assert!(questions[0].prompt.contains("unsettled"));
    }

    #[test]
    fn test_verify_question_ids_are_unique_for_non_ascii_scenarios() {
        let results = vec![
            scenario("信封携带阶段", crate::spec_core::Verdict::Skip, Vec::new()),
            scenario("超过四项候选", crate::spec_core::Verdict::Skip, Vec::new()),
            scenario("空候选表合法", crate::spec_core::Verdict::Skip, Vec::new()),
        ];
        let questions = build_verification_questions("中文合约", "specs/task.spec.md", &results);
        let ids = questions
            .iter()
            .map(|question| question.id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), results.len(), "question ids must be unique");
        assert_eq!(
            questions[0].scenario_name.as_deref(),
            Some("信封携带阶段"),
            "the exact scenario identity is structured, not parsed from the id"
        );
        assert!(validate_envelope(&questions).is_empty());
    }

    #[test]
    fn test_envelope_carries_kind_and_structured_options() {
        let q = question(
            QuestionKind::Requirements,
            vec![
                option("Split", "Break the clause into two MUST statements"),
                option("Keep", "The obligations are inseparable in practice"),
            ],
        );
        let json = serde_json::to_string(&QuestionEnvelope::new(vec![q])).unwrap();
        assert!(json.contains("\"kind\":\"requirements\""), "{json}");
        assert!(
            json.contains("\"label\":\"Split\"") && json.contains("\"description\":"),
            "{json}"
        );
    }

    #[test]
    fn test_envelope_rejects_more_than_four_options() {
        let q = question(
            QuestionKind::Requirements,
            (1..=5).map(|i| option(&format!("O{i}"), "desc")).collect(),
        );
        let diags = validate_envelope(&[q]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "envelope-too-many-options")
            .unwrap_or_else(|| panic!("five options must be rejected: {diags:?}"));
        assert!(
            hit.message.contains("Q-REQ-A-1"),
            "names the question: {}",
            hit.message
        );
        assert!(
            hit.message.contains('4'),
            "names the bound: {}",
            hit.message
        );
    }

    #[test]
    fn test_envelope_rejects_option_without_description() {
        let q = question(QuestionKind::Requirements, vec![option("Split", "  ")]);
        let diags = validate_envelope(&[q]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "envelope-option-missing-description")
            .unwrap_or_else(|| panic!("description-less option must be rejected: {diags:?}"));
        assert!(
            hit.message.contains("description"),
            "names the field: {}",
            hit.message
        );
    }

    #[test]
    fn test_envelope_rejects_option_without_label() {
        let q = question(
            QuestionKind::Requirements,
            vec![option("", "a description")],
        );
        let diags = validate_envelope(&[q]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "envelope-option-missing-label")
            .unwrap_or_else(|| panic!("label-less option must be rejected: {diags:?}"));
        assert!(
            hit.message.contains("label"),
            "names the field: {}",
            hit.message
        );
    }

    #[test]
    fn test_envelope_rejects_duplicate_question_ids() {
        let q = question(QuestionKind::Requirements, Vec::new());
        let diagnostics = validate_envelope(&[q.clone(), q]);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "envelope-duplicate-question-id"),
            "duplicate ids must fail before id-based merging: {diagnostics:?}"
        );
    }

    #[test]
    fn test_drafted_options_reject_unknown_id_atomically() {
        let mut current = vec![question(QuestionKind::Requirements, Vec::new())];
        let before = current.clone();
        let mut unknown = question(
            QuestionKind::Requirements,
            vec![option("Split", "Split the requirement")],
        );
        unknown.id = "Q-REQ-STALE".into();
        let diagnostics = merge_drafted_options(&mut current, &[unknown]);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "envelope-unknown-question-id"),
            "a stale id must be named: {diagnostics:?}"
        );
        assert_eq!(current, before, "no option is merged after any violation");
    }

    #[test]
    fn test_envelope_accepts_empty_options() {
        let q = question(QuestionKind::Knowledge, Vec::new());
        assert!(
            validate_envelope(&[q]).is_empty(),
            "an ungrounded question stays free-form and is valid"
        );
    }

    #[test]
    fn test_questions_json_carries_envelope_version() {
        let json = serde_json::to_string(&QuestionEnvelope::new(vec![question(
            QuestionKind::Verification,
            Vec::new(),
        )]))
        .unwrap();
        assert!(json.contains("\"envelope_version\":1"), "{json}");
        assert_eq!(ENVELOPE_VERSION, 1);
    }

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_build_clarification_questions_from_open_question_diagnostic() {
        let plan = RequirementPlan {
            version: 1,
            requirements: vec![RequirementPlanNode {
                id: "REQ-A".into(),
                title: "A".into(),
                source_path: PathBuf::from("knowledge/requirements/req-a.md"),
                status: RequirementPlanStatus::Blocked,
                mode: "blocked_questions".into(),
                scenario_count: 1,
                blocked_by: vec!["Should export support CSV?".into()],
            }],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: Vec::<RequirementPlanBatch>::new(),
            coverage: Vec::<RequirementSpecCoverage>::new(),
            diagnostics: vec![RequirementPlanDiagnostic {
                code: "blocked-open-questions".into(),
                severity: "warning".into(),
                requirement_id: Some("REQ-A".into()),
                message: "REQ-A has open questions".into(),
            }],
            parse_errors: Vec::new(),
        };

        let questions = build_clarification_questions(&plan, &[]);
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].target_id, "REQ-A");
        assert_eq!(questions[0].diagnostic_code, "blocked-open-questions");
        assert!(questions[0].blocking);
        assert!(questions[0].prompt.contains("Should export support CSV?"));
    }

    #[test]
    fn test_build_clarification_questions_from_lint_diagnostic() {
        let plan = RequirementPlan {
            version: 1,
            requirements: vec![RequirementPlanNode {
                id: "REQ-A".into(),
                title: "A".into(),
                source_path: PathBuf::from("knowledge/requirements/req-a.md"),
                status: RequirementPlanStatus::Ready,
                mode: "leaf_full".into(),
                scenario_count: 1,
                blocked_by: Vec::new(),
            }],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: Vec::<RequirementPlanBatch>::new(),
            coverage: Vec::<RequirementSpecCoverage>::new(),
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };
        let lint_diagnostics = vec![ClarificationDiagnostic {
            target_id: "REQ-A".into(),
            code: "requirement-weak-then".into(),
            severity: "warning".into(),
            message: "Then step needs an observable outcome".into(),
            source: "knowledge/requirements/req-a.md".into(),
        }];

        let questions = build_clarification_questions(&plan, &lint_diagnostics);
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].target_id, "REQ-A");
        assert_eq!(questions[0].diagnostic_code, "requirement-weak-then");
        assert!(!questions[0].blocking);
    }

    #[test]
    fn test_collect_clarification_lint_diagnostics_surfaces_quality_convergence_rules() {
        let dir = make_temp_dir("requirements-quality-questions");
        let knowledge = dir.join("knowledge");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::write(
            knowledge.join("requirements/req-quality.md"),
            "---\nkind: requirement\nid: REQ-QUALITY\ntitle: \"Quality\"\nliveness: auto\n---\n## Problem\nNeed quality.\n## Requirements\n[REQ-QUALITY] The service MUST reject invalid tokens and the audit log MUST record the rejection.\n## Scenarios\nScenario: Invalid token\n  Given an invalid token\n  When the request is submitted\n  Then the response returns a 401 error\n## Open Questions\nNone.\n",
        )
        .unwrap();

        let diagnostics = collect_clarification_lint_diagnostics(&knowledge);
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&"requirement-source-trace-required"));
        assert!(codes.contains(&"requirement-compound-clause"));

        fs::remove_dir_all(dir).ok();
    }
}
