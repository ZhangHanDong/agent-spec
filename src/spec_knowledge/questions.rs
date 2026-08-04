use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::{
    KnowledgeKind, RequirementPlan, collect_knowledge_checked, lint_requirement,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
}

/// The questions payload as emitted and ingested: a versioned envelope around
/// the question list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionEnvelope {
    pub envelope_version: u32,
    pub questions: Vec<ClarificationQuestion>,
}

impl QuestionEnvelope {
    pub fn new(questions: Vec<ClarificationQuestion>) -> Self {
        Self {
            envelope_version: ENVELOPE_VERSION,
            questions,
        }
    }
}

/// Validate agent-drafted candidates: bounded count, and every candidate
/// readable on its own. Returns one diagnostic per violation, each naming the
/// question id and the offending field. An empty option list is valid.
pub fn validate_envelope(questions: &[ClarificationQuestion]) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();
    for question in questions {
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
        }
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
