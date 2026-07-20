use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CORPUS_SCHEMA: &str = "agent-spec/atlas-eval/corpus-v1";
pub const RUN_PLAN_SCHEMA: &str = "agent-spec/atlas-eval/run-plan-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub schema: String,
    pub model: String,
    pub prompt: String,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arm {
    Atlas,
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunPlan {
    pub schema: String,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// The rubric verdict recorded for one benchmark run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Correctness {
    pub passed: bool,
}

/// Measurements and correctness evidence produced by one benchmark run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunReceipt {
    pub case_id: String,
    pub arm: Arm,
    pub trial: u32,
    pub correctness: Option<Correctness>,
    pub file_reads: u64,
    pub graph_calls: u64,
    pub tool_calls: u64,
    pub duration_ms: u64,
    pub context_bytes: u64,
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub response_bytes: u64,
    #[serde(default)]
    pub read_back_calls: u64,
    #[serde(default)]
    pub follow_up_queries: u64,
    #[serde(default)]
    pub truncated_queries: u64,
}

/// Robust distribution statistics for one metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricSummary {
    pub samples: usize,
    pub median: f64,
    pub mad: f64,
}

/// Summary of all numeric measurements in a receipt set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsSummary {
    pub file_reads: MetricSummary,
    pub graph_calls: MetricSummary,
    pub tool_calls: MetricSummary,
    pub duration_ms: MetricSummary,
    pub context_bytes: MetricSummary,
    pub cost_usd: Option<MetricSummary>,
    pub response_bytes: MetricSummary,
    pub read_back_calls: MetricSummary,
    pub follow_up_queries: MetricSummary,
    pub truncated_queries: MetricSummary,
}

/// Count of correctness verdicts represented in a summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectnessSummary {
    pub passed: usize,
    pub failed: usize,
}

/// Per-arm evaluation summary, retained so baseline and Atlas remain comparable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmSummary {
    pub receipts: usize,
    pub correctness: CorrectnessSummary,
    pub metrics: MetricsSummary,
}

/// Aggregate evaluation results for fully graded receipts only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalSummary {
    pub receipts: usize,
    pub correctness: CorrectnessSummary,
    pub metrics: MetricsSummary,
    pub arms: BTreeMap<Arm, ArmSummary>,
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
            "atlas-eval-corpus",
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
    Ok(RunPlan {
        schema: RUN_PLAN_SCHEMA.to_string(),
        runs,
    })
}

/// Loads either a JSON array or newline-delimited JSON receipts.
pub fn load_receipts(path: &Path) -> Result<Vec<RunReceipt>, EvalError> {
    let bytes = std::fs::read(path).map_err(|error| {
        EvalError::new(
            "atlas-eval-load",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        EvalError::new(
            "atlas-eval-receipt",
            format!("receipts {} are not UTF-8: {error}", path.display()),
        )
    })?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(EvalError::new(
            "atlas-eval-receipt",
            "receipt input is empty",
        ));
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str(trimmed).map_err(|error| {
            EvalError::new(
                "atlas-eval-receipt",
                format!("failed to parse receipt array {}: {error}", path.display()),
            )
        });
    }

    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|error| {
                EvalError::new(
                    "atlas-eval-receipt",
                    format!(
                        "failed to parse receipt line {} in {}: {error}",
                        index + 1,
                        path.display()
                    ),
                )
            })
        })
        .collect()
}

/// Summarizes graded receipts with median and median absolute deviation.
pub fn summarize(receipts: &[RunReceipt]) -> Result<EvalSummary, EvalError> {
    if receipts.is_empty() {
        return Err(EvalError::new(
            "atlas-eval-receipt",
            "receipt input is empty",
        ));
    }

    // Correctness is a hard precondition: never publish a performance aggregate
    // whose underlying runs have not all been graded.
    for receipt in receipts {
        if receipt.correctness.is_none() {
            return Err(EvalError::new(
                "atlas-eval-receipt",
                format!(
                    "receipt {} {} trial {} is missing correctness",
                    receipt.case_id,
                    match receipt.arm {
                        Arm::Atlas => "atlas",
                        Arm::Baseline => "baseline",
                    },
                    receipt.trial
                ),
            ));
        }
        if receipt.case_id.trim().is_empty() || receipt.trial == 0 {
            return Err(EvalError::new(
                "atlas-eval-receipt",
                "receipt case_id must not be empty and trial must be positive",
            ));
        }
        if receipt
            .cost_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        {
            return Err(EvalError::new(
                "atlas-eval-receipt",
                "receipt cost_usd must be finite and non-negative",
            ));
        }
    }

    let mut arms = BTreeMap::new();
    for arm in [Arm::Atlas, Arm::Baseline] {
        let arm_receipts: Vec<_> = receipts
            .iter()
            .filter(|receipt| receipt.arm == arm)
            .cloned()
            .collect();
        if !arm_receipts.is_empty() {
            arms.insert(arm, summarize_arm(&arm_receipts));
        }
    }

    Ok(EvalSummary {
        receipts: receipts.len(),
        correctness: summarize_correctness(receipts),
        metrics: summarize_metrics(receipts),
        arms,
    })
}

/// Atomically writes JSON without exposing a partial target file.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), EvalError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        EvalError::new(
            "atlas-eval-output",
            format!("JSON serialization failed: {error}"),
        )
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            EvalError::new(
                "atlas-eval-output",
                format!("output path has no file name: {}", path.display()),
            )
        })?;
    let temp = temporary_path(parent, name);
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| {
                EvalError::new(
                    "atlas-eval-output",
                    format!("failed to create {}: {error}", temp.display()),
                )
            })?;
        file.write_all(&bytes).map_err(|error| {
            EvalError::new(
                "atlas-eval-output",
                format!("failed to write {}: {error}", temp.display()),
            )
        })?;
        file.write_all(b"\n").map_err(|error| {
            EvalError::new(
                "atlas-eval-output",
                format!("failed to write {}: {error}", temp.display()),
            )
        })?;
        file.sync_all().map_err(|error| {
            EvalError::new(
                "atlas-eval-output",
                format!("failed to sync {}: {error}", temp.display()),
            )
        })?;
        std::fs::rename(&temp, path).map_err(|error| {
            EvalError::new(
                "atlas-eval-output",
                format!("failed to replace {}: {error}", path.display()),
            )
        })
    })();
    if result.is_err() {
        std::fs::remove_file(&temp).ok();
    }
    result
}

fn temporary_path(parent: &Path, name: &str) -> PathBuf {
    parent.join(format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

fn summarize_arm(receipts: &[RunReceipt]) -> ArmSummary {
    ArmSummary {
        receipts: receipts.len(),
        correctness: summarize_correctness(receipts),
        metrics: summarize_metrics(receipts),
    }
}

fn summarize_correctness(receipts: &[RunReceipt]) -> CorrectnessSummary {
    let passed = receipts
        .iter()
        .filter(|receipt| {
            receipt
                .correctness
                .is_some_and(|correctness| correctness.passed)
        })
        .count();
    CorrectnessSummary {
        passed,
        failed: receipts.len() - passed,
    }
}

fn summarize_metrics(receipts: &[RunReceipt]) -> MetricsSummary {
    MetricsSummary {
        file_reads: metric(receipts.iter().map(|receipt| receipt.file_reads as f64)),
        graph_calls: metric(receipts.iter().map(|receipt| receipt.graph_calls as f64)),
        tool_calls: metric(receipts.iter().map(|receipt| receipt.tool_calls as f64)),
        duration_ms: metric(receipts.iter().map(|receipt| receipt.duration_ms as f64)),
        context_bytes: metric(receipts.iter().map(|receipt| receipt.context_bytes as f64)),
        cost_usd: optional_metric(receipts.iter().filter_map(|receipt| receipt.cost_usd)),
        response_bytes: metric(receipts.iter().map(|receipt| receipt.response_bytes as f64)),
        read_back_calls: metric(
            receipts
                .iter()
                .map(|receipt| receipt.read_back_calls as f64),
        ),
        follow_up_queries: metric(
            receipts
                .iter()
                .map(|receipt| receipt.follow_up_queries as f64),
        ),
        truncated_queries: metric(
            receipts
                .iter()
                .map(|receipt| receipt.truncated_queries as f64),
        ),
    }
}

fn metric(values: impl Iterator<Item = f64>) -> MetricSummary {
    let mut values: Vec<_> = values.collect();
    values.sort_by(f64::total_cmp);
    let median = median(&values);
    let deviations = values.iter().map(|value| (value - median).abs());
    MetricSummary {
        samples: values.len(),
        median,
        mad: median_of(deviations),
    }
}

fn optional_metric(values: impl Iterator<Item = f64>) -> Option<MetricSummary> {
    let values: Vec<_> = values.collect();
    (!values.is_empty()).then(|| metric(values.into_iter()))
}

fn median_of(values: impl Iterator<Item = f64>) -> f64 {
    let mut values: Vec<_> = values.collect();
    values.sort_by(f64::total_cmp);
    median(&values)
}

fn median(values: &[f64]) -> f64 {
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
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

    const LEGACY_RECEIPT_JSON: &str = r#"{
        "case_id": "workspace-navigation",
        "arm": "baseline",
        "trial": 1,
        "correctness": { "passed": true },
        "file_reads": 3,
        "graph_calls": 2,
        "tool_calls": 5,
        "duration_ms": 120,
        "context_bytes": 1024,
        "cost_usd": 0.12
    }"#;

    fn receipt(arm: Arm, trial: u32) -> RunReceipt {
        RunReceipt {
            case_id: "workspace-navigation".to_string(),
            arm,
            trial,
            correctness: Some(Correctness { passed: true }),
            file_reads: 3,
            graph_calls: 2,
            tool_calls: 5,
            duration_ms: 120,
            context_bytes: 1024,
            cost_usd: Some(0.12),
            response_bytes: 0,
            read_back_calls: 0,
            follow_up_queries: 0,
            truncated_queries: 0,
        }
    }

    fn receipt_with_correctness(correctness: Option<Correctness>) -> RunReceipt {
        RunReceipt {
            correctness,
            ..receipt(Arm::Atlas, 1)
        }
    }

    trait ReceiptTestExt {
        fn with_query_metrics(
            self,
            response_bytes: u64,
            read_back_calls: u64,
            follow_up_queries: u64,
            truncated_queries: u64,
        ) -> Self;
    }

    impl ReceiptTestExt for RunReceipt {
        fn with_query_metrics(
            mut self,
            response_bytes: u64,
            read_back_calls: u64,
            follow_up_queries: u64,
            truncated_queries: u64,
        ) -> Self {
            self.response_bytes = response_bytes;
            self.read_back_calls = read_back_calls;
            self.follow_up_queries = follow_up_queries;
            self.truncated_queries = truncated_queries;
            self
        }
    }

    fn assert_metric(metric: &MetricSummary, median: f64, mad: f64, samples: usize) {
        assert_eq!(metric.median, median);
        assert_eq!(metric.mad, mad);
        assert_eq!(metric.samples, samples);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

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
        let mut corpus = valid_corpus(3);
        corpus.model = "pinned-model".to_string();
        corpus.prompt = "pinned prompt".to_string();
        let mut second = corpus.cases[0].clone();
        second.id = "impact-analysis".to_string();
        second.repository = "fixtures/atlas/medium".to_string();
        second.revision = "b86d85f".to_string();
        second.permissions = Permissions::WorkspaceWrite;
        second.cache_condition = CacheCondition::Warm;
        corpus.cases.push(second);

        let plan = compile_plan(&corpus).expect("valid plan");
        assert_eq!(plan.schema, RUN_PLAN_SCHEMA);
        assert_eq!(plan.runs.len(), corpus.cases.len() * 2 * 3);
        let expected_trials = [
            (Arm::Atlas, 1),
            (Arm::Atlas, 2),
            (Arm::Atlas, 3),
            (Arm::Baseline, 1),
            (Arm::Baseline, 2),
            (Arm::Baseline, 3),
        ];
        for (case, runs) in corpus.cases.iter().zip(plan.runs.chunks_exact(6)) {
            for (run, (arm, trial)) in runs.iter().zip(expected_trials) {
                assert_eq!(run.case_id, case.id);
                assert_eq!(run.arm, arm);
                assert_eq!(run.trial, trial);
                assert_eq!(run.model, corpus.model);
                assert_eq!(run.prompt, corpus.prompt);
                assert_eq!(run.repository, case.repository);
                assert_eq!(run.revision, case.revision);
                assert_eq!(run.permissions, case.permissions);
                assert_eq!(run.cache_condition, case.cache_condition);
            }
        }

        let second_plan = compile_plan(&corpus).expect("same corpus compiles twice");
        assert_eq!(
            serde_json::to_vec(&plan).unwrap(),
            serde_json::to_vec(&second_plan).unwrap()
        );
    }

    #[test]
    fn test_atlas_eval_rejects_unknown_top_level_field() {
        let path = std::env::temp_dir().join(format!(
            "atlas-eval-unknown-top-level-{}.json",
            std::process::id()
        ));
        let mut value = serde_json::to_value(valid_corpus(3)).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let result = load_corpus(&path);
        std::fs::remove_file(path).unwrap();

        assert_eq!(result.unwrap_err().code(), "atlas-eval-corpus");
    }

    #[test]
    fn test_atlas_eval_rejects_unknown_case_field() {
        let path = std::env::temp_dir().join(format!(
            "atlas-eval-unknown-case-{}.json",
            std::process::id()
        ));
        let mut value = serde_json::to_value(valid_corpus(3)).unwrap();
        value["cases"][0]
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let result = load_corpus(&path);
        std::fs::remove_file(path).unwrap();

        assert_eq!(result.unwrap_err().code(), "atlas-eval-corpus");
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

    #[test]
    fn test_atlas_eval_summary_rejects_missing_correctness() {
        let receipts = vec![receipt_with_correctness(None)];
        assert_eq!(
            summarize(&receipts).unwrap_err().code(),
            "atlas-eval-receipt"
        );
    }

    #[test]
    fn test_atlas_eval_plan_writes_atomic_output() {
        let dir = temp_dir("atlas-eval-out");
        let out = dir.join("plan.json");
        write_json_atomic(&out, &compile_plan(&valid_corpus(3)).unwrap()).unwrap();
        let parsed: RunPlan =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert!(!parsed.runs.is_empty());
        assert!(!dir.join("plan.json.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_atlas_eval_summary_uses_median_and_mad() {
        let mut first = receipt_with_correctness(Some(Correctness { passed: true }));
        first.file_reads = 1;
        first.duration_ms = 10;
        first.cost_usd = Some(0.10);

        let mut second = receipt_with_correctness(Some(Correctness { passed: false }));
        second.file_reads = 2;
        second.duration_ms = 20;
        second.cost_usd = Some(0.20);

        let mut third = receipt_with_correctness(Some(Correctness { passed: true }));
        third.file_reads = 100;
        third.duration_ms = 1_000;
        third.cost_usd = Some(10.0);

        let summary = summarize(&[first, second, third]).unwrap();
        assert_eq!(summary.correctness.passed, 2);
        assert_eq!(summary.correctness.failed, 1);
        assert_eq!(summary.metrics.file_reads.median, 2.0);
        assert_eq!(summary.metrics.file_reads.mad, 1.0);
        assert_eq!(summary.metrics.duration_ms.median, 20.0);
        assert_eq!(summary.metrics.duration_ms.mad, 10.0);
        assert_eq!(summary.metrics.cost_usd.unwrap().median, 0.20);
    }

    #[test]
    fn test_atlas_eval_receipts_measure_explore_readback_and_response_bytes() {
        let receipts = vec![
            receipt(Arm::Atlas, 1).with_query_metrics(12_000, 0, 1, 0),
            receipt(Arm::Atlas, 2).with_query_metrics(16_000, 1, 2, 1),
            receipt(Arm::Baseline, 1),
        ];
        let summary = summarize(&receipts).unwrap();
        assert_metric(&summary.metrics.response_bytes, 12_000.0, 4_000.0, 3);
        assert_metric(&summary.metrics.read_back_calls, 0.0, 0.0, 3);
        assert_metric(&summary.metrics.follow_up_queries, 1.0, 1.0, 3);
        assert_metric(&summary.metrics.truncated_queries, 0.0, 0.0, 3);

        let atlas = &summary.arms[&Arm::Atlas].metrics;
        assert_metric(&atlas.response_bytes, 14_000.0, 2_000.0, 2);
        assert_metric(&atlas.read_back_calls, 0.5, 0.5, 2);
        assert_metric(&atlas.follow_up_queries, 1.5, 0.5, 2);
        assert_metric(&atlas.truncated_queries, 0.5, 0.5, 2);

        let baseline = &summary.arms[&Arm::Baseline].metrics;
        for metric in [
            &baseline.response_bytes,
            &baseline.read_back_calls,
            &baseline.follow_up_queries,
            &baseline.truncated_queries,
        ] {
            assert_metric(metric, 0.0, 0.0, 1);
        }

        let legacy: RunReceipt = serde_json::from_str(LEGACY_RECEIPT_JSON).unwrap();
        assert_eq!(
            (
                legacy.response_bytes,
                legacy.read_back_calls,
                legacy.follow_up_queries,
                legacy.truncated_queries,
            ),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn test_atlas_eval_load_receipts_accepts_ndjson_and_rejects_unknown_fields() {
        let dir = temp_dir("atlas-eval-receipts");
        let receipts = dir.join("receipts.ndjson");
        let valid = receipt_with_correctness(Some(Correctness { passed: true }));
        let mut invalid = serde_json::to_value(&valid).unwrap();
        invalid
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        std::fs::write(
            &receipts,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&valid).unwrap(),
                serde_json::to_string(&invalid).unwrap()
            ),
        )
        .unwrap();

        assert_eq!(
            load_receipts(&receipts).unwrap_err().code(),
            "atlas-eval-receipt"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_eval_opt_in_runner_requires_command() {
        let dir = temp_dir("atlas-eval-runner");
        let child_receipt = dir.join("child-receipt.json");
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/atlas-eval/run-opt-in.sh");

        let output = std::process::Command::new(script)
            .env_remove("ATLAS_EVAL_AGENT_COMMAND")
            .env("ATLAS_EVAL_CHILD_RECEIPT", &child_receipt)
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "atlas-eval-agent-command: set ATLAS_EVAL_AGENT_COMMAND explicitly\n"
        );
        assert!(!child_receipt.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_eval_opt_in_runner_rejects_malformed_and_empty_plans() {
        use std::os::unix::fs::PermissionsExt;

        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/atlas-eval/run-opt-in.sh");

        for (name, plan_json) in [
            (
                "malformed",
                "{\"schema\":\"agent-spec/atlas-eval/run-plan-v1\",\"runs\":[",
            ),
            (
                "empty",
                "{\"schema\":\"agent-spec/atlas-eval/run-plan-v1\",\"runs\":[]}",
            ),
        ] {
            let dir = temp_dir(&format!("atlas-eval-runner-{name}"));
            let plan = dir.join("plan.json");
            let receipts = dir.join("receipts.ndjson");
            let agent = dir.join("fake-agent.sh");
            let started = dir.join("agent-started");
            std::fs::write(&plan, plan_json).unwrap();
            std::fs::write(
                &agent,
                "#!/usr/bin/env bash\nprintf started >\"$ATLAS_EVAL_STARTED\"\n",
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&agent).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&agent, permissions).unwrap();

            let output = std::process::Command::new(&script)
                .arg(&plan)
                .arg(&receipts)
                .env("ATLAS_EVAL_AGENT_COMMAND", &agent)
                .env("ATLAS_EVAL_STARTED", &started)
                .output()
                .unwrap();

            assert!(!output.status.success(), "{name} plan was accepted");
            assert!(output.stdout.is_empty());
            assert!(!started.exists(), "{name} plan started the agent");
            assert!(!receipts.exists(), "{name} plan produced receipts");
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_eval_opt_in_runner_rejects_invalid_run_shapes() {
        use std::os::unix::fs::PermissionsExt;

        if !std::process::Command::new("jq")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return;
        }

        let dir = temp_dir("atlas-eval-runner-invalid-runs");
        let agent = dir.join("fake-agent.sh");
        let started = dir.join("agent-started");
        std::fs::write(
            &agent,
            "#!/usr/bin/env bash\nprintf started >\"$ATLAS_EVAL_STARTED\"\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&agent).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&agent, permissions).unwrap();

        let valid = serde_json::to_value(compile_plan(&valid_corpus(3)).unwrap()).unwrap();
        let mut null_run = valid.clone();
        null_run["runs"][0] = serde_json::Value::Null;
        let mut unknown_field = valid.clone();
        unknown_field["runs"][0]
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        let mut missing_field = valid.clone();
        missing_field["runs"][0]
            .as_object_mut()
            .unwrap()
            .remove("prompt");
        let mut empty_string = valid.clone();
        empty_string["runs"][0]["model"] = serde_json::json!("");
        let mut wrong_arm = valid.clone();
        wrong_arm["runs"][0]["arm"] = serde_json::json!("control");
        let mut wrong_permissions = valid.clone();
        wrong_permissions["runs"][0]["permissions"] = serde_json::json!("admin");
        let mut wrong_cache = valid.clone();
        wrong_cache["runs"][0]["cache_condition"] = serde_json::json!("unknown");
        let mut wrong_type = valid.clone();
        wrong_type["runs"][0]["case_id"] = serde_json::json!(7);
        let mut fractional_trial = valid.clone();
        fractional_trial["runs"][0]["trial"] = serde_json::json!(1.5);
        let mut zero_trial = valid;
        zero_trial["runs"][0]["trial"] = serde_json::json!(0);

        let cases = [
            ("null-run", null_run),
            ("unknown-field", unknown_field),
            ("missing-field", missing_field),
            ("empty-string", empty_string),
            ("wrong-arm", wrong_arm),
            ("wrong-permissions", wrong_permissions),
            ("wrong-cache", wrong_cache),
            ("wrong-type", wrong_type),
            ("fractional-trial", fractional_trial),
            ("zero-trial", zero_trial),
        ];
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/atlas-eval/run-opt-in.sh");

        for (name, value) in cases {
            let plan = dir.join(format!("{name}.json"));
            let receipts = dir.join(format!("{name}.ndjson"));
            std::fs::write(&plan, serde_json::to_vec(&value).unwrap()).unwrap();
            std::fs::remove_file(&started).ok();

            let output = std::process::Command::new(&script)
                .arg(&plan)
                .arg(&receipts)
                .env("ATLAS_EVAL_AGENT_COMMAND", &agent)
                .env("ATLAS_EVAL_STARTED", &started)
                .output()
                .unwrap();

            assert!(!output.status.success(), "{name} run was accepted");
            assert!(output.stdout.is_empty());
            assert!(!started.exists(), "{name} run started the agent");
            assert!(!receipts.exists(), "{name} run produced receipts");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_eval_opt_in_runner_rejects_shell_builtin_command() {
        let dir = temp_dir("atlas-eval-runner-builtin");
        let plan = dir.join("plan.json");
        let receipts = dir.join("receipts.ndjson");
        write_json_atomic(&plan, &compile_plan(&valid_corpus(3)).unwrap()).unwrap();

        let output = std::process::Command::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("scripts/atlas-eval/run-opt-in.sh"),
        )
        .arg(&plan)
        .arg(&receipts)
        .env("ATLAS_EVAL_AGENT_COMMAND", "echo")
        .output()
        .unwrap();

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!receipts.exists(), "shell builtin was launched");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_eval_opt_in_runner_reports_missing_jq() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir("atlas-eval-runner-no-jq");
        let plan = dir.join("plan.json");
        let receipts = dir.join("receipts.ndjson");
        let agent = dir.join("fake-agent.sh");
        write_json_atomic(&plan, &compile_plan(&valid_corpus(3)).unwrap()).unwrap();
        std::fs::write(&agent, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = std::fs::metadata(&agent).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&agent, permissions).unwrap();

        let output = std::process::Command::new("/bin/bash")
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("scripts/atlas-eval/run-opt-in.sh"),
            )
            .arg(&plan)
            .arg(&receipts)
            .env("ATLAS_EVAL_AGENT_COMMAND", &agent)
            .env("PATH", &dir)
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "atlas-eval-jq: jq is required by the opt-in runner\n"
        );
        assert!(!receipts.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_eval_opt_in_runner_passes_literal_argv_without_eval() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir("atlas-eval-runner-argv");
        let plan = dir.join("plan.json");
        let receipts = dir.join("receipts.ndjson");
        let agent = dir.join("fake-agent.sh");
        let captured_plan = dir.join("captured-plan.txt");
        let captured_arg = dir.join("captured-arg.txt");
        let injected_marker = dir.join("injected-marker");
        write_json_atomic(&plan, &compile_plan(&valid_corpus(3)).unwrap()).unwrap();
        std::fs::write(
            &agent,
            r#"#!/usr/bin/env bash
set -euo pipefail
printf '%s' "$1" >"$ATLAS_EVAL_PLAN_CAPTURE"
printf '%s' "$2" >"$ATLAS_EVAL_ARG_CAPTURE"
printf '%s\n' '{"case_id":"workspace-navigation","arm":"atlas","trial":1,"correctness":{"passed":true},"file_reads":1,"graph_calls":1,"tool_calls":2,"duration_ms":10,"context_bytes":100,"cost_usd":null,"response_bytes":100,"read_back_calls":0,"follow_up_queries":1,"truncated_queries":0}'
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&agent).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&agent, permissions).unwrap();
        let literal_arg = format!("literal; touch {}", injected_marker.display());

        let output = std::process::Command::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("scripts/atlas-eval/run-opt-in.sh"),
        )
        .arg(&plan)
        .arg(&receipts)
        .arg("--")
        .arg(&literal_arg)
        .env("ATLAS_EVAL_AGENT_COMMAND", &agent)
        .env("ATLAS_EVAL_PLAN_CAPTURE", &captured_plan)
        .env("ATLAS_EVAL_ARG_CAPTURE", &captured_arg)
        .output()
        .unwrap();

        assert!(
            output.status.success(),
            "runner failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert_eq!(
            std::fs::read_to_string(captured_plan).unwrap(),
            plan.to_str().unwrap()
        );
        assert_eq!(std::fs::read_to_string(captured_arg).unwrap(), literal_arg);
        assert!(!injected_marker.exists());
        assert_eq!(
            std::fs::read(receipts).unwrap(),
            b"{\"case_id\":\"workspace-navigation\",\"arm\":\"atlas\",\"trial\":1,\"correctness\":{\"passed\":true},\"file_reads\":1,\"graph_calls\":1,\"tool_calls\":2,\"duration_ms\":10,\"context_bytes\":100,\"cost_usd\":null,\"response_bytes\":100,\"read_back_calls\":0,\"follow_up_queries\":1,\"truncated_queries\":0}\n"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
