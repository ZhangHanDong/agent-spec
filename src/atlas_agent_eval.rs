use crate::atlas_eval::{CacheCondition, Corpus, Permissions, TaskClass, WorkspaceSize};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub const AGENT_EXPERIMENT_SCHEMA: &str = "agent-spec/atlas-eval/agent-experiment-v1";
pub const AGENT_PLAN_SCHEMA: &str = "agent-spec/atlas-eval/agent-plan-v1";

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
}
