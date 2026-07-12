use crate::spec_knowledge::{RequirementPlan, RequirementPlanEdgeKind, RequirementPlanStatus};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeManifest {
    pub version: u32,
    pub entries: Vec<WorktreeEntry>,
    pub diagnostics: Vec<WorktreeDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub work_unit_id: String,
    pub requirement_id: String,
    pub batch: usize,
    pub base_branch: String,
    pub branch: String,
    pub path: PathBuf,
    pub spec_path: PathBuf,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeDiagnostic {
    pub code: String,
    pub severity: String,
    pub requirement_id: Option<String>,
    pub message: String,
}

pub fn build_worktree_manifest(
    plan: &RequirementPlan,
    base_branch: &str,
    path_prefix: &Path,
) -> WorktreeManifest {
    let mut entries = Vec::new();
    let mut diagnostics = plan
        .diagnostics
        .iter()
        .map(|diagnostic| WorktreeDiagnostic {
            code: diagnostic.code.clone(),
            severity: diagnostic.severity.clone(),
            requirement_id: diagnostic.requirement_id.clone(),
            message: diagnostic.message.clone(),
        })
        .collect::<Vec<_>>();
    diagnostics.extend(plan.parse_errors.iter().map(|error| WorktreeDiagnostic {
        code: "knowledge-parse-error".into(),
        severity: "error".into(),
        requirement_id: None,
        message: format!("{}: {}", error.path.display(), error.message),
    }));
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return WorktreeManifest {
            version: 1,
            entries,
            diagnostics,
        };
    }

    for node in &plan.requirements {
        if node.status != RequirementPlanStatus::Ready {
            continue;
        }
        let Some(coverage) = plan
            .coverage
            .iter()
            .find(|entry| entry.requirement_id == node.id)
        else {
            diagnostics.push(missing_spec_diag(&node.id));
            continue;
        };
        let Some(spec_path) = coverage.spec_paths.first() else {
            diagnostics.push(missing_spec_diag(&node.id));
            continue;
        };
        let slug = requirement_slug(&node.id);
        let Some(batch) = batch_for(plan, &node.id) else {
            diagnostics.push(WorktreeDiagnostic {
                code: "worktree-batch-missing".into(),
                severity: "error".into(),
                requirement_id: Some(node.id.clone()),
                message: format!("{} has no executable plan batch", node.id),
            });
            continue;
        };
        entries.push(WorktreeEntry {
            work_unit_id: format!("WU-{}", node.id),
            requirement_id: node.id.clone(),
            batch,
            base_branch: base_branch.to_string(),
            branch: format!("feat/wu-{slug}"),
            path: path_prefix.join(format!("wu-{slug}")),
            spec_path: spec_path.clone(),
            depends_on: dependency_ids(plan, &node.id),
        });
    }

    entries.sort_by(|a, b| {
        a.batch
            .cmp(&b.batch)
            .then_with(|| a.requirement_id.cmp(&b.requirement_id))
    });
    diagnostics.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.code.cmp(&b.code))
    });

    WorktreeManifest {
        version: 1,
        entries,
        diagnostics,
    }
}

fn requirement_slug(id: &str) -> String {
    id.to_ascii_lowercase().replace('_', "-")
}

fn batch_for(plan: &RequirementPlan, requirement_id: &str) -> Option<usize> {
    plan.batches
        .iter()
        .find(|batch| batch.requirement_ids.iter().any(|id| id == requirement_id))
        .map(|batch| batch.order)
}

fn dependency_ids(plan: &RequirementPlan, requirement_id: &str) -> Vec<String> {
    let mut deps = plan
        .edges
        .iter()
        .filter(|edge| {
            edge.to == requirement_id && edge.kind == RequirementPlanEdgeKind::Dependency
        })
        .map(|edge| edge.from.clone())
        .collect::<Vec<_>>();
    deps.sort();
    deps.dedup();
    deps
}

fn missing_spec_diag(requirement_id: &str) -> WorktreeDiagnostic {
    WorktreeDiagnostic {
        code: "worktree-missing-spec".into(),
        severity: "error".into(),
        requirement_id: Some(requirement_id.to_string()),
        message: format!("{requirement_id} is ready but has no spec path for worktree execution"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::{
        RequirementPlan, RequirementPlanBatch, RequirementPlanNode, RequirementPlanStatus,
        RequirementSpecCoverage,
    };
    use std::path::PathBuf;

    #[test]
    fn test_worktree_manifest_maps_ready_units_only() {
        let plan = RequirementPlan {
            version: 1,
            requirements: vec![
                RequirementPlanNode {
                    id: "REQ-NOTE-CREATE".into(),
                    title: "Create Note".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-note-create.md"),
                    status: RequirementPlanStatus::Ready,
                    mode: "leaf_full".into(),
                    scenario_count: 1,
                    blocked_by: Vec::new(),
                },
                RequirementPlanNode {
                    id: "REQ-NOTE-EXPORT".into(),
                    title: "Export Note".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-note-export.md"),
                    status: RequirementPlanStatus::Blocked,
                    mode: "blocked_questions".into(),
                    scenario_count: 1,
                    blocked_by: vec!["Should CSV be supported?".into()],
                },
            ],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: vec![RequirementPlanBatch {
                order: 1,
                requirement_ids: vec!["REQ-NOTE-CREATE".into(), "REQ-NOTE-EXPORT".into()],
            }],
            coverage: vec![RequirementSpecCoverage {
                requirement_id: "REQ-NOTE-CREATE".into(),
                spec_paths: vec![PathBuf::from("specs/task-req-note-create.spec.md")],
                spec_depends: Vec::new(),
            }],
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };

        let manifest =
            build_worktree_manifest(&plan, "main", PathBuf::from("../worktrees").as_path());

        assert_eq!(manifest.entries.len(), 1);
        let entry = &manifest.entries[0];
        assert_eq!(entry.work_unit_id, "WU-REQ-NOTE-CREATE");
        assert_eq!(entry.requirement_id, "REQ-NOTE-CREATE");
        assert_eq!(entry.batch, 1);
        assert_eq!(entry.base_branch, "main");
        assert_eq!(entry.branch, "feat/wu-req-note-create");
        assert_eq!(entry.path, PathBuf::from("../worktrees/wu-req-note-create"));
        assert_eq!(
            entry.spec_path,
            PathBuf::from("specs/task-req-note-create.spec.md")
        );
    }

    #[test]
    fn test_worktree_manifest_propagates_plan_errors_and_emits_no_entries() {
        let mut plan = RequirementPlan {
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
            batches: Vec::new(),
            coverage: vec![RequirementSpecCoverage {
                requirement_id: "REQ-A".into(),
                spec_paths: vec![PathBuf::from("specs/task-a.spec.md")],
                spec_depends: Vec::new(),
            }],
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };
        plan.diagnostics
            .push(crate::spec_knowledge::RequirementPlanDiagnostic {
                code: "dependency-cycle".into(),
                severity: "error".into(),
                requirement_id: Some("REQ-A".into()),
                message: "cycle".into(),
            });

        let manifest = build_worktree_manifest(&plan, "main", Path::new("../worktrees"));
        assert!(manifest.entries.is_empty());
        assert!(
            manifest
                .diagnostics
                .iter()
                .any(|diag| diag.code == "dependency-cycle")
        );
    }
}
