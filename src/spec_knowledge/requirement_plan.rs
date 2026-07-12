use crate::spec_knowledge::{
    RequirementGraphDiagnostic, WorkUnit, WorkUnitMode, WorkUnitStatus, build_requirement_graph,
    build_work_units, validate_requirement_graph,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementPlan {
    pub version: u32,
    pub requirements: Vec<RequirementPlanNode>,
    pub work_units: Vec<WorkUnit>,
    pub specs: Vec<RequirementPlanSpecNode>,
    pub edges: Vec<RequirementPlanEdge>,
    pub batches: Vec<RequirementPlanBatch>,
    pub coverage: Vec<RequirementSpecCoverage>,
    pub diagnostics: Vec<RequirementPlanDiagnostic>,
    pub parse_errors: Vec<crate::spec_knowledge::KnowledgeParseErrorView>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementPlanSpecNode {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub depends: Vec<String>,
    pub satisfies: Vec<String>,
    pub risk: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementPlanNode {
    pub id: String,
    pub title: String,
    pub source_path: PathBuf,
    pub status: RequirementPlanStatus,
    pub mode: String,
    pub scenario_count: usize,
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementPlanStatus {
    Ready,
    Informational,
    Blocked,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementPlanEdge {
    pub from: String,
    pub to: String,
    pub kind: RequirementPlanEdgeKind,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RequirementPlanEdgeKind {
    Dependency,
    Child,
    WorkUnit,
    Satisfies,
    SpecDepends,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementPlanBatch {
    pub order: usize,
    pub requirement_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementSpecCoverage {
    pub requirement_id: String,
    pub spec_paths: Vec<PathBuf>,
    pub spec_depends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementPlanDiagnostic {
    pub code: String,
    pub severity: String,
    pub requirement_id: Option<String>,
    pub message: String,
}

pub fn build_requirement_plan(knowledge_dir: &Path, specs_dir: &Path) -> RequirementPlan {
    let mut graph = build_requirement_graph(knowledge_dir);
    graph.diagnostics.extend(validate_requirement_graph(&graph));
    graph.diagnostics.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });

    let work_units = build_work_units(&graph);
    let (spec_nodes, coverage_by_req, mut spec_diagnostics) = collect_spec_coverage(specs_dir);

    let requirements = work_units
        .units
        .iter()
        .map(|unit| RequirementPlanNode {
            id: unit.requirement_id.clone(),
            title: unit.title.clone(),
            source_path: unit.source_path.clone(),
            status: match unit.status {
                WorkUnitStatus::Ready => RequirementPlanStatus::Ready,
                WorkUnitStatus::Informational => RequirementPlanStatus::Informational,
                WorkUnitStatus::Blocked => RequirementPlanStatus::Blocked,
            },
            mode: work_unit_mode_name(unit.mode).to_string(),
            scenario_count: unit.scenario_count,
            blocked_by: unit.blocked_by.clone(),
        })
        .collect::<Vec<_>>();

    let mut edges = Vec::new();
    for node in &graph.nodes {
        for dep in &node.dependencies {
            edges.push(RequirementPlanEdge {
                from: dep.clone(),
                to: node.id.clone(),
                kind: RequirementPlanEdgeKind::Dependency,
            });
        }
        for child in &node.children {
            edges.push(RequirementPlanEdge {
                from: node.id.clone(),
                to: child.clone(),
                kind: RequirementPlanEdgeKind::Child,
            });
        }
    }
    for unit in &work_units.units {
        edges.push(RequirementPlanEdge {
            from: unit.requirement_id.clone(),
            to: unit.id.clone(),
            kind: RequirementPlanEdgeKind::WorkUnit,
        });
    }
    let known_requirements = requirements
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let known_specs = spec_nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    for spec in &spec_nodes {
        for requirement_id in &spec.satisfies {
            if known_requirements.contains(requirement_id.as_str()) {
                edges.push(RequirementPlanEdge {
                    from: format!("WU-{requirement_id}"),
                    to: spec.id.clone(),
                    kind: RequirementPlanEdgeKind::Satisfies,
                });
            } else {
                spec_diagnostics.push(RequirementPlanDiagnostic {
                    code: "dangling-spec-coverage".into(),
                    severity: "error".into(),
                    requirement_id: Some(requirement_id.clone()),
                    message: format!(
                        "{} satisfies missing requirement {requirement_id}",
                        spec.path.display()
                    ),
                });
            }
        }
        for dependency in &spec.depends {
            if known_specs.contains(dependency.as_str()) {
                edges.push(RequirementPlanEdge {
                    from: dependency.clone(),
                    to: spec.id.clone(),
                    kind: RequirementPlanEdgeKind::SpecDepends,
                });
            } else {
                spec_diagnostics.push(RequirementPlanDiagnostic {
                    code: "dangling-spec-dependency".into(),
                    severity: "error".into(),
                    requirement_id: None,
                    message: format!(
                        "{} depends on missing spec {dependency}",
                        spec.path.display()
                    ),
                });
            }
        }
    }

    let mut coverage = coverage_by_req
        .into_iter()
        .map(|(requirement_id, spec_info)| RequirementSpecCoverage {
            requirement_id,
            spec_paths: spec_info.spec_paths,
            spec_depends: spec_info.spec_depends,
        })
        .collect::<Vec<_>>();

    for req in &requirements {
        if !coverage.iter().any(|entry| entry.requirement_id == req.id) {
            coverage.push(RequirementSpecCoverage {
                requirement_id: req.id.clone(),
                spec_paths: Vec::new(),
                spec_depends: Vec::new(),
            });
        }
    }

    coverage.sort_by(|a, b| a.requirement_id.cmp(&b.requirement_id));
    edges.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then_with(|| a.to.cmp(&b.to))
            .then_with(|| a.kind.cmp(&b.kind))
    });

    let batches = build_batches(&requirements, &edges);
    let diagnostics = graph
        .diagnostics
        .iter()
        .map(plan_diag_from_graph_diag)
        .collect::<Vec<_>>();

    let mut plan = RequirementPlan {
        version: 1,
        work_units: work_units.units.clone(),
        requirements,
        specs: spec_nodes,
        edges,
        batches,
        coverage,
        diagnostics,
        parse_errors: graph.parse_errors,
    };
    plan.diagnostics.extend(validate_requirement_plan(&plan));
    plan.diagnostics.append(&mut spec_diagnostics);
    plan.diagnostics.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });
    plan
}

#[derive(Default)]
struct SpecCoverageAccumulator {
    spec_paths: Vec<PathBuf>,
    spec_depends: Vec<String>,
}

fn collect_spec_coverage(
    specs_dir: &Path,
) -> (
    Vec<RequirementPlanSpecNode>,
    BTreeMap<String, SpecCoverageAccumulator>,
    Vec<RequirementPlanDiagnostic>,
) {
    let mut out: BTreeMap<String, SpecCoverageAccumulator> = BTreeMap::new();
    let mut specs = Vec::new();
    let mut diagnostics = Vec::new();
    if !specs_dir.is_dir() {
        diagnostics.push(RequirementPlanDiagnostic {
            code: "spec-root-missing".into(),
            severity: "error".into(),
            requirement_id: None,
            message: format!(
                "spec root does not exist or is not a directory: {}",
                specs_dir.display()
            ),
        });
        return (specs, out, diagnostics);
    }
    for path in spec_files(specs_dir, &mut diagnostics) {
        let doc = match crate::spec_parser::parse_spec(&path) {
            Ok(doc) => doc,
            Err(err) => {
                diagnostics.push(RequirementPlanDiagnostic {
                    code: "spec-parse-error".into(),
                    severity: "error".into(),
                    requirement_id: None,
                    message: format!("cannot parse {}: {err}", path.display()),
                });
                continue;
            }
        };
        let Some(id) = spec_id_from_path(&path) else {
            diagnostics.push(RequirementPlanDiagnostic {
                code: "spec-id-invalid".into(),
                severity: "error".into(),
                requirement_id: None,
                message: format!("cannot derive stable spec id from {}", path.display()),
            });
            continue;
        };
        specs.push(RequirementPlanSpecNode {
            id,
            name: doc.meta.name.clone(),
            path: path.clone(),
            depends: doc.meta.depends.clone(),
            satisfies: doc.meta.satisfies.clone(),
            risk: doc.meta.risk.clone(),
        });
        for req in &doc.meta.satisfies {
            if !req.starts_with("REQ-") {
                continue;
            }
            let entry = out.entry(req.clone()).or_default();
            entry.spec_paths.push(path.clone());
            entry.spec_depends.extend(doc.meta.depends.clone());
        }
    }
    for value in out.values_mut() {
        value.spec_paths.sort();
        value.spec_paths.dedup();
        value.spec_depends.sort();
        value.spec_depends.dedup();
    }
    specs.sort_by(|left, right| left.id.cmp(&right.id));
    for pair in specs.windows(2) {
        if pair[0].id == pair[1].id {
            diagnostics.push(RequirementPlanDiagnostic {
                code: "duplicate-spec-id".into(),
                severity: "error".into(),
                requirement_id: None,
                message: format!("spec id {} is declared more than once", pair[0].id),
            });
        }
    }
    (specs, out, diagnostics)
}

fn spec_id_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    name.strip_suffix(".spec.md")
        .or_else(|| name.strip_suffix(".spec"))
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn build_batches(
    nodes: &[RequirementPlanNode],
    edges: &[RequirementPlanEdge],
) -> Vec<RequirementPlanBatch> {
    let ready_ids = nodes
        .iter()
        .filter(|node| node.status == RequirementPlanStatus::Ready)
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let mut indegree = ready_ids
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for edge in edges {
        if edge.kind != RequirementPlanEdgeKind::Dependency {
            continue;
        }
        if ready_ids.contains(&edge.from) && ready_ids.contains(&edge.to) {
            outgoing
                .entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
            *indegree.entry(edge.to.clone()).or_insert(0) += 1;
        }
    }

    let mut queue = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
        .collect::<VecDeque<_>>();
    let mut batches = Vec::new();
    let mut order = 1;

    while !queue.is_empty() {
        let count = queue.len();
        let mut ids = Vec::new();
        for _ in 0..count {
            if let Some(id) = queue.pop_front() {
                ids.push(id.clone());
                for next in outgoing.get(&id).into_iter().flatten() {
                    if let Some(degree) = indegree.get_mut(next) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(next.clone());
                        }
                    }
                }
            }
        }
        ids.sort();
        batches.push(RequirementPlanBatch {
            order,
            requirement_ids: ids,
        });
        order += 1;
    }

    batches
}

fn spec_files(dir: &Path, diagnostics: &mut Vec<RequirementPlanDiagnostic>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_spec_files(dir, &mut out, diagnostics);
    out.sort();
    out
}

fn collect_spec_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<RequirementPlanDiagnostic>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            diagnostics.push(RequirementPlanDiagnostic {
                code: "spec-entry-type-unreadable".into(),
                severity: "error".into(),
                requirement_id: None,
                message: format!("cannot read spec entry type: {}", path.display()),
            });
            continue;
        };
        if file_type.is_symlink() {
            diagnostics.push(RequirementPlanDiagnostic {
                code: "spec-symlink-rejected".into(),
                severity: "error".into(),
                requirement_id: None,
                message: format!("spec plan traversal rejects symlink: {}", path.display()),
            });
        } else if file_type.is_dir() {
            if !should_skip_spec_dir(&path) {
                collect_spec_files(&path, out, diagnostics);
            }
        } else if file_type.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name.ends_with(".spec.md") || name.ends_with(".spec") {
                out.push(path);
            }
        }
    }
}

fn should_skip_spec_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".agent-spec" | "_archive" | "archive" | "roadmap")
    )
}

pub fn validate_requirement_plan(plan: &RequirementPlan) -> Vec<RequirementPlanDiagnostic> {
    let mut diagnostics = Vec::new();
    for node in &plan.requirements {
        if node.status == RequirementPlanStatus::Ready {
            let coverage = plan
                .coverage
                .iter()
                .find(|entry| entry.requirement_id == node.id);
            if coverage.is_none_or(|entry| entry.spec_paths.is_empty()) {
                diagnostics.push(RequirementPlanDiagnostic {
                    code: "requirement-uncovered".into(),
                    severity: "error".into(),
                    requirement_id: Some(node.id.clone()),
                    message: format!("{} is ready but has no satisfying spec", node.id),
                });
            }
            if coverage.is_some_and(|entry| entry.spec_paths.len() > 1) {
                diagnostics.push(RequirementPlanDiagnostic {
                    code: "requirement-multiple-specs".into(),
                    severity: "error".into(),
                    requirement_id: Some(node.id.clone()),
                    message: format!(
                        "{} has multiple satisfying specs; one ready work unit requires one task contract",
                        node.id
                    ),
                });
            }
        }
        if node.status == RequirementPlanStatus::Blocked {
            diagnostics.push(RequirementPlanDiagnostic {
                code: "requirement-blocked".into(),
                severity: "warning".into(),
                requirement_id: Some(node.id.clone()),
                message: format!("{} is blocked: {}", node.id, node.blocked_by.join("; ")),
            });
        }
    }
    diagnostics.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });
    diagnostics
}

fn plan_diag_from_graph_diag(diag: &RequirementGraphDiagnostic) -> RequirementPlanDiagnostic {
    RequirementPlanDiagnostic {
        code: diag.code.clone(),
        severity: diag.severity.clone(),
        requirement_id: diag.requirement_id.clone(),
        message: diag.message.clone(),
    }
}

fn work_unit_mode_name(mode: WorkUnitMode) -> &'static str {
    match mode {
        WorkUnitMode::LeafFull => "leaf_full",
        WorkUnitMode::ParentScenario => "parent_scenario",
        WorkUnitMode::GroupingOnly => "grouping_only",
        WorkUnitMode::BlockedQuestions => "blocked_questions",
        WorkUnitMode::MissingScenarios => "missing_scenarios",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
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
    fn test_requirement_plan_builds_batches_edges_and_coverage() {
        let dir = make_temp_dir("requirement-plan-batches");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::create_dir_all(&specs).unwrap();

        fs::write(
            knowledge.join("requirements/req-note-create.md"),
            "---\nkind: requirement\nid: REQ-NOTE-CREATE\ntitle: \"Create Note\"\nstatus: accepted\nliveness: auto\n---\n## Problem\nCreate notes.\n## Requirements\n[REQ-NOTE-CREATE] The note service MUST create a note.\n## Scenarios\nScenario: Create note\n  Given an empty store\n  When a note is created\n  Then the created note is available in the store\n## Dependencies\nNone.\n## Source Trace\n- example:noteapp\n## Open Questions\nNone.\n",
        )
        .unwrap();
        fs::write(
            knowledge.join("requirements/req-note-list.md"),
            "---\nkind: requirement\nid: REQ-NOTE-LIST\ntitle: \"List Notes\"\nstatus: accepted\nliveness: auto\n---\n## Problem\nList notes.\n## Requirements\n[REQ-NOTE-LIST] The note service MUST list created notes.\n## Scenarios\nScenario: List notes\n  Given one created note\n  When notes are listed\n  Then the created note appears in the list\n## Dependencies\n- REQ-NOTE-CREATE\n## Source Trace\n- example:noteapp\n## Open Questions\nNone.\n",
        )
        .unwrap();
        fs::write(
            specs.join("task-req-note-create.spec.md"),
            "spec: task\nname: \"Create Note\"\nsatisfies: [REQ-NOTE-CREATE]\n---\n## Intent\nCreate note.\n## Completion Criteria\nScenario: Create note\n  Test: note_create_adds_note\n  Given an empty store\n  When a note is created\n  Then the note exists\n",
        )
        .unwrap();
        fs::write(
            specs.join("task-req-note-list.spec.md"),
            "spec: task\nname: \"List Notes\"\nsatisfies: [REQ-NOTE-LIST]\ndepends: [task-req-note-create]\n---\n## Intent\nList notes.\n## Completion Criteria\nScenario: List notes\n  Test: note_list_returns_created_notes\n  Given one note\n  When notes are listed\n  Then the note appears\n",
        )
        .unwrap();

        let plan = build_requirement_plan(&knowledge, &specs);

        assert_eq!(plan.version, 1);
        assert_eq!(plan.requirements.len(), 2);
        assert!(plan.edges.iter().any(|edge| {
            edge.from == "REQ-NOTE-CREATE"
                && edge.to == "REQ-NOTE-LIST"
                && edge.kind == RequirementPlanEdgeKind::Dependency
        }));
        assert_eq!(plan.batches.len(), 2);
        assert_eq!(plan.batches[0].requirement_ids, vec!["REQ-NOTE-CREATE"]);
        assert_eq!(plan.batches[1].requirement_ids, vec!["REQ-NOTE-LIST"]);
        assert_eq!(plan.specs.len(), 2);
        assert!(plan.edges.iter().any(|edge| {
            edge.from == "WU-REQ-NOTE-CREATE"
                && edge.to == "task-req-note-create"
                && edge.kind == RequirementPlanEdgeKind::Satisfies
        }));
        assert!(plan.edges.iter().any(|edge| {
            edge.from == "task-req-note-create"
                && edge.to == "task-req-note-list"
                && edge.kind == RequirementPlanEdgeKind::SpecDepends
        }));
        assert_eq!(
            plan.coverage
                .iter()
                .find(|coverage| coverage.requirement_id == "REQ-NOTE-CREATE")
                .unwrap()
                .spec_paths
                .len(),
            1
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirement_plan_validation_reports_uncovered_ready_requirement() {
        let mut plan = RequirementPlan {
            version: 1,
            requirements: vec![RequirementPlanNode {
                id: "REQ-UNCOVERED".into(),
                title: "Uncovered".into(),
                source_path: PathBuf::from("knowledge/requirements/req-uncovered.md"),
                status: RequirementPlanStatus::Ready,
                mode: "leaf_full".into(),
                scenario_count: 1,
                blocked_by: Vec::new(),
            }],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: vec![RequirementPlanBatch {
                order: 1,
                requirement_ids: vec!["REQ-UNCOVERED".into()],
            }],
            coverage: vec![RequirementSpecCoverage {
                requirement_id: "REQ-UNCOVERED".into(),
                spec_paths: Vec::new(),
                spec_depends: Vec::new(),
            }],
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };

        let diagnostics = validate_requirement_plan(&plan);
        assert!(
            diagnostics
                .iter()
                .any(|diag| { diag.code == "requirement-uncovered" && diag.severity == "error" })
        );

        plan.coverage[0]
            .spec_paths
            .push(PathBuf::from("specs/task.spec.md"));
        let diagnostics = validate_requirement_plan(&plan);
        assert!(
            !diagnostics
                .iter()
                .any(|diag| diag.code == "requirement-uncovered")
        );
    }

    #[test]
    fn test_requirement_plan_reports_missing_spec_root_and_dangling_spec_coverage() {
        let dir = make_temp_dir("requirement-plan-spec-errors");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::write(
            knowledge.join("requirements/req-a.md"),
            "---\nkind: requirement\nid: REQ-A\ntitle: A\n---\n## Problem\nA.\n## Requirements\n[REQ-A] The system MUST do A.\n## Scenarios\nScenario: A\n  Given input\n  When A runs\n  Then output is visible\n## Source Trace\n- test\n## Open Questions\nNone.\n",
        )
        .unwrap();

        let missing = build_requirement_plan(&knowledge, &specs);
        assert!(
            missing
                .diagnostics
                .iter()
                .any(|diag| { diag.code == "spec-root-missing" && diag.severity == "error" })
        );

        fs::create_dir_all(&specs).unwrap();
        fs::write(
            specs.join("task-typo.spec.md"),
            "spec: task\nname: Typo\nsatisfies: [REQ-TYPO]\n---\n## Intent\nTypo.\n## Completion Criteria\nScenario: Typo\n  Test: test_typo\n  Given input\n  When typo runs\n  Then output is visible\n",
        )
        .unwrap();
        let dangling = build_requirement_plan(&knowledge, &specs);
        assert!(
            dangling
                .diagnostics
                .iter()
                .any(|diag| { diag.code == "dangling-spec-coverage" && diag.severity == "error" })
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirement_plan_treats_superseded_requirements_and_roadmap_specs_as_inactive() {
        let dir = make_temp_dir("requirement-plan-active-set");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::create_dir_all(specs.join("roadmap")).unwrap();
        fs::write(
            knowledge.join("requirements/req-old.md"),
            "---\nkind: requirement\nid: REQ-OLD\ntitle: Old\nstatus: superseded\n---\n## Problem\nOld.\n## Requirements\n[REQ-OLD] The old system MUST remain historical.\n## Scenarios\nScenario: Old\n  Given history\n  When inspected\n  Then the historical marker is visible\n## Source Trace\n- test\n## Open Questions\nNone.\n",
        )
        .unwrap();
        let active = "spec: task\nname: Active\n---\n## Intent\nActive.\n## Completion Criteria\nScenario: Active\n  Test: test_active\n  Given input\n  When active runs\n  Then output is visible\n";
        fs::write(specs.join("task-active.spec.md"), active).unwrap();
        fs::write(specs.join("roadmap/task-active.spec.md"), active).unwrap();

        let plan = build_requirement_plan(&knowledge, &specs);

        assert_eq!(plan.specs.len(), 1);
        assert_eq!(
            plan.requirements[0].status,
            RequirementPlanStatus::Informational
        );
        assert!(!plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "duplicate-spec-id" || diagnostic.code == "requirement-uncovered"
        }));
        fs::remove_dir_all(dir).ok();
    }
}
