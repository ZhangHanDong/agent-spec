//! Three-axis requirement status: governance (persisted KLL), execution
//! progress (derived), and requirement liveness (recomputed). One aggregate
//! query, no conflation — per `docs/intent-compiler/architecture.md`,
//! "Three Independent State Axes". Read-only: nothing is stored.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::spec_core::Verdict;

#[derive(Debug, Clone, Serialize)]
pub struct RequirementStatusReport {
    pub id: String,
    pub governance: String,
    pub execution: String,
    pub liveness: String,
    pub active_specs: Vec<String>,
    pub staged_specs: Vec<String>,
    pub archived_specs: Vec<String>,
    pub work_unit: Option<String>,
}

/// Aggregate the three axes for one requirement id. `verify_fn` runs
/// verification for one active spec path (injectable for tests).
pub fn requirement_status<F>(
    knowledge_dir: &Path,
    specs_dir: &Path,
    archive_dir: &Path,
    id: &str,
    verify_fn: F,
) -> Result<RequirementStatusReport, String>
where
    F: FnMut(&Path) -> Verdict,
{
    let wanted = id.trim().to_ascii_uppercase();
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge_dir);
    let Some(node) = graph.node(&wanted) else {
        return Err(format!(
            "no requirement document under {} declares id {wanted}",
            knowledge_dir.display()
        ));
    };

    // axis 1: governance (persisted)
    let governance = match node.status {
        Some(crate::spec_knowledge::DecisionStatus::Proposed) => "proposed",
        Some(crate::spec_knowledge::DecisionStatus::Accepted) => "accepted",
        Some(crate::spec_knowledge::DecisionStatus::Superseded) => "superseded",
        Some(crate::spec_knowledge::DecisionStatus::Deprecated) => "deprecated",
        Some(crate::spec_knowledge::DecisionStatus::Rejected) => "rejected",
        None => "missing",
    }
    .to_string();

    // satisfying specs per tier; the staged tier is the roadmap subdirectory
    let active_index = crate::spec_knowledge::build_satisfies_index(specs_dir);
    let mut active_specs = index_paths(&active_index, &wanted);
    // exclude staged specs that the recursive index may have picked up
    let roadmap = specs_dir.join("roadmap");
    active_specs.retain(|p| !Path::new(p).starts_with(&roadmap));
    let staged_specs = if roadmap.is_dir() {
        index_paths(
            &crate::spec_knowledge::build_satisfies_index(&roadmap),
            &wanted,
        )
    } else {
        Vec::new()
    };
    let archived_specs = if archive_dir.is_dir() {
        index_paths(
            &crate::spec_knowledge::build_satisfies_index(archive_dir),
            &wanted,
        )
    } else {
        Vec::new()
    };

    // axis 3: liveness (recomputed from active specs only)
    let doc = crate::spec_knowledge::parse_requirement(&node.source_path)
        .map_err(|e| format!("{}: {e}", node.source_path.display()))?;
    let trace = crate::spec_knowledge::trace::build_trace(&doc, &active_index, verify_fn);
    let liveness = match trace.liveness {
        crate::spec_knowledge::Liveness::Honored => "honored",
        crate::spec_knowledge::Liveness::Violated => "violated",
        crate::spec_knowledge::Liveness::Unproven => "unproven",
        crate::spec_knowledge::Liveness::Na => "na",
    }
    .to_string();

    // work unit state feeds the execution ladder
    let units = crate::spec_knowledge::build_work_units(&graph);
    let work_unit = units
        .units
        .iter()
        .find(|u| u.requirement_id == wanted)
        .map(|u| {
            match u.status {
                crate::spec_knowledge::WorkUnitStatus::Ready => "ready",
                crate::spec_knowledge::WorkUnitStatus::Blocked => "blocked",
                crate::spec_knowledge::WorkUnitStatus::Informational => "informational",
            }
            .to_string()
        });

    // axis 2: execution ladder, top-down
    let execution = if !archived_specs.is_empty() {
        "archived"
    } else if !active_specs.is_empty() && liveness == "honored" {
        "verified"
    } else if !active_specs.is_empty() {
        "active"
    } else if work_unit.as_deref() == Some("ready") {
        "ready"
    } else if !staged_specs.is_empty() {
        "planned"
    } else {
        "unplanned"
    }
    .to_string();

    Ok(RequirementStatusReport {
        id: wanted,
        governance,
        execution,
        liveness,
        active_specs,
        staged_specs,
        archived_specs,
        work_unit,
    })
}

/// Three-line human summary.
pub fn format_status_text(report: &RequirementStatusReport) -> String {
    format!(
        "{}\n  governance: {}\n  execution:  {}{}\n  liveness:   {}\n",
        report.id,
        report.governance,
        report.execution,
        report
            .work_unit
            .as_ref()
            .map(|w| format!(" (work unit: {w})"))
            .unwrap_or_default(),
        report.liveness
    )
}

fn index_paths(index: &crate::spec_knowledge::SatisfiesIndex, id: &str) -> Vec<String> {
    index
        .get(id)
        .map(|paths| {
            let mut out: Vec<String> = paths
                .iter()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect();
            out.sort();
            out
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn make_tree(name: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let knowledge = base.join("knowledge");
        let specs = base.join("specs");
        let archive = base.join(".agent-spec/archive/specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::create_dir_all(&specs).unwrap();
        (base, knowledge, specs, archive)
    }

    fn write_req(knowledge: &Path, id: &str, status: &str, with_scenarios: bool) {
        let scenarios = if with_scenarios {
            "\n## Scenarios\n\nScenario: works\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n"
        } else {
            ""
        };
        fs::write(
            knowledge
                .join("requirements")
                .join(format!("req-{}.md", id.trim_start_matches("REQ-").to_ascii_lowercase())),
            format!(
                "---\nkind: requirement\nid: {id}\ntitle: \"T\"\nstatus: {status}\nliveness: auto\ntags: []\n---\n\n# T\n\n## Problem\n\np\n\n## Requirements\n\n[{id}-ONE] The system MUST hold.\n{scenarios}"
            ),
        )
        .unwrap();
    }

    fn write_spec(dir: &Path, name: &str, satisfies: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        fs::write(
            &path,
            format!(
                "spec: task\nname: \"S\"\nsatisfies: [{satisfies}]\n---\n## Intent\ni.\n## Completion Criteria\nScenario: s\n  Test: test_stub\n  Given a\n  When b\n  Then c\n"
            ),
        )
        .unwrap();
        path
    }

    #[test]
    fn test_requirement_status_reports_verified_axes() {
        let (base, knowledge, specs, archive) = make_tree("status-verified");
        write_req(&knowledge, "REQ-S-A", "accepted", true);
        let spec = write_spec(&specs, "task-a.spec.md", "REQ-S-A");

        let report =
            requirement_status(&knowledge, &specs, &archive, "REQ-S-A", |_| Verdict::Pass).unwrap();
        assert_eq!(report.governance, "accepted");
        assert_eq!(report.execution, "verified");
        assert_eq!(report.liveness, "honored");
        assert!(
            report
                .active_specs
                .iter()
                .any(|p| p.ends_with("task-a.spec.md")),
            "{report:?}"
        );
        assert!(spec.exists());
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_requirement_status_reports_planned_for_staged_spec() {
        let (base, knowledge, specs, archive) = make_tree("status-planned");
        write_req(&knowledge, "REQ-S-B", "accepted", false);
        write_spec(&specs.join("roadmap"), "task-b.spec.md", "REQ-S-B");

        let report =
            requirement_status(&knowledge, &specs, &archive, "REQ-S-B", |_| Verdict::Pass).unwrap();
        assert_eq!(report.execution, "planned", "{report:?}");
        assert!(
            report
                .staged_specs
                .iter()
                .any(|p| p.ends_with("roadmap/task-b.spec.md"))
        );
        assert!(report.active_specs.is_empty());
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_requirement_status_reports_ready_without_spec() {
        let (base, knowledge, specs, archive) = make_tree("status-ready");
        write_req(&knowledge, "REQ-S-C", "accepted", true);

        let report =
            requirement_status(&knowledge, &specs, &archive, "REQ-S-C", |_| Verdict::Pass).unwrap();
        assert_eq!(report.execution, "ready");
        assert_eq!(report.liveness, "unproven");
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_requirement_status_reports_unplanned() {
        let (base, knowledge, specs, archive) = make_tree("status-unplanned");
        write_req(&knowledge, "REQ-S-D", "proposed", false);
        let snapshot: Vec<(PathBuf, String)> = fs::read_dir(knowledge.join("requirements"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .map(|p| (p.clone(), fs::read_to_string(&p).unwrap()))
            .collect();

        let report =
            requirement_status(&knowledge, &specs, &archive, "REQ-S-D", |_| Verdict::Pass).unwrap();
        assert_eq!(report.governance, "proposed");
        assert_eq!(report.execution, "unplanned");
        for (path, content) in snapshot {
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                content,
                "query must be read-only"
            );
        }
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_requirement_status_reports_archived() {
        let (base, knowledge, specs, archive) = make_tree("status-archived");
        write_req(&knowledge, "REQ-S-E", "accepted", true);
        write_spec(&archive, "task-e.spec.md", "REQ-S-E");

        let report =
            requirement_status(&knowledge, &specs, &archive, "REQ-S-E", |_| Verdict::Pass).unwrap();
        assert_eq!(report.execution, "archived");
        assert!(
            report
                .archived_specs
                .iter()
                .any(|p| p.ends_with("task-e.spec.md"))
        );
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_requirement_status_rejects_unknown_id() {
        let (base, knowledge, specs, archive) = make_tree("status-unknown");
        write_req(&knowledge, "REQ-S-F", "accepted", false);
        let err = requirement_status(&knowledge, &specs, &archive, "REQ-GHOST", |_| Verdict::Pass)
            .unwrap_err();
        assert!(err.contains("REQ-GHOST"), "{err}");
        fs::remove_dir_all(base).ok();
    }
}
