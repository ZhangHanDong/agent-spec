use crate::atlas_eval::{CacheCondition, Corpus, Permissions, TaskClass, WorkspaceSize};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub const AGENT_EXPERIMENT_SCHEMA: &str = "agent-spec/atlas-eval/agent-experiment-v1";
pub const AGENT_PLAN_SCHEMA: &str = "agent-spec/atlas-eval/agent-plan-v1";
pub const AGENT_RECEIPTS_SCHEMA: &str = "agent-spec/atlas-eval/agent-receipts-v1";
pub const AGENT_GATE_SCHEMA: &str = "agent-spec/atlas-eval/agent-gate-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentExperiment {
    pub schema: String,
    pub version: String,
    pub controls: AgentControls,
    pub session_store: String,
    pub surfaces: AgentToolSurfaces,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentControls {
    pub prompt_hooks: SymmetricControl,
    pub mcp_config: SymmetricControl,
    pub user_skills: SymmetricControl,
    pub tool_instructions: SymmetricControl,
    pub judge: JudgeConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SymmetricControl {
    Disabled,
    Pinned { fingerprint: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JudgeMode {
    Rubric,
    BlindReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JudgeConfig {
    pub mode: JudgeMode,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentToolSurfaces {
    pub baseline: Vec<String>,
    pub atlas_primitives: Vec<String>,
    pub atlas_context: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentArm {
    Baseline,
    AtlasPrimitives,
    AtlasContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRunPlan {
    pub schema: String,
    pub experiment_version: String,
    pub corpus_fingerprint: String,
    pub experiment_fingerprint: String,
    pub runs: Vec<AgentPlannedRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentPlannedRun {
    pub run_id: String,
    pub case_id: String,
    pub arm: AgentArm,
    pub trial: u32,
    pub size: WorkspaceSize,
    pub task_class: TaskClass,
    pub model: String,
    pub prompt: String,
    pub repository: String,
    pub revision: String,
    pub permissions: Permissions,
    pub cache_condition: CacheCondition,
    pub rubric: Vec<String>,
    pub rubric_fingerprint: String,
    pub controls: AgentControls,
    pub environment_fingerprint: String,
    pub tools: Vec<String>,
    pub surface_fingerprint: String,
    pub session_store: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentReceiptBundle {
    pub schema: String,
    pub experiment_version: String,
    pub plan_fingerprint: String,
    pub runs: Vec<AgentRunReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentRunOutcome {
    Completed,
    Failed,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRunReceipt {
    pub run_id: String,
    pub outcome: AgentRunOutcome,
    pub correctness: AgentCorrectness,
    pub judge_version: String,
    pub rubric_fingerprint: String,
    pub raw_session: EvidenceArtifact,
    pub answer_hash: String,
    pub tool_trace_hash: String,
    pub query_metrics_schema: String,
    pub stale_as_fresh: bool,
    pub metrics: AgentRunMetrics,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCorrectness {
    pub passed: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceArtifact {
    pub path: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRunMetrics {
    pub file_reads: u64,
    pub grep_calls: u64,
    pub graph_calls: u64,
    pub tool_calls: u64,
    pub round_trips: u64,
    pub duration_ms: u64,
    pub response_bytes: u64,
    pub context_bytes: u64,
    pub cost_usd: Option<f64>,
    pub read_back_calls: u64,
    pub follow_up_queries: u64,
    pub truncated_queries: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentGateReceipt {
    pub schema: String,
    pub experiment_version: String,
    pub plan_fingerprint: String,
    pub receipts: usize,
    pub failed_runs: Vec<FailedAgentRun>,
    pub comparisons: std::collections::BTreeMap<PromotionCandidate, AgentPromotionComparison>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromotionCandidate {
    AtlasPrimitives,
    AtlasContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateState {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentPromotionComparison {
    pub candidate: PromotionCandidate,
    pub reference_arm: AgentArm,
    pub candidate_arm: AgentArm,
    pub state: GateState,
    pub cases: Vec<AgentCaseComparison>,
    pub diagnostics: Vec<GateDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaseDecision {
    Improved,
    Tie,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetricDecision {
    Improved,
    Tie,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCaseComparison {
    pub case_id: String,
    pub size: WorkspaceSize,
    pub task_class: TaskClass,
    pub state: CaseDecision,
    pub read_grep: MetricComparison,
    pub round_trips: MetricComparison,
    pub tool_calls: MetricComparison,
    pub reference_metrics: AgentMetricAggregate,
    pub candidate_metrics: AgentMetricAggregate,
    pub diagnostics: Vec<GateDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricComparison {
    pub reference: MetricBand,
    pub candidate: MetricBand,
    pub decision: MetricDecision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentMetricAggregate {
    pub file_reads: MetricBand,
    pub grep_calls: MetricBand,
    pub graph_calls: MetricBand,
    pub tool_calls: MetricBand,
    pub round_trips: MetricBand,
    pub duration_ms: MetricBand,
    pub response_bytes: MetricBand,
    pub context_bytes: MetricBand,
    pub cost_usd: Option<MetricBand>,
    pub read_back_calls: MetricBand,
    pub follow_up_queries: MetricBand,
    pub truncated_queries: MetricBand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricBand {
    pub samples: usize,
    pub median: f64,
    pub mad: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateDiagnostic {
    pub code: String,
    pub message: String,
    pub run_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailedAgentRun {
    pub run_id: String,
    pub outcome: AgentRunOutcome,
    pub diagnostic: String,
}

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct AgentEvalError {
    code: &'static str,
    message: String,
}

impl AgentEvalError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

pub fn compile_agent_plan(
    corpus: &Corpus,
    experiment: &AgentExperiment,
) -> Result<AgentRunPlan, AgentEvalError> {
    validate_agent_experiment(experiment)?;
    if let Some(case) = corpus.cases.iter().find(|case| case.trials_per_arm < 3) {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-trials",
            format!("case {} must have at least three trials per arm", case.id),
        ));
    }
    crate::atlas_eval::compile_plan(corpus).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-corpus",
            format!("E0 corpus is not valid: {error}"),
        )
    })?;

    let corpus_fingerprint = fingerprint(corpus)?;
    let experiment_fingerprint = fingerprint(experiment)?;
    let arms = [
        (AgentArm::Baseline, &experiment.surfaces.baseline),
        (
            AgentArm::AtlasPrimitives,
            &experiment.surfaces.atlas_primitives,
        ),
        (AgentArm::AtlasContext, &experiment.surfaces.atlas_context),
    ];
    let mut runs = Vec::new();
    for case in &corpus.cases {
        let rubric_fingerprint = fingerprint(&case.rubric)?;
        let environment_fingerprint = fingerprint(&(
            &corpus.model,
            &corpus.prompt,
            &case.repository,
            &case.revision,
            case.permissions,
            case.cache_condition,
            &experiment.controls,
            &experiment.session_store,
        ))?;
        for trial in 1..=case.trials_per_arm {
            for (arm, tools) in arms {
                let surface_fingerprint = fingerprint(tools)?;
                let run_id = fingerprint(&(
                    &case.id,
                    trial,
                    arm,
                    &environment_fingerprint,
                    &surface_fingerprint,
                ))?;
                runs.push(AgentPlannedRun {
                    run_id,
                    case_id: case.id.clone(),
                    arm,
                    trial,
                    size: case.size,
                    task_class: case.task_class,
                    model: corpus.model.clone(),
                    prompt: corpus.prompt.clone(),
                    repository: case.repository.clone(),
                    revision: case.revision.clone(),
                    permissions: case.permissions,
                    cache_condition: case.cache_condition,
                    rubric: case.rubric.clone(),
                    rubric_fingerprint: rubric_fingerprint.clone(),
                    controls: experiment.controls.clone(),
                    environment_fingerprint: environment_fingerprint.clone(),
                    tools: tools.clone(),
                    surface_fingerprint,
                    session_store: experiment.session_store.clone(),
                });
            }
        }
    }

    Ok(AgentRunPlan {
        schema: AGENT_PLAN_SCHEMA.to_string(),
        experiment_version: experiment.version.clone(),
        corpus_fingerprint,
        experiment_fingerprint,
        runs,
    })
}

pub fn load_agent_experiment(path: &Path) -> Result<AgentExperiment, AgentEvalError> {
    let bytes = std::fs::read(path).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-load",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    let experiment = serde_json::from_slice(&bytes).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-experiment",
            format!("failed to parse {}: {error}", path.display()),
        )
    })?;
    validate_agent_experiment(&experiment)?;
    Ok(experiment)
}

pub fn load_agent_plan(path: &Path) -> Result<AgentRunPlan, AgentEvalError> {
    let bytes = std::fs::read(path).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-load",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    let plan = serde_json::from_slice(&bytes).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-plan",
            format!("failed to parse {}: {error}", path.display()),
        )
    })?;
    validate_agent_plan(&plan)?;
    Ok(plan)
}

pub fn validate_agent_plan(plan: &AgentRunPlan) -> Result<(), AgentEvalError> {
    if plan.schema != AGENT_PLAN_SCHEMA
        || plan.experiment_version.trim().is_empty()
        || !is_lower_hex(&plan.corpus_fingerprint)
        || !is_lower_hex(&plan.experiment_fingerprint)
        || plan.runs.is_empty()
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-plan",
            "Agent plan has an invalid schema, version, fingerprint, or empty run set",
        ));
    }
    let mut run_ids = BTreeSet::new();
    let mut groups = std::collections::BTreeMap::<(&str, u32), Vec<&AgentPlannedRun>>::new();
    let mut trials = std::collections::BTreeMap::<&str, BTreeSet<u32>>::new();
    for run in &plan.runs {
        validate_planned_run(run)?;
        if !run_ids.insert(run.run_id.as_str()) {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-plan",
                format!("duplicate planned run {}", run.run_id),
            ));
        }
        groups
            .entry((run.case_id.as_str(), run.trial))
            .or_default()
            .push(run);
        trials
            .entry(run.case_id.as_str())
            .or_default()
            .insert(run.trial);
    }
    for ((case_id, trial), runs) in groups {
        if runs.len() != 3 {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-plan",
                format!("case {case_id} trial {trial} must have exactly three arms"),
            ));
        }
        let by_arm = runs
            .iter()
            .map(|run| (run.arm, *run))
            .collect::<std::collections::BTreeMap<_, _>>();
        let Some(baseline) = by_arm.get(&AgentArm::Baseline) else {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-plan",
                format!("case {case_id} trial {trial} is missing baseline"),
            ));
        };
        let Some(primitives) = by_arm.get(&AgentArm::AtlasPrimitives) else {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-plan",
                format!("case {case_id} trial {trial} is missing atlas-primitives"),
            ));
        };
        let Some(context) = by_arm.get(&AgentArm::AtlasContext) else {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-plan",
                format!("case {case_id} trial {trial} is missing atlas-context"),
            ));
        };
        if runs
            .iter()
            .any(|run| run.environment_fingerprint != baseline.environment_fingerprint)
        {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-environment",
                format!("case {case_id} trial {trial} has asymmetric controls"),
            ));
        }
        validate_surfaces(&AgentToolSurfaces {
            baseline: baseline.tools.clone(),
            atlas_primitives: primitives.tools.clone(),
            atlas_context: context.tools.clone(),
        })?;
    }
    for (case_id, case_trials) in trials {
        let expected = (1..=case_trials.len() as u32).collect::<BTreeSet<_>>();
        if case_trials.len() < 3 || case_trials != expected {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-trials",
                format!("case {case_id} must have at least three contiguous trials"),
            ));
        }
    }
    Ok(())
}

fn validate_planned_run(run: &AgentPlannedRun) -> Result<(), AgentEvalError> {
    validate_controls(&run.controls)?;
    let expected_surface = fingerprint(&run.tools)?;
    let expected_rubric = fingerprint(&run.rubric)?;
    let expected_environment = fingerprint(&(
        &run.model,
        &run.prompt,
        &run.repository,
        &run.revision,
        run.permissions,
        run.cache_condition,
        &run.controls,
        &run.session_store,
    ))?;
    let expected_run_id = fingerprint(&(
        &run.case_id,
        run.trial,
        run.arm,
        &run.environment_fingerprint,
        &run.surface_fingerprint,
    ))?;
    if run.case_id.trim().is_empty()
        || run.model.trim().is_empty()
        || run.prompt.trim().is_empty()
        || run.repository.trim().is_empty()
        || run.revision.trim().is_empty()
        || run.rubric.is_empty()
        || run.rubric.iter().any(|item| item.trim().is_empty())
        || run.trial == 0
        || is_temporary_path(&run.session_store)
        || expected_surface != run.surface_fingerprint
        || expected_rubric != run.rubric_fingerprint
        || expected_environment != run.environment_fingerprint
        || expected_run_id != run.run_id
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-plan",
            format!("planned run {} failed self-validation", run.run_id),
        ));
    }
    tool_set("planned run", &run.tools)?;
    Ok(())
}

pub fn parse_agent_receipts(bytes: &[u8]) -> Result<AgentReceiptBundle, AgentEvalError> {
    serde_json::from_slice(bytes).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-receipt",
            format!("failed to parse strict Agent receipt bundle: {error}"),
        )
    })
}

pub fn load_agent_receipts(path: &Path) -> Result<AgentReceiptBundle, AgentEvalError> {
    let bytes = std::fs::read(path).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-load",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    parse_agent_receipts(&bytes)
}

pub fn validate_agent_receipts(
    plan: &AgentRunPlan,
    bundle: &AgentReceiptBundle,
) -> Result<(), AgentEvalError> {
    if plan.schema != AGENT_PLAN_SCHEMA
        || bundle.schema != AGENT_RECEIPTS_SCHEMA
        || bundle.experiment_version != plan.experiment_version
        || bundle.plan_fingerprint != fingerprint(plan)?
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-receipt",
            "receipt bundle does not match the versioned Agent plan",
        ));
    }

    let planned = plan
        .runs
        .iter()
        .map(|run| (run.run_id.as_str(), run))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for receipt in &bundle.runs {
        if !seen.insert(receipt.run_id.as_str()) || !planned.contains_key(receipt.run_id.as_str()) {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-completeness",
                format!("duplicate or unknown receipt run {}", receipt.run_id),
            ));
        }
    }
    if seen.len() != planned.len() {
        let missing = planned
            .keys()
            .filter(|run_id| !seen.contains(**run_id))
            .copied()
            .collect::<Vec<_>>();
        return Err(AgentEvalError::new(
            "atlas-agent-ab-completeness",
            format!(
                "receipt bundle is missing planned runs: {}",
                missing.join(", ")
            ),
        ));
    }

    for receipt in &bundle.runs {
        let run = planned[receipt.run_id.as_str()];
        validate_agent_run_receipt(run, receipt)?;
    }
    Ok(())
}

pub fn gate_agent_receipts(
    plan: &AgentRunPlan,
    bundle: &AgentReceiptBundle,
) -> Result<AgentGateReceipt, AgentEvalError> {
    validate_agent_receipts(plan, bundle)?;
    let receipts = bundle
        .runs
        .iter()
        .map(|receipt| (receipt.run_id.as_str(), receipt))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut comparisons = std::collections::BTreeMap::new();
    for (candidate, reference_arm, candidate_arm) in [
        (
            PromotionCandidate::AtlasPrimitives,
            AgentArm::Baseline,
            AgentArm::AtlasPrimitives,
        ),
        (
            PromotionCandidate::AtlasContext,
            AgentArm::AtlasPrimitives,
            AgentArm::AtlasContext,
        ),
    ] {
        comparisons.insert(
            candidate,
            compare_agent_arms(plan, &receipts, candidate, reference_arm, candidate_arm),
        );
    }
    let failed_runs = bundle
        .runs
        .iter()
        .filter(|receipt| receipt.outcome != AgentRunOutcome::Completed)
        .map(|receipt| FailedAgentRun {
            run_id: receipt.run_id.clone(),
            outcome: receipt.outcome,
            diagnostic: receipt.diagnostic.clone().unwrap_or_default(),
        })
        .collect();

    Ok(AgentGateReceipt {
        schema: AGENT_GATE_SCHEMA.to_string(),
        experiment_version: plan.experiment_version.clone(),
        plan_fingerprint: fingerprint(plan)?,
        receipts: bundle.runs.len(),
        failed_runs,
        comparisons,
    })
}

pub fn enforce_agent_gate(gate: &AgentGateReceipt) -> Result<(), AgentEvalError> {
    if gate
        .comparisons
        .values()
        .any(|comparison| comparison.state == GateState::Blocked)
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-blocked",
            "one or more Agent surface promotion candidates are blocked",
        ));
    }
    Ok(())
}

fn compare_agent_arms(
    plan: &AgentRunPlan,
    receipts: &std::collections::BTreeMap<&str, &AgentRunReceipt>,
    candidate: PromotionCandidate,
    reference_arm: AgentArm,
    candidate_arm: AgentArm,
) -> AgentPromotionComparison {
    let mut case_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for run in &plan.runs {
        if seen.insert(run.case_id.as_str()) {
            case_ids.push(run.case_id.as_str());
        }
    }

    let mut cases = Vec::new();
    let mut diagnostics = Vec::new();
    for case_id in case_ids {
        let reference_runs = runs_for_case(plan, receipts, case_id, reference_arm);
        let candidate_runs = runs_for_case(plan, receipts, case_id, candidate_arm);
        let Some(exemplar) = plan.runs.iter().find(|run| run.case_id == case_id) else {
            continue;
        };
        let mut case_diagnostics = candidate_run_diagnostics(&candidate_runs);
        diagnostics.extend(case_diagnostics.iter().cloned());
        let reference_metrics = aggregate_agent_metrics(&reference_runs);
        let candidate_metrics = aggregate_agent_metrics(&candidate_runs);
        let read_grep = metric_comparison(
            exemplar.size,
            metric_band(reference_runs.iter().map(|run| {
                run.metrics
                    .file_reads
                    .saturating_add(run.metrics.grep_calls)
            })),
            metric_band(candidate_runs.iter().map(|run| {
                run.metrics
                    .file_reads
                    .saturating_add(run.metrics.grep_calls)
            })),
        );
        let round_trips = metric_comparison(
            exemplar.size,
            metric_band(reference_runs.iter().map(|run| run.metrics.round_trips)),
            metric_band(candidate_runs.iter().map(|run| run.metrics.round_trips)),
        );
        let tool_calls = metric_comparison(
            exemplar.size,
            metric_band(reference_runs.iter().map(|run| run.metrics.tool_calls)),
            metric_band(candidate_runs.iter().map(|run| run.metrics.tool_calls)),
        );
        let metric_decisions = [
            read_grep.decision,
            round_trips.decision,
            tool_calls.decision,
        ];
        let state = if !case_diagnostics.is_empty()
            || metric_decisions.contains(&MetricDecision::Blocked)
        {
            CaseDecision::Blocked
        } else if metric_decisions
            .iter()
            .all(|decision| *decision == MetricDecision::Improved)
        {
            CaseDecision::Improved
        } else {
            CaseDecision::Tie
        };
        if state == CaseDecision::Blocked && case_diagnostics.is_empty() {
            let run_ids = candidate_runs
                .iter()
                .map(|receipt| receipt.run_id.clone())
                .collect();
            let diagnostic = GateDiagnostic {
                code: "atlas-agent-ab-efficiency".to_string(),
                message: format!(
                    "candidate {candidate_arm:?} did not clear the baseline MAD gate for {case_id}"
                ),
                run_ids,
            };
            case_diagnostics.push(diagnostic.clone());
            diagnostics.push(diagnostic);
        }
        cases.push(AgentCaseComparison {
            case_id: case_id.to_string(),
            size: exemplar.size,
            task_class: exemplar.task_class,
            state,
            read_grep,
            round_trips,
            tool_calls,
            reference_metrics,
            candidate_metrics,
            diagnostics: case_diagnostics,
        });
    }
    let state = if cases.iter().any(|case| case.state == CaseDecision::Blocked) {
        GateState::Blocked
    } else {
        GateState::Passed
    };
    AgentPromotionComparison {
        candidate,
        reference_arm,
        candidate_arm,
        state,
        cases,
        diagnostics,
    }
}

fn runs_for_case<'a>(
    plan: &'a AgentRunPlan,
    receipts: &std::collections::BTreeMap<&str, &'a AgentRunReceipt>,
    case_id: &str,
    arm: AgentArm,
) -> Vec<&'a AgentRunReceipt> {
    plan.runs
        .iter()
        .filter(|run| run.case_id == case_id && run.arm == arm)
        .map(|run| receipts[run.run_id.as_str()])
        .collect()
}

fn candidate_run_diagnostics(runs: &[&AgentRunReceipt]) -> Vec<GateDiagnostic> {
    let mut diagnostics = Vec::new();
    let failures = runs
        .iter()
        .filter(|run| run.outcome != AgentRunOutcome::Completed)
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        diagnostics.push(GateDiagnostic {
            code: "atlas-agent-ab-run-failure".to_string(),
            message: "candidate arm contains failed, timed out, or cancelled runs".to_string(),
            run_ids: failures,
        });
    }
    let incorrect = runs
        .iter()
        .filter(|run| !run.correctness.passed)
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    if !incorrect.is_empty() {
        diagnostics.push(GateDiagnostic {
            code: "atlas-agent-ab-correctness".to_string(),
            message: "candidate arm contains an incorrect answer".to_string(),
            run_ids: incorrect,
        });
    }
    let stale = runs
        .iter()
        .filter(|run| run.stale_as_fresh)
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        diagnostics.push(GateDiagnostic {
            code: "atlas-agent-ab-stale-as-fresh".to_string(),
            message: "candidate arm presented stale evidence as fresh".to_string(),
            run_ids: stale,
        });
    }
    diagnostics
}

fn metric_comparison(
    size: WorkspaceSize,
    reference: MetricBand,
    candidate: MetricBand,
) -> MetricComparison {
    let lower = (reference.median - reference.mad).max(0.0);
    let upper = reference.median + reference.mad;
    let decision = match size {
        WorkspaceSize::Small if candidate.median < lower => MetricDecision::Improved,
        WorkspaceSize::Small if candidate.median <= upper => MetricDecision::Tie,
        WorkspaceSize::Small => MetricDecision::Blocked,
        WorkspaceSize::Medium | WorkspaceSize::Large
            if candidate.median + reference.mad < reference.median =>
        {
            MetricDecision::Improved
        }
        WorkspaceSize::Medium | WorkspaceSize::Large => MetricDecision::Blocked,
    };
    MetricComparison {
        reference,
        candidate,
        decision,
    }
}

fn aggregate_agent_metrics(runs: &[&AgentRunReceipt]) -> AgentMetricAggregate {
    AgentMetricAggregate {
        file_reads: metric_band(runs.iter().map(|run| run.metrics.file_reads)),
        grep_calls: metric_band(runs.iter().map(|run| run.metrics.grep_calls)),
        graph_calls: metric_band(runs.iter().map(|run| run.metrics.graph_calls)),
        tool_calls: metric_band(runs.iter().map(|run| run.metrics.tool_calls)),
        round_trips: metric_band(runs.iter().map(|run| run.metrics.round_trips)),
        duration_ms: metric_band(runs.iter().map(|run| run.metrics.duration_ms)),
        response_bytes: metric_band(runs.iter().map(|run| run.metrics.response_bytes)),
        context_bytes: metric_band(runs.iter().map(|run| run.metrics.context_bytes)),
        cost_usd: metric_band_f64(runs.iter().filter_map(|run| run.metrics.cost_usd)),
        read_back_calls: metric_band(runs.iter().map(|run| run.metrics.read_back_calls)),
        follow_up_queries: metric_band(runs.iter().map(|run| run.metrics.follow_up_queries)),
        truncated_queries: metric_band(runs.iter().map(|run| run.metrics.truncated_queries)),
    }
}

fn metric_band(values: impl Iterator<Item = u64>) -> MetricBand {
    metric_band_values(values.map(|value| value as f64).collect())
}

fn metric_band_f64(values: impl Iterator<Item = f64>) -> Option<MetricBand> {
    let values = values.collect::<Vec<_>>();
    (!values.is_empty()).then(|| metric_band_values(values))
}

fn metric_band_values(mut values: Vec<f64>) -> MetricBand {
    values.sort_by(f64::total_cmp);
    let median = median_sorted(&values);
    let mut deviations = values
        .iter()
        .map(|value| (value - median).abs())
        .collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    MetricBand {
        samples: values.len(),
        median,
        mad: median_sorted(&deviations),
    }
}

fn median_sorted(values: &[f64]) -> f64 {
    match values.len() {
        0 => 0.0,
        len if len % 2 == 1 => values[len / 2],
        len => (values[len / 2 - 1] + values[len / 2]) / 2.0,
    }
}

fn validate_agent_run_receipt(
    run: &AgentPlannedRun,
    receipt: &AgentRunReceipt,
) -> Result<(), AgentEvalError> {
    let session_path = Path::new(&receipt.raw_session.path);
    if receipt.judge_version != run.controls.judge.version
        || receipt.rubric_fingerprint != run.rubric_fingerprint
        || receipt.correctness.rationale.trim().is_empty()
        || receipt.raw_session.path.trim().is_empty()
        || is_temporary_path(&receipt.raw_session.path)
        || !session_path.starts_with(Path::new(&run.session_store))
        || !is_lower_hex(&receipt.raw_session.hash)
        || !is_lower_hex(&receipt.answer_hash)
        || !is_lower_hex(&receipt.tool_trace_hash)
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-evidence",
            format!(
                "run {} has invalid judge, session, or trace evidence",
                run.run_id
            ),
        ));
    }
    if receipt.query_metrics_schema != crate::atlas_eval::QUERY_METRICS_SCHEMA {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-receipt",
            format!("run {} uses legacy query metrics", run.run_id),
        ));
    }
    if receipt
        .metrics
        .cost_usd
        .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-receipt",
            format!("run {} has invalid cost", run.run_id),
        ));
    }
    let completed = receipt.outcome == AgentRunOutcome::Completed;
    if completed && receipt.diagnostic.is_some()
        || !completed
            && (receipt.correctness.passed
                || receipt.diagnostic.as_deref().is_none_or(str::is_empty))
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-receipt",
            format!("run {} has inconsistent outcome fields", run.run_id),
        ));
    }
    Ok(())
}

pub fn validate_agent_experiment(experiment: &AgentExperiment) -> Result<(), AgentEvalError> {
    if experiment.schema != AGENT_EXPERIMENT_SCHEMA || experiment.version.trim().is_empty() {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-schema",
            format!("expected non-empty {AGENT_EXPERIMENT_SCHEMA} experiment"),
        ));
    }
    validate_controls(&experiment.controls)?;
    if experiment.session_store.trim().is_empty() || is_temporary_path(&experiment.session_store) {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-evidence",
            "session_store must be non-empty and outside /tmp",
        ));
    }
    validate_surfaces(&experiment.surfaces)
}

fn validate_controls(controls: &AgentControls) -> Result<(), AgentEvalError> {
    for control in [
        &controls.prompt_hooks,
        &controls.mcp_config,
        &controls.user_skills,
        &controls.tool_instructions,
    ] {
        if let SymmetricControl::Pinned { fingerprint } = control
            && !is_lower_hex(fingerprint)
        {
            return Err(AgentEvalError::new(
                "atlas-agent-ab-environment",
                "pinned controls require a 64-character lowercase hex fingerprint",
            ));
        }
    }
    if controls.judge.version.trim().is_empty() {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-environment",
            "judge version must not be empty",
        ));
    }
    Ok(())
}

fn validate_surfaces(surfaces: &AgentToolSurfaces) -> Result<(), AgentEvalError> {
    let baseline = tool_set("baseline", &surfaces.baseline)?;
    let primitives = tool_set("atlas_primitives", &surfaces.atlas_primitives)?;
    let context = tool_set("atlas_context", &surfaces.atlas_context)?;
    if !baseline.contains("read")
        || !baseline.contains("grep")
        || baseline.iter().any(|tool| tool.starts_with("atlas-"))
        || !baseline.is_subset(&primitives)
        || !primitives.contains("atlas-explore")
        || primitives.contains("atlas-context")
        || primitives
            .difference(&baseline)
            .any(|tool| !tool.starts_with("atlas-"))
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-surface",
            "baseline and atlas-primitives do not form the declared Atlas ablation",
        ));
    }
    let expected_context = primitives
        .iter()
        .copied()
        .chain(std::iter::once("atlas-context"))
        .collect::<BTreeSet<_>>();
    if context != expected_context {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-surface",
            "atlas-context must equal atlas-primitives plus atlas-context",
        ));
    }
    Ok(())
}

fn tool_set<'a>(name: &str, tools: &'a [String]) -> Result<BTreeSet<&'a str>, AgentEvalError> {
    if tools.is_empty()
        || tools.iter().any(|tool| tool.trim().is_empty())
        || tools.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(AgentEvalError::new(
            "atlas-agent-ab-surface",
            format!("{name} tools must be non-empty, unique, and sorted"),
        ));
    }
    Ok(tools.iter().map(String::as_str).collect())
}

fn fingerprint<T: Serialize + ?Sized>(value: &T) -> Result<String, AgentEvalError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        AgentEvalError::new(
            "atlas-agent-ab-fingerprint",
            format!("failed to serialize fingerprint input: {error}"),
        )
    })?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn is_lower_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_temporary_path(value: &str) -> bool {
    let path = Path::new(value);
    path.starts_with("/tmp") || path.starts_with("/private/tmp")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::atlas_eval::{
        CORPUS_SCHEMA, CacheCondition, Case, Corpus, Permissions, TaskClass, WorkspaceSize,
    };
    use std::collections::BTreeSet;

    fn corpus(trials_per_arm: u32) -> Corpus {
        Corpus {
            schema: CORPUS_SCHEMA.to_string(),
            model: "fixture-model".to_string(),
            prompt: "Answer with evidence.".to_string(),
            cases: vec![Case {
                id: "implementation-case".to_string(),
                size: WorkspaceSize::Medium,
                task_class: TaskClass::Implementation,
                repository: "repos/example".to_string(),
                revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
                trials_per_arm,
                rubric: vec!["Names the implementation symbol.".to_string()],
                permissions: Permissions::WorkspaceWrite,
                cache_condition: CacheCondition::Cold,
            }],
        }
    }

    fn experiment() -> AgentExperiment {
        serde_json::from_value(serde_json::json!({
            "schema": AGENT_EXPERIMENT_SCHEMA,
            "version": "e1-fixture-v1",
            "controls": {
                "prompt_hooks": { "mode": "disabled" },
                "mcp_config": {
                    "mode": "pinned",
                    "fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                },
                "user_skills": { "mode": "disabled" },
                "tool_instructions": {
                    "mode": "pinned",
                    "fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                },
                "judge": { "mode": "rubric", "version": "judge-v1" }
            },
            "session_store": "artifacts/atlas-agent-ab",
            "surfaces": {
                "baseline": ["grep", "read"],
                "atlas_primitives": [
                    "atlas-explore", "atlas-flow", "atlas-impact", "atlas-search", "grep", "read"
                ],
                "atlas_context": [
                    "atlas-context", "atlas-explore", "atlas-flow", "atlas-impact",
                    "atlas-search", "grep", "read"
                ]
            }
        }))
        .unwrap()
    }

    fn agent_plan() -> AgentRunPlan {
        compile_agent_plan(&corpus(3), &experiment()).unwrap()
    }

    fn receipt_for(run: &AgentPlannedRun) -> AgentRunReceipt {
        AgentRunReceipt {
            run_id: run.run_id.clone(),
            outcome: AgentRunOutcome::Completed,
            correctness: AgentCorrectness {
                passed: true,
                rationale: "rubric satisfied".to_string(),
            },
            judge_version: run.controls.judge.version.clone(),
            rubric_fingerprint: run.rubric_fingerprint.clone(),
            raw_session: EvidenceArtifact {
                path: format!("{}/{}.json", run.session_store, run.run_id),
                hash: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
            },
            answer_hash: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                .to_string(),
            tool_trace_hash: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
                .to_string(),
            query_metrics_schema: crate::atlas_eval::QUERY_METRICS_SCHEMA.to_string(),
            stale_as_fresh: false,
            metrics: AgentRunMetrics {
                file_reads: 8,
                grep_calls: 4,
                graph_calls: u64::from(run.arm != AgentArm::Baseline),
                tool_calls: 14,
                round_trips: 10,
                duration_ms: 100,
                response_bytes: 1200,
                context_bytes: 2400,
                cost_usd: Some(0.01),
                read_back_calls: 1,
                follow_up_queries: 1,
                truncated_queries: 0,
            },
            diagnostic: None,
        }
    }

    fn receipt_bundle(plan: &AgentRunPlan) -> AgentReceiptBundle {
        AgentReceiptBundle {
            schema: AGENT_RECEIPTS_SCHEMA.to_string(),
            experiment_version: plan.experiment_version.clone(),
            plan_fingerprint: fingerprint(plan).unwrap(),
            runs: plan.runs.iter().map(receipt_for).collect(),
        }
    }

    fn set_arm_metrics(
        plan: &AgentRunPlan,
        bundle: &mut AgentReceiptBundle,
        arm: AgentArm,
        read_grep: [u64; 3],
        round_trips: [u64; 3],
        tool_calls: [u64; 3],
    ) {
        for (index, run) in plan.runs.iter().filter(|run| run.arm == arm).enumerate() {
            let receipt = bundle
                .runs
                .iter_mut()
                .find(|receipt| receipt.run_id == run.run_id)
                .unwrap();
            receipt.metrics.file_reads = read_grep[index];
            receipt.metrics.grep_calls = 0;
            receipt.metrics.round_trips = round_trips[index];
            receipt.metrics.tool_calls = tool_calls[index];
        }
    }

    fn passing_metric_bundle(plan: &AgentRunPlan) -> AgentReceiptBundle {
        let mut bundle = receipt_bundle(plan);
        set_arm_metrics(
            plan,
            &mut bundle,
            AgentArm::Baseline,
            [30, 40, 50],
            [12, 15, 18],
            [20, 25, 30],
        );
        set_arm_metrics(
            plan,
            &mut bundle,
            AgentArm::AtlasPrimitives,
            [10, 15, 20],
            [5, 7, 8],
            [8, 12, 15],
        );
        set_arm_metrics(
            plan,
            &mut bundle,
            AgentArm::AtlasContext,
            [2, 4, 6],
            [1, 2, 3],
            [2, 4, 6],
        );
        bundle
    }

    #[test]
    fn test_agent_ab_plan_builds_three_symmetric_arms() {
        let plan = compile_agent_plan(&corpus(3), &experiment()).unwrap();

        assert_eq!(plan.schema, AGENT_PLAN_SCHEMA);
        assert_eq!(plan.runs.len(), 9);
        for trial_runs in plan.runs.chunks_exact(3) {
            assert_eq!(
                trial_runs.iter().map(|run| run.arm).collect::<Vec<_>>(),
                vec![
                    AgentArm::Baseline,
                    AgentArm::AtlasPrimitives,
                    AgentArm::AtlasContext,
                ]
            );
            assert!(trial_runs
                .iter()
                .all(|run| run.environment_fingerprint == trial_runs[0].environment_fingerprint));
            assert_eq!(trial_runs[0].case_id, trial_runs[1].case_id);
            assert_eq!(trial_runs[0].trial, trial_runs[2].trial);
        }

        let baseline = plan.runs[0].tools.iter().cloned().collect::<BTreeSet<_>>();
        let primitives = plan.runs[1].tools.iter().cloned().collect::<BTreeSet<_>>();
        let context = plan.runs[2].tools.iter().cloned().collect::<BTreeSet<_>>();
        assert!(baseline.is_subset(&primitives));
        assert_eq!(
            context.difference(&primitives).cloned().collect::<Vec<_>>(),
            vec!["atlas-context".to_string()]
        );
        assert_ne!(
            plan.runs[0].surface_fingerprint,
            plan.runs[1].surface_fingerprint
        );
        assert_ne!(
            plan.runs[1].surface_fingerprint,
            plan.runs[2].surface_fingerprint
        );
    }

    #[test]
    fn test_agent_ab_plan_rejects_asymmetric_surface() {
        let mut missing_baseline_tool = experiment();
        missing_baseline_tool
            .surfaces
            .atlas_primitives
            .retain(|tool| tool != "read");
        let error = compile_agent_plan(&corpus(3), &missing_baseline_tool).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-surface");

        let mut extra_context_tool = experiment();
        extra_context_tool
            .surfaces
            .atlas_context
            .push("atlas-secret".to_string());
        let error = compile_agent_plan(&corpus(3), &extra_context_tool).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-surface");
    }

    #[test]
    fn test_agent_ab_plan_requires_three_trials() {
        let error = compile_agent_plan(&corpus(2), &experiment()).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-trials");
    }

    #[test]
    fn test_agent_ab_gate_requires_exact_planned_runs() {
        let plan = agent_plan();
        let mut missing = receipt_bundle(&plan);
        missing.runs.pop();
        let error = validate_agent_receipts(&plan, &missing).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-completeness");

        let mut duplicate = receipt_bundle(&plan);
        duplicate.runs.push(duplicate.runs[0].clone());
        let error = validate_agent_receipts(&plan, &duplicate).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-completeness");

        let mut unknown = receipt_bundle(&plan);
        unknown.runs[0].run_id =
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        let error = validate_agent_receipts(&plan, &unknown).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-completeness");
    }

    #[test]
    fn test_agent_ab_gate_retains_failed_runs() {
        let plan = agent_plan();
        let mut bundle = receipt_bundle(&plan);
        bundle.runs[1].outcome = AgentRunOutcome::Failed;
        bundle.runs[1].correctness.passed = false;
        bundle.runs[1].correctness.rationale = "agent process failed".to_string();
        bundle.runs[1].diagnostic = Some("driver exited 17".to_string());

        validate_agent_receipts(&plan, &bundle).unwrap();
        assert_eq!(bundle.runs.len(), plan.runs.len());
        assert_eq!(bundle.runs[1].outcome, AgentRunOutcome::Failed);
        assert_eq!(
            bundle.runs[1].diagnostic.as_deref(),
            Some("driver exited 17")
        );
    }

    #[test]
    fn test_agent_ab_gate_rejects_legacy_query_metrics() {
        let plan = agent_plan();
        let bundle = receipt_bundle(&plan);
        let mut value = serde_json::to_value(bundle).unwrap();
        value["runs"][0]
            .as_object_mut()
            .unwrap()
            .remove("query_metrics_schema");
        let bytes = serde_json::to_vec(&value).unwrap();

        let error = parse_agent_receipts(&bytes).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-receipt");
    }

    #[test]
    fn test_agent_ab_gate_validates_session_evidence() {
        let plan = agent_plan();
        let mut invalid_session = receipt_bundle(&plan);
        invalid_session.runs[0].raw_session.path = "/tmp/session.json".to_string();
        let error = validate_agent_receipts(&plan, &invalid_session).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-evidence");

        let mut invalid_judge = receipt_bundle(&plan);
        invalid_judge.runs[0].judge_version.clear();
        let error = validate_agent_receipts(&plan, &invalid_judge).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-evidence");

        let mut invalid_hash = receipt_bundle(&plan);
        invalid_hash.runs[0].tool_trace_hash = "not-a-hash".to_string();
        let error = validate_agent_receipts(&plan, &invalid_hash).unwrap_err();
        assert_eq!(error.code(), "atlas-agent-ab-evidence");
    }

    #[test]
    fn test_agent_ab_gate_blocks_correctness_and_stale_regression() {
        let plan = agent_plan();
        let mut incorrect = passing_metric_bundle(&plan);
        let candidate = plan
            .runs
            .iter()
            .find(|run| run.arm == AgentArm::AtlasPrimitives)
            .unwrap();
        let receipt = incorrect
            .runs
            .iter_mut()
            .find(|receipt| receipt.run_id == candidate.run_id)
            .unwrap();
        receipt.correctness.passed = false;
        receipt.correctness.rationale = "missed required symbol".to_string();
        let gate = gate_agent_receipts(&plan, &incorrect).unwrap();
        let comparison = gate
            .comparisons
            .get(&PromotionCandidate::AtlasPrimitives)
            .unwrap();
        assert_eq!(comparison.state, GateState::Blocked);
        assert!(
            comparison
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "atlas-agent-ab-correctness")
        );

        let mut stale = passing_metric_bundle(&plan);
        let receipt = stale
            .runs
            .iter_mut()
            .find(|receipt| receipt.run_id == candidate.run_id)
            .unwrap();
        receipt.stale_as_fresh = true;
        let gate = gate_agent_receipts(&plan, &stale).unwrap();
        assert_eq!(
            gate.comparisons[&PromotionCandidate::AtlasPrimitives].state,
            GateState::Blocked
        );
        assert!(
            gate.comparisons[&PromotionCandidate::AtlasPrimitives]
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "atlas-agent-ab-stale-as-fresh")
        );
    }

    #[test]
    fn test_agent_ab_gate_derives_benefit_from_baseline_mad() {
        let plan = agent_plan();
        let gate = gate_agent_receipts(&plan, &passing_metric_bundle(&plan)).unwrap();
        let comparison = &gate.comparisons[&PromotionCandidate::AtlasPrimitives];

        assert_eq!(comparison.state, GateState::Passed);
        assert_eq!(comparison.cases.len(), 1);
        assert_eq!(comparison.cases[0].state, CaseDecision::Improved);
        assert_eq!(comparison.cases[0].read_grep.reference.median, 40.0);
        assert_eq!(comparison.cases[0].read_grep.reference.mad, 10.0);
        assert_eq!(comparison.cases[0].read_grep.candidate.median, 15.0);
        assert_eq!(
            comparison.cases[0].read_grep.decision,
            MetricDecision::Improved
        );
    }

    #[test]
    fn test_agent_ab_gate_keeps_small_tie_zone_visible() {
        let mut small_corpus = corpus(3);
        small_corpus.cases[0].size = WorkspaceSize::Small;
        let plan = compile_agent_plan(&small_corpus, &experiment()).unwrap();
        let mut tie = passing_metric_bundle(&plan);
        set_arm_metrics(
            &plan,
            &mut tie,
            AgentArm::AtlasPrimitives,
            [40, 45, 50],
            [15, 16, 17],
            [25, 27, 29],
        );
        let gate = gate_agent_receipts(&plan, &tie).unwrap();
        assert_eq!(
            gate.comparisons[&PromotionCandidate::AtlasPrimitives].cases[0].state,
            CaseDecision::Tie
        );
        assert_eq!(
            gate.comparisons[&PromotionCandidate::AtlasPrimitives].state,
            GateState::Passed
        );

        set_arm_metrics(
            &plan,
            &mut tie,
            AgentArm::AtlasPrimitives,
            [60, 65, 70],
            [25, 26, 27],
            [40, 42, 44],
        );
        let gate = gate_agent_receipts(&plan, &tie).unwrap();
        assert_eq!(
            gate.comparisons[&PromotionCandidate::AtlasPrimitives].cases[0].state,
            CaseDecision::Blocked
        );
    }

    #[test]
    fn test_agent_ab_gate_scopes_surface_promotions() {
        let plan = agent_plan();
        let mut bundle = passing_metric_bundle(&plan);
        set_arm_metrics(
            &plan,
            &mut bundle,
            AgentArm::AtlasContext,
            [20, 25, 30],
            [9, 10, 11],
            [16, 18, 20],
        );
        let gate = gate_agent_receipts(&plan, &bundle).unwrap();

        assert_eq!(
            gate.comparisons[&PromotionCandidate::AtlasPrimitives].state,
            GateState::Passed
        );
        assert_eq!(
            gate.comparisons[&PromotionCandidate::AtlasContext].state,
            GateState::Blocked
        );
    }
}
