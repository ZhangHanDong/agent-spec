use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CORPUS_SCHEMA: &str = "agent-spec/atlas-eval/corpus-v1";
pub const RUN_PLAN_SCHEMA: &str = "agent-spec/atlas-eval/run-plan-v1";
pub const QUERY_METRICS_SCHEMA: &str = "agent-spec/atlas-eval/query-metrics-v1";
pub const QUERY_CORPUS_SCHEMA: &str = "agent-spec/atlas-eval/query-corpus-v1";
pub const QUERY_RESULTS_SCHEMA: &str = "agent-spec/atlas-eval/query-results-v1";
pub const QUERY_REGRESSION_SCHEMA: &str = "agent-spec/atlas-eval/query-regression-v1";
pub const QUERY_MAX_AMBIGUITY: usize = 64;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryTier {
    DeterministicFixture,
    PinnedRepository,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryDiagnosticKind {
    Capability,
    Stale,
    WorktreeMismatch,
    Truncated,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryCorpus {
    pub schema: String,
    pub version: String,
    pub cases: Vec<QueryCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryCase {
    pub id: String,
    pub tier: QueryTier,
    pub task_class: TaskClass,
    pub repository: String,
    pub revision: String,
    pub query: String,
    pub expected_symbols: Vec<String>,
    pub expected_paths: Vec<Vec<String>>,
    pub forbidden_symbols: Vec<String>,
    pub forbidden_paths: Vec<Vec<String>>,
    pub required_evidence: Vec<String>,
    pub required_diagnostics: Vec<QueryDiagnostic>,
    pub allowed_ambiguity: usize,
    pub rubric: Vec<String>,
    pub source_ref: String,
    pub paired_fixture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryResults {
    pub schema: String,
    pub corpus_version: String,
    pub observations: Vec<QueryObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryObservation {
    pub case_id: String,
    pub ranked_symbols: Vec<String>,
    pub paths: Vec<Vec<String>>,
    pub evidence: Vec<String>,
    pub diagnostics: Vec<QueryDiagnostic>,
    pub response_bytes: u64,
    pub duration_ms: u64,
    pub read_back_calls: u64,
    pub follow_up_queries: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryDiagnostic {
    pub kind: QueryDiagnosticKind,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryRegressionReceipt {
    pub schema: String,
    pub corpus_version: String,
    pub corpus_fingerprint: String,
    pub cases: Vec<QueryCaseScore>,
    pub aggregate: QueryAggregateScore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryCaseScore {
    pub case_id: String,
    pub tier: QueryTier,
    pub passed: bool,
    pub symbol_recall: f64,
    pub reciprocal_rank: f64,
    pub path_precision: f64,
    pub path_recall: f64,
    pub forbidden_hits: usize,
    pub unexpected_items: usize,
    pub evidence_recall: f64,
    pub missing_evidence: Vec<String>,
    pub missing_diagnostics: Vec<QueryDiagnostic>,
    pub diagnostics: Vec<QueryDiagnostic>,
    pub failure_reasons: Vec<String>,
    pub returned_items: usize,
    pub response_bytes: u64,
    pub duration_ms: u64,
    pub read_back_calls: u64,
    pub follow_up_queries: u64,
    pub capability_diagnostics: usize,
    pub stale_diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryAggregateScore {
    pub cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub symbol_recall: f64,
    pub mean_reciprocal_rank: f64,
    pub path_precision: f64,
    pub path_recall: f64,
    pub forbidden_hit_rate: f64,
    pub evidence_recall: f64,
    pub response_bytes: MetricSummary,
    pub duration_ms: MetricSummary,
    pub read_back_calls: MetricSummary,
    pub follow_up_queries: MetricSummary,
    pub capability_diagnostics: usize,
    pub stale_diagnostics: usize,
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
#[serde(try_from = "RunReceiptWire")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_metrics_schema: Option<String>,
    pub response_bytes: u64,
    pub read_back_calls: u64,
    pub follow_up_queries: u64,
    pub truncated_queries: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunReceiptWire {
    case_id: String,
    arm: Arm,
    trial: u32,
    correctness: Option<Correctness>,
    file_reads: u64,
    graph_calls: u64,
    tool_calls: u64,
    duration_ms: u64,
    context_bytes: u64,
    cost_usd: Option<f64>,
    #[serde(default)]
    query_metrics_schema: Option<String>,
    #[serde(default)]
    response_bytes: Option<u64>,
    #[serde(default)]
    read_back_calls: Option<u64>,
    #[serde(default)]
    follow_up_queries: Option<u64>,
    #[serde(default)]
    truncated_queries: Option<u64>,
}

impl TryFrom<RunReceiptWire> for RunReceipt {
    type Error = String;

    fn try_from(wire: RunReceiptWire) -> Result<Self, Self::Error> {
        let values = [
            wire.response_bytes,
            wire.read_back_calls,
            wire.follow_up_queries,
            wire.truncated_queries,
        ];
        let present = values.iter().filter(|value| value.is_some()).count();
        if present != 0 && present != values.len() {
            return Err("query metrics must provide all four fields or none".into());
        }
        if let Some(schema) = wire.query_metrics_schema.as_deref()
            && schema != QUERY_METRICS_SCHEMA
        {
            return Err(format!("unsupported query metrics schema `{schema}`"));
        }
        if wire.query_metrics_schema.is_some() && present == 0 {
            return Err("query metrics schema requires all four metric fields".into());
        }
        if wire.query_metrics_schema.is_none() && values.iter().flatten().any(|value| *value != 0) {
            return Err("non-zero query metrics require query_metrics_schema".into());
        }

        Ok(Self {
            case_id: wire.case_id,
            arm: wire.arm,
            trial: wire.trial,
            correctness: wire.correctness,
            file_reads: wire.file_reads,
            graph_calls: wire.graph_calls,
            tool_calls: wire.tool_calls,
            duration_ms: wire.duration_ms,
            context_bytes: wire.context_bytes,
            cost_usd: wire.cost_usd,
            query_metrics_schema: wire.query_metrics_schema,
            response_bytes: wire.response_bytes.unwrap_or(0),
            read_back_calls: wire.read_back_calls.unwrap_or(0),
            follow_up_queries: wire.follow_up_queries.unwrap_or(0),
            truncated_queries: wire.truncated_queries.unwrap_or(0),
        })
    }
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
    pub query_metrics_receipts: usize,
    pub legacy_query_metrics_receipts: usize,
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

pub fn load_query_corpus(path: &Path) -> Result<QueryCorpus, EvalError> {
    let bytes = std::fs::read(path).map_err(|error| {
        EvalError::new(
            "atlas-query-corpus-load",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    let corpus = serde_json::from_slice(&bytes).map_err(|error| {
        EvalError::new(
            "atlas-query-corpus-parse",
            format!("failed to parse {}: {error}", path.display()),
        )
    })?;
    validate_query_corpus(&corpus)?;
    Ok(corpus)
}

pub fn load_query_results(path: &Path) -> Result<QueryResults, EvalError> {
    let bytes = std::fs::read(path).map_err(|error| {
        EvalError::new(
            "atlas-query-results-load",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        EvalError::new(
            "atlas-query-results-parse",
            format!("failed to parse {}: {error}", path.display()),
        )
    })
}

pub fn validate_query_corpus(corpus: &QueryCorpus) -> Result<(), EvalError> {
    if corpus.schema != QUERY_CORPUS_SCHEMA {
        return Err(EvalError::new(
            "atlas-query-corpus-schema",
            format!(
                "expected schema {QUERY_CORPUS_SCHEMA}, found {}",
                corpus.schema
            ),
        ));
    }
    if corpus.version.trim().is_empty() {
        return Err(EvalError::new(
            "atlas-query-corpus-version",
            "query corpus version must not be empty",
        ));
    }
    if corpus.cases.is_empty() {
        return Err(EvalError::new(
            "atlas-query-corpus-empty",
            "query corpus must contain at least one case",
        ));
    }

    let mut ids = BTreeMap::new();
    let mut has_fixture = false;
    let mut has_pinned = false;
    for case in &corpus.cases {
        if case.id.trim().is_empty() {
            return Err(EvalError::new(
                "atlas-query-corpus-case-id",
                "query case id must not be empty",
            ));
        }
        if ids.insert(case.id.as_str(), case.tier).is_some() {
            return Err(EvalError::new(
                "atlas-query-corpus-duplicate",
                format!("duplicate query case id {}", case.id),
            ));
        }
        match case.tier {
            QueryTier::DeterministicFixture => has_fixture = true,
            QueryTier::PinnedRepository => has_pinned = true,
        }
    }
    if !has_fixture || !has_pinned {
        return Err(EvalError::new(
            "atlas-query-corpus-tier",
            "query corpus must contain deterministic-fixture and pinned-repository tiers",
        ));
    }

    for case in &corpus.cases {
        for (field, value) in [
            ("repository", case.repository.as_str()),
            ("revision", case.revision.as_str()),
            ("query", case.query.as_str()),
            ("source_ref", case.source_ref.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(EvalError::new(
                    "atlas-query-corpus-field",
                    format!("case {} has an empty {field}", case.id),
                ));
            }
        }
        validate_nonempty_unique_strings(&case.id, "expected_symbols", &case.expected_symbols)?;
        validate_nonempty_unique_paths(&case.id, "expected_paths", &case.expected_paths)?;
        validate_unique_strings(&case.id, "forbidden_symbols", &case.forbidden_symbols)?;
        validate_unique_paths(&case.id, "forbidden_paths", &case.forbidden_paths)?;
        validate_nonempty_unique_strings(&case.id, "required_evidence", &case.required_evidence)?;
        validate_nonempty_unique_strings(&case.id, "rubric", &case.rubric)?;
        validate_unique_diagnostics(&case.id, &case.required_diagnostics)?;
        if case.allowed_ambiguity > QUERY_MAX_AMBIGUITY {
            return Err(EvalError::new(
                "atlas-query-corpus-ambiguity",
                format!(
                    "case {} allowed_ambiguity {} exceeds {}",
                    case.id, case.allowed_ambiguity, QUERY_MAX_AMBIGUITY
                ),
            ));
        }

        let expected_symbols = case.expected_symbols.iter().collect::<BTreeSet<_>>();
        if let Some(symbol) = case
            .forbidden_symbols
            .iter()
            .find(|symbol| expected_symbols.contains(symbol))
        {
            return Err(EvalError::new(
                "atlas-query-corpus-conflict",
                format!("case {} both expects and forbids symbol {symbol}", case.id),
            ));
        }
        let expected_paths = case
            .expected_paths
            .iter()
            .map(|path| path_key(path))
            .collect::<BTreeSet<_>>();
        if case
            .forbidden_paths
            .iter()
            .map(|path| path_key(path))
            .any(|path| expected_paths.contains(&path))
        {
            return Err(EvalError::new(
                "atlas-query-corpus-conflict",
                format!("case {} both expects and forbids the same path", case.id),
            ));
        }

        match case.tier {
            QueryTier::DeterministicFixture => {
                if !case.repository.starts_with("fixtures/") {
                    return Err(EvalError::new(
                        "atlas-query-corpus-fixture",
                        format!(
                            "fixture case {} repository must be under fixtures/",
                            case.id
                        ),
                    ));
                }
            }
            QueryTier::PinnedRepository => {
                if case.repository.starts_with("fixtures/") {
                    return Err(EvalError::new(
                        "atlas-query-corpus-pinned-repository",
                        format!(
                            "pinned repository case {} must not use a fixtures/ repository",
                            case.id
                        ),
                    ));
                }
                if !is_full_git_revision(&case.revision) {
                    return Err(EvalError::new(
                        "atlas-query-corpus-revision",
                        format!(
                            "pinned repository case {} must use a full 40-hex Git revision",
                            case.id
                        ),
                    ));
                }
                let Some(paired) = case.paired_fixture.as_deref() else {
                    return Err(EvalError::new(
                        "atlas-query-corpus-pair",
                        format!("pinned repository case {} has no paired_fixture", case.id),
                    ));
                };
                if ids.get(paired) != Some(&QueryTier::DeterministicFixture) {
                    return Err(EvalError::new(
                        "atlas-query-corpus-pair",
                        format!(
                            "pinned repository case {} references non-fixture case {paired}",
                            case.id
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn score_query_results(
    corpus: &QueryCorpus,
    results: &QueryResults,
) -> Result<QueryRegressionReceipt, EvalError> {
    validate_query_corpus(corpus)?;
    validate_query_results(corpus, results)?;

    let observations = results
        .observations
        .iter()
        .map(|observation| (observation.case_id.as_str(), observation))
        .collect::<BTreeMap<_, _>>();
    let cases = corpus
        .cases
        .iter()
        .map(|case| score_query_case(case, observations[case.id.as_str()]))
        .collect::<Vec<_>>();
    let aggregate = aggregate_query_scores(&cases);
    let corpus_bytes = serde_json::to_vec(corpus).map_err(|error| {
        EvalError::new(
            "atlas-query-score",
            format!("failed to fingerprint query corpus: {error}"),
        )
    })?;

    Ok(QueryRegressionReceipt {
        schema: QUERY_REGRESSION_SCHEMA.to_string(),
        corpus_version: corpus.version.clone(),
        corpus_fingerprint: blake3::hash(&corpus_bytes).to_hex().to_string(),
        cases,
        aggregate,
    })
}

pub fn gate_query_regression(receipt: &QueryRegressionReceipt) -> Result<(), EvalError> {
    if receipt.aggregate.failed == 0 {
        Ok(())
    } else {
        Err(EvalError::new(
            "atlas-query-regression",
            format!(
                "{} of {} query cases failed correctness",
                receipt.aggregate.failed, receipt.aggregate.cases
            ),
        ))
    }
}

fn validate_query_results(corpus: &QueryCorpus, results: &QueryResults) -> Result<(), EvalError> {
    if results.schema != QUERY_RESULTS_SCHEMA {
        return Err(EvalError::new(
            "atlas-query-results-schema",
            format!(
                "expected schema {QUERY_RESULTS_SCHEMA}, found {}",
                results.schema
            ),
        ));
    }
    if results.corpus_version != corpus.version {
        return Err(EvalError::new(
            "atlas-query-results-version",
            format!(
                "results target corpus version {}, expected {}",
                results.corpus_version, corpus.version
            ),
        ));
    }

    let corpus_ids = corpus
        .cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut observed_ids = BTreeSet::new();
    for observation in &results.observations {
        if !observed_ids.insert(observation.case_id.as_str()) {
            return Err(EvalError::new(
                "atlas-query-results-duplicate",
                format!("duplicate observation for case {}", observation.case_id),
            ));
        }
        if !corpus_ids.contains(observation.case_id.as_str()) {
            return Err(EvalError::new(
                "atlas-query-results-unknown",
                format!(
                    "observation references unknown case {}",
                    observation.case_id
                ),
            ));
        }
        validate_observation_strings(
            &observation.case_id,
            "ranked_symbols",
            &observation.ranked_symbols,
        )?;
        validate_observation_paths(&observation.case_id, "paths", &observation.paths)?;
        validate_observation_strings(&observation.case_id, "evidence", &observation.evidence)?;
        let mut diagnostics = BTreeSet::new();
        for diagnostic in &observation.diagnostics {
            if diagnostic.code.trim().is_empty()
                || !diagnostics.insert((diagnostic.kind, diagnostic.code.as_str()))
            {
                return Err(EvalError::new(
                    "atlas-query-results-observation",
                    format!(
                        "case {} has an empty or duplicate diagnostic",
                        observation.case_id
                    ),
                ));
            }
        }
    }

    let missing = corpus_ids
        .difference(&observed_ids)
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(EvalError::new(
            "atlas-query-results-missing",
            format!("missing observations for cases: {}", missing.join(", ")),
        ));
    }
    Ok(())
}

fn score_query_case(case: &QueryCase, observation: &QueryObservation) -> QueryCaseScore {
    let expected_symbols = case
        .expected_symbols
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let forbidden_symbols = case
        .forbidden_symbols
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let observed_symbols = observation
        .ranked_symbols
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let symbol_matches = expected_symbols.intersection(&observed_symbols).count();
    let symbol_recall = ratio(symbol_matches, expected_symbols.len());
    let reciprocal_rank = observation
        .ranked_symbols
        .iter()
        .position(|symbol| expected_symbols.contains(symbol.as_str()))
        .map_or(0.0, |index| 1.0 / (index + 1) as f64);

    let expected_paths = case
        .expected_paths
        .iter()
        .map(|path| path_key(path))
        .collect::<BTreeSet<_>>();
    let forbidden_paths = case
        .forbidden_paths
        .iter()
        .map(|path| path_key(path))
        .collect::<BTreeSet<_>>();
    let observed_paths = observation
        .paths
        .iter()
        .map(|path| path_key(path))
        .collect::<BTreeSet<_>>();
    let path_matches = expected_paths.intersection(&observed_paths).count();
    let path_precision = precision(
        path_matches,
        observed_paths.len(),
        expected_paths.is_empty(),
    );
    let path_recall = ratio(path_matches, expected_paths.len());

    let forbidden_hits = observation
        .ranked_symbols
        .iter()
        .filter(|symbol| forbidden_symbols.contains(symbol.as_str()))
        .count()
        + observed_paths.intersection(&forbidden_paths).count();
    let unexpected_items = observed_symbols.difference(&expected_symbols).count()
        + observed_paths.difference(&expected_paths).count();

    let observed_evidence = observation
        .evidence
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let missing_evidence = case
        .required_evidence
        .iter()
        .filter(|evidence| !observed_evidence.contains(evidence.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let evidence_recall = ratio(
        case.required_evidence.len() - missing_evidence.len(),
        case.required_evidence.len(),
    );

    let observed_diagnostics = observation
        .diagnostics
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_diagnostics = case
        .required_diagnostics
        .iter()
        .filter(|diagnostic| !observed_diagnostics.contains(*diagnostic))
        .cloned()
        .collect::<Vec<_>>();

    let mut failure_reasons = Vec::new();
    if symbol_matches != expected_symbols.len() {
        failure_reasons.push("missing-expected-symbol".to_string());
    }
    if path_matches != expected_paths.len() {
        failure_reasons.push("missing-expected-path".to_string());
    }
    if forbidden_hits != 0 {
        failure_reasons.push("forbidden-hit".to_string());
    }
    if !missing_evidence.is_empty() {
        failure_reasons.push("missing-evidence".to_string());
    }
    if !missing_diagnostics.is_empty() {
        failure_reasons.push("missing-required-diagnostic".to_string());
    }
    if unexpected_items > case.allowed_ambiguity {
        failure_reasons.push("ambiguity-exceeded".to_string());
    }

    QueryCaseScore {
        case_id: case.id.clone(),
        tier: case.tier,
        passed: failure_reasons.is_empty(),
        symbol_recall,
        reciprocal_rank,
        path_precision,
        path_recall,
        forbidden_hits,
        unexpected_items,
        evidence_recall,
        missing_evidence,
        missing_diagnostics,
        diagnostics: observation.diagnostics.clone(),
        failure_reasons,
        returned_items: observed_symbols.len() + observed_paths.len(),
        response_bytes: observation.response_bytes,
        duration_ms: observation.duration_ms,
        read_back_calls: observation.read_back_calls,
        follow_up_queries: observation.follow_up_queries,
        capability_diagnostics: observation
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == QueryDiagnosticKind::Capability)
            .count(),
        stale_diagnostics: observation
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == QueryDiagnosticKind::Stale)
            .count(),
    }
}

fn aggregate_query_scores(cases: &[QueryCaseScore]) -> QueryAggregateScore {
    let case_count = cases.len();
    let passed = cases.iter().filter(|case| case.passed).count();
    let forbidden_hits = cases.iter().map(|case| case.forbidden_hits).sum::<usize>();
    let returned_items = cases.iter().map(|case| case.returned_items).sum::<usize>();
    QueryAggregateScore {
        cases: case_count,
        passed,
        failed: case_count - passed,
        symbol_recall: mean(cases.iter().map(|case| case.symbol_recall)),
        mean_reciprocal_rank: mean(cases.iter().map(|case| case.reciprocal_rank)),
        path_precision: mean(cases.iter().map(|case| case.path_precision)),
        path_recall: mean(cases.iter().map(|case| case.path_recall)),
        forbidden_hit_rate: if returned_items == 0 {
            0.0
        } else {
            forbidden_hits as f64 / returned_items as f64
        },
        evidence_recall: mean(cases.iter().map(|case| case.evidence_recall)),
        response_bytes: metric(cases.iter().map(|case| case.response_bytes as f64)),
        duration_ms: metric(cases.iter().map(|case| case.duration_ms as f64)),
        read_back_calls: metric(cases.iter().map(|case| case.read_back_calls as f64)),
        follow_up_queries: metric(cases.iter().map(|case| case.follow_up_queries as f64)),
        capability_diagnostics: cases.iter().map(|case| case.capability_diagnostics).sum(),
        stale_diagnostics: cases.iter().map(|case| case.stale_diagnostics).sum(),
    }
}

fn validate_nonempty_unique_strings(
    case_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), EvalError> {
    if values.is_empty() {
        return Err(EvalError::new(
            "atlas-query-corpus-field",
            format!("case {case_id} has no {field}"),
        ));
    }
    validate_unique_strings(case_id, field, values)
}

fn validate_unique_strings(case_id: &str, field: &str, values: &[String]) -> Result<(), EvalError> {
    let mut unique = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() || !unique.insert(value.as_str()) {
            return Err(EvalError::new(
                "atlas-query-corpus-field",
                format!("case {case_id} has an empty or duplicate {field} item"),
            ));
        }
    }
    Ok(())
}

fn validate_nonempty_unique_paths(
    case_id: &str,
    field: &str,
    paths: &[Vec<String>],
) -> Result<(), EvalError> {
    if paths.is_empty() {
        return Err(EvalError::new(
            "atlas-query-corpus-field",
            format!("case {case_id} has no {field}"),
        ));
    }
    validate_unique_paths(case_id, field, paths)
}

fn validate_unique_paths(
    case_id: &str,
    field: &str,
    paths: &[Vec<String>],
) -> Result<(), EvalError> {
    let mut unique = BTreeSet::new();
    for path in paths {
        if path.is_empty()
            || path.iter().any(|segment| segment.trim().is_empty())
            || !unique.insert(path_key(path))
        {
            return Err(EvalError::new(
                "atlas-query-corpus-field",
                format!("case {case_id} has an empty or duplicate {field} item"),
            ));
        }
    }
    Ok(())
}

fn validate_unique_diagnostics(
    case_id: &str,
    diagnostics: &[QueryDiagnostic],
) -> Result<(), EvalError> {
    let mut unique = BTreeSet::new();
    for diagnostic in diagnostics {
        if diagnostic.code.trim().is_empty() || !unique.insert(diagnostic) {
            return Err(EvalError::new(
                "atlas-query-corpus-field",
                format!("case {case_id} has an empty or duplicate required_diagnostics item"),
            ));
        }
    }
    Ok(())
}

fn validate_observation_strings(
    case_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), EvalError> {
    let mut unique = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() || !unique.insert(value.as_str()) {
            return Err(EvalError::new(
                "atlas-query-results-observation",
                format!("case {case_id} has an empty or duplicate {field} item"),
            ));
        }
    }
    Ok(())
}

fn validate_observation_paths(
    case_id: &str,
    field: &str,
    paths: &[Vec<String>],
) -> Result<(), EvalError> {
    let mut unique = BTreeSet::new();
    for path in paths {
        if path.is_empty()
            || path.iter().any(|segment| segment.trim().is_empty())
            || !unique.insert(path_key(path))
        {
            return Err(EvalError::new(
                "atlas-query-results-observation",
                format!("case {case_id} has an empty or duplicate {field} item"),
            ));
        }
    }
    Ok(())
}

fn path_key(path: &[String]) -> String {
    path.join("\u{1f}")
}

fn is_full_git_revision(revision: &str) -> bool {
    revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn precision(matches: usize, returned: usize, expected_empty: bool) -> f64 {
    if returned == 0 {
        f64::from(expected_empty)
    } else {
        matches as f64 / returned as f64
    }
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
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
    let query_metrics = receipts
        .iter()
        .filter(|receipt| receipt.query_metrics_schema.as_deref() == Some(QUERY_METRICS_SCHEMA))
        .collect::<Vec<_>>();
    MetricsSummary {
        file_reads: metric(receipts.iter().map(|receipt| receipt.file_reads as f64)),
        graph_calls: metric(receipts.iter().map(|receipt| receipt.graph_calls as f64)),
        tool_calls: metric(receipts.iter().map(|receipt| receipt.tool_calls as f64)),
        duration_ms: metric(receipts.iter().map(|receipt| receipt.duration_ms as f64)),
        context_bytes: metric(receipts.iter().map(|receipt| receipt.context_bytes as f64)),
        cost_usd: optional_metric(receipts.iter().filter_map(|receipt| receipt.cost_usd)),
        query_metrics_receipts: query_metrics.len(),
        legacy_query_metrics_receipts: receipts.len() - query_metrics.len(),
        response_bytes: metric(
            query_metrics
                .iter()
                .map(|receipt| receipt.response_bytes as f64),
        ),
        read_back_calls: metric(
            query_metrics
                .iter()
                .map(|receipt| receipt.read_back_calls as f64),
        ),
        follow_up_queries: metric(
            query_metrics
                .iter()
                .map(|receipt| receipt.follow_up_queries as f64),
        ),
        truncated_queries: metric(
            query_metrics
                .iter()
                .map(|receipt| receipt.truncated_queries as f64),
        ),
    }
}

fn metric(values: impl Iterator<Item = f64>) -> MetricSummary {
    let mut values: Vec<_> = values.collect();
    if values.is_empty() {
        return MetricSummary {
            samples: 0,
            median: 0.0,
            mad: 0.0,
        };
    }
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
            query_metrics_schema: Some(QUERY_METRICS_SCHEMA.into()),
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

    fn valid_query_corpus() -> QueryCorpus {
        QueryCorpus {
            schema: QUERY_CORPUS_SCHEMA.to_string(),
            version: "test-v1".to_string(),
            cases: vec![
                QueryCase {
                    id: "fixture-flow".to_string(),
                    tier: QueryTier::DeterministicFixture,
                    task_class: TaskClass::Flow,
                    repository: "fixtures/atlas/basic".to_string(),
                    revision: "fixture-v1".to_string(),
                    query: "Trace service run to store get.".to_string(),
                    expected_symbols: vec![
                        "atlas_basic::service::run".to_string(),
                        "atlas_basic::store::Store::get".to_string(),
                    ],
                    expected_paths: vec![vec![
                        "atlas_basic::service::run".to_string(),
                        "atlas_basic::store::Store::get".to_string(),
                    ]],
                    forbidden_symbols: vec!["atlas_basic::open_default".to_string()],
                    forbidden_paths: vec![vec![
                        "atlas_basic::service::run".to_string(),
                        "atlas_basic::open_default".to_string(),
                    ]],
                    required_evidence: vec!["fixtures/atlas/basic/src/service.rs".to_string()],
                    required_diagnostics: vec![QueryDiagnostic {
                        kind: QueryDiagnosticKind::Stale,
                        code: "atlas-stale-syn".to_string(),
                    }],
                    allowed_ambiguity: 0,
                    rubric: vec!["Returns the complete evidence-backed path.".to_string()],
                    source_ref: "test-fixture".to_string(),
                    paired_fixture: None,
                },
                QueryCase {
                    id: "pinned-flow".to_string(),
                    tier: QueryTier::PinnedRepository,
                    task_class: TaskClass::Flow,
                    repository: "https://github.com/example/project".to_string(),
                    revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
                    query: "Trace command dispatch to scoring.".to_string(),
                    expected_symbols: vec!["example::score".to_string()],
                    expected_paths: vec![vec![
                        "example::dispatch".to_string(),
                        "example::score".to_string(),
                    ]],
                    forbidden_symbols: vec!["example::legacy_score".to_string()],
                    forbidden_paths: vec![vec![
                        "example::dispatch".to_string(),
                        "example::legacy_score".to_string(),
                    ]],
                    required_evidence: vec!["src/eval.rs".to_string()],
                    required_diagnostics: vec![QueryDiagnostic {
                        kind: QueryDiagnosticKind::Capability,
                        code: "atlas-capability-mir-unavailable".to_string(),
                    }],
                    allowed_ambiguity: 1,
                    rubric: vec!["Uses the current scoring entry point.".to_string()],
                    source_ref: "test-pinned".to_string(),
                    paired_fixture: Some("fixture-flow".to_string()),
                },
            ],
        }
    }

    fn matching_query_results() -> QueryResults {
        QueryResults {
            schema: QUERY_RESULTS_SCHEMA.to_string(),
            corpus_version: "test-v1".to_string(),
            observations: vec![
                QueryObservation {
                    case_id: "fixture-flow".to_string(),
                    ranked_symbols: vec![
                        "atlas_basic::service::run".to_string(),
                        "atlas_basic::store::Store::get".to_string(),
                    ],
                    paths: vec![vec![
                        "atlas_basic::service::run".to_string(),
                        "atlas_basic::store::Store::get".to_string(),
                    ]],
                    evidence: vec!["fixtures/atlas/basic/src/service.rs".to_string()],
                    diagnostics: vec![QueryDiagnostic {
                        kind: QueryDiagnosticKind::Stale,
                        code: "atlas-stale-syn".to_string(),
                    }],
                    response_bytes: 1_200,
                    duration_ms: 25,
                    read_back_calls: 1,
                    follow_up_queries: 2,
                },
                QueryObservation {
                    case_id: "pinned-flow".to_string(),
                    ranked_symbols: vec![
                        "example::unresolved_candidate".to_string(),
                        "example::score".to_string(),
                    ],
                    paths: vec![vec![
                        "example::dispatch".to_string(),
                        "example::score".to_string(),
                    ]],
                    evidence: vec!["src/eval.rs".to_string()],
                    diagnostics: vec![QueryDiagnostic {
                        kind: QueryDiagnosticKind::Capability,
                        code: "atlas-capability-mir-unavailable".to_string(),
                    }],
                    response_bytes: 2_400,
                    duration_ms: 75,
                    read_back_calls: 0,
                    follow_up_queries: 1,
                },
            ],
        }
    }

    #[test]
    fn test_atlas_query_checked_in_corpus_has_fixture_and_pinned_repository_tiers() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("benchmarks/atlas/query-corpus.json");
        let corpus = load_query_corpus(&path).expect("checked-in query corpus loads");

        assert!(
            corpus
                .cases
                .iter()
                .any(|case| case.tier == QueryTier::DeterministicFixture)
        );
        assert!(
            corpus
                .cases
                .iter()
                .any(|case| case.tier == QueryTier::PinnedRepository)
        );
        for case in corpus
            .cases
            .iter()
            .filter(|case| case.tier == QueryTier::PinnedRepository)
        {
            assert_eq!(case.revision.len(), 40);
            assert!(case.revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
            let paired = case.paired_fixture.as_deref().expect("paired fixture");
            assert!(corpus.cases.iter().any(|candidate| {
                candidate.id == paired && candidate.tier == QueryTier::DeterministicFixture
            }));
        }
    }

    #[test]
    fn test_atlas_query_checked_in_regression_receipt_is_passing() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let corpus = load_query_corpus(&root.join("benchmarks/atlas/query-corpus.json"))
            .expect("checked-in query corpus loads");
        let results = load_query_results(&root.join("benchmarks/atlas/query-results.json"))
            .expect("checked-in query results load");

        let receipt = score_query_results(&corpus, &results).expect("checked-in results score");

        assert_eq!(receipt.aggregate.cases, corpus.cases.len());
        assert_eq!(receipt.aggregate.passed, corpus.cases.len());
        assert_eq!(receipt.aggregate.failed, 0);
        assert_eq!(receipt.aggregate.forbidden_hit_rate, 0.0);
    }

    #[test]
    fn test_atlas_query_live_fixture_probe_scores_current_search_and_flow() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let code = root.join("fixtures/atlas/basic");
        let graph = temp_dir("atlas-query-live-probe").join("graph");
        rust_atlas::build(
            &code,
            &graph,
            &rust_atlas::BuildOptions {
                full: true,
                scip_index: Some(root.join("fixtures/atlas/scip/index.scip")),
                dynamic_dispatch: false,
            },
        )
        .expect("fixture graph builds offline");

        let corpus = load_query_corpus(&root.join("benchmarks/atlas/query-corpus.json"))
            .expect("checked-in query corpus loads");
        let mut results = load_query_results(&root.join("benchmarks/atlas/query-results.json"))
            .expect("checked-in query results load");
        let flow_options = rust_atlas::FlowOptions {
            frozen: true,
            ..Default::default()
        };

        let search = rust_atlas::search(
            &code,
            &graph,
            "MemStore",
            &rust_atlas::SearchOptions {
                limit: 1,
                frozen: true,
            },
        )
        .expect("fixture search runs");
        let symbol_flow = rust_atlas::flow(
            &code,
            &graph,
            rust_atlas::FlowQuery::Between {
                from: "atlas_basic::open_default".to_string(),
                to: "atlas_basic::store::MemStore".to_string(),
            },
            &flow_options,
        )
        .expect("fixture symbol path runs");
        replace_live_observation(
            &corpus,
            &mut results,
            "fixture-symbol-mem-store",
            search
                .matches
                .iter()
                .map(|hit| hit.node.symbol.clone())
                .collect(),
            &symbol_flow,
            serde_json::to_vec(&search).unwrap().len()
                + serde_json::to_vec(&symbol_flow).unwrap().len(),
        );

        let call_flow = rust_atlas::flow(
            &code,
            &graph,
            rust_atlas::FlowQuery::Between {
                from: "atlas_basic::service::run".to_string(),
                to: "atlas_basic::store::Store::get".to_string(),
            },
            &flow_options,
        )
        .expect("fixture call path runs");
        let call_symbols = call_flow
            .shortest
            .as_ref()
            .expect("fixture call path exists")
            .nodes
            .iter()
            .map(|node| node.symbol.clone())
            .collect();
        replace_live_observation(
            &corpus,
            &mut results,
            "fixture-flow-store-get",
            call_symbols,
            &call_flow,
            serde_json::to_vec(&call_flow).unwrap().len(),
        );

        let receipt = score_query_results(&corpus, &results).expect("live fixture results score");
        for case_id in ["fixture-symbol-mem-store", "fixture-flow-store-get"] {
            let score = receipt
                .cases
                .iter()
                .find(|score| score.case_id == case_id)
                .expect("live fixture score exists");
            assert!(score.passed, "{case_id}: {:?}", score.failure_reasons);
            assert!(score.response_bytes > 0);
        }

        std::fs::remove_dir_all(graph.parent().unwrap()).ok();
    }

    fn replace_live_observation(
        corpus: &QueryCorpus,
        results: &mut QueryResults,
        case_id: &str,
        ranked_symbols: Vec<String>,
        flow: &rust_atlas::FlowResult,
        response_bytes: usize,
    ) {
        let case = corpus
            .cases
            .iter()
            .find(|case| case.id == case_id)
            .expect("live fixture case exists");
        let path = flow
            .shortest
            .as_ref()
            .expect("live fixture path exists")
            .nodes
            .iter()
            .map(|node| node.symbol.clone())
            .collect::<Vec<_>>();
        let evidence = flow
            .shortest
            .as_ref()
            .expect("live fixture path exists")
            .nodes
            .iter()
            .map(|node| format!("{}/{}", case.repository, node.file))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let observation = results
            .observations
            .iter_mut()
            .find(|observation| observation.case_id == case_id)
            .expect("live fixture observation exists");
        observation.ranked_symbols = ranked_symbols;
        observation.paths = vec![path];
        observation.evidence = evidence;
        observation.diagnostics.clear();
        observation.response_bytes = response_bytes as u64;
        observation.duration_ms = 0;
        observation.read_back_calls = 0;
        observation.follow_up_queries = 0;
    }

    #[test]
    fn test_atlas_query_score_computes_recall_mrr_paths_and_costs() {
        let receipt = score_query_results(&valid_query_corpus(), &matching_query_results())
            .expect("valid query observations score");

        assert_eq!(receipt.schema, QUERY_REGRESSION_SCHEMA);
        assert_eq!(receipt.corpus_version, "test-v1");
        assert_eq!(receipt.corpus_fingerprint.len(), 64);
        assert_eq!(receipt.aggregate.cases, 2);
        assert_eq!(receipt.aggregate.passed, 2);
        assert_eq!(receipt.aggregate.failed, 0);
        assert_eq!(receipt.aggregate.symbol_recall, 1.0);
        assert_eq!(receipt.aggregate.mean_reciprocal_rank, 0.75);
        assert_eq!(receipt.aggregate.path_precision, 1.0);
        assert_eq!(receipt.aggregate.path_recall, 1.0);
        assert_eq!(receipt.aggregate.forbidden_hit_rate, 0.0);
        assert_eq!(receipt.aggregate.evidence_recall, 1.0);
        assert_metric(&receipt.aggregate.response_bytes, 1_800.0, 600.0, 2);
        assert_metric(&receipt.aggregate.duration_ms, 50.0, 25.0, 2);
        assert_metric(&receipt.aggregate.read_back_calls, 0.5, 0.5, 2);
        assert_metric(&receipt.aggregate.follow_up_queries, 1.5, 0.5, 2);
        assert_eq!(receipt.aggregate.capability_diagnostics, 1);
        assert_eq!(receipt.aggregate.stale_diagnostics, 1);
        assert_eq!(receipt.cases[0].diagnostics[0].code, "atlas-stale-syn");
    }

    #[test]
    fn test_atlas_query_score_rejects_wrong_paths_forbidden_hits_and_missing_evidence() {
        let corpus = valid_query_corpus();
        let mut results = matching_query_results();
        let observation = &mut results.observations[0];
        observation.paths = vec![vec![
            "atlas_basic::service::run".to_string(),
            "atlas_basic::open_default".to_string(),
        ]];
        observation
            .ranked_symbols
            .push("atlas_basic::open_default".to_string());
        observation.evidence.clear();

        let receipt = score_query_results(&corpus, &results).expect("invalid answer still scores");
        let score = &receipt.cases[0];
        assert!(!score.passed);
        assert_eq!(score.symbol_recall, 1.0);
        assert_eq!(score.path_recall, 0.0);
        assert_eq!(score.path_precision, 0.0);
        assert_eq!(score.forbidden_hits, 2);
        assert_eq!(score.evidence_recall, 0.0);
        assert_eq!(
            score.missing_evidence,
            ["fixtures/atlas/basic/src/service.rs"]
        );
        assert!(
            score
                .failure_reasons
                .contains(&"missing-expected-path".to_string())
        );
        assert!(score.failure_reasons.contains(&"forbidden-hit".to_string()));
        assert!(
            score
                .failure_reasons
                .contains(&"missing-evidence".to_string())
        );
    }

    #[test]
    fn test_atlas_query_score_requires_declared_stale_diagnostic() {
        let corpus = valid_query_corpus();
        let mut results = matching_query_results();
        results.observations[0].diagnostics[0].code = "atlas-stale-other-layer".to_string();

        let receipt = score_query_results(&corpus, &results).expect("missing diagnostic scores");
        let score = &receipt.cases[0];
        assert!(!score.passed);
        assert_eq!(
            score.missing_diagnostics,
            [QueryDiagnostic {
                kind: QueryDiagnosticKind::Stale,
                code: "atlas-stale-syn".to_string(),
            }]
        );
        assert!(
            score
                .failure_reasons
                .contains(&"missing-required-diagnostic".to_string())
        );
    }

    #[test]
    fn test_atlas_query_score_rejects_duplicate_missing_and_unknown_observations() {
        let corpus = valid_query_corpus();

        let mut duplicate = matching_query_results();
        duplicate
            .observations
            .push(duplicate.observations[0].clone());
        assert_eq!(
            score_query_results(&corpus, &duplicate).unwrap_err().code(),
            "atlas-query-results-duplicate"
        );

        let mut missing = matching_query_results();
        missing.observations.pop();
        assert_eq!(
            score_query_results(&corpus, &missing).unwrap_err().code(),
            "atlas-query-results-missing"
        );

        let mut unknown = matching_query_results();
        unknown.observations[0].case_id = "unknown-case".to_string();
        assert_eq!(
            score_query_results(&corpus, &unknown).unwrap_err().code(),
            "atlas-query-results-unknown"
        );

        let mut wrong_version = matching_query_results();
        wrong_version.corpus_version = "other-version".to_string();
        assert_eq!(
            score_query_results(&corpus, &wrong_version)
                .unwrap_err()
                .code(),
            "atlas-query-results-version"
        );

        let mut duplicate_diagnostic = matching_query_results();
        let repeated_diagnostic = duplicate_diagnostic.observations[0].diagnostics[0].clone();
        duplicate_diagnostic.observations[0]
            .diagnostics
            .push(repeated_diagnostic);
        assert_eq!(
            score_query_results(&corpus, &duplicate_diagnostic)
                .unwrap_err()
                .code(),
            "atlas-query-results-observation"
        );
    }

    #[test]
    fn test_atlas_query_corpus_rejects_mutable_pinned_revision() {
        let mut corpus = valid_query_corpus();
        corpus.cases[1].revision = "main".to_string();
        assert_eq!(
            validate_query_corpus(&corpus).unwrap_err().code(),
            "atlas-query-corpus-revision"
        );
    }

    #[test]
    fn test_atlas_query_corpus_rejects_fixture_path_as_pinned_repository() {
        let mut corpus = valid_query_corpus();
        corpus.cases[1].repository = "fixtures/atlas/basic".to_string();
        assert_eq!(
            validate_query_corpus(&corpus).unwrap_err().code(),
            "atlas-query-corpus-pinned-repository"
        );
    }

    #[test]
    fn test_atlas_query_corpus_rejects_unbounded_ambiguity() {
        let mut corpus = valid_query_corpus();
        corpus.cases[0].allowed_ambiguity = QUERY_MAX_AMBIGUITY + 1;
        assert_eq!(
            validate_query_corpus(&corpus).unwrap_err().code(),
            "atlas-query-corpus-ambiguity"
        );
    }

    #[test]
    fn test_atlas_query_corpus_and_results_reject_nested_unknown_fields() {
        let dir = temp_dir("atlas-query-unknown-fields");
        let corpus_path = dir.join("corpus.json");
        let results_path = dir.join("results.json");
        let mut corpus = serde_json::to_value(valid_query_corpus()).unwrap();
        corpus["cases"][0]["required_diagnostics"][0]["unexpected"] = serde_json::json!(true);
        std::fs::write(&corpus_path, serde_json::to_vec(&corpus).unwrap()).unwrap();
        assert_eq!(
            load_query_corpus(&corpus_path).unwrap_err().code(),
            "atlas-query-corpus-parse"
        );

        let mut results = serde_json::to_value(matching_query_results()).unwrap();
        results["observations"][0]["diagnostics"][0]["unexpected"] = serde_json::json!(true);
        std::fs::write(&results_path, serde_json::to_vec(&results).unwrap()).unwrap();
        assert_eq!(
            load_query_results(&results_path).unwrap_err().code(),
            "atlas-query-results-parse"
        );

        std::fs::remove_dir_all(dir).unwrap();
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
        assert_eq!(summary.metrics.query_metrics_receipts, 3);
        assert_eq!(summary.metrics.legacy_query_metrics_receipts, 0);
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
        let legacy_summary = summarize(&[legacy]).unwrap();
        assert_eq!(legacy_summary.metrics.query_metrics_receipts, 0);
        assert_eq!(legacy_summary.metrics.legacy_query_metrics_receipts, 1);
        assert_metric(&legacy_summary.metrics.response_bytes, 0.0, 0.0, 0);
        assert_metric(&legacy_summary.metrics.read_back_calls, 0.0, 0.0, 0);

        let partial =
            LEGACY_RECEIPT_JSON.replace("\n    }", ",\n        \"response_bytes\": 12000\n    }");
        assert!(serde_json::from_str::<RunReceipt>(&partial).is_err());
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
