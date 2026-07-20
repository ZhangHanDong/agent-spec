use crate::spec_core::{Evidence, Verdict, VerificationReport};
use crate::spec_knowledge::{
    AffectedExecutionBundle, IntentImpactGap, IntentImpactReport, QualityOutcome, RequirementPlan,
    WorktreeManifest,
};
use crate::vcs::VcsContext;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const REQUIREMENT_TRACE_LEDGER_VERSION: u32 = 2;

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
    /// Typed code targets resolved from the contract's `### Symbols` against
    /// a fresh provider graph; empty when no symbols or no fresh graph.
    pub code_target_facts: Vec<CodeTargetFact>,
}

/// A typed, stale-aware code target: which provider resolved which node in
/// which graph state. Derived enrichment — never durable KLL truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeTargetFact {
    pub provider: String,
    pub node_id: String,
    pub kind: String,
    pub file: String,
    pub provenance: String,
    pub graph_fingerprint: String,
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
    pub code_target_facts: Vec<CodeTargetFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementTraceLedger {
    pub version: u32,
    pub records: Vec<RequirementTraceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_records: Vec<AffectedTraceRecord>,
    pub diagnostics: Vec<RequirementTraceDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedTraceRecord {
    pub run_id: String,
    pub timestamp: u64,
    pub requirement_ids: Vec<String>,
    pub intent_impact_digest: String,
    pub intent_impact: IntentImpactReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_bundle: Option<AffectedExecutionBundle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_outcomes: Vec<AffectedQualityOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedQualityOutcome {
    pub provider_id: String,
    pub outcome: QualityOutcome,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedRequirementReplay {
    pub requirement_id: String,
    pub lifecycle_records: Vec<RequirementTraceRecord>,
    pub affected_record: Option<AffectedTraceRecord>,
    pub gaps: Vec<IntentImpactGap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedRequirementFailure {
    pub requirement_id: String,
    pub lifecycle_non_pass_records: Vec<RequirementTraceRecord>,
    pub quality_failures: Vec<AffectedQualityOutcome>,
    pub affected_record: Option<AffectedTraceRecord>,
    pub gaps: Vec<IntentImpactGap>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_target_facts: Vec<CodeTargetFact>,
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

        code_targets.extend(
            input
                .code_target_facts
                .iter()
                .map(|fact| fact.node_id.clone()),
        );

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
            code_target_facts: input.code_target_facts,
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
                code_target_facts: input.code_target_facts.clone(),
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
        version: REQUIREMENT_TRACE_LEDGER_VERSION,
        records,
        affected_records: Vec::new(),
        diagnostics,
    }
}

pub fn write_requirement_trace_ledger(
    base_dir: &Path,
    ledger: &RequirementTraceLedger,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_requirement_trace_ledger_to_dir(&base_dir.join(".agent-spec/trace"), ledger)
}

pub fn write_requirement_trace_ledger_to_dir(
    trace_dir: &Path,
    ledger: &RequirementTraceLedger,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let run_id = ledger
        .records
        .first()
        .map(|record| record.run_id.as_str())
        .or_else(|| {
            ledger
                .affected_records
                .first()
                .map(|record| record.run_id.as_str())
        })
        .unwrap_or("empty");
    let safe_run_id = run_id.replace(['/', '\\'], "-");
    std::fs::create_dir_all(trace_dir)?;
    let path = trace_dir.join(format!("{safe_run_id}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(ledger)?)?;
    Ok(path)
}

pub fn read_requirement_trace_ledgers(trace_dir: &Path) -> RequirementTraceLedger {
    let mut merged = RequirementTraceLedger {
        version: REQUIREMENT_TRACE_LEDGER_VERSION,
        records: Vec::new(),
        affected_records: Vec::new(),
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
                merged.version = merged.version.max(ledger.version);
                merged.records.append(&mut ledger.records);
                merged.affected_records.append(&mut ledger.affected_records);
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
    merged.affected_records.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then_with(|| a.run_id.cmp(&b.run_id))
            .then_with(|| a.requirement_ids.cmp(&b.requirement_ids))
    });
    merged
}

pub fn record_affected_trace(
    ledger: &mut RequirementTraceLedger,
    mut record: AffectedTraceRecord,
) -> Result<(), String> {
    record.requirement_ids.sort();
    record.requirement_ids.dedup();
    record.quality_outcomes.sort_by(|left, right| {
        left.provider_id
            .cmp(&right.provider_id)
            .then_with(|| left.summary.cmp(&right.summary))
    });
    ledger.version = REQUIREMENT_TRACE_LEDGER_VERSION;
    if let Some(existing) = ledger
        .affected_records
        .iter_mut()
        .find(|existing| existing.run_id == record.run_id)
    {
        if existing.timestamp != record.timestamp
            || existing.intent_impact_digest != record.intent_impact_digest
            || existing.requirement_ids != record.requirement_ids
        {
            return Err(format!(
                "affected-trace-run-conflict: run `{}` already names different immutable impact evidence",
                record.run_id
            ));
        }
        let mut merged = existing.clone();
        match (&merged.execution_bundle, record.execution_bundle.take()) {
            (Some(current), Some(incoming)) if current != &incoming => {
                return Err(format!(
                    "affected-trace-run-conflict: run `{}` already names a different execution bundle",
                    record.run_id
                ));
            }
            (None, Some(incoming)) => merged.execution_bundle = Some(incoming),
            _ => {}
        }
        for outcome in record.quality_outcomes {
            if let Some(current) = merged
                .quality_outcomes
                .iter()
                .find(|current| current.provider_id == outcome.provider_id)
            {
                if current != &outcome {
                    return Err(format!(
                        "affected-trace-run-conflict: run `{}` already records different evidence for quality provider `{}`",
                        record.run_id, outcome.provider_id
                    ));
                }
            } else {
                merged.quality_outcomes.push(outcome);
            }
        }
        merged.quality_outcomes.sort_by(|left, right| {
            left.provider_id
                .cmp(&right.provider_id)
                .then_with(|| left.summary.cmp(&right.summary))
        });
        *existing = merged;
    } else {
        ledger.affected_records.push(record);
    }
    ledger.affected_records.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.run_id.cmp(&right.run_id))
            .then_with(|| left.requirement_ids.cmp(&right.requirement_ids))
    });
    Ok(())
}

pub fn write_affected_trace_record_to_dir(
    trace_dir: &Path,
    record: AffectedTraceRecord,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let safe_run_id = record.run_id.replace(['/', '\\'], "-");
    let path = trace_dir.join(format!("{safe_run_id}.json"));
    let mut ledger = if path.is_file() {
        serde_json::from_str::<RequirementTraceLedger>(&std::fs::read_to_string(&path)?)?
    } else {
        RequirementTraceLedger {
            version: REQUIREMENT_TRACE_LEDGER_VERSION,
            records: Vec::new(),
            affected_records: Vec::new(),
            diagnostics: Vec::new(),
        }
    };
    record_affected_trace(&mut ledger, record).map_err(std::io::Error::other)?;
    std::fs::create_dir_all(trace_dir)?;
    std::fs::write(&path, serde_json::to_string_pretty(&ledger)?)?;
    Ok(path)
}

pub fn build_affected_trace_record(
    run_id: String,
    timestamp: u64,
    intent_impact: IntentImpactReport,
    execution_bundle: Option<AffectedExecutionBundle>,
    quality_outcomes: Vec<AffectedQualityOutcome>,
) -> Result<AffectedTraceRecord, String> {
    if run_id.trim().is_empty() {
        return Err("affected-trace-run-id-missing: run id must not be empty".into());
    }
    let digest = crate::spec_knowledge::blake3_hex(
        &serde_json::to_vec(&intent_impact).map_err(|error| error.to_string())?,
    );
    if let Some(bundle) = &execution_bundle
        && bundle.intent_impact_digest != digest
    {
        return Err(format!(
            "affected-trace-digest-mismatch: bundle references {} but report digest is {digest}",
            bundle.intent_impact_digest
        ));
    }
    let mut requirement_ids = intent_impact
        .affected
        .iter()
        .flat_map(|node| node.links.iter())
        .map(|link| link.requirement_id.clone())
        .collect::<Vec<_>>();
    requirement_ids.sort();
    requirement_ids.dedup();
    if requirement_ids.is_empty() {
        return Err(
            "affected-trace-requirements-missing: report has no linked requirement ids".into(),
        );
    }
    let mut record = AffectedTraceRecord {
        run_id,
        timestamp,
        requirement_ids,
        intent_impact_digest: digest,
        intent_impact,
        execution_bundle,
        quality_outcomes,
    };
    record.quality_outcomes.sort_by(|left, right| {
        left.provider_id
            .cmp(&right.provider_id)
            .then_with(|| left.summary.cmp(&right.summary))
    });
    Ok(record)
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

pub fn replay_affected_requirement(
    ledger: &RequirementTraceLedger,
    requirement_id: &str,
) -> AffectedRequirementReplay {
    let affected_record = latest_affected_trace_record(ledger, requirement_id);
    let mut gaps = affected_record
        .as_ref()
        .map(affected_record_gaps)
        .unwrap_or_default();
    gaps.extend(
        ledger
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.requirement_id.is_empty() || diagnostic.requirement_id == requirement_id
            })
            .map(|diagnostic| IntentImpactGap {
                code: diagnostic.code.clone(),
                severity: diagnostic.severity.clone(),
                node_id: None,
                requirement_id: (!diagnostic.requirement_id.is_empty())
                    .then(|| diagnostic.requirement_id.clone()),
                spec_path: None,
                message: diagnostic.message.clone(),
            }),
    );
    let lifecycle_records = if let Some(record) = &affected_record {
        let mut matching = ledger
            .records
            .iter()
            .filter(|candidate| {
                candidate.requirement_id == requirement_id && candidate.run_id == record.run_id
            })
            .cloned()
            .collect::<Vec<_>>();
        matching.sort_by(|left, right| {
            left.spec_path
                .cmp(&right.spec_path)
                .then_with(|| left.scenario_name.cmp(&right.scenario_name))
        });
        if matching.is_empty() {
            gaps.push(affected_gap(
                "lifecycle-trace-missing",
                requirement_id,
                format!(
                    "affected run `{}` has no lifecycle trace records",
                    record.run_id
                ),
            ));
        }
        matching
    } else {
        gaps.push(affected_gap(
            "affected-trace-missing",
            requirement_id,
            "no stored intent-aware affected context is available; replay remains lifecycle-only",
        ));
        let records = latest_requirement_trace_records(ledger, requirement_id);
        if records.is_empty() {
            gaps.push(affected_gap(
                "lifecycle-trace-missing",
                requirement_id,
                "no stored lifecycle trace records are available",
            ));
        }
        records
    };
    sort_affected_gaps(&mut gaps);
    AffectedRequirementReplay {
        requirement_id: requirement_id.into(),
        lifecycle_records,
        affected_record,
        gaps,
    }
}

pub fn explain_affected_requirement_failure(
    ledger: &RequirementTraceLedger,
    requirement_id: &str,
) -> AffectedRequirementFailure {
    let replay = replay_affected_requirement(ledger, requirement_id);
    let lifecycle_non_pass_records = replay
        .lifecycle_records
        .iter()
        .filter(|record| record.verdict != Verdict::Pass)
        .cloned()
        .collect();
    let quality_failures = replay
        .affected_record
        .as_ref()
        .into_iter()
        .flat_map(|record| record.quality_outcomes.iter())
        .filter(|outcome| outcome.outcome != QualityOutcome::Pass)
        .cloned()
        .collect();
    AffectedRequirementFailure {
        requirement_id: replay.requirement_id,
        lifecycle_non_pass_records,
        quality_failures,
        affected_record: replay.affected_record,
        gaps: replay.gaps,
    }
}

pub fn format_affected_requirement_replay_text(replay: &AffectedRequirementReplay) -> String {
    let mut out = format!(
        "affected evidence replay for {}: {} lifecycle scenarios\n",
        replay.requirement_id,
        replay.lifecycle_records.len()
    );
    if let Some(record) = &replay.affected_record {
        out.push_str(&format!(
            "run: {}\ndigest: {}\nprovider: {}\ngraph: {}\nvcs: {}\n",
            record.run_id,
            record.intent_impact_digest,
            record.intent_impact.provider,
            record
                .intent_impact
                .graph_fingerprint
                .as_deref()
                .unwrap_or("<none>"),
            record
                .intent_impact
                .observed_vcs
                .as_ref()
                .map(|vcs| format!("{:?} {}", vcs.vcs_type, vcs.change_ref))
                .unwrap_or_else(|| "<none>".into())
        ));
        for affected in &record.intent_impact.affected {
            out.push_str(&format!(
                "affected: {} {}:{}-{} distance={}\n",
                affected.impact.node.node_id,
                affected.impact.node.file,
                affected.impact.node.line_start,
                affected.impact.node.line_end,
                affected.impact.distance
            ));
            for hop in &affected.impact.path.hops {
                let site = hop
                    .site
                    .as_ref()
                    .map(|site| {
                        format!(
                            "{}:{}:{}-{}:{}",
                            site.file,
                            site.line_start,
                            site.column_start,
                            site.line_end,
                            site.column_end
                        )
                    })
                    .unwrap_or_else(|| "<none>".into());
                out.push_str(&format!(
                    "  path: {} -> {} kind={} confidence={} site={}\n",
                    hop.from,
                    hop.chosen_target,
                    hop.kind,
                    hop.confidence.as_deref().unwrap_or("unknown"),
                    site
                ));
            }
            for link in &affected.links {
                out.push_str(&format!(
                    "  requirement: {}\n  work unit: {}\n",
                    link.requirement_id, link.work_unit_id
                ));
                if let Some(worktree) = &link.worktree {
                    out.push_str(&format!(
                        "  worktree: {}\n  branch: {}\n",
                        worktree.path.display(),
                        worktree.branch
                    ));
                }
                for spec in &link.specs {
                    for scenario in &spec.scenarios {
                        out.push_str(&format!(
                            "  spec: {}\n  scenario: {}\n  test: {}\n",
                            spec.path.display(),
                            scenario.name,
                            scenario
                                .authoritative_selector
                                .as_deref()
                                .unwrap_or("<none>")
                        ));
                    }
                }
            }
        }
        for outcome in &record.quality_outcomes {
            out.push_str(&format!(
                "quality: {} {:?} {}\n",
                outcome.provider_id, outcome.outcome, outcome.summary
            ));
        }
    }
    for lifecycle in &replay.lifecycle_records {
        out.push_str(&format!(
            "lifecycle: {} {} {:?}\n",
            lifecycle.scenario_name,
            lifecycle.test_selector.as_deref().unwrap_or("<none>"),
            lifecycle.verdict
        ));
    }
    for gap in &replay.gaps {
        out.push_str(&format!("gap: {} {}\n", gap.code, gap.message));
    }
    out
}

pub fn format_affected_requirement_failure_text(failure: &AffectedRequirementFailure) -> String {
    let mut out = format!(
        "affected failure explanation for {}: {} lifecycle failures, {} quality failures\n",
        failure.requirement_id,
        failure.lifecycle_non_pass_records.len(),
        failure.quality_failures.len()
    );
    for record in &failure.lifecycle_non_pass_records {
        out.push_str(&format!(
            "lifecycle failure: {} {} {:?}\n",
            record.scenario_name,
            record.test_selector.as_deref().unwrap_or("<none>"),
            record.verdict
        ));
    }
    for outcome in &failure.quality_failures {
        out.push_str(&format!(
            "quality failure: {} {:?} {}\n",
            outcome.provider_id, outcome.outcome, outcome.summary
        ));
    }
    if let Some(record) = &failure.affected_record {
        out.push_str(&format!(
            "affected run: {}\ndigest: {}\n",
            record.run_id, record.intent_impact_digest
        ));
        for affected in &record.intent_impact.affected {
            out.push_str(&format!(
                "affected code: {} {}:{}-{}\n",
                affected.impact.node.node_id,
                affected.impact.node.file,
                affected.impact.node.line_start,
                affected.impact.node.line_end
            ));
        }
    }
    for gap in &failure.gaps {
        out.push_str(&format!("gap: {} {}\n", gap.code, gap.message));
    }
    if failure.affected_record.is_some() {
        out.push_str(&format_affected_requirement_replay_text(
            &AffectedRequirementReplay {
                requirement_id: failure.requirement_id.clone(),
                lifecycle_records: failure.lifecycle_non_pass_records.clone(),
                affected_record: failure.affected_record.clone(),
                gaps: Vec::new(),
            },
        ));
    }
    out
}

fn latest_affected_trace_record(
    ledger: &RequirementTraceLedger,
    requirement_id: &str,
) -> Option<AffectedTraceRecord> {
    ledger
        .affected_records
        .iter()
        .filter(|record| {
            record
                .requirement_ids
                .iter()
                .any(|candidate| candidate == requirement_id)
        })
        .max_by(|left, right| {
            left.timestamp
                .cmp(&right.timestamp)
                .then_with(|| left.run_id.cmp(&right.run_id))
        })
        .cloned()
}

fn affected_record_gaps(record: &AffectedTraceRecord) -> Vec<IntentImpactGap> {
    let mut gaps = record.intent_impact.gaps.clone();
    if let Some(bundle) = &record.execution_bundle {
        gaps.extend(bundle.gaps.iter().cloned());
    }
    gaps
}

fn affected_gap(code: &str, requirement_id: &str, message: impl Into<String>) -> IntentImpactGap {
    IntentImpactGap {
        code: code.into(),
        severity: "warning".into(),
        node_id: None,
        requirement_id: Some(requirement_id.into()),
        spec_path: None,
        message: message.into(),
    }
}

fn sort_affected_gaps(gaps: &mut Vec<IntentImpactGap>) {
    gaps.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.node_id.cmp(&right.node_id))
            .then_with(|| left.requirement_id.cmp(&right.requirement_id))
            .then_with(|| left.spec_path.cmp(&right.spec_path))
            .then_with(|| left.message.cmp(&right.message))
    });
    gaps.dedup();
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
            "work unit: {}\nspec: {}\nscenario: {}\ntest: {}\ncode targets: {}\nworktree: {}\nbranch: {}\nvcs: {}\nverdict: {:?}\n",
            record.work_unit_id,
            record.spec_path.display(),
            record.scenario_name,
            record.test_selector.as_deref().unwrap_or("<none>"),
            if record.code_targets.is_empty() {
                "<none>".to_string()
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
        if let Some(worktree_path) = &record.worktree_path {
            out.push_str(&format!(
                "  {prefix}_worktree[\"{}\"]\n",
                escape_mermaid(&worktree_path.display().to_string())
            ));
        }
        if let Some(branch) = &record.branch {
            out.push_str(&format!(
                "  {prefix}_branch[\"Branch: {}\"]\n",
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
        let evidence_tail = if record.test_selector.is_some() {
            out.push_str(&format!("  {prefix}_scenario --> {prefix}_test\n"));
            format!("{prefix}_test")
        } else {
            format!("{prefix}_scenario")
        };
        for code_idx in 0..record.code_targets.len() {
            out.push_str(&format!("  {evidence_tail} --> {prefix}_code{code_idx}\n"));
        }

        let mut authority_tails = if record.code_targets.is_empty() {
            vec![evidence_tail]
        } else {
            (0..record.code_targets.len())
                .map(|code_idx| format!("{prefix}_code{code_idx}"))
                .collect::<Vec<_>>()
        };
        if record.worktree_path.is_some() {
            for tail in &authority_tails {
                out.push_str(&format!("  {tail} --> {prefix}_worktree\n"));
            }
            authority_tails = vec![format!("{prefix}_worktree")];
        }
        if record.branch.is_some() {
            for tail in &authority_tails {
                out.push_str(&format!("  {tail} --> {prefix}_branch\n"));
            }
            authority_tails = vec![format!("{prefix}_branch")];
        }
        if record.vcs.is_some() {
            for tail in &authority_tails {
                out.push_str(&format!("  {tail} --> {prefix}_vcs\n"));
            }
        }
    }
    out
}

pub fn format_affected_requirement_trace_mermaid(replay: &AffectedRequirementReplay) -> String {
    let mut out = format_requirement_trace_mermaid(&replay.lifecycle_records);
    let Some(record) = &replay.affected_record else {
        return out;
    };

    for (affected_index, affected) in record.intent_impact.affected.iter().enumerate() {
        let prefix = format!("a{affected_index}");
        let path = &affected.impact.path;
        for (node_index, node) in path.nodes.iter().enumerate() {
            out.push_str(&format!(
                "  {prefix}_path{node_index}[\"{}<br/>{}:{}-{}\"]\n",
                escape_mermaid(&node.node_id),
                escape_mermaid(&node.file),
                node.line_start,
                node.line_end
            ));
        }
        for (hop_index, hop) in path.hops.iter().enumerate() {
            let site = hop
                .site
                .as_ref()
                .map(|site| format!(" @ {}:{}", site.file, site.line_start))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {prefix}_path{hop_index} -->|\"{}{}\"| {prefix}_path{}\n",
                escape_mermaid(&hop.kind),
                escape_mermaid(&site),
                hop_index + 1
            ));
        }
        let path_head = (!path.nodes.is_empty()).then(|| format!("{prefix}_path0"));
        let path_tail = path
            .nodes
            .len()
            .checked_sub(1)
            .map(|index| format!("{prefix}_path{index}"));

        for (link_index, link) in affected.links.iter().enumerate() {
            let link_prefix = format!("{prefix}_l{link_index}");
            out.push_str(&format!(
                "  {link_prefix}_req[\"{}\"]\n  {link_prefix}_wu[\"{}\"]\n",
                escape_mermaid(&link.requirement_id),
                escape_mermaid(&link.work_unit_id)
            ));
            out.push_str(&format!("  {link_prefix}_req --> {link_prefix}_wu\n"));
            let mut evidence_tails = Vec::new();
            for (spec_index, spec) in link.specs.iter().enumerate() {
                let spec_prefix = format!("{link_prefix}_s{spec_index}");
                out.push_str(&format!(
                    "  {spec_prefix}[\"{}\"]\n  {link_prefix}_wu --> {spec_prefix}\n",
                    escape_mermaid(&spec.path.display().to_string())
                ));
                if spec.scenarios.is_empty() {
                    evidence_tails.push(spec_prefix.clone());
                }
                for (scenario_index, scenario) in spec.scenarios.iter().enumerate() {
                    let scenario_id = format!("{spec_prefix}_c{scenario_index}");
                    out.push_str(&format!(
                        "  {scenario_id}[\"Scenario: {}\"]\n  {spec_prefix} --> {scenario_id}\n",
                        escape_mermaid(&scenario.name)
                    ));
                    if let Some(selector) = &scenario.authoritative_selector {
                        let test_id = format!("{scenario_id}_test");
                        out.push_str(&format!(
                            "  {test_id}[\"Test: {}\"]\n  {scenario_id} --> {test_id}\n",
                            escape_mermaid(selector)
                        ));
                        evidence_tails.push(test_id);
                    } else {
                        evidence_tails.push(scenario_id);
                    }
                }
            }
            if let Some(head) = &path_head {
                for tail in &evidence_tails {
                    out.push_str(&format!("  {tail} --> {head}\n"));
                }
            }
            let mut authority_tail = path_tail
                .clone()
                .or_else(|| evidence_tails.first().cloned())
                .unwrap_or_else(|| format!("{link_prefix}_wu"));
            if let Some(worktree) = &link.worktree {
                out.push_str(&format!(
                    "  {link_prefix}_worktree[\"{}\"]\n  {authority_tail} --> {link_prefix}_worktree\n  {link_prefix}_branch[\"{}\"]\n  {link_prefix}_worktree --> {link_prefix}_branch\n",
                    escape_mermaid(&worktree.path.display().to_string()),
                    escape_mermaid(&worktree.branch)
                ));
                authority_tail = format!("{link_prefix}_branch");
            }
            if let Some(vcs) = &record.intent_impact.observed_vcs {
                out.push_str(&format!(
                    "  {link_prefix}_vcs[\"{:?} {}\"]\n  {authority_tail} --> {link_prefix}_vcs\n",
                    vcs.vcs_type,
                    escape_mermaid(&vcs.change_ref)
                ));
            }
        }

        if let Some(tail) = path_tail {
            for (quality_index, outcome) in record.quality_outcomes.iter().enumerate() {
                let label = quality_outcome_label(&outcome.outcome);
                out.push_str(&format!(
                    "  {prefix}_quality{quality_index}[\"{}: {}\"]\n  {tail} --> {prefix}_quality{quality_index}\n",
                    escape_mermaid(&outcome.provider_id),
                    escape_mermaid(&label)
                ));
            }
        }
    }
    out
}

fn quality_outcome_label(outcome: &QualityOutcome) -> String {
    serde_json::to_value(outcome)
        .ok()
        .and_then(|value| {
            value
                .get("outcome")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".into())
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
    use crate::spec_knowledge::{
        CodeImpactInput, INTENT_IMPACT_SCHEMA_ID, ImpactCodeNode, ImpactPath, IntentAffectedNode,
        IntentBindingLink, IntentScenarioLink, IntentSpecLink, PlannedWorktreeLink,
        ProviderImpactEntry, RequirementPlanNode, RequirementPlanStatus,
    };
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
            code_target_facts: Vec::new(),
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
            affected_records: Vec::new(),
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
        for expected in [
            "code targets: src/lib.rs",
            "worktree: ../agent-spec-worktrees/wu-req-note-create",
            "branch: feat/wu-req-note-create",
            "vcs: <none>",
        ] {
            assert!(text.contains(expected), "missing `{expected}` in:\n{text}");
        }
    }

    #[test]
    fn test_latest_requirement_trace_records_returns_only_latest_run() {
        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![
                trace_record("REQ-NOTE-CREATE", "old-fail", 1, Verdict::Fail),
                trace_record("REQ-NOTE-CREATE", "new-pass", 2, Verdict::Pass),
            ],
            affected_records: Vec::new(),
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
            code_target_facts: Vec::new(),
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
            affected_records: Vec::new(),
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
            code_target_facts: Vec::new(),
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
            affected_records: Vec::new(),
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
            affected_records: Vec::new(),
            diagnostics: Vec::new(),
        };

        let explanation = explain_requirement_failure(&ledger, "REQ-NOTE-CREATE");
        assert!(explanation.non_pass_records.is_empty());
    }

    #[test]
    fn test_requirements_trace_graph_mermaid_contains_evidence_nodes() {
        let mut record = failing_record("REQ-NOTE-CREATE");
        record.vcs = Some(VcsContext {
            vcs_type: VcsType::Git,
            change_ref: "abc1234".into(),
            operation_ref: None,
        });
        let mermaid = format_requirement_trace_mermaid(&[record]);
        assert!(mermaid.contains("flowchart LR"));
        assert!(mermaid.contains("REQ-NOTE-CREATE"));
        assert!(mermaid.contains("WU-REQ-NOTE-CREATE"));
        assert!(mermaid.contains("Scenario: Create note"));
        assert!(mermaid.contains("Test: note_create_adds_note"));
        assert!(mermaid.contains("src/lib.rs"));
        assert!(mermaid.contains("../agent-spec-worktrees/wu-req-note-create"));
        assert!(mermaid.contains("feat/wu-req-note-create"));
        assert!(mermaid.contains("r0_test --> r0_code0"));
        assert!(mermaid.contains("r0_code0 --> r0_worktree"));
        assert!(mermaid.contains("r0_worktree --> r0_branch"));
        assert!(mermaid.contains("r0_branch --> r0_vcs"));
    }

    #[test]
    fn test_requirement_trace_promotes_typed_atlas_facts_to_code_targets() {
        let report = VerificationReport::from_results(
            "Atlas".into(),
            vec![trace_result("Resolve symbol", "test_resolve_symbol")],
        );
        let fact = CodeTargetFact {
            provider: "rust-atlas".into(),
            node_id: "agent_spec::atlas_eval::build_run_plan".into(),
            kind: "function".into(),
            file: "src/atlas_eval.rs".into(),
            provenance: "syn".into(),
            graph_fingerprint: "graph-1".into(),
        };

        let record = RequirementTraceRecord::from_parts(RequirementTraceRecordInput {
            run_id: "run-atlas".into(),
            timestamp: 1,
            requirement_id: "REQ-ATLAS".into(),
            requirement_source: PathBuf::from("knowledge/requirements/req-atlas.md"),
            work_unit_id: "WU-REQ-ATLAS".into(),
            spec_path: PathBuf::from("specs/task-atlas.spec.md"),
            scenario_name: "Resolve symbol".into(),
            test_selector: Some("test_resolve_symbol".into()),
            report: &report,
            worktree_path: None,
            branch: None,
            vcs: None,
            code_target_facts: vec![fact],
        })
        .unwrap();

        assert_eq!(
            record.code_targets,
            vec!["agent_spec::atlas_eval::build_run_plan", "src/lib.rs"]
        );
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
            affected_records: Vec::new(),
            diagnostics: Vec::new(),
        };
        write_requirement_trace_ledger(&dir, &ledger).unwrap();

        let merged = read_requirement_trace_ledgers(&dir.join(".agent-spec/trace"));
        assert_eq!(merged.records.len(), 1);
        assert_eq!(merged.records[0].requirement_id, "REQ-NOTE-CREATE");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn affected_trace_writer_merges_with_existing_lifecycle_run() {
        let dir =
            std::env::temp_dir().join(format!("affected-trace-writer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut lifecycle = trace_record("REQ-AFFECTED", "same-run", 10, Verdict::Pass);
        lifecycle.run_id = "same-run".into();
        let ledger = RequirementTraceLedger {
            version: 1,
            records: vec![lifecycle],
            affected_records: Vec::new(),
            diagnostics: Vec::new(),
        };
        write_requirement_trace_ledger_to_dir(&dir, &ledger).unwrap();

        write_affected_trace_record_to_dir(
            &dir,
            affected_record("same-run", 11, QualityOutcome::Pass),
        )
        .unwrap();

        let merged = read_requirement_trace_ledgers(&dir);
        assert_eq!(merged.version, REQUIREMENT_TRACE_LEDGER_VERSION);
        assert_eq!(merged.records.len(), 1);
        assert_eq!(merged.affected_records.len(), 1);
        assert_eq!(merged.affected_records[0].run_id, "same-run");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn affected_trace_writer_preserves_existing_same_run_evidence() {
        let dir = std::env::temp_dir().join(format!(
            "affected-trace-preserve-existing-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut complete = affected_record("same-run", 11, QualityOutcome::Fail);
        complete.execution_bundle = Some(AffectedExecutionBundle {
            schema: crate::spec_knowledge::AFFECTED_EXECUTION_BUNDLE_SCHEMA_ID.into(),
            intent_impact_digest: complete.intent_impact_digest.clone(),
            risk: Some("A".into()),
            required_evidence: vec!["lifecycle".into()],
            quality_profile: Vec::new(),
            fast_checks: Vec::new(),
            acceptance_gates: Vec::new(),
            authoritative_tests: Vec::new(),
            test_candidates: Vec::new(),
            guidance: Vec::new(),
            required_skills: Vec::new(),
            skill_receipts: Vec::new(),
            gaps: Vec::new(),
        });
        write_affected_trace_record_to_dir(&dir, complete).unwrap();

        let mut impact_only = affected_record("same-run", 11, QualityOutcome::Pass);
        impact_only.execution_bundle = None;
        impact_only.quality_outcomes.clear();
        write_affected_trace_record_to_dir(&dir, impact_only).unwrap();

        let merged = read_requirement_trace_ledgers(&dir);
        let record = &merged.affected_records[0];
        assert!(record.execution_bundle.is_some());
        assert_eq!(record.quality_outcomes.len(), 1);
        assert_eq!(record.quality_outcomes[0].outcome, QualityOutcome::Fail);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn affected_trace_conflict_keeps_in_memory_ledger_atomic() {
        let existing = affected_record("same-run", 11, QualityOutcome::Fail);
        let mut ledger = RequirementTraceLedger {
            version: 2,
            records: Vec::new(),
            affected_records: vec![existing],
            diagnostics: Vec::new(),
        };
        let before = serde_json::to_vec(&ledger).unwrap();
        let mut conflicting = affected_record("same-run", 11, QualityOutcome::Pass);
        conflicting.execution_bundle = Some(AffectedExecutionBundle {
            schema: crate::spec_knowledge::AFFECTED_EXECUTION_BUNDLE_SCHEMA_ID.into(),
            intent_impact_digest: conflicting.intent_impact_digest.clone(),
            risk: Some("A".into()),
            required_evidence: Vec::new(),
            quality_profile: Vec::new(),
            fast_checks: Vec::new(),
            acceptance_gates: Vec::new(),
            authoritative_tests: Vec::new(),
            test_candidates: Vec::new(),
            guidance: Vec::new(),
            required_skills: Vec::new(),
            skill_receipts: Vec::new(),
            gaps: Vec::new(),
        });

        let error = record_affected_trace(&mut ledger, conflicting).unwrap_err();

        assert!(error.contains("already records different evidence"));
        assert_eq!(serde_json::to_vec(&ledger).unwrap(), before);
    }

    #[test]
    fn test_affected_failure_replay_returns_latest_full_chain() {
        let ledger = RequirementTraceLedger {
            version: 2,
            records: vec![
                trace_record("REQ-AFFECTED", "old", 1, Verdict::Fail),
                trace_record("REQ-AFFECTED", "new", 2, Verdict::Pass),
            ],
            affected_records: vec![
                affected_record("old", 1, QualityOutcome::Fail),
                affected_record("new", 2, QualityOutcome::Pass),
            ],
            diagnostics: Vec::new(),
        };

        let replay = replay_affected_requirement(&ledger, "REQ-AFFECTED");

        assert_eq!(replay.lifecycle_records.len(), 1);
        assert_eq!(replay.lifecycle_records[0].run_id, "new");
        let affected = replay.affected_record.unwrap();
        assert_eq!(affected.run_id, "new");
        assert_eq!(
            affected.intent_impact.affected[0].impact.node.file,
            "src/feature.rs"
        );
        assert_eq!(
            affected.intent_impact.affected[0].links[0].specs[0].scenarios[0]
                .authoritative_selector
                .as_deref(),
            Some("test_feature")
        );
        assert_eq!(
            affected.intent_impact.affected[0].links[0]
                .worktree
                .as_ref()
                .unwrap()
                .branch,
            "feat/affected"
        );
        assert_eq!(
            affected
                .intent_impact
                .observed_vcs
                .as_ref()
                .unwrap()
                .change_ref,
            "abc123"
        );
    }

    #[test]
    fn test_affected_failure_replay_includes_lifecycle_and_quality_failures() {
        let ledger = RequirementTraceLedger {
            version: 2,
            records: vec![trace_record("REQ-AFFECTED", "failed", 3, Verdict::Fail)],
            affected_records: vec![affected_record("failed", 3, QualityOutcome::Fail)],
            diagnostics: Vec::new(),
        };

        let failure = explain_affected_requirement_failure(&ledger, "REQ-AFFECTED");

        assert_eq!(failure.lifecycle_non_pass_records.len(), 1);
        assert_eq!(failure.quality_failures.len(), 1);
        assert_eq!(failure.quality_failures[0].provider_id, "cargo-clippy");
    }

    #[test]
    fn test_affected_failure_replay_preserves_link_gaps() {
        let mut record = affected_record("gaps", 4, QualityOutcome::Pass);
        record.intent_impact.gaps.push(IntentImpactGap {
            code: "selector-missing".into(),
            severity: "error".into(),
            node_id: Some("feature".into()),
            requirement_id: Some("REQ-AFFECTED".into()),
            spec_path: Some(PathBuf::from("specs/task-affected.spec.md")),
            message: "selector is missing".into(),
        });
        let ledger = RequirementTraceLedger {
            version: 2,
            records: Vec::new(),
            affected_records: vec![record],
            diagnostics: Vec::new(),
        };

        let replay = replay_affected_requirement(&ledger, "REQ-AFFECTED");

        assert!(replay.gaps.iter().any(|gap| gap.code == "selector-missing"));
        assert!(
            replay
                .gaps
                .iter()
                .any(|gap| gap.code == "lifecycle-trace-missing")
        );
    }

    #[test]
    fn test_affected_failure_replay_reads_v1_with_missing_context_gap() {
        let legacy = serde_json::json!({
            "version": 1,
            "records": [trace_record("REQ-AFFECTED", "legacy", 1, Verdict::Pass)],
            "diagnostics": []
        });
        let ledger: RequirementTraceLedger = serde_json::from_value(legacy).unwrap();

        let replay = replay_affected_requirement(&ledger, "REQ-AFFECTED");

        assert_eq!(replay.lifecycle_records.len(), 1);
        assert!(replay.affected_record.is_none());
        assert!(
            replay
                .gaps
                .iter()
                .any(|gap| gap.code == "affected-trace-missing")
        );
    }

    #[test]
    fn test_affected_failure_replay_never_reruns_tools_or_models() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static EXTERNAL_INVOCATIONS: AtomicUsize = AtomicUsize::new(0);
        let ledger = RequirementTraceLedger {
            version: 2,
            records: Vec::new(),
            affected_records: vec![affected_record("pure", 5, QualityOutcome::Pass)],
            diagnostics: Vec::new(),
        };

        let _ = replay_affected_requirement(&ledger, "REQ-AFFECTED");
        let _ = explain_affected_requirement_failure(&ledger, "REQ-AFFECTED");

        assert_eq!(EXTERNAL_INVOCATIONS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_affected_trace_graph_contains_saved_code_and_authority_chain() {
        let ledger = RequirementTraceLedger {
            version: 2,
            records: vec![trace_record("REQ-AFFECTED", "graph", 7, Verdict::Fail)],
            affected_records: vec![affected_record("graph", 7, QualityOutcome::Fail)],
            diagnostics: Vec::new(),
        };
        let replay = replay_affected_requirement(&ledger, "REQ-AFFECTED");
        let mermaid = format_affected_requirement_trace_mermaid(&replay);

        for expected in [
            "REQ-AFFECTED",
            "WU-REQ-AFFECTED",
            "src/feature.rs:10-20",
            "Test: test_feature",
            "../worktrees/affected",
            "feat/affected",
            "abc123",
            "cargo-clippy: fail",
        ] {
            assert!(
                mermaid.contains(expected),
                "missing `{expected}` in:\n{mermaid}"
            );
        }
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
            code_target_facts: Vec::new(),
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

    fn affected_record(
        run_id: &str,
        timestamp: u64,
        outcome: QualityOutcome,
    ) -> AffectedTraceRecord {
        let node = ImpactCodeNode {
            node_id: "feature".into(),
            symbol: "crate::feature".into(),
            kind: "fn".into(),
            file: "src/feature.rs".into(),
            line_start: 10,
            line_end: 20,
            provenance: "syn".into(),
        };
        AffectedTraceRecord {
            run_id: run_id.into(),
            timestamp,
            requirement_ids: vec!["REQ-AFFECTED".into()],
            intent_impact_digest: format!("digest-{run_id}"),
            intent_impact: IntentImpactReport {
                schema: INTENT_IMPACT_SCHEMA_ID.into(),
                provider: "rust-atlas".into(),
                graph_fingerprint: Some("graph-1".into()),
                input: CodeImpactInput::Symbol {
                    symbol: "crate::feature".into(),
                },
                affected: vec![IntentAffectedNode {
                    impact: ProviderImpactEntry {
                        node: node.clone(),
                        distance: 0,
                        path: ImpactPath {
                            nodes: vec![node],
                            hops: Vec::new(),
                            confidence: "exact".into(),
                        },
                    },
                    links: vec![IntentBindingLink {
                        requirement_id: "REQ-AFFECTED".into(),
                        work_unit_id: "WU-REQ-AFFECTED".into(),
                        provider: "rust-atlas".into(),
                        graph_fingerprint: "graph-1".into(),
                        specs: vec![IntentSpecLink {
                            path: PathBuf::from("specs/task-affected.spec.md"),
                            risk: Some("B".into()),
                            scenarios: vec![IntentScenarioLink {
                                name: "Affected behavior".into(),
                                authoritative_selector: Some("test_feature".into()),
                                test_candidate: None,
                                test_obligation: None,
                                required_evidence: vec!["test".into()],
                            }],
                        }],
                        worktree: Some(PlannedWorktreeLink {
                            path: PathBuf::from("../worktrees/affected"),
                            branch: "feat/affected".into(),
                            base_branch: "main".into(),
                            batch: 1,
                        }),
                    }],
                }],
                truncated: false,
                gaps: Vec::new(),
                provider_diagnostics: Vec::new(),
                observed_vcs: Some(VcsContext {
                    vcs_type: VcsType::Git,
                    change_ref: "abc123".into(),
                    operation_ref: None,
                }),
            },
            execution_bundle: None,
            quality_outcomes: vec![AffectedQualityOutcome {
                provider_id: "cargo-clippy".into(),
                outcome,
                summary: "recorded outcome".into(),
            }],
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
