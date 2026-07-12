use crate::spec_core::Severity;
use crate::spec_knowledge::{
    KnowledgeKind, RequirementPlan, collect_knowledge_checked, lint_requirement,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ClarificationQuestion {
    pub id: String,
    pub target_id: String,
    pub diagnostic_code: String,
    pub blocking: bool,
    pub prompt: String,
    pub source: String,
    pub options: Vec<String>,
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
