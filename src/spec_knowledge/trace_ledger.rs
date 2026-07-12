use crate::spec_core::{Evidence, Verdict, VerificationReport};
use crate::spec_knowledge::{RequirementPlan, WorktreeManifest};
use crate::vcs::VcsContext;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RequirementTraceRunInput<'a> {
    pub run_id: String,
    pub timestamp: u64,
    pub requirement_plan: &'a RequirementPlan,
    pub worktree_manifest: Option<&'a WorktreeManifest>,
    pub spec_path: PathBuf,
    pub spec_satisfies: Vec<String>,
    pub scenario_selectors: BTreeMap<String, String>,
    pub requirement_scenarios: BTreeMap<String, Vec<String>>,
    pub report: &'a VerificationReport,
    pub vcs: Option<VcsContext>,
}

#[derive(Debug, Clone)]
pub struct RequirementTraceRecordInput<'a> {
    pub run_id: String,
    pub timestamp: u64,
    pub requirement_id: String,
    pub requirement_source: PathBuf,
    pub work_unit_id: String,
    pub spec_path: PathBuf,
    pub scenario_name: String,
    pub test_selector: Option<String>,
    pub report: &'a VerificationReport,
    pub worktree_path: Option<PathBuf>,
    pub branch: Option<String>,
    pub vcs: Option<VcsContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementTraceLedger {
    pub version: u32,
    pub records: Vec<RequirementTraceRecord>,
    pub diagnostics: Vec<RequirementTraceDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementTraceRecord {
    pub run_id: String,
    pub requirement_id: String,
    pub requirement_source: PathBuf,
    pub work_unit_id: String,
    pub spec_path: PathBuf,
    pub scenario_name: String,
    pub test_selector: Option<String>,
    pub code_targets: Vec<String>,
    pub verdict: Verdict,
    pub evidence: Vec<RequirementTraceEvidence>,
    pub worktree_path: Option<PathBuf>,
    pub branch: Option<String>,
    pub vcs: Option<VcsContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wiki_articles: Vec<PathBuf>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementTraceEvidence {
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementTraceDiagnostic {
    pub requirement_id: String,
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementFailureExplanation {
    pub requirement_id: String,
    pub non_pass_records: Vec<RequirementTraceRecord>,
    pub diagnostics: Vec<RequirementTraceDiagnostic>,
}

impl RequirementTraceRecord {
    pub fn from_parts(input: RequirementTraceRecordInput<'_>) -> Option<Self> {
        let result = input
            .report
            .results
            .iter()
            .find(|result| result.scenario_name == input.scenario_name)?;
        let mut code_targets = Vec::new();
        let mut evidence = Vec::new();

        for item in &result.evidence {
            collect_code_targets(item, &mut code_targets);
            evidence.push(trace_evidence_summary(item));
        }
        code_targets.sort();
        code_targets.dedup();

        Some(Self {
            run_id: input.run_id,
            requirement_id: input.requirement_id,
            requirement_source: input.requirement_source,
            work_unit_id: input.work_unit_id,
            spec_path: input.spec_path,
            scenario_name: input.scenario_name,
            test_selector: input.test_selector,
            code_targets,
            verdict: result.verdict,
            evidence,
            worktree_path: input.worktree_path,
            branch: input.branch,
            vcs: input.vcs,
            wiki_articles: Vec::new(),
            timestamp: input.timestamp,
        })
    }
}

pub fn record_requirement_trace_run(input: RequirementTraceRunInput<'_>) -> RequirementTraceLedger {
    let mut records = Vec::new();
    let mut diagnostics = Vec::new();
    let mut scenario_owners = BTreeMap::<String, Vec<String>>::new();
    for requirement_id in &input.spec_satisfies {
        for scenario_name in input
            .requirement_scenarios
            .get(requirement_id)
            .into_iter()
            .flatten()
        {
            scenario_owners
                .entry(scenario_name.clone())
                .or_default()
                .push(requirement_id.clone());
        }
    }

    for requirement_id in &input.spec_satisfies {
        let Some(node) = input
            .requirement_plan
            .requirements
            .iter()
            .find(|node| node.id == *requirement_id)
        else {
            diagnostics.push(RequirementTraceDiagnostic {
                requirement_id: requirement_id.clone(),
                code: "trace-requirement-not-in-plan".into(),
                severity: "warning".into(),
                message: format!(
                    "{requirement_id} is declared by the spec but absent from the requirement plan"
                ),
            });
            continue;
        };

        let worktree = input.worktree_manifest.and_then(|manifest| {
            manifest
                .entries
                .iter()
                .find(|entry| entry.requirement_id == *requirement_id)
        });

        let Some(requirement_scenarios) = input.requirement_scenarios.get(requirement_id) else {
            diagnostics.push(RequirementTraceDiagnostic {
                requirement_id: requirement_id.clone(),
                code: "trace-scenario-mapping-missing".into(),
                severity: "error".into(),
                message: format!(
                    "{requirement_id} has no requirement-to-scenario mapping; no trace records were emitted"
                ),
            });
            continue;
        };

        for scenario_name in requirement_scenarios {
            let owners = scenario_owners
                .get(scenario_name)
                .map(Vec::as_slice)
                .unwrap_or_default();
            if owners.len() > 1 {
                diagnostics.push(RequirementTraceDiagnostic {
                    requirement_id: requirement_id.clone(),
                    code: "trace-scenario-mapping-ambiguous".into(),
                    severity: "error".into(),
                    message: format!(
                        "scenario `{scenario_name}` is mapped to multiple requirements: {}",
                        owners.join(", ")
                    ),
                });
                continue;
            }
            if !input
                .report
                .results
                .iter()
                .any(|result| result.scenario_name == *scenario_name)
            {
                diagnostics.push(RequirementTraceDiagnostic {
                    requirement_id: requirement_id.clone(),
                    code: "trace-scenario-result-missing".into(),
                    severity: "error".into(),
                    message: format!(
                        "mapped scenario `{scenario_name}` has no lifecycle verification result"
                    ),
                });
                continue;
            }
            let record = RequirementTraceRecord::from_parts(RequirementTraceRecordInput {
                run_id: input.run_id.clone(),
                timestamp: input.timestamp,
                requirement_id: requirement_id.clone(),
                requirement_source: node.source_path.clone(),
                work_unit_id: format!("WU-{requirement_id}"),
                spec_path: input.spec_path.clone(),
                scenario_name: scenario_name.clone(),
                test_selector: input.scenario_selectors.get(scenario_name).cloned(),
                report: input.report,
                worktree_path: worktree.map(|entry| entry.path.clone()),
                branch: worktree.map(|entry| entry.branch.clone()),
                vcs: input.vcs.clone(),
            });
            if let Some(record) = record {
                if record.code_targets.is_empty() {
                    diagnostics.push(RequirementTraceDiagnostic {
                        requirement_id: requirement_id.clone(),
                        code: "trace-code-target-unknown".into(),
                        severity: "warning".into(),
                        message: format!(
                            "{} has no code target evidence for scenario `{}`",
                            requirement_id, record.scenario_name
                        ),
                    });
                }
                records.push(record);
            }
        }
    }

    records.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.spec_path.cmp(&b.spec_path))
            .then_with(|| a.scenario_name.cmp(&b.scenario_name))
            .then_with(|| a.run_id.cmp(&b.run_id))
    });
    diagnostics.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.code.cmp(&b.code))
    });

    RequirementTraceLedger {
        version: 1,
        records,
        diagnostics,
    }
}

pub fn write_requirement_trace_ledger(
    base_dir: &Path,
    ledger: &RequirementTraceLedger,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let run_id = ledger
        .records
        .first()
        .map(|record| record.run_id.as_str())
        .unwrap_or("empty");
    let safe_run_id = run_id.replace(['/', '\\'], "-");
    let dir = base_dir.join(".agent-spec/trace");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{safe_run_id}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(ledger)?)?;
    Ok(path)
}

pub fn read_requirement_trace_ledgers(trace_dir: &Path) -> RequirementTraceLedger {
    let mut merged = RequirementTraceLedger {
        version: 1,
        records: Vec::new(),
        diagnostics: Vec::new(),
    };
    let Ok(entries) = std::fs::read_dir(trace_dir) else {
        return merged;
    };
    let mut paths = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            merged.diagnostics.push(RequirementTraceDiagnostic {
                requirement_id: String::new(),
                code: "trace-ledger-symlink-rejected".into(),
                severity: "error".into(),
                message: format!(
                    "trace ledger traversal rejects symlink `{}`",
                    path.display()
                ),
            });
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            paths.push(path);
        }
    }
    paths.sort();

    for path in paths {
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<RequirementTraceLedger>(&content).ok())
        {
            Some(mut ledger) => {
                merged.records.append(&mut ledger.records);
                merged.diagnostics.append(&mut ledger.diagnostics);
            }
            None => merged.diagnostics.push(RequirementTraceDiagnostic {
                requirement_id: String::new(),
                code: "trace-ledger-parse-error".into(),
                severity: "warning".into(),
                message: format!("failed to parse trace ledger `{}`", path.display()),
            }),
        }
    }
    merged.records.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.timestamp.cmp(&b.timestamp))
            .then_with(|| a.run_id.cmp(&b.run_id))
            .then_with(|| a.spec_path.cmp(&b.spec_path))
            .then_with(|| a.scenario_name.cmp(&b.scenario_name))
    });
    merged
}

pub fn replay_requirement_trace(
    ledger: &RequirementTraceLedger,
    requirement_id: &str,
) -> Vec<RequirementTraceRecord> {
    latest_requirement_trace_records(ledger, requirement_id)
}

pub fn latest_requirement_trace_records(
    ledger: &RequirementTraceLedger,
    requirement_id: &str,
) -> Vec<RequirementTraceRecord> {
    let Some((latest_timestamp, latest_run_id)) = ledger
        .records
        .iter()
        .filter(|record| record.requirement_id == requirement_id)
        .max_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then_with(|| a.run_id.cmp(&b.run_id))
        })
        .map(|record| (record.timestamp, record.run_id.clone()))
    else {
        return Vec::new();
    };

    let mut records = ledger
        .records
        .iter()
        .filter(|record| {
            record.requirement_id == requirement_id
                && record.timestamp == latest_timestamp
                && record.run_id == latest_run_id
        })
        .cloned()
        .collect::<Vec<_>>();
    records.sort_by(|a, b| {
        a.spec_path
            .cmp(&b.spec_path)
            .then_with(|| a.scenario_name.cmp(&b.scenario_name))
    });
    records
}

pub fn explain_requirement_failure(
    ledger: &RequirementTraceLedger,
    requirement_id: &str,
) -> RequirementFailureExplanation {
    let mut non_pass_records = latest_requirement_trace_records(ledger, requirement_id)
        .into_iter()
        .filter(|record| record.verdict != Verdict::Pass)
        .collect::<Vec<_>>();
    non_pass_records.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then_with(|| a.run_id.cmp(&b.run_id))
            .then_with(|| a.scenario_name.cmp(&b.scenario_name))
    });
    RequirementFailureExplanation {
        requirement_id: requirement_id.to_string(),
        non_pass_records,
        diagnostics: Vec::new(),
    }
}

pub fn format_requirement_trace_text(records: &[RequirementTraceRecord]) -> String {
    let mut out = format!("requirement trace records: {}\n", records.len());
    for record in records {
        out.push_str(&format!(
            "{} {} {} {:?}\n",
            record.requirement_id, record.scenario_name, record.run_id, record.verdict
        ));
        if !record.wiki_articles.is_empty() {
            out.push_str(&format!(
                "  wiki articles: {}\n",
                record
                    .wiki_articles
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    out
}

pub fn format_requirement_replay_text(records: &[RequirementTraceRecord]) -> String {
    let requirement_id = records
        .first()
        .map(|record| record.requirement_id.as_str())
        .unwrap_or("<unknown>");
    let mut out = format!(
        "evidence replay for {requirement_id}: {} scenarios\n",
        records.len()
    );
    for record in records {
        out.push_str(&format!(
            "work unit: {}\nspec: {}\nscenario: {}\ntest: {}\nverdict: {:?}\n",
            record.work_unit_id,
            record.spec_path.display(),
            record.scenario_name,
            record.test_selector.as_deref().unwrap_or("<none>"),
            record.verdict
        ));
    }
    out
}

pub fn format_requirement_failure_text(explanation: &RequirementFailureExplanation) -> String {
    let mut out = format!(
        "failure explanation for {}: {} non-pass records\n",
        explanation.requirement_id,
        explanation.non_pass_records.len()
    );
    for record in &explanation.non_pass_records {
        out.push_str(&format!(
            "run: {}\nrequirement: {}\nwork unit: {}\nspec: {}\nscenario: {}\ntest: {}\ncode targets: {}\nworktree: {}\nbranch: {}\nvcs: {}\nverdict: {:?}\n",
            record.run_id,
            record.requirement_id,
            record.work_unit_id,
            record.spec_path.display(),
            record.scenario_name,
            record.test_selector.as_deref().unwrap_or("<none>"),
            if record.code_targets.is_empty() {
                "unknown".to_string()
            } else {
                record.code_targets.join(", ")
            },
            record
                .worktree_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<none>".into()),
            record.branch.as_deref().unwrap_or("<none>"),
            record
                .vcs
                .as_ref()
                .map(|vcs| format!("{:?} {}", vcs.vcs_type, vcs.change_ref))
                .unwrap_or_else(|| "<none>".into()),
            record.verdict
        ));
        if !record.wiki_articles.is_empty() {
            out.push_str(&format!(
                "  suggested wiki articles: {}\n",
                record
                    .wiki_articles
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    out
}

pub fn format_requirement_trace_mermaid(records: &[RequirementTraceRecord]) -> String {
    let mut out = String::from("flowchart LR\n");
    for (idx, record) in records.iter().enumerate() {
        let prefix = format!("r{idx}");
        out.push_str(&format!(
            "  {prefix}_req[\"{}\"]\n  {prefix}_wu[\"{}\"]\n  {prefix}_spec[\"{}\"]\n  {prefix}_scenario[\"Scenario: {}\"]\n",
            escape_mermaid(&record.requirement_id),
            escape_mermaid(&record.work_unit_id),
            escape_mermaid(&record.spec_path.display().to_string()),
            escape_mermaid(&record.scenario_name)
        ));
        if let Some(test) = &record.test_selector {
            out.push_str(&format!(
                "  {prefix}_test[\"Test: {}\"]\n",
                escape_mermaid(test)
            ));
        }
        for (code_idx, target) in record.code_targets.iter().enumerate() {
            out.push_str(&format!(
                "  {prefix}_code{code_idx}[\"{}\"]\n",
                escape_mermaid(target)
            ));
        }
        if let Some(branch) = &record.branch {
            out.push_str(&format!(
                "  {prefix}_worktree[\"{}\"]\n",
                escape_mermaid(branch)
            ));
        }
        if let Some(vcs) = &record.vcs {
            out.push_str(&format!(
                "  {prefix}_vcs[\"{:?} {}\"]\n",
                vcs.vcs_type,
                escape_mermaid(&vcs.change_ref)
            ));
        }
        out.push_str(&format!(
            "  {prefix}_req --> {prefix}_wu --> {prefix}_spec --> {prefix}_scenario\n"
        ));
        if record.test_selector.is_some() {
            out.push_str(&format!("  {prefix}_scenario --> {prefix}_test\n"));
            for code_idx in 0..record.code_targets.len() {
                out.push_str(&format!("  {prefix}_test --> {prefix}_code{code_idx}\n"));
            }
        }
        if record.branch.is_some() {
            out.push_str(&format!("  {prefix}_wu --> {prefix}_worktree\n"));
            if record.vcs.is_some() {
                out.push_str(&format!("  {prefix}_worktree --> {prefix}_vcs\n"));
            }
        }
    }
    out
}

fn collect_code_targets(evidence: &Evidence, targets: &mut Vec<String>) {
    match evidence {
        Evidence::TestOutput {
            targets: Some(raw), ..
        } => {
            targets.extend(
                raw.split(',')
                    .map(str::trim)
                    .filter(|target| !target.is_empty())
                    .map(str::to_string),
            );
        }
        Evidence::CodeSnippet { file, .. } => targets.push(file.clone()),
        _ => {}
    }
}

fn trace_evidence_summary(evidence: &Evidence) -> RequirementTraceEvidence {
    match evidence {
        Evidence::TestOutput {
            test_name, passed, ..
        } => RequirementTraceEvidence {
            kind: "test_output".into(),
            summary: format!("test {test_name} passed={passed}"),
        },
        Evidence::CodeSnippet { file, line, .. } => RequirementTraceEvidence {
            kind: "code_snippet".into(),
            summary: format!("{file}:{line}"),
        },
        Evidence::AiAnalysis {
            model, confidence, ..
        } => RequirementTraceEvidence {
            kind: "ai_analysis".into(),
            summary: format!("{model} confidence={confidence}"),
        },
        Evidence::PatternMatch {
            pattern, matched, ..
        } => RequirementTraceEvidence {
            kind: "pattern_match".into(),
            summary: format!("{pattern} matched={matched}"),
        },
    }
}

fn escape_mermaid(input: &str) -> String {
    input.replace('"', "\\\"")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_core::{Evidence, ScenarioResult, Verdict, VerificationReport};
    use crate::spec_knowledge::{RequirementPlanNode, RequirementPlanStatus};
    use crate::vcs::{VcsContext, VcsType};
    use std::path::PathBuf;

    #[test]
    fn test_requirement_trace_ledger_records_req_to_scenario_test_code_and_vcs() {
        let report = VerificationReport::from_results(
            "Create Note".into(),
            vec![ScenarioResult {
                scenario_name: "Create note".into(),
                verdict: Verdict::Fail,
                step_results: vec![],
                evidence: vec![Evidence::TestOutput {
                    test_name: "note_create_adds_note".into(),
                    stdout: "assertion failed".into(),
                    passed: false,
                    package: None,
                    level: Some("unit".into()),
                    test_double: None,
                    targets: Some("src/lib.rs,tests/noteapp_contract.rs".into()),
                }],
                duration_ms: 7,
                provenance: None,
            }],
        );

        let record = RequirementTraceRecord::from_parts(RequirementTraceRecordInput {
            run_id: "run-1".into(),
            timestamp: 1,
            requirement_id: "REQ-NOTE-CREATE".into(),
            requirement_source: PathBuf::from("knowledge/requirements/req-note-create.md"),
            work_unit_id: "WU-REQ-NOTE-CREATE".into(),
            spec_path: PathBuf::from("specs/task-req-note-create.spec.md"),
            scenario_name: "Create note".into(),
            test_selector: Some("note_create_adds_note".into()),
            report: &report,
            worktree_path: Some(PathBuf::from("../agent-spec-worktrees/wu-req-note-create")),
            branch: Some("feat/wu-req-note-create".into()),
            vcs: Some(VcsContext {
                vcs_type: VcsType::Git,
                change_ref: "abc1234".into(),
                operation_ref: None,
            }),
        })
        .unwrap();

        assert_eq!(record.requirement_id, "REQ-NOTE-CREATE");
        assert_eq!(record.work_unit_id, "WU-REQ-NOTE-CREATE");
        assert_eq!(
            record.spec_path,
            PathBuf::from("specs/task-req-note-create.spec.md")
        );
        assert_eq!(record.scenario_name, "Create note");
        assert_eq!(
            record.test_selector.as_deref(),
            Some("note_create_adds_note")
        );
        assert_eq!(
            record.code_targets,
            vec!["src/lib.rs", "tests/noteapp_contract.rs"]
        );
        assert_eq!(record.verdict, Verdict::Fail);
        assert_eq!(
            record.worktree_path.as_ref().unwrap(),
            &PathBuf::from("../agent-spec-worktrees/wu-req-note-create")
        );
        assert_eq!(record.branch.as_deref(), Some("feat/wu-req-note-create"));
        assert_eq!(record.vcs.as_ref().unwrap().change_ref, "abc1234");
    }

    #[test]
    fn test_requirements_replay_uses_latest_trace_record_for_requirement() {
        let mut ledger = RequirementTraceLedger {
            version: 1,
            records: vec![old_record("REQ-NOTE-CREATE"), new_record("REQ-NOTE-CREATE")],
            diagnostics: Vec::new(),
        };
        ledger.records[0].timestamp = 1;
        ledger.records[1].timestamp = 2;

        let replay = replay_requirement_trace(&ledger, "REQ-NOTE-CREATE");
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].run_id, "new");
        let text = format_requirement_replay_text(&replay);
        assert!(text.contains("evidence replay"));
        assert!(!text.contains("deterministic LLM replay"));
    }

    #[test]
    fn test_latest_requirement_trace_records_returns_only_latest_run() {
        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![
                trace_record("REQ-NOTE-CREATE", "old-fail", 1, Verdict::Fail),
                trace_record("REQ-NOTE-CREATE", "new-pass", 2, Verdict::Pass),
            ],
            diagnostics: Vec::new(),
        };

        let records = latest_requirement_trace_records(&ledger, "REQ-NOTE-CREATE");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].run_id, "new-pass");
        assert_eq!(records[0].verdict, Verdict::Pass);
    }

    #[test]
    fn test_requirement_trace_avoids_multi_requirement_scenario_cartesian_product() {
        let plan = RequirementPlan {
            version: 1,
            requirements: vec![
                RequirementPlanNode {
                    id: "REQ-A".into(),
                    title: "A".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-a.md"),
                    status: RequirementPlanStatus::Ready,
                    mode: "leaf_full".into(),
                    scenario_count: 1,
                    blocked_by: Vec::new(),
                },
                RequirementPlanNode {
                    id: "REQ-B".into(),
                    title: "B".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-b.md"),
                    status: RequirementPlanStatus::Ready,
                    mode: "leaf_full".into(),
                    scenario_count: 1,
                    blocked_by: Vec::new(),
                },
            ],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: Vec::new(),
            coverage: Vec::new(),
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };
        let report = VerificationReport::from_results(
            "Combined".into(),
            vec![
                trace_result("Scenario A", "test_a"),
                trace_result("Scenario B", "test_b"),
            ],
        );
        let ledger = record_requirement_trace_run(RequirementTraceRunInput {
            run_id: "run".into(),
            timestamp: 1,
            requirement_plan: &plan,
            worktree_manifest: None,
            spec_path: PathBuf::from("specs/task-combined.spec.md"),
            spec_satisfies: vec!["REQ-A".into(), "REQ-B".into()],
            scenario_selectors: BTreeMap::from([
                ("Scenario A".into(), "test_a".into()),
                ("Scenario B".into(), "test_b".into()),
            ]),
            requirement_scenarios: BTreeMap::from([
                ("REQ-A".into(), vec!["Scenario A".into()]),
                ("REQ-B".into(), vec!["Scenario B".into()]),
            ]),
            report: &report,
            vcs: None,
        });

        assert_eq!(ledger.records.len(), 2);
        assert!(ledger.records.iter().any(|record| {
            record.requirement_id == "REQ-A" && record.scenario_name == "Scenario A"
        }));
        assert!(ledger.records.iter().any(|record| {
            record.requirement_id == "REQ-B" && record.scenario_name == "Scenario B"
        }));
    }

    #[test]
    fn test_requirement_replay_returns_all_scenarios_from_latest_run() {
        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![
                trace_record("REQ-A", "old", 1, Verdict::Fail),
                trace_record_named("REQ-A", "new", 2, Verdict::Pass, "Scenario A"),
                trace_record_named("REQ-A", "new", 2, Verdict::Pass, "Scenario B"),
            ],
            diagnostics: Vec::new(),
        };
        assert_eq!(replay_requirement_trace(&ledger, "REQ-A").len(), 2);
    }

    #[test]
    fn test_requirement_trace_rejects_ambiguous_scenario_ownership() {
        let plan = RequirementPlan {
            version: 1,
            requirements: vec![
                RequirementPlanNode {
                    id: "REQ-A".into(),
                    title: "A".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-a.md"),
                    status: RequirementPlanStatus::Ready,
                    mode: "leaf_full".into(),
                    scenario_count: 1,
                    blocked_by: Vec::new(),
                },
                RequirementPlanNode {
                    id: "REQ-B".into(),
                    title: "B".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-b.md"),
                    status: RequirementPlanStatus::Ready,
                    mode: "leaf_full".into(),
                    scenario_count: 1,
                    blocked_by: Vec::new(),
                },
            ],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: Vec::new(),
            coverage: Vec::new(),
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };
        let report = VerificationReport::from_results(
            "Combined".into(),
            vec![trace_result("Shared scenario", "test_shared")],
        );
        let ledger = record_requirement_trace_run(RequirementTraceRunInput {
            run_id: "run".into(),
            timestamp: 1,
            requirement_plan: &plan,
            worktree_manifest: None,
            spec_path: PathBuf::from("specs/task-combined.spec.md"),
            spec_satisfies: vec!["REQ-A".into(), "REQ-B".into()],
            scenario_selectors: BTreeMap::from([("Shared scenario".into(), "test_shared".into())]),
            requirement_scenarios: BTreeMap::from([
                ("REQ-A".into(), vec!["Shared scenario".into()]),
                ("REQ-B".into(), vec!["Shared scenario".into()]),
            ]),
            report: &report,
            vcs: None,
        });

        assert!(ledger.records.is_empty());
        assert!(ledger.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "trace-scenario-mapping-ambiguous" && diagnostic.severity == "error"
        }));
    }

    #[test]
    fn test_requirements_explain_failure_reports_non_pass_chain() {
        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![
                passing_record("REQ-NOTE-CREATE"),
                failing_record("REQ-NOTE-CREATE"),
            ],
            diagnostics: Vec::new(),
        };

        let explanation = explain_requirement_failure(&ledger, "REQ-NOTE-CREATE");
        assert_eq!(explanation.requirement_id, "REQ-NOTE-CREATE");
        assert_eq!(explanation.non_pass_records.len(), 1);
        assert_eq!(explanation.non_pass_records[0].verdict, Verdict::Fail);
    }

    #[test]
    fn test_requirements_explain_failure_ignores_older_failures_when_latest_run_passes() {
        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![
                trace_record("REQ-NOTE-CREATE", "old-fail", 1, Verdict::Fail),
                trace_record("REQ-NOTE-CREATE", "new-pass", 2, Verdict::Pass),
            ],
            diagnostics: Vec::new(),
        };

        let explanation = explain_requirement_failure(&ledger, "REQ-NOTE-CREATE");
        assert!(explanation.non_pass_records.is_empty());
    }

    #[test]
    fn test_requirements_trace_graph_mermaid_contains_evidence_nodes() {
        let mermaid = format_requirement_trace_mermaid(&[failing_record("REQ-NOTE-CREATE")]);
        assert!(mermaid.contains("flowchart LR"));
        assert!(mermaid.contains("REQ-NOTE-CREATE"));
        assert!(mermaid.contains("WU-REQ-NOTE-CREATE"));
        assert!(mermaid.contains("Scenario: Create note"));
        assert!(mermaid.contains("Test: note_create_adds_note"));
        assert!(mermaid.contains("src/lib.rs"));
        assert!(mermaid.contains("feat/wu-req-note-create"));
    }

    #[test]
    fn test_requirement_trace_ledger_reads_and_merges_json() {
        let dir =
            std::env::temp_dir().join(format!("requirement-trace-ledger-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![failing_record("REQ-NOTE-CREATE")],
            diagnostics: Vec::new(),
        };
        write_requirement_trace_ledger(&dir, &ledger).unwrap();

        let merged = read_requirement_trace_ledgers(&dir.join(".agent-spec/trace"));
        assert_eq!(merged.records.len(), 1);
        assert_eq!(merged.records[0].requirement_id, "REQ-NOTE-CREATE");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_failure_text_reports_full_requirement_trace_chain() {
        let mut record = failing_record("REQ-NOTE-CREATE");
        record.vcs = Some(VcsContext {
            vcs_type: VcsType::Git,
            change_ref: "abc1234".into(),
            operation_ref: None,
        });
        let explanation = RequirementFailureExplanation {
            requirement_id: "REQ-NOTE-CREATE".into(),
            non_pass_records: vec![record],
            diagnostics: Vec::new(),
        };

        let text = format_requirement_failure_text(&explanation);

        for expected in [
            "requirement: REQ-NOTE-CREATE",
            "work unit: WU-REQ-NOTE-CREATE",
            "spec: specs/task-req-note-create.spec.md",
            "scenario: Create note",
            "test: note_create_adds_note",
            "code targets: src/lib.rs",
            "worktree: ../agent-spec-worktrees/wu-req-note-create",
            "branch: feat/wu-req-note-create",
            "vcs: Git abc1234",
            "verdict: Fail",
        ] {
            assert!(text.contains(expected), "missing `{expected}` in:\n{text}");
        }
    }

    fn trace_record(
        req_id: &str,
        run_id: &str,
        timestamp: u64,
        verdict: Verdict,
    ) -> RequirementTraceRecord {
        RequirementTraceRecord {
            run_id: run_id.into(),
            requirement_id: req_id.into(),
            requirement_source: PathBuf::from(format!(
                "knowledge/requirements/{}.md",
                req_id.to_ascii_lowercase()
            )),
            work_unit_id: format!("WU-{req_id}"),
            spec_path: PathBuf::from("specs/task-req-note-create.spec.md"),
            scenario_name: "Create note".into(),
            test_selector: Some("note_create_adds_note".into()),
            code_targets: vec!["src/lib.rs".into()],
            verdict,
            evidence: vec![RequirementTraceEvidence {
                kind: "test_output".into(),
                summary: "test note_create_adds_note".into(),
            }],
            worktree_path: Some(PathBuf::from("../agent-spec-worktrees/wu-req-note-create")),
            branch: Some("feat/wu-req-note-create".into()),
            vcs: None,
            wiki_articles: Vec::new(),
            timestamp,
        }
    }

    fn trace_record_named(
        req_id: &str,
        run_id: &str,
        timestamp: u64,
        verdict: Verdict,
        scenario_name: &str,
    ) -> RequirementTraceRecord {
        let mut record = trace_record(req_id, run_id, timestamp, verdict);
        record.scenario_name = scenario_name.into();
        record
    }

    fn trace_result(name: &str, test_name: &str) -> ScenarioResult {
        ScenarioResult {
            scenario_name: name.into(),
            verdict: Verdict::Pass,
            step_results: Vec::new(),
            evidence: vec![Evidence::TestOutput {
                test_name: test_name.into(),
                stdout: String::new(),
                passed: true,
                package: None,
                level: Some("unit".into()),
                test_double: None,
                targets: Some("src/lib.rs".into()),
            }],
            duration_ms: 1,
            provenance: None,
        }
    }

    fn old_record(req_id: &str) -> RequirementTraceRecord {
        trace_record(req_id, "old", 1, Verdict::Pass)
    }

    fn new_record(req_id: &str) -> RequirementTraceRecord {
        trace_record(req_id, "new", 2, Verdict::Pass)
    }

    fn passing_record(req_id: &str) -> RequirementTraceRecord {
        trace_record(req_id, "pass", 1, Verdict::Pass)
    }

    fn failing_record(req_id: &str) -> RequirementTraceRecord {
        trace_record(req_id, "fail", 2, Verdict::Fail)
    }
}
