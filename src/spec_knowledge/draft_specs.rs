//! Reviewable Task Contract drafts from ready requirement work units.

use crate::spec_knowledge::{RequirementNode, WorkUnit, WorkUnitStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSpec {
    pub filename: String,
    pub content: String,
}

pub fn render_draft_spec(node: &RequirementNode, unit: &WorkUnit) -> Option<DraftSpec> {
    if unit.status != WorkUnitStatus::Ready {
        return None;
    }
    if crate::spec_knowledge::validate_knowledge_id(&node.id).is_err()
        || crate::spec_knowledge::validate_knowledge_id(&unit.requirement_id).is_err()
        || unit
            .satisfies
            .iter()
            .any(|id| crate::spec_knowledge::validate_knowledge_id(id).is_err())
    {
        return None;
    }
    let filename = draft_spec_filename(unit);
    let mut content = String::new();
    content.push_str("spec: task\n");
    content.push_str(&format!("name: \"{}\"\n", escape_title(&node.title)));
    content.push_str(&format!("tags: [{}]\n", draft_tags(node)));
    if !unit.depends_on.is_empty() {
        content.push_str(&format!("depends: [{}]\n", unit.depends_on.join(", ")));
    }
    content.push_str(&format!("satisfies: [{}]\n", unit.satisfies.join(", ")));
    content.push_str("---\n\n");

    content.push_str("## Intent\n\n");
    if node.problem.trim().is_empty() {
        content.push_str(&format!("Implement requirement `{}`.", node.id));
    } else {
        content.push_str(node.problem.trim());
    }
    content.push_str("\n\n## Decisions\n\n");
    content.push_str("- Generated draft from KLL requirement artifact; human review must confirm boundaries and test selectors before implementation.\n");
    for clause in &node.clauses {
        content.push_str("- ");
        content.push_str(&clause.text);
        content.push('\n');
    }

    content.push_str("\n## Boundaries\n\n");
    content.push_str("### Allowed Changes\n");
    content.push_str("- src/**\n");
    content.push_str("- tests/**\n\n");
    content.push_str("### Forbidden\n");
    content.push_str("- Do not weaken or remove the source requirement clauses.\n");
    content.push_str("- Do not mark this generated draft complete until each `Test:` selector names a real test.\n\n");

    content.push_str("## Completion Criteria\n\n");
    for scenario in &node.scenarios {
        content.push_str(&format!("Scenario: {}\n", scenario.name));
        content.push_str(&format!(
            "  Test: {}\n",
            pending_test_name(&node.id, &scenario.name)
        ));
        for step in &scenario.steps {
            content.push_str(&format!("  {} {}\n", step.keyword, step.content));
        }
        content.push('\n');
    }

    if !node.source_trace.is_empty() || !node.open_questions.is_empty() {
        content.push_str("## Questions\n\n");
        if !node.source_trace.is_empty() {
            content.push_str("- Source trace: ");
            content.push_str(&node.source_trace.join(", "));
            content.push('\n');
        }
        for question in &node.open_questions {
            content.push_str("- ");
            content.push_str(question);
            content.push('\n');
        }
        content.push_str("- Replace pending test selectors with real test names before lifecycle verification.\n");
    }

    Some(DraftSpec { filename, content })
}

pub fn draft_spec_filename(unit: &WorkUnit) -> String {
    format!(
        "task-{}-{}.spec.md",
        unit.requirement_id.to_ascii_lowercase(),
        slugify(&unit.title)
    )
}

fn draft_tags(node: &RequirementNode) -> String {
    let mut tags = vec!["requirements".to_string(), "generated-draft".to_string()];
    for tag in &node.tags {
        if !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.clone());
        }
    }
    tags.join(", ")
}

fn pending_test_name(requirement_id: &str, scenario_name: &str) -> String {
    format!(
        "pending_{}_{}",
        requirement_id.to_ascii_lowercase().replace('-', "_"),
        slugify(scenario_name).replace('-', "_")
    )
}

fn escape_title(input: &str) -> String {
    input.replace('"', "\\\"")
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        "requirement".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::{
        RequirementClauseView, RequirementNode, RequirementScenario, RequirementStep, WorkUnit,
        WorkUnitMode, WorkUnitStatus,
    };
    use std::path::PathBuf;

    #[test]
    fn test_draft_specs_render_satisfies_and_bdd_scenarios() {
        let node = RequirementNode {
            id: "REQ-101".into(),
            title: "User Login".into(),
            status: None,
            source_path: PathBuf::from("knowledge/requirements/req-101-user-login.md"),
            problem: "Users need to log in with existing accounts.".into(),
            clauses: vec![RequirementClauseView {
                id: Some("REQ-101".into()),
                keyword: Some("MUST".into()),
                text: "The authentication service MUST create a login session.".into(),
            }],
            dependencies: vec!["REQ-100".into()],
            children: Vec::new(),
            scenarios: vec![RequirementScenario {
                name: "Valid login".into(),
                steps: vec![
                    RequirementStep {
                        keyword: "Given".into(),
                        content: "the visitor has a valid persisted account".into(),
                    },
                    RequirementStep {
                        keyword: "When".into(),
                        content: "the visitor submits valid credentials".into(),
                    },
                    RequirementStep {
                        keyword: "Then".into(),
                        content: "the system establishes a login session".into(),
                    },
                ],
            }],
            source_trace: vec!["issue:#123".into()],
            open_questions: Vec::new(),
            tags: vec!["auth".into()],
        };
        let unit = WorkUnit {
            id: "WU-REQ-101".into(),
            requirement_id: "REQ-101".into(),
            title: "User Login".into(),
            source_path: node.source_path.clone(),
            mode: WorkUnitMode::LeafFull,
            status: WorkUnitStatus::Ready,
            depends_on: vec!["REQ-100".into()],
            satisfies: vec!["REQ-101".into()],
            scenario_count: 1,
            blocked_by: Vec::new(),
        };

        let draft = render_draft_spec(&node, &unit).unwrap();
        assert_eq!(draft.filename, "task-req-101-user-login.spec.md");
        assert!(draft.content.contains("spec: task"));
        assert!(draft.content.contains("satisfies: [REQ-101]"));
        assert!(draft.content.contains("## Intent"));
        assert!(
            draft
                .content
                .contains("Users need to log in with existing accounts.")
        );
        assert!(draft.content.contains("Scenario: Valid login"));
        assert!(draft.content.contains("Test: pending_req_101_valid_login"));
        assert!(
            draft
                .content
                .contains("Given the visitor has a valid persisted account")
        );
    }

    #[test]
    fn test_draft_specs_reject_unsafe_requirement_ids() {
        let mut node = RequirementNode {
            id: "../../REQ-ESCAPE".into(),
            title: "Escape".into(),
            status: None,
            source_path: PathBuf::from("knowledge/requirements/escape.md"),
            problem: "Escape output root.".into(),
            clauses: Vec::new(),
            dependencies: Vec::new(),
            children: Vec::new(),
            scenarios: vec![RequirementScenario {
                name: "Escape".into(),
                steps: Vec::new(),
            }],
            source_trace: Vec::new(),
            open_questions: Vec::new(),
            tags: Vec::new(),
        };
        let mut unit = WorkUnit {
            id: "WU-../../REQ-ESCAPE".into(),
            requirement_id: node.id.clone(),
            title: node.title.clone(),
            source_path: node.source_path.clone(),
            mode: WorkUnitMode::LeafFull,
            status: WorkUnitStatus::Ready,
            depends_on: Vec::new(),
            satisfies: vec![node.id.clone()],
            scenario_count: 1,
            blocked_by: Vec::new(),
        };
        assert!(render_draft_spec(&node, &unit).is_none());

        node.id = "/tmp/REQ-ESCAPE".into();
        unit.requirement_id = node.id.clone();
        unit.satisfies = vec![node.id.clone()];
        assert!(render_draft_spec(&node, &unit).is_none());
    }
}
