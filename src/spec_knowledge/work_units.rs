//! Executable work-unit generation from requirement graph nodes.

use crate::spec_knowledge::{RequirementGraph, RequirementGraphDiagnostic, RequirementNode};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkUnitSet {
    pub version: u32,
    pub units: Vec<WorkUnit>,
    pub diagnostics: Vec<RequirementGraphDiagnostic>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkUnit {
    pub id: String,
    pub requirement_id: String,
    pub title: String,
    pub source_path: PathBuf,
    pub mode: WorkUnitMode,
    pub status: WorkUnitStatus,
    pub depends_on: Vec<String>,
    pub satisfies: Vec<String>,
    pub scenario_count: usize,
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitMode {
    LeafFull,
    ParentScenario,
    GroupingOnly,
    BlockedQuestions,
    MissingScenarios,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitStatus {
    Ready,
    Informational,
    Blocked,
}

impl WorkUnitSet {
    pub fn unit(&self, requirement_id: &str) -> Option<&WorkUnit> {
        self.units
            .iter()
            .find(|unit| unit.requirement_id == requirement_id)
    }
}

pub fn build_work_units(graph: &RequirementGraph) -> WorkUnitSet {
    let mut units = graph.nodes.iter().map(unit_from_node).collect::<Vec<_>>();
    units.sort_by(|a, b| a.requirement_id.cmp(&b.requirement_id));
    WorkUnitSet {
        version: 1,
        units,
        diagnostics: graph.diagnostics.clone(),
    }
}

fn unit_from_node(node: &RequirementNode) -> WorkUnit {
    let has_questions = !node.open_questions.is_empty();
    let has_scenarios = !node.scenarios.is_empty();
    let has_children = !node.children.is_empty();
    let mode = if has_questions {
        WorkUnitMode::BlockedQuestions
    } else if has_scenarios && has_children {
        WorkUnitMode::ParentScenario
    } else if has_scenarios {
        WorkUnitMode::LeafFull
    } else if has_children {
        WorkUnitMode::GroupingOnly
    } else {
        WorkUnitMode::MissingScenarios
    };
    let mut status = match mode {
        WorkUnitMode::LeafFull | WorkUnitMode::ParentScenario => WorkUnitStatus::Ready,
        WorkUnitMode::GroupingOnly => WorkUnitStatus::Informational,
        WorkUnitMode::BlockedQuestions | WorkUnitMode::MissingScenarios => WorkUnitStatus::Blocked,
    };
    if !matches!(
        node.status,
        Some(crate::spec_knowledge::DecisionStatus::Accepted)
    ) {
        // Missing or non-accepted governance status never schedules work
        // (docs/intent-compiler/architecture.md, Requirement Governance Gate).
        status = WorkUnitStatus::Informational;
    }
    WorkUnit {
        id: format!("WU-{}", node.id),
        requirement_id: node.id.clone(),
        title: node.title.clone(),
        source_path: node.source_path.clone(),
        mode,
        status,
        depends_on: node.dependencies.clone(),
        satisfies: vec![node.id.clone()],
        scenario_count: node.scenarios.len(),
        blocked_by: node.open_questions.clone(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::{
        RequirementClauseView, RequirementGraph, RequirementNode, RequirementScenario,
        RequirementStep,
    };
    use std::path::PathBuf;

    fn node(
        id: &str,
        scenarios: usize,
        children: Vec<&str>,
        open_questions: Vec<&str>,
    ) -> RequirementNode {
        RequirementNode {
            id: id.to_string(),
            title: format!("{id} title"),
            status: Some(crate::spec_knowledge::DecisionStatus::Accepted),
            source_path: PathBuf::from(format!(
                "knowledge/requirements/{}.md",
                id.to_ascii_lowercase()
            )),
            problem: format!("{id} problem"),
            clauses: vec![RequirementClauseView {
                id: Some(id.to_string()),
                keyword: Some("MUST".to_string()),
                text: format!("The system MUST satisfy {id}."),
            }],
            dependencies: Vec::new(),
            children: children.into_iter().map(str::to_string).collect(),
            scenarios: (0..scenarios)
                .map(|idx| RequirementScenario {
                    name: format!("Scenario {idx}"),
                    steps: vec![
                        RequirementStep {
                            keyword: "Given".into(),
                            content: "a precondition".into(),
                        },
                        RequirementStep {
                            keyword: "When".into(),
                            content: "an action happens".into(),
                        },
                        RequirementStep {
                            keyword: "Then".into(),
                            content: "an outcome is visible".into(),
                        },
                    ],
                })
                .collect(),
            source_trace: Vec::new(),
            open_questions: open_questions.into_iter().map(str::to_string).collect(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn test_work_units_skip_grouping_and_block_open_questions() {
        let graph = RequirementGraph {
            nodes: vec![
                node("REQ-101", 1, vec![], vec![]),
                node("REQ-200", 0, vec!["REQ-201"], vec![]),
                node("REQ-300", 1, vec![], vec!["Should this support SSO?"]),
                node("REQ-400", 0, vec![], vec![]),
            ],
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };

        let set = build_work_units(&graph);
        assert_eq!(set.units.len(), 4);
        assert_eq!(set.unit("REQ-101").unwrap().mode, WorkUnitMode::LeafFull);
        assert_eq!(set.unit("REQ-101").unwrap().status, WorkUnitStatus::Ready);
        assert_eq!(
            set.unit("REQ-200").unwrap().mode,
            WorkUnitMode::GroupingOnly
        );
        assert_eq!(
            set.unit("REQ-300").unwrap().mode,
            WorkUnitMode::BlockedQuestions
        );
        assert_eq!(set.unit("REQ-300").unwrap().status, WorkUnitStatus::Blocked);
        assert_eq!(
            set.unit("REQ-400").unwrap().mode,
            WorkUnitMode::MissingScenarios
        );
        assert_eq!(set.unit("REQ-400").unwrap().status, WorkUnitStatus::Blocked);
    }
}
