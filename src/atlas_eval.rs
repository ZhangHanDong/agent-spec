use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub const CORPUS_SCHEMA: &str = "agent-spec/atlas-eval/corpus-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corpus {
    pub schema: String,
    pub model: String,
    pub prompt: String,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    pub size: WorkspaceSize,
    pub task_class: TaskClass,
    pub repository: String,
    pub revision: String,
    pub trials_per_arm: u32,
    pub rubric: Vec<String>,
    pub permissions: Permissions,
    pub cache_condition: CacheCondition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskClass {
    Symbol,
    Flow,
    Impact,
    Implementation,
    Stale,
    ScipUnavailable,
    CompileFailing,
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permissions {
    ReadOnly,
    WorkspaceWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheCondition {
    Cold,
    Warm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arm {
    Atlas,
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunPlan {
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Run {
    pub case_id: String,
    pub arm: Arm,
    pub trial: u32,
    pub model: String,
    pub prompt: String,
    pub repository: String,
    pub revision: String,
    pub permissions: Permissions,
    pub cache_condition: CacheCondition,
}

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct EvalError {
    code: &'static str,
    message: String,
}

impl EvalError {
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

pub fn load_corpus(path: &Path) -> Result<Corpus, EvalError> {
    let bytes = std::fs::read(path).map_err(|error| {
        EvalError::new(
            "atlas-eval-load",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    let corpus = serde_json::from_slice(&bytes).map_err(|error| {
        EvalError::new(
            "atlas-eval-load",
            format!("failed to parse {}: {error}", path.display()),
        )
    })?;
    validate_corpus(&corpus)?;
    Ok(corpus)
}

pub fn compile_plan(corpus: &Corpus) -> Result<RunPlan, EvalError> {
    validate_corpus(corpus)?;

    let mut runs = Vec::new();
    for case in &corpus.cases {
        for arm in [Arm::Atlas, Arm::Baseline] {
            for trial in 1..=case.trials_per_arm {
                runs.push(Run {
                    case_id: case.id.clone(),
                    arm,
                    trial,
                    model: corpus.model.clone(),
                    prompt: corpus.prompt.clone(),
                    repository: case.repository.clone(),
                    revision: case.revision.clone(),
                    permissions: case.permissions,
                    cache_condition: case.cache_condition,
                });
            }
        }
    }
    Ok(RunPlan { runs })
}

fn validate_corpus(corpus: &Corpus) -> Result<(), EvalError> {
    if corpus.schema != CORPUS_SCHEMA {
        return Err(EvalError::new(
            "atlas-eval-schema",
            format!("expected schema {CORPUS_SCHEMA}, found {}", corpus.schema),
        ));
    }

    let mut ids = BTreeSet::new();
    for case in &corpus.cases {
        if case.id.trim().is_empty() {
            return Err(EvalError::new(
                "atlas-eval-case-id",
                "case id must not be empty",
            ));
        }
        if !ids.insert(case.id.as_str()) {
            return Err(EvalError::new(
                "atlas-eval-duplicate-case",
                format!("duplicate case id {}", case.id),
            ));
        }
        if case.revision.trim().is_empty() {
            return Err(EvalError::new(
                "atlas-eval-revision",
                format!("case {} has an empty revision", case.id),
            ));
        }
        if case.rubric.is_empty() || case.rubric.iter().any(|item| item.trim().is_empty()) {
            return Err(EvalError::new(
                "atlas-eval-rubric",
                format!("case {} has an empty rubric", case.id),
            ));
        }
        if case.trials_per_arm < 3 {
            return Err(EvalError::new(
                "atlas-eval-trials",
                format!("case {} must have at least three trials per arm", case.id),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn valid_corpus(trials_per_arm: u32) -> Corpus {
        Corpus {
            schema: "agent-spec/atlas-eval/corpus-v1".to_string(),
            model: "offline-fixture-model".to_string(),
            prompt: "Plan the requested change.".to_string(),
            cases: vec![Case {
                id: "workspace-navigation".to_string(),
                size: WorkspaceSize::Small,
                task_class: TaskClass::Symbol,
                repository: "fixtures/atlas/basic".to_string(),
                revision: "a2d4282".to_string(),
                trials_per_arm,
                rubric: vec!["returns the requested symbol".to_string()],
                permissions: Permissions::ReadOnly,
                cache_condition: CacheCondition::Cold,
            }],
        }
    }

    #[test]
    fn test_atlas_eval_plan_pairs_arms_and_trials() {
        let corpus = valid_corpus(3);
        let plan = compile_plan(&corpus).expect("valid plan");
        assert_eq!(plan.runs.len(), corpus.cases.len() * 2 * 3);
        assert!(plan.runs.chunks_exact(6).all(|runs| {
            runs.iter().filter(|run| run.arm == Arm::Atlas).count() == 3
                && runs.iter().filter(|run| run.arm == Arm::Baseline).count() == 3
        }));
    }

    #[test]
    fn test_atlas_eval_rejects_duplicate_case_ids() {
        let mut corpus = valid_corpus(3);
        corpus.cases.push(corpus.cases[0].clone());
        assert_eq!(
            compile_plan(&corpus).unwrap_err().code(),
            "atlas-eval-duplicate-case"
        );
    }

    #[test]
    fn test_atlas_eval_rejects_too_few_trials() {
        let corpus = valid_corpus(2);
        assert_eq!(
            compile_plan(&corpus).unwrap_err().code(),
            "atlas-eval-trials"
        );
    }

    #[test]
    fn test_atlas_eval_rejects_wrong_schema() {
        let mut corpus = valid_corpus(3);
        corpus.schema = "agent-spec/atlas-eval/corpus-v0".to_string();
        assert_eq!(
            compile_plan(&corpus).unwrap_err().code(),
            "atlas-eval-schema"
        );
    }

    #[test]
    fn test_atlas_eval_rejects_empty_case_id() {
        let mut corpus = valid_corpus(3);
        corpus.cases[0].id.clear();
        assert_eq!(
            compile_plan(&corpus).unwrap_err().code(),
            "atlas-eval-case-id"
        );
    }

    #[test]
    fn test_atlas_eval_rejects_empty_revision() {
        let mut corpus = valid_corpus(3);
        corpus.cases[0].revision.clear();
        assert_eq!(
            compile_plan(&corpus).unwrap_err().code(),
            "atlas-eval-revision"
        );
    }

    #[test]
    fn test_atlas_eval_rejects_empty_rubric() {
        let mut corpus = valid_corpus(3);
        corpus.cases[0].rubric.clear();
        assert_eq!(
            compile_plan(&corpus).unwrap_err().code(),
            "atlas-eval-rubric"
        );
    }

    #[test]
    fn test_atlas_eval_loads_typed_corpus() {
        let path = std::env::temp_dir().join(format!(
            "atlas-eval-corpus-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let expected = valid_corpus(3);
        std::fs::write(&path, serde_json::to_vec(&expected).unwrap()).unwrap();

        let actual = load_corpus(&path).expect("typed corpus loads");

        assert_eq!(actual.cases[0].id, expected.cases[0].id);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_atlas_eval_checked_in_corpus_covers_the_full_task_matrix() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/atlas/corpus.json");
        let corpus = load_corpus(&path).expect("checked-in corpus loads");

        for task_class in [
            TaskClass::Symbol,
            TaskClass::Flow,
            TaskClass::Impact,
            TaskClass::Implementation,
            TaskClass::Stale,
            TaskClass::ScipUnavailable,
            TaskClass::CompileFailing,
            TaskClass::Worktree,
        ] {
            assert!(
                corpus
                    .cases
                    .iter()
                    .any(|case| case.task_class == task_class),
                "missing task class {task_class:?}"
            );
        }
        for size in [
            WorkspaceSize::Small,
            WorkspaceSize::Medium,
            WorkspaceSize::Large,
        ] {
            assert!(
                corpus.cases.iter().any(|case| case.size == size),
                "missing workspace size {size:?}"
            );
        }
        for case in &corpus.cases {
            let case_text = format!(
                "{} {} {}",
                case.repository,
                case.revision,
                case.rubric.join(" ")
            )
            .to_ascii_lowercase();
            assert!(!case_text.contains("pending"), "placeholder in {}", case.id);
            assert!(!case_text.contains("todo"), "placeholder in {}", case.id);
        }

        let plan = compile_plan(&corpus).expect("checked-in corpus compiles");
        for run in &plan.runs {
            let case = corpus
                .cases
                .iter()
                .find(|case| case.id == run.case_id)
                .expect("run references a corpus case");
            assert_eq!(run.permissions, case.permissions);
            assert_eq!(run.cache_condition, case.cache_condition);
        }
    }
}
