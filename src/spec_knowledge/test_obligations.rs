use crate::spec_core::Section;
use crate::spec_knowledge::{build_requirement_graph, build_requirement_plan};
use crate::spec_qa::{QaClass, QaEvidenceKind, required_evidence_for};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TestObligationSet {
    pub version: u32,
    pub obligations: Vec<TestObligation>,
    pub diagnostics: Vec<TestObligationDiagnostic>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TestObligation {
    pub requirement_id: String,
    pub scenario_name: String,
    pub suggested_selector: String,
    pub verification_strength: String,
    pub spec_path: Option<PathBuf>,
    pub required_evidence: Vec<QaEvidenceKind>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TestObligationDiagnostic {
    pub code: String,
    pub severity: String,
    pub requirement_id: Option<String>,
    pub message: String,
}

pub fn build_test_obligations(knowledge_dir: &Path, specs_dir: &Path) -> TestObligationSet {
    let graph = build_requirement_graph(knowledge_dir);
    let plan = build_requirement_plan(knowledge_dir, specs_dir);
    let mut obligations = Vec::new();
    let mut diagnostics = plan
        .diagnostics
        .iter()
        .map(|diagnostic| TestObligationDiagnostic {
            code: diagnostic.code.clone(),
            severity: diagnostic.severity.clone(),
            requirement_id: diagnostic.requirement_id.clone(),
            message: diagnostic.message.clone(),
        })
        .collect::<Vec<_>>();
    diagnostics.extend(
        plan.parse_errors
            .iter()
            .map(|error| TestObligationDiagnostic {
                code: if error.message.contains("knowledge root") {
                    "knowledge-root-missing".into()
                } else {
                    "knowledge-parse-error".into()
                },
                severity: "error".into(),
                requirement_id: None,
                message: format!("{}: {}", error.path.display(), error.message),
            }),
    );

    for node in graph.nodes {
        let spec_paths = plan
            .coverage
            .iter()
            .find(|coverage| coverage.requirement_id == node.id)
            .map(|coverage| coverage.spec_paths.clone())
            .unwrap_or_default();
        let spec_doc = spec_paths
            .first()
            .and_then(|path| crate::spec_parser::parse_spec(path).ok());
        let qa_class = spec_doc
            .as_ref()
            .map(|doc| QaClass::parse(doc.meta.risk.as_deref()))
            .unwrap_or(QaClass::B);
        let selectors = spec_doc
            .as_ref()
            .map(scenario_selectors)
            .unwrap_or_default();

        for scenario in node.scenarios {
            let suggested_selector = selectors
                .get(&scenario.name)
                .cloned()
                .unwrap_or_else(|| selector_from_scenario(&scenario.name));
            obligations.push(TestObligation {
                requirement_id: node.id.clone(),
                scenario_name: scenario.name.clone(),
                suggested_selector,
                verification_strength: "contract".into(),
                spec_path: spec_paths.first().cloned(),
                required_evidence: required_evidence_for(qa_class),
            });
        }
        if spec_paths.is_empty() {
            diagnostics.push(TestObligationDiagnostic {
                code: "test-obligation-missing-spec".into(),
                severity: "warning".into(),
                requirement_id: Some(node.id),
                message: "requirement has test obligations but no satisfying spec".into(),
            });
        }
    }

    obligations.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.scenario_name.cmp(&b.scenario_name))
    });
    diagnostics.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.code.cmp(&b.code))
    });

    TestObligationSet {
        version: 1,
        obligations,
        diagnostics,
    }
}

fn scenario_selectors(doc: &crate::spec_core::SpecDocument) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for section in &doc.sections {
        if let Section::AcceptanceCriteria { scenarios, .. } = section {
            for scenario in scenarios {
                if let Some(selector) = &scenario.test_selector {
                    out.insert(scenario.name.clone(), selector.filter.clone());
                }
            }
        }
    }
    out
}

fn selector_from_scenario(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    format!("test_{}", slug.trim_matches('_'))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_test_obligations_are_derived_from_requirements_and_specs() {
        let dir = make_temp_dir("test-obligations");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::create_dir_all(&specs).unwrap();

        fs::write(
            knowledge.join("requirements/req-note-create.md"),
            "---\nkind: requirement\nid: REQ-NOTE-CREATE\ntitle: \"Create Note\"\nliveness: auto\n---\n## Problem\nCreate notes.\n## Requirements\n[REQ-NOTE-CREATE] The note store MUST create notes.\n## Scenarios\nScenario: Create note\n  Given an empty store\n  When a note is created\n  Then the returned note appears in the list\n## Source Trace\n- test\n## Open Questions\nNone.\n",
        )
        .unwrap();
        fs::write(
            specs.join("task-req-note-create.spec.md"),
            "spec: task\nname: \"Create Note\"\nsatisfies: [REQ-NOTE-CREATE]\nrisk: A\n---\n## Intent\nCreate note.\n## Completion Criteria\nScenario: Create note\n  Test: note_create_adds_note\n  Given an empty store\n  When a note is created\n  Then the returned note appears in the list\n",
        )
        .unwrap();

        let obligations = build_test_obligations(&knowledge, &specs);

        assert_eq!(obligations.obligations.len(), 1);
        let obligation = &obligations.obligations[0];
        assert_eq!(obligation.requirement_id, "REQ-NOTE-CREATE");
        assert_eq!(obligation.scenario_name, "Create note");
        assert_eq!(obligation.suggested_selector, "note_create_adds_note");
        assert_eq!(obligation.verification_strength, "contract");
        assert_eq!(
            obligation.required_evidence,
            vec![
                crate::spec_qa::QaEvidenceKind::Lifecycle,
                crate::spec_qa::QaEvidenceKind::Trace,
                crate::spec_qa::QaEvidenceKind::TargetedTests,
                crate::spec_qa::QaEvidenceKind::AdversarialReview,
            ]
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_test_obligations_propagate_missing_and_invalid_compiler_inputs() {
        let dir = make_temp_dir("test-obligations-invalid-input");
        let missing =
            build_test_obligations(&dir.join("missing-knowledge"), &dir.join("missing-specs"));
        assert!(missing.obligations.is_empty());
        assert!(
            missing
                .diagnostics
                .iter()
                .any(|diag| { diag.code == "knowledge-root-missing" && diag.severity == "error" })
        );
        assert!(
            missing
                .diagnostics
                .iter()
                .any(|diag| { diag.code == "spec-root-missing" && diag.severity == "error" })
        );
        fs::remove_dir_all(dir).ok();
    }
}
