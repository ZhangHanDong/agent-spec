use crate::atlas_eval::{CacheCondition, Corpus, Permissions, TaskClass, WorkspaceSize};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub const AGENT_EXPERIMENT_SCHEMA: &str = "agent-spec/atlas-eval/agent-experiment-v1";
pub const AGENT_PLAN_SCHEMA: &str = "agent-spec/atlas-eval/agent-plan-v1";
pub const AGENT_RECEIPTS_SCHEMA: &str = "agent-spec/atlas-eval/agent-receipts-v1";

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
}
