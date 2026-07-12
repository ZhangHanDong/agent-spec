//! `requirements traceability <ID>`: one deterministic JSON projection of a
//! requirement's evidence chain — clauses, satisfying specs, scenarios, bound
//! test selectors, latest recorded verdicts, and derived liveness.
//!
//! The projection is a pure read: verdicts come from stored trace-ledger
//! records (never from executing tests here), liveness derives through the
//! canonical `decision_liveness`, and every collection carries a fixed sort
//! order so two runs over identical inputs are byte-identical. Per ADR-001
//! the output carries facts and digests only — no actor, authority, approval,
//! or policy fields.

use crate::spec_core::{Section, Verdict};
use crate::spec_knowledge::liveness::decision_liveness;
use crate::spec_knowledge::model::Liveness;
use crate::spec_knowledge::trace_ledger::{
    latest_requirement_trace_records, read_requirement_trace_ledgers,
};
use serde::Serialize;
use std::path::Path;

pub const TRACEABILITY_SCHEMA_ID: &str = "agent-spec/intent-compiler/requirement-traceability-v1";

#[derive(Debug, Clone, Serialize)]
pub struct TraceabilityProjection {
    pub schema: String,
    pub id: String,
    pub title: String,
    pub governance: String,
    pub source: String,
    /// Clauses in document order.
    pub clauses: Vec<TraceabilityClause>,
    /// Satisfying specs sorted by path.
    pub specs: Vec<TraceabilitySpec>,
    /// Run id of the latest recorded trace run, when any exists.
    pub latest_run: Option<String>,
    pub liveness: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceabilityClause {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceabilitySpec {
    pub path: String,
    pub name: String,
    /// Scenarios in document order.
    pub scenarios: Vec<TraceabilityScenario>,
    /// Rolled-up verdict from the latest recorded run, when any exists.
    pub latest_verdict: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceabilityScenario {
    pub name: String,
    pub test_selector: Option<String>,
    /// Verdict recorded for this scenario in the latest run, when any exists.
    pub latest_verdict: Option<String>,
}

fn verdict_str(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
        Verdict::Skip => "skip",
        Verdict::Uncertain => "uncertain",
        Verdict::PendingReview => "pending_review",
    }
}

/// Build the projection for one requirement id. Reads the knowledge tree,
/// the satisfying specs, and the stored trace ledger under `trace_dir`.
pub fn build_traceability_projection(
    knowledge_dir: &Path,
    specs_dir: &Path,
    trace_dir: &Path,
    id: &str,
) -> Result<TraceabilityProjection, String> {
    let wanted = id.trim().to_ascii_uppercase();
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge_dir);
    let Some(node) = graph.node(&wanted) else {
        return Err(format!(
            "no requirement document under {} declares id {wanted}",
            knowledge_dir.display()
        ));
    };
    let doc = crate::spec_knowledge::parse_requirement(&node.source_path)
        .map_err(|e| format!("{}: {e}", node.source_path.display()))?;

    let governance = match node.status {
        Some(crate::spec_knowledge::DecisionStatus::Proposed) => "proposed",
        Some(crate::spec_knowledge::DecisionStatus::Accepted) => "accepted",
        Some(crate::spec_knowledge::DecisionStatus::Superseded) => "superseded",
        Some(crate::spec_knowledge::DecisionStatus::Deprecated) => "deprecated",
        Some(crate::spec_knowledge::DecisionStatus::Rejected) => "rejected",
        None => "missing",
    }
    .to_string();

    let clauses = node
        .clauses
        .iter()
        .map(|clause| TraceabilityClause {
            id: clause.id.clone().unwrap_or_default(),
            text: clause.text.clone(),
        })
        .collect::<Vec<_>>();

    // Latest recorded evidence, keyed for scenario/spec lookup.
    let ledger = read_requirement_trace_ledgers(trace_dir);
    let records = latest_requirement_trace_records(&ledger, &wanted);
    let latest_run = records.first().map(|r| r.run_id.clone());

    let index = crate::spec_knowledge::build_satisfies_index(specs_dir);
    let mut spec_paths = index.get(&wanted).cloned().unwrap_or_default();
    spec_paths.sort();

    let mut specs = Vec::new();
    let mut spec_verdicts = Vec::new();
    for path in &spec_paths {
        let parsed =
            crate::spec_parser::parse_spec(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let spec_records: Vec<_> = records
            .iter()
            .filter(|r| r.spec_path.as_path() == path.as_path())
            .collect();
        let mut scenarios = Vec::new();
        for section in &parsed.sections {
            let Section::AcceptanceCriteria {
                scenarios: list, ..
            } = section
            else {
                continue;
            };
            for scenario in list {
                let latest = spec_records
                    .iter()
                    .find(|r| r.scenario_name == scenario.name)
                    .map(|r| verdict_str(r.verdict).to_string());
                scenarios.push(TraceabilityScenario {
                    name: scenario.name.clone(),
                    test_selector: scenario.test_selector.as_ref().map(|s| s.filter.clone()),
                    latest_verdict: latest,
                });
            }
        }
        let rollup = rollup_verdict(&spec_records);
        if let Some(v) = rollup {
            spec_verdicts.push(v);
        }
        specs.push(TraceabilitySpec {
            path: path.to_string_lossy().into_owned(),
            name: parsed.meta.name.clone(),
            scenarios,
            latest_verdict: rollup.map(|v| verdict_str(v).to_string()),
        });
    }

    let liveness = match decision_liveness(doc.meta.liveness, &spec_verdicts) {
        Liveness::Honored => "honored",
        Liveness::Violated => "violated",
        Liveness::Unproven => "unproven",
        Liveness::Na => "na",
    }
    .to_string();

    Ok(TraceabilityProjection {
        schema: TRACEABILITY_SCHEMA_ID.to_string(),
        id: wanted,
        title: node.title.clone(),
        governance,
        source: node.source_path.to_string_lossy().into_owned(),
        clauses,
        specs,
        latest_run,
        liveness,
    })
}

/// Roll one spec's latest records up to a single verdict: any fail fails,
/// otherwise any non-pass is uncertain, otherwise pass. No records → None.
fn rollup_verdict(
    records: &[&crate::spec_knowledge::trace_ledger::RequirementTraceRecord],
) -> Option<Verdict> {
    if records.is_empty() {
        return None;
    }
    if records.iter().any(|r| r.verdict == Verdict::Fail) {
        return Some(Verdict::Fail);
    }
    if records.iter().all(|r| r.verdict == Verdict::Pass) {
        return Some(Verdict::Pass);
    }
    Some(Verdict::Uncertain)
}

/// Render the projection as pretty JSON with a trailing newline.
pub fn render_traceability_json(projection: &TraceabilityProjection) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(projection).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

/// Short human summary for `--format text`.
pub fn format_traceability_text(projection: &TraceabilityProjection) -> String {
    let mut out = format!(
        "{}\n  governance: {}\n  liveness:   {}\n",
        projection.id, projection.governance, projection.liveness
    );
    for spec in &projection.specs {
        out.push_str(&format!(
            "  spec {} ({}): {}\n",
            spec.path,
            spec.scenarios.len(),
            spec.latest_verdict.as_deref().unwrap_or("no recorded run")
        ));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::trace_ledger::{RequirementTraceLedger, RequirementTraceRecord};
    use std::fs;
    use std::path::PathBuf;

    fn make_temp_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::create_dir_all(dir.join("trace")).unwrap();
        dir
    }

    fn write_fixture(dir: &Path) -> PathBuf {
        let req = dir.join("knowledge/requirements/req-t.md");
        fs::write(
            &req,
            "---\nkind: requirement\nid: REQ-TRACE-T\ntitle: \"Trace T\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Trace T\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-TRACE-T-ONE] The system MUST hold the first obligation.\n\n## Scenarios\n\nScenario: holds\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-t.spec.md"),
            "spec: task\nname: \"T\"\nsatisfies: [REQ-TRACE-T]\n---\n\n## Intent\n\nx\n\n## Boundaries\n\n### Allowed Changes\n- src/**\n\n## Completion Criteria\n\nScenario: holds\n  Test: test_holds\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n",
        )
        .unwrap();
        let record = RequirementTraceRecord {
            run_id: "run-1".to_string(),
            requirement_id: "REQ-TRACE-T".to_string(),
            requirement_source: req.clone(),
            work_unit_id: "WU-REQ-TRACE-T".to_string(),
            spec_path: dir.join("specs/task-t.spec.md"),
            scenario_name: "holds".to_string(),
            test_selector: Some("test_holds".to_string()),
            code_targets: Vec::new(),
            code_target_facts: Vec::new(),
            verdict: crate::spec_core::Verdict::Pass,
            evidence: Vec::new(),
            worktree_path: None,
            branch: None,
            vcs: None,
            wiki_articles: Vec::new(),
            timestamp: 100,
        };
        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![record],
            diagnostics: Vec::new(),
        };
        fs::write(
            dir.join("trace/run-1.json"),
            serde_json::to_string_pretty(&ledger).unwrap(),
        )
        .unwrap();
        req
    }

    #[test]
    fn test_requirements_traceability_projection_is_byte_stable() {
        let dir = make_temp_tree("traceability-stable");
        write_fixture(&dir);

        let first = build_traceability_projection(
            &dir.join("knowledge"),
            &dir.join("specs"),
            &dir.join("trace"),
            "REQ-TRACE-T",
        )
        .unwrap();
        let second = build_traceability_projection(
            &dir.join("knowledge"),
            &dir.join("specs"),
            &dir.join("trace"),
            "REQ-TRACE-T",
        )
        .unwrap();
        let a = render_traceability_json(&first).unwrap();
        let b = render_traceability_json(&second).unwrap();
        assert_eq!(
            a, b,
            "two runs over identical inputs must be byte-identical"
        );

        let value: serde_json::Value = serde_json::from_str(&a).unwrap();
        assert_eq!(value["schema"], TRACEABILITY_SCHEMA_ID);
        assert_eq!(value["governance"], "accepted");
        assert_eq!(value["liveness"], "honored");
        assert_eq!(value["latest_run"], "run-1");
        assert_eq!(value["clauses"][0]["id"], "REQ-TRACE-T-ONE");
        let scenario = &value["specs"][0]["scenarios"][0];
        assert_eq!(scenario["name"], "holds");
        assert_eq!(scenario["test_selector"], "test_holds");
        assert_eq!(scenario["latest_verdict"], "pass");

        let schema_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/intent-compiler/schemas/requirement-traceability-v1.schema.json");
        let schema_text = fs::read_to_string(&schema_file)
            .unwrap_or_else(|e| panic!("{} must exist: {e}", schema_file.display()));
        assert!(
            schema_text.contains(TRACEABILITY_SCHEMA_ID),
            "the published schema must carry the projection's $id"
        );
        for forbidden in ["actor", "authority", "approval", "policy"] {
            assert!(
                !schema_text.contains(&format!("\"{forbidden}\"")),
                "schema must not define orchestrator field '{forbidden}'"
            );
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirements_traceability_rejects_unknown_id() {
        let dir = make_temp_tree("traceability-unknown");
        write_fixture(&dir);
        let err = build_traceability_projection(
            &dir.join("knowledge"),
            &dir.join("specs"),
            &dir.join("trace"),
            "REQ-GHOST",
        )
        .unwrap_err();
        assert!(err.contains("REQ-GHOST"), "{err}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirements_traceability_keeps_knowledge_byte_identical() {
        let dir = make_temp_tree("traceability-pure");
        write_fixture(&dir);
        let snapshot: Vec<(PathBuf, String)> = fs::read_dir(dir.join("knowledge/requirements"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .map(|p| (p.clone(), fs::read_to_string(&p).unwrap()))
            .collect();

        build_traceability_projection(
            &dir.join("knowledge"),
            &dir.join("specs"),
            &dir.join("trace"),
            "REQ-TRACE-T",
        )
        .unwrap();

        for (path, content) in snapshot {
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                content,
                "traceability must not mutate {}",
                path.display()
            );
        }
        fs::remove_dir_all(dir).ok();
    }
}
