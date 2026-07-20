use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::affected::normalize_affected_path;
use crate::flow::{FlowOptions, FlowQuery, flow_index};
use crate::impact::impact_many_index;
use crate::runtime_boundary::project_runtime_boundaries;
use crate::{
    AtlasError, AtlasStatus, Edge, EdgeKind, FlowState, GraphPath, ImpactOptions, MatchKind, Meta,
    Node, NodeKind, QueryIndex, QueryOptions, RuntimeBoundaryHint, TraversalLimits,
    indexed_query_state,
};

const CONTEXT_SCHEMA: &str = "agent-spec/rust-atlas/context-v1";
const RETRIEVAL_SCHEMA: &str = "agent-spec/rust-atlas/context-retrieval-v1";
const MAX_CONTEXT_BYTES: usize = 1_000_000;
const MIN_CONTEXT_BYTES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextProfile {
    Symbol,
    Flow,
    Architecture,
    Impact,
}

impl ContextProfile {
    pub fn limits(self) -> ContextLimits {
        match self {
            Self::Symbol => ContextLimits {
                max_candidates: 256,
                max_paths: 8,
                max_source_slices: 8,
                max_source_lines: 32,
                max_serialized_bytes: 16_000,
                relevance_threshold: 300,
            },
            Self::Flow => ContextLimits {
                max_candidates: 384,
                max_paths: 16,
                max_source_slices: 16,
                max_source_lines: 48,
                max_serialized_bytes: 32_000,
                relevance_threshold: 250,
            },
            Self::Architecture => ContextLimits {
                max_candidates: 512,
                max_paths: 8,
                max_source_slices: 8,
                max_source_lines: 32,
                max_serialized_bytes: 24_000,
                relevance_threshold: 200,
            },
            Self::Impact => ContextLimits {
                max_candidates: 384,
                max_paths: 16,
                max_source_slices: 12,
                max_source_lines: 40,
                max_serialized_bytes: 24_000,
                relevance_threshold: 250,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextLimits {
    pub max_candidates: usize,
    pub max_paths: usize,
    pub max_source_slices: usize,
    pub max_source_lines: usize,
    pub max_serialized_bytes: usize,
    pub relevance_threshold: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextRelation {
    Calls,
    Callers,
    Callees,
    References,
    UsesType,
    Implements,
    Contains,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryIntent {
    pub query: String,
    pub profile: ContextProfile,
    pub identifiers: Vec<String>,
    pub paths: Vec<String>,
    pub relations: Vec<ContextRelation>,
    pub unrecognized: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOptions {
    pub profile: ContextProfile,
    pub frozen: bool,
    pub max_serialized_bytes: Option<usize>,
    pub min_score: Option<u16>,
    pub after: Option<String>,
    pub expected_graph_fingerprint: Option<String>,
    pub failure_evidence: Vec<String>,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            profile: ContextProfile::Symbol,
            frozen: false,
            max_serialized_bytes: None,
            min_score: None,
            after: None,
            expected_graph_fingerprint: None,
            failure_evidence: Vec::new(),
        }
    }
}

pub struct PinnedContextSnapshot {
    graph_root: PathBuf,
    snapshot: crate::generation::GraphSnapshot,
    persisted: crate::PersistedMeta,
    estimated_index_bytes: u64,
}

impl PinnedContextSnapshot {
    pub fn generation(&self) -> Option<&str> {
        self.snapshot.generation.as_deref()
    }

    pub fn graph_fingerprint(&self) -> &str {
        &self.persisted.meta.graph_fingerprint
    }

    pub fn estimated_index_bytes(&self) -> u64 {
        self.estimated_index_bytes
    }
}

#[derive(Clone)]
pub struct ContextExecutionControl {
    cancelled: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl ContextExecutionControl {
    pub fn unlimited() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: None,
        }
    }

    pub fn with_deadline(deadline: Instant) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Some(deadline),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn checkpoint(&self) -> Result<(), AtlasError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(AtlasError::QueryCancelled);
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(AtlasError::QueryTimeout);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceClass {
    NamedSymbol,
    FailureEvidence,
    PrimarySpine,
    BoundarySite,
    UniqueImplementation,
    RepresentativeImplementation,
    ImpactPath,
    AlternativePath,
    Relationship,
    AdjacentStructure,
    OffSpineSibling,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct EvidenceSpan {
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceCandidate {
    pub id: String,
    pub class: EvidenceClass,
    pub score: u16,
    pub required: bool,
    pub scoring_reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<Node>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<Edge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<GraphPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_boundary: Option<RuntimeBoundaryHint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<EvidenceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievalCandidateSet {
    pub schema: String,
    pub intent: QueryIntent,
    pub graph_fingerprint: String,
    pub total_candidates: usize,
    pub eligible_candidates: usize,
    pub after_cursor_omitted: usize,
    pub hard_cap_omitted: usize,
    pub candidates: Vec<EvidenceCandidate>,
    pub diagnostics: Vec<ContextDiagnostic>,
    pub status: AtlasStatus,
    #[serde(skip)]
    recorded_files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidencePriorityPlan {
    pub profile: ContextProfile,
    pub limits: ContextLimits,
    pub class_order: Vec<EvidenceClass>,
    pub tie_break: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceSlice {
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub text: String,
    pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectedEvidence {
    pub id: String,
    pub class: EvidenceClass,
    pub score: u16,
    pub required: bool,
    pub scoring_reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<Node>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<Edge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<GraphPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_boundary: Option<RuntimeBoundaryHint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSlice>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signature_skeleton: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextProjection {
    pub evidence: Vec<ProjectedEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OmissionReason {
    RetrievalCap,
    BelowRelevance,
    ByteCeiling,
    AfterCursor,
    SourceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContinuationQuery {
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OmissionEntry {
    pub class: EvidenceClass,
    pub reason: OmissionReason,
    pub count: usize,
    pub highest_score: u16,
    pub highest_candidate: String,
    pub continuation: ContinuationQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetrievalReceipt {
    pub total: usize,
    pub eligible: usize,
    pub returned: usize,
    pub after_cursor_omitted: usize,
    pub hard_cap_omitted: usize,
    pub coverage_numerator: usize,
    pub coverage_denominator: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionReceipt {
    pub above_relevance: usize,
    pub retained: usize,
    pub below_relevance_omitted: usize,
    pub byte_omitted: usize,
    pub after_cursor_omitted: usize,
    pub skeletonized: usize,
    pub policy_skeletonized: usize,
    pub retention_numerator: usize,
    pub retention_denominator: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryLoadProfile {
    Light,
    Traversal,
    SourceHeavy,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueryReceipt {
    pub retrieval: RetrievalReceipt,
    pub projection: ProjectionReceipt,
    pub profile: ContextProfile,
    pub limits: ContextLimits,
    pub serialized_bytes: usize,
    pub truncated_evidence_classes: Vec<EvidenceClass>,
    pub graph_fingerprint: String,
    pub read_back_required: bool,
    pub follow_up_queries: usize,
    pub load_profile: QueryLoadProfile,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextResult {
    pub schema: String,
    pub intent: QueryIntent,
    pub priority_plan: EvidencePriorityPlan,
    pub projection: ContextProjection,
    pub omissions: Vec<OmissionEntry>,
    pub receipt: QueryReceipt,
    pub diagnostics: Vec<ContextDiagnostic>,
    pub status: AtlasStatus,
}

#[derive(Debug, Clone)]
struct OmittedCandidate {
    class: EvidenceClass,
    reason: OmissionReason,
    score: u16,
    id: String,
}

pub fn parse_query_intent(query: &str, profile: ContextProfile) -> QueryIntent {
    let mut identifiers = Vec::new();
    let mut paths = Vec::new();
    let mut relations = Vec::new();
    let mut unrecognized = Vec::new();
    for raw in query.split_whitespace() {
        let token = raw
            .trim_matches(|character: char| matches!(character, ',' | ';' | '(' | ')' | '[' | ']'));
        if token.is_empty() {
            continue;
        }
        if let Some(relation) = parse_relation(token) {
            push_unique(&mut relations, relation);
            continue;
        }
        if looks_like_path(token) {
            match normalize_query_path(token) {
                Some(path) => {
                    push_unique(&mut paths, path);
                }
                None => {
                    push_unique(&mut unrecognized, token.to_string());
                }
            }
            continue;
        }
        if looks_like_identifier(token) {
            push_unique(&mut identifiers, token.trim_matches(':').to_string());
        } else {
            push_unique(&mut unrecognized, token.to_string());
        }
    }
    QueryIntent {
        query: query.to_string(),
        profile,
        identifiers,
        paths,
        relations,
        unrecognized,
    }
}

fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn parse_relation(token: &str) -> Option<ContextRelation> {
    match token.to_ascii_lowercase().as_str() {
        "calls" | "call" => Some(ContextRelation::Calls),
        "callers" | "caller" => Some(ContextRelation::Callers),
        "callees" | "callee" => Some(ContextRelation::Callees),
        "references" | "reference" | "refs" => Some(ContextRelation::References),
        "uses-type" | "uses_type" => Some(ContextRelation::UsesType),
        "implements" | "impl" => Some(ContextRelation::Implements),
        "contains" | "containment" => Some(ContextRelation::Contains),
        _ => None,
    }
}

fn looks_like_path(token: &str) -> bool {
    token.contains('/') || token.contains('\\') || token.ends_with(".rs")
}

fn normalize_query_path(token: &str) -> Option<String> {
    let replaced = token.replace('\\', "/");
    let path = Path::new(replaced.trim_start_matches("./"));
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    (!normalized.is_empty()).then_some(normalized)
}

fn looks_like_identifier(token: &str) -> bool {
    let trimmed = token.trim_matches(':');
    !trimmed.is_empty()
        && trimmed.split("::").all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

pub fn evidence_priority_plan(
    profile: ContextProfile,
    options: &ContextOptions,
) -> Result<EvidencePriorityPlan, AtlasError> {
    let mut limits = profile.limits();
    if let Some(max_bytes) = options.max_serialized_bytes {
        if !(MIN_CONTEXT_BYTES..=MAX_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(AtlasError::ContextLimit {
                detail: format!(
                    "max serialized bytes {max_bytes} is outside {MIN_CONTEXT_BYTES}..={MAX_CONTEXT_BYTES}"
                ),
            });
        }
        limits.max_serialized_bytes = max_bytes;
    }
    if let Some(min_score) = options.min_score {
        limits.relevance_threshold = min_score;
    }
    let mut class_order = vec![
        EvidenceClass::NamedSymbol,
        EvidenceClass::FailureEvidence,
        EvidenceClass::PrimarySpine,
        EvidenceClass::BoundarySite,
        EvidenceClass::UniqueImplementation,
    ];
    match profile {
        ContextProfile::Symbol => class_order.extend([
            EvidenceClass::RepresentativeImplementation,
            EvidenceClass::Relationship,
            EvidenceClass::AdjacentStructure,
            EvidenceClass::ImpactPath,
            EvidenceClass::AlternativePath,
            EvidenceClass::OffSpineSibling,
        ]),
        ContextProfile::Flow => class_order.extend([
            EvidenceClass::AlternativePath,
            EvidenceClass::RepresentativeImplementation,
            EvidenceClass::Relationship,
            EvidenceClass::AdjacentStructure,
            EvidenceClass::ImpactPath,
            EvidenceClass::OffSpineSibling,
        ]),
        ContextProfile::Architecture => class_order.extend([
            EvidenceClass::RepresentativeImplementation,
            EvidenceClass::Relationship,
            EvidenceClass::AdjacentStructure,
            EvidenceClass::AlternativePath,
            EvidenceClass::ImpactPath,
            EvidenceClass::OffSpineSibling,
        ]),
        ContextProfile::Impact => class_order.extend([
            EvidenceClass::ImpactPath,
            EvidenceClass::RepresentativeImplementation,
            EvidenceClass::Relationship,
            EvidenceClass::AdjacentStructure,
            EvidenceClass::AlternativePath,
            EvidenceClass::OffSpineSibling,
        ]),
    }
    Ok(EvidencePriorityPlan {
        profile,
        limits,
        class_order,
        tie_break: vec![
            "required-desc".into(),
            "class-rank-asc".into(),
            "score-desc".into(),
            "evidence-id-asc".into(),
        ],
    })
}

pub fn retrieve_context(
    code_root: &Path,
    graph_dir: &Path,
    intent: &QueryIntent,
    options: &ContextOptions,
) -> Result<RetrievalCandidateSet, AtlasError> {
    let (meta, index, status) = indexed_query_state(
        code_root,
        graph_dir,
        &QueryOptions {
            frozen: options.frozen,
        },
    )?;
    retrieve_context_index_controlled(
        code_root,
        &meta,
        &index,
        &status,
        intent,
        options,
        &ContextExecutionControl::unlimited(),
    )
}

pub fn pin_context_snapshot(graph_dir: &Path) -> Result<PinnedContextSnapshot, AtlasError> {
    let graph_root = canonical_graph_root(graph_dir)?;
    let snapshot = crate::generation::resolve_snapshot(&graph_root)?;
    let persisted = crate::read_persisted_meta_at(&snapshot.data_dir)?;
    let index_path = snapshot.data_dir.join("query-index.json");
    let estimated_index_bytes = std::fs::metadata(&index_path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AtlasError::QueryIndexMissing {
                    index_path: index_path.display().to_string(),
                }
            } else {
                AtlasError::Io(error.to_string())
            }
        })?
        .len();
    Ok(PinnedContextSnapshot {
        graph_root,
        snapshot,
        persisted,
        estimated_index_bytes,
    })
}

fn canonical_graph_root(graph_dir: &Path) -> Result<PathBuf, AtlasError> {
    std::fs::canonicalize(graph_dir).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AtlasError::MissingGraph {
                graph_dir: graph_dir.display().to_string(),
            }
        } else {
            AtlasError::Io(error.to_string())
        }
    })
}

pub fn retrieve_context_pinned(
    code_root: &Path,
    graph_dir: &Path,
    snapshot: &PinnedContextSnapshot,
    intent: &QueryIntent,
    options: &ContextOptions,
    control: &ContextExecutionControl,
) -> Result<RetrievalCandidateSet, AtlasError> {
    control.checkpoint()?;
    let requested_graph_root = canonical_graph_root(graph_dir)?;
    if requested_graph_root != snapshot.graph_root {
        return Err(AtlasError::Invariant(format!(
            "pinned context graph {} cannot serve request for {}",
            snapshot.graph_root.display(),
            requested_graph_root.display()
        )));
    }
    let status = crate::status::status_with_meta(
        code_root,
        &snapshot.graph_root,
        &snapshot.persisted,
        snapshot.snapshot.generation.clone(),
    )?;
    crate::status::require_worktree_match(&status)?;
    let index =
        crate::index::load_query_index_at(&snapshot.snapshot.data_dir, &snapshot.persisted.meta)?;
    control.checkpoint()?;
    if let Some(expected) = &options.expected_graph_fingerprint
        && expected != &snapshot.persisted.meta.graph_fingerprint
    {
        return Err(AtlasError::ContextGraphMismatch {
            expected: expected.clone(),
            found: snapshot.persisted.meta.graph_fingerprint.clone(),
        });
    }
    retrieve_context_index_controlled(
        code_root,
        &snapshot.persisted.meta,
        &index,
        &status,
        intent,
        options,
        control,
    )
}

#[cfg(test)]
fn retrieve_context_index(
    code_root: &Path,
    meta: &Meta,
    index: &QueryIndex,
    status: &AtlasStatus,
    intent: &QueryIntent,
    options: &ContextOptions,
) -> Result<RetrievalCandidateSet, AtlasError> {
    retrieve_context_index_controlled(
        code_root,
        meta,
        index,
        status,
        intent,
        options,
        &ContextExecutionControl::unlimited(),
    )
}

fn retrieve_context_index_controlled(
    code_root: &Path,
    meta: &Meta,
    index: &QueryIndex,
    status: &AtlasStatus,
    intent: &QueryIntent,
    options: &ContextOptions,
    control: &ContextExecutionControl,
) -> Result<RetrievalCandidateSet, AtlasError> {
    control.checkpoint()?;
    let plan = evidence_priority_plan(intent.profile, options)?;
    let mut candidates = BTreeMap::<String, EvidenceCandidate>::new();
    let mut seed_nodes = BTreeMap::<String, Node>::new();
    let mut diagnostics = intent
        .unrecognized
        .iter()
        .map(|token| ContextDiagnostic {
            code: "atlas-context-unrecognized-token".into(),
            message: format!("token `{token}` is not an identifier, repository path or relation"),
            file: None,
        })
        .collect::<Vec<_>>();

    for failure in &options.failure_evidence {
        control.checkpoint()?;
        let value = failure.trim();
        if value.is_empty() {
            continue;
        }
        insert_candidate(
            &mut candidates,
            candidate(
                EvidenceClass::FailureEvidence,
                1_200,
                true,
                vec!["explicit-failure-evidence".into()],
                None,
                None,
                None,
                None,
                Some(value.into()),
                None,
            ),
        );
    }

    for path in &intent.paths {
        control.checkpoint()?;
        for position in index.file.get(path).into_iter().flatten() {
            if let Some(node) = index.nodes.get(*position) {
                seed_nodes.insert(node.id.clone(), node.clone());
                insert_candidate(
                    &mut candidates,
                    node_candidate(node, EvidenceClass::NamedSymbol, 1_050, true, "exact-path"),
                );
            }
        }
    }
    for identifier in &intent.identifiers {
        control.checkpoint()?;
        let hits = index.search_nodes(identifier);
        let ambiguous = hits
            .iter()
            .filter(|hit| {
                matches!(
                    hit.match_kind,
                    MatchKind::ExactId | MatchKind::ExactSymbol | MatchKind::QualifiedSuffix
                )
            })
            .count();
        if ambiguous > 1 {
            diagnostics.push(ContextDiagnostic {
                code: "atlas-context-ambiguous-identifier".into(),
                message: format!(
                    "identifier `{identifier}` matches {ambiguous} exact or qualified declarations"
                ),
                file: None,
            });
        }
        for hit in hits {
            let exact = matches!(hit.match_kind, MatchKind::ExactId | MatchKind::ExactSymbol);
            let suffix = matches!(hit.match_kind, MatchKind::QualifiedSuffix);
            let score = match hit.match_kind {
                MatchKind::ExactId => 1_100,
                MatchKind::ExactSymbol => 1_075,
                MatchKind::CaseInsensitiveExact => 900,
                MatchKind::QualifiedSuffix => 850,
                MatchKind::SegmentedIdentifier => 550,
                MatchKind::NormalizedSubstring => 300,
            };
            if exact || suffix {
                seed_nodes.insert(hit.node.id.clone(), hit.node.clone());
            }
            insert_candidate(
                &mut candidates,
                node_candidate(
                    &hit.node,
                    EvidenceClass::NamedSymbol,
                    score,
                    exact || suffix,
                    match hit.match_kind {
                        MatchKind::ExactId => "exact-id",
                        MatchKind::ExactSymbol => "exact-symbol",
                        MatchKind::CaseInsensitiveExact => "case-insensitive-exact",
                        MatchKind::QualifiedSuffix => "qualified-suffix",
                        MatchKind::SegmentedIdentifier => "segmented-identifier",
                        MatchKind::NormalizedSubstring => "normalized-substring",
                    },
                ),
            );
        }
        control.checkpoint()?;
    }
    if seed_nodes.is_empty() {
        diagnostics.push(ContextDiagnostic {
            code: "atlas-context-no-match".into(),
            message: "no exact identifier or repository path established a retrieval seed".into(),
            file: None,
        });
    }

    for seed in seed_nodes.values() {
        collect_adjacent_candidates(index, seed, &mut candidates, control)?;
    }
    collect_implementation_candidates(index, seed_nodes.values(), &mut candidates, control)?;

    match intent.profile {
        ContextProfile::Flow => collect_flow_candidates(
            code_root,
            meta,
            index,
            status,
            intent,
            &plan,
            &mut candidates,
            &mut diagnostics,
            control,
        )?,
        ContextProfile::Impact => collect_impact_candidates(
            index,
            seed_nodes.values().cloned().collect::<Vec<_>>().as_slice(),
            &plan,
            options.frozen,
            &mut candidates,
            &mut diagnostics,
            control,
        )?,
        ContextProfile::Architecture => {
            collect_architecture_candidates(index, seed_nodes.values(), &mut candidates, control)?
        }
        ContextProfile::Symbol => {}
    }
    control.checkpoint()?;

    let total_candidates = candidates.len();
    let mut candidates = candidates.into_values().collect::<Vec<_>>();
    sort_candidates(&mut candidates, &plan);
    let after_cursor_omitted = match options.after.as_deref() {
        None | Some("START") => 0,
        Some(after) => candidates
            .iter()
            .position(|candidate| candidate.id == after)
            .map(|position| position + 1)
            .ok_or_else(|| AtlasError::ContextCursor {
                cursor: after.to_string(),
            })?,
    };
    let mut candidates = candidates
        .into_iter()
        .skip(after_cursor_omitted)
        .collect::<Vec<_>>();
    let eligible_candidates = candidates.len();
    let required_candidates = candidates
        .iter()
        .filter(|candidate| candidate.required)
        .count();
    if required_candidates > plan.limits.max_candidates {
        return Err(AtlasError::ContextRequiredCandidateCap {
            required: required_candidates,
            max: plan.limits.max_candidates,
        });
    }
    let hard_cap_omitted = candidates.len().saturating_sub(plan.limits.max_candidates);
    candidates.truncate(plan.limits.max_candidates);
    if hard_cap_omitted > 0 {
        diagnostics.push(ContextDiagnostic {
            code: "atlas-context-retrieval-truncated".into(),
            message: format!(
                "retrieval hard cap omitted {hard_cap_omitted} of {eligible_candidates} eligible candidates"
            ),
            file: None,
        });
    }
    control.checkpoint()?;
    canonicalize_diagnostics(&mut diagnostics);
    Ok(RetrievalCandidateSet {
        schema: RETRIEVAL_SCHEMA.into(),
        intent: intent.clone(),
        graph_fingerprint: meta.graph_fingerprint.clone(),
        total_candidates,
        eligible_candidates,
        after_cursor_omitted,
        hard_cap_omitted,
        candidates,
        diagnostics,
        status: status.clone(),
        recorded_files: meta.files.clone(),
    })
}

fn collect_adjacent_candidates(
    index: &QueryIndex,
    seed: &Node,
    candidates: &mut BTreeMap<String, EvidenceCandidate>,
    control: &ContextExecutionControl,
) -> Result<(), AtlasError> {
    for (node, edge) in index
        .incoming_neighbors_for(&seed.id)
        .chain(index.outgoing_neighbors_for(&seed.id))
    {
        control.checkpoint()?;
        insert_candidate(
            candidates,
            node_candidate(
                node,
                EvidenceClass::AdjacentStructure,
                600,
                false,
                "direct-adjacency",
            ),
        );
        insert_candidate(
            candidates,
            edge_candidate(
                edge,
                EvidenceClass::Relationship,
                650,
                false,
                "direct-relation",
            ),
        );
    }
    Ok(())
}

fn collect_implementation_candidates<'a>(
    index: &QueryIndex,
    seeds: impl Iterator<Item = &'a Node>,
    candidates: &mut BTreeMap<String, EvidenceCandidate>,
    control: &ContextExecutionControl,
) -> Result<(), AtlasError> {
    let mut implementations = BTreeMap::<String, Node>::new();
    for seed in seeds {
        control.checkpoint()?;
        for (node, edge) in index
            .incoming_neighbors_for(&seed.id)
            .chain(index.outgoing_neighbors_for(&seed.id))
        {
            control.checkpoint()?;
            if matches!(edge.kind, EdgeKind::ImplFor | EdgeKind::ImplsTrait)
                || node.kind == NodeKind::Impl
            {
                implementations.insert(node.id.clone(), node.clone());
            }
        }
    }
    let unique = implementations.len() == 1;
    for (position, node) in implementations.into_values().enumerate() {
        control.checkpoint()?;
        let (class, score, required, reason) = if unique {
            (
                EvidenceClass::UniqueImplementation,
                950,
                true,
                "unique-implementation",
            )
        } else if position == 0 {
            (
                EvidenceClass::RepresentativeImplementation,
                750,
                false,
                "representative-implementation",
            )
        } else {
            (
                EvidenceClass::OffSpineSibling,
                350,
                false,
                "off-spine-sibling",
            )
        };
        insert_candidate(
            candidates,
            node_candidate(&node, class, score, required, reason),
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_flow_candidates(
    code_root: &Path,
    meta: &Meta,
    index: &QueryIndex,
    status: &AtlasStatus,
    intent: &QueryIntent,
    plan: &EvidencePriorityPlan,
    candidates: &mut BTreeMap<String, EvidenceCandidate>,
    diagnostics: &mut Vec<ContextDiagnostic>,
    control: &ContextExecutionControl,
) -> Result<(), AtlasError> {
    control.checkpoint()?;
    let query = match intent.identifiers.as_slice() {
        [through] => FlowQuery::Through {
            symbol: through.clone(),
        },
        [from, to, ..] => FlowQuery::Between {
            from: from.clone(),
            to: to.clone(),
        },
        [] => return Ok(()),
    };
    let limits = TraversalLimits {
        max_depth: 8,
        max_expansions: 4_000,
        max_paths: plan.limits.max_paths,
    };
    let result = flow_index(
        index,
        query.clone(),
        FlowOptions {
            limits,
            frozen: false,
        },
        status,
    )?;
    control.checkpoint()?;
    let primary = result
        .highest_confidence
        .as_ref()
        .or(result.shortest.as_ref());
    if let Some(path) = primary {
        add_path_candidates(
            candidates,
            path,
            EvidenceClass::PrimarySpine,
            1_000,
            true,
            "primary-spine",
        );
    }
    let primary_signature = primary.map(path_identity);
    for path in result.alternatives.iter().take(plan.limits.max_paths) {
        control.checkpoint()?;
        if primary_signature.as_deref() == Some(path_identity(path).as_str()) {
            continue;
        }
        add_path_candidates(
            candidates,
            path,
            EvidenceClass::AlternativePath,
            675,
            false,
            "alternative-path",
        );
    }
    for diagnostic in result.diagnostics {
        control.checkpoint()?;
        diagnostics.push(ContextDiagnostic {
            code: diagnostic.code,
            message: diagnostic.message,
            file: None,
        });
    }
    if matches!(
        result.state,
        FlowState::NoPath | FlowState::CapabilityUnavailable
    ) {
        let projection = project_runtime_boundaries(code_root, meta, index, status, &query, limits);
        control.checkpoint()?;
        for hint in projection.hints {
            control.checkpoint()?;
            let span = EvidenceSpan {
                file: hint.site.file.clone(),
                line_start: hint.site.line_start,
                line_end: hint.site.line_end,
            };
            insert_candidate(
                candidates,
                candidate(
                    EvidenceClass::BoundarySite,
                    1_025,
                    true,
                    vec!["runtime-boundary-site".into()],
                    Some(hint.source.clone()),
                    None,
                    None,
                    Some(hint),
                    None,
                    Some(span),
                ),
            );
        }
        if projection.truncated {
            diagnostics.push(ContextDiagnostic {
                code: "atlas-context-runtime-boundary-truncated".into(),
                message: "runtime boundary retrieval reached a configured cap".into(),
                file: None,
            });
        }
    }
    Ok(())
}

fn add_path_candidates(
    candidates: &mut BTreeMap<String, EvidenceCandidate>,
    path: &GraphPath,
    class: EvidenceClass,
    score: u16,
    required: bool,
    reason: &str,
) {
    insert_candidate(
        candidates,
        candidate(
            class,
            score,
            required,
            vec![reason.into()],
            None,
            None,
            Some(path.clone()),
            None,
            None,
            None,
        ),
    );
    for node in &path.nodes {
        insert_candidate(
            candidates,
            node_candidate(node, class, score.saturating_sub(10), required, reason),
        );
    }
    for hop in &path.hops {
        insert_candidate(
            candidates,
            edge_candidate(&hop.edge, class, score.saturating_sub(5), required, reason),
        );
    }
}

fn collect_impact_candidates(
    index: &QueryIndex,
    seeds: &[Node],
    plan: &EvidencePriorityPlan,
    frozen: bool,
    candidates: &mut BTreeMap<String, EvidenceCandidate>,
    diagnostics: &mut Vec<ContextDiagnostic>,
    control: &ContextExecutionControl,
) -> Result<(), AtlasError> {
    if seeds.is_empty() {
        return Ok(());
    }
    let traversal = impact_many_index(
        index,
        seeds,
        &ImpactOptions {
            max_depth: 3,
            max_nodes: plan.limits.max_candidates.min(200),
            frozen,
        },
    )?;
    control.checkpoint()?;
    for entry in traversal.affected {
        control.checkpoint()?;
        let score = 900u16.saturating_sub((entry.distance as u16).saturating_mul(75));
        let source_span = EvidenceSpan {
            file: entry.node.file.clone(),
            line_start: entry.node.line_start,
            line_end: entry.node.line_end,
        };
        insert_candidate(
            candidates,
            candidate(
                EvidenceClass::ImpactPath,
                score,
                false,
                vec![format!("reverse-distance-{}", entry.distance)],
                Some(entry.node),
                None,
                Some(entry.path),
                None,
                None,
                Some(source_span),
            ),
        );
    }
    for diagnostic in traversal.diagnostics {
        control.checkpoint()?;
        diagnostics.push(ContextDiagnostic {
            code: diagnostic.code,
            message: diagnostic.message,
            file: None,
        });
    }
    if traversal.truncated {
        diagnostics.push(ContextDiagnostic {
            code: "atlas-context-impact-truncated".into(),
            message: "impact retrieval reached its node or depth cap".into(),
            file: None,
        });
    }
    Ok(())
}

fn collect_architecture_candidates<'a>(
    index: &QueryIndex,
    seeds: impl Iterator<Item = &'a Node>,
    candidates: &mut BTreeMap<String, EvidenceCandidate>,
    control: &ContextExecutionControl,
) -> Result<(), AtlasError> {
    let roots = seeds
        .filter_map(|node| node.symbol.split("::").next())
        .collect::<BTreeSet<_>>();
    for node in &index.nodes {
        control.checkpoint()?;
        if !matches!(node.kind, NodeKind::Crate | NodeKind::Module)
            || (!roots.is_empty()
                && !roots.iter().any(|root| {
                    node.symbol == *root || node.symbol.starts_with(&format!("{root}::"))
                }))
        {
            continue;
        }
        insert_candidate(
            candidates,
            node_candidate(
                node,
                EvidenceClass::AdjacentStructure,
                if node.kind == NodeKind::Crate {
                    700
                } else {
                    550
                },
                false,
                "architecture-module",
            ),
        );
    }
    Ok(())
}

fn node_candidate(
    node: &Node,
    class: EvidenceClass,
    score: u16,
    required: bool,
    reason: &str,
) -> EvidenceCandidate {
    candidate(
        class,
        score,
        required,
        vec![reason.into()],
        Some(node.clone()),
        None,
        None,
        None,
        None,
        Some(EvidenceSpan {
            file: node.file.clone(),
            line_start: node.line_start,
            line_end: node.line_end,
        }),
    )
}

fn edge_candidate(
    edge: &Edge,
    class: EvidenceClass,
    score: u16,
    required: bool,
    reason: &str,
) -> EvidenceCandidate {
    candidate(
        class,
        score,
        required,
        vec![reason.into()],
        None,
        Some(edge.clone()),
        None,
        None,
        None,
        edge.site.as_ref().map(|site| EvidenceSpan {
            file: site.file.clone(),
            line_start: site.line_start,
            line_end: site.line_end,
        }),
    )
}

#[allow(clippy::too_many_arguments)]
fn candidate(
    class: EvidenceClass,
    score: u16,
    required: bool,
    scoring_reasons: Vec<String>,
    node: Option<Node>,
    edge: Option<Edge>,
    path: Option<GraphPath>,
    runtime_boundary: Option<RuntimeBoundaryHint>,
    failure: Option<String>,
    source_span: Option<EvidenceSpan>,
) -> EvidenceCandidate {
    let identity = candidate_identity(
        class,
        node.as_ref(),
        edge.as_ref(),
        path.as_ref(),
        runtime_boundary.as_ref(),
        failure.as_deref(),
    );
    let digest = blake3::hash(identity.as_bytes()).to_hex().to_string();
    EvidenceCandidate {
        id: format!("ev-{}-{}", evidence_class_name(class), &digest[..16]),
        class,
        score,
        required,
        scoring_reasons,
        node,
        edge,
        path,
        runtime_boundary,
        failure,
        source_span,
    }
}

fn candidate_identity(
    class: EvidenceClass,
    node: Option<&Node>,
    edge: Option<&Edge>,
    path: Option<&GraphPath>,
    boundary: Option<&RuntimeBoundaryHint>,
    failure: Option<&str>,
) -> String {
    let node = node.map(|node| node.id.clone()).unwrap_or_default();
    let edge = edge
        .map(|edge| format!("{}|{:?}|{}|{:?}", edge.from, edge.kind, edge.to, edge.site))
        .unwrap_or_default();
    let path = path.map(path_identity).unwrap_or_default();
    let boundary = boundary
        .map(|hint| {
            format!(
                "{}|{}:{}:{}|{:?}",
                hint.source.id,
                hint.site.file,
                hint.site.line_start,
                hint.site.column_start,
                hint.mechanism
            )
        })
        .unwrap_or_default();
    format!(
        "{class:?}|{node}|{edge}|{path}|{boundary}|{}",
        failure.unwrap_or_default()
    )
}

fn path_identity(path: &GraphPath) -> String {
    path.nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>()
        .join("->")
}

fn evidence_class_name(class: EvidenceClass) -> &'static str {
    match class {
        EvidenceClass::NamedSymbol => "named",
        EvidenceClass::FailureEvidence => "failure",
        EvidenceClass::PrimarySpine => "spine",
        EvidenceClass::BoundarySite => "boundary",
        EvidenceClass::UniqueImplementation => "unique-impl",
        EvidenceClass::RepresentativeImplementation => "representative-impl",
        EvidenceClass::ImpactPath => "impact",
        EvidenceClass::AlternativePath => "alternative",
        EvidenceClass::Relationship => "relationship",
        EvidenceClass::AdjacentStructure => "adjacent",
        EvidenceClass::OffSpineSibling => "sibling",
    }
}

fn insert_candidate(
    candidates: &mut BTreeMap<String, EvidenceCandidate>,
    mut incoming: EvidenceCandidate,
) {
    match candidates.get_mut(&incoming.id) {
        Some(existing) => {
            existing.score = existing.score.max(incoming.score);
            existing.required |= incoming.required;
            existing
                .scoring_reasons
                .append(&mut incoming.scoring_reasons);
            existing.scoring_reasons.sort();
            existing.scoring_reasons.dedup();
        }
        None => {
            incoming.scoring_reasons.sort();
            incoming.scoring_reasons.dedup();
            candidates.insert(incoming.id.clone(), incoming);
        }
    }
}

fn sort_candidates(candidates: &mut [EvidenceCandidate], plan: &EvidencePriorityPlan) {
    let ranks = plan
        .class_order
        .iter()
        .enumerate()
        .map(|(rank, class)| (*class, rank))
        .collect::<BTreeMap<_, _>>();
    candidates.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| {
                ranks
                    .get(&left.class)
                    .unwrap_or(&usize::MAX)
                    .cmp(ranks.get(&right.class).unwrap_or(&usize::MAX))
            })
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub fn project_context(
    code_root: &Path,
    retrieval: &RetrievalCandidateSet,
    options: &ContextOptions,
) -> Result<ContextResult, AtlasError> {
    project_context_controlled(
        code_root,
        retrieval,
        options,
        &ContextExecutionControl::unlimited(),
    )
}

pub fn project_context_controlled(
    code_root: &Path,
    retrieval: &RetrievalCandidateSet,
    options: &ContextOptions,
    control: &ContextExecutionControl,
) -> Result<ContextResult, AtlasError> {
    control.checkpoint()?;
    let plan = evidence_priority_plan(retrieval.intent.profile, options)?;
    let threshold = plan.limits.relevance_threshold;
    let mut diagnostics = retrieval.diagnostics.clone();
    let mut omitted = Vec::<OmittedCandidate>::new();
    if retrieval.hard_cap_omitted > 0 {
        omitted.extend(
            (0..retrieval.hard_cap_omitted).map(|index| OmittedCandidate {
                class: EvidenceClass::OffSpineSibling,
                reason: OmissionReason::RetrievalCap,
                score: 0,
                id: format!("retrieval-hard-cap-{index}"),
            }),
        );
    }

    let mut above_relevance = 0;
    let mut projected = Vec::new();
    let mut source_slices = 0;
    let mut source_cache = BTreeMap::<EvidenceSpan, SourceSlice>::new();
    for candidate in &retrieval.candidates {
        control.checkpoint()?;
        if candidate.score < threshold && !candidate.required {
            omitted.push(omitted_candidate(candidate, OmissionReason::BelowRelevance));
            continue;
        }
        above_relevance += 1;
        let source_body_admitted = source_body_admitted(candidate, &retrieval.intent);
        let should_read_source = source_body_admitted
            && candidate.source_span.is_some()
            && (candidate.required || source_slices < plan.limits.max_source_slices);
        let source = if should_read_source {
            let Some(span) = candidate.source_span.as_ref() else {
                return Err(AtlasError::Invariant(
                    "context source-selection state lost its source span".into(),
                ));
            };
            if let Some(slice) = source_cache.get(span) {
                Some(slice.clone())
            } else if source_slices == plan.limits.max_source_slices && candidate.required {
                return Err(AtlasError::ContextRequiredSourceCap {
                    required: source_slices + 1,
                    max: plan.limits.max_source_slices,
                });
            } else if source_slices == plan.limits.max_source_slices {
                None
            } else {
                match source_slice(
                    code_root,
                    &retrieval.recorded_files,
                    span,
                    plan.limits.max_source_lines,
                ) {
                    Ok(slice) => {
                        control.checkpoint()?;
                        source_slices += 1;
                        source_cache.insert(span.clone(), slice.clone());
                        Some(slice)
                    }
                    Err(diagnostic) => {
                        omitted.push(omitted_candidate(
                            candidate,
                            OmissionReason::SourceUnavailable,
                        ));
                        diagnostics.push(diagnostic);
                        None
                    }
                }
            }
        } else {
            None
        };
        let signature_skeleton = candidate.source_span.is_some()
            && source.is_none()
            && (candidate.class == EvidenceClass::OffSpineSibling || !source_body_admitted);
        projected.push(ProjectedEvidence {
            id: candidate.id.clone(),
            class: candidate.class,
            score: candidate.score,
            required: candidate.required,
            scoring_reasons: candidate.scoring_reasons.clone(),
            node: candidate.node.clone(),
            edge: candidate.edge.clone(),
            path: candidate.path.clone(),
            runtime_boundary: candidate.runtime_boundary.clone(),
            failure: candidate.failure.clone(),
            source,
            signature_skeleton,
        });
    }
    canonicalize_diagnostics(&mut diagnostics);
    let initial_above_relevance = above_relevance;
    let mut result = context_result(
        retrieval,
        &plan,
        projected,
        omitted,
        diagnostics,
        initial_above_relevance,
        retrieval.after_cursor_omitted,
    );
    loop {
        control.checkpoint()?;
        refresh_context_receipt_controlled(&mut result, control)?;
        if result.receipt.serialized_bytes <= plan.limits.max_serialized_bytes {
            let mut represented = represented_files(&result);
            crate::status::scope_live_status(&mut result.status, std::mem::take(&mut represented));
            refresh_context_receipt_controlled(&mut result, control)?;
            if result.receipt.serialized_bytes <= plan.limits.max_serialized_bytes {
                control.checkpoint()?;
                return Ok(result);
            }
        }
        let Some(position) = result
            .projection
            .evidence
            .iter()
            .rposition(|evidence| !evidence.required)
        else {
            return Err(AtlasError::ContextRequiredBudget {
                required_bytes: result.receipt.serialized_bytes,
                max_bytes: plan.limits.max_serialized_bytes,
            });
        };
        let removed = result.projection.evidence.remove(position);
        if !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "atlas-context-byte-ceiling")
        {
            result.diagnostics.push(ContextDiagnostic {
                code: "atlas-context-byte-ceiling".into(),
                message: format!(
                    "optional evidence was omitted to fit the {} byte ceiling",
                    plan.limits.max_serialized_bytes
                ),
                file: None,
            });
            canonicalize_diagnostics(&mut result.diagnostics);
        }
        result.omissions = merge_omissions(
            retrieval,
            &plan,
            result
                .omissions
                .iter()
                .flat_map(expand_omission)
                .chain(std::iter::once(OmittedCandidate {
                    class: removed.class,
                    reason: OmissionReason::ByteCeiling,
                    score: removed.score,
                    id: removed.id,
                }))
                .collect(),
        );
    }
}

fn context_result(
    retrieval: &RetrievalCandidateSet,
    plan: &EvidencePriorityPlan,
    projected: Vec<ProjectedEvidence>,
    omitted: Vec<OmittedCandidate>,
    diagnostics: Vec<ContextDiagnostic>,
    above_relevance: usize,
    after_cursor_omitted: usize,
) -> ContextResult {
    let omissions = merge_omissions(retrieval, plan, omitted);
    let skeletonized = projected
        .iter()
        .filter(|evidence| evidence.signature_skeleton)
        .count();
    let policy_skeletonized = projected
        .iter()
        .filter(|evidence| {
            evidence.signature_skeleton
                && evidence
                    .source_span_file()
                    .is_some_and(is_restricted_source_path)
        })
        .count();
    let below_relevance_omitted = omission_count(&omissions, OmissionReason::BelowRelevance);
    let byte_omitted = omission_count(&omissions, OmissionReason::ByteCeiling);
    let retained = projected.len();
    let load_profile = load_profile(&projected);
    ContextResult {
        schema: CONTEXT_SCHEMA.into(),
        intent: retrieval.intent.clone(),
        priority_plan: plan.clone(),
        projection: ContextProjection {
            evidence: projected,
        },
        omissions,
        receipt: QueryReceipt {
            retrieval: RetrievalReceipt {
                total: retrieval.total_candidates,
                eligible: retrieval.eligible_candidates,
                returned: retrieval.candidates.len(),
                after_cursor_omitted: retrieval.after_cursor_omitted,
                hard_cap_omitted: retrieval.hard_cap_omitted,
                coverage_numerator: retrieval.candidates.len(),
                coverage_denominator: retrieval.eligible_candidates,
            },
            projection: ProjectionReceipt {
                above_relevance,
                retained,
                below_relevance_omitted,
                byte_omitted,
                after_cursor_omitted,
                skeletonized,
                policy_skeletonized,
                retention_numerator: retained,
                retention_denominator: above_relevance,
            },
            profile: retrieval.intent.profile,
            limits: plan.limits,
            serialized_bytes: 0,
            truncated_evidence_classes: Vec::new(),
            graph_fingerprint: retrieval.graph_fingerprint.clone(),
            read_back_required: false,
            follow_up_queries: 0,
            load_profile,
        },
        diagnostics,
        status: retrieval.status.clone(),
    }
}

fn refresh_context_receipt_controlled(
    result: &mut ContextResult,
    control: &ContextExecutionControl,
) -> Result<(), AtlasError> {
    result.receipt.projection.retained = result.projection.evidence.len();
    result.receipt.projection.retention_numerator = result.projection.evidence.len();
    result.receipt.projection.byte_omitted =
        omission_count(&result.omissions, OmissionReason::ByteCeiling);
    result.receipt.projection.skeletonized = result
        .projection
        .evidence
        .iter()
        .filter(|evidence| evidence.signature_skeleton)
        .count();
    result.receipt.projection.policy_skeletonized = result
        .projection
        .evidence
        .iter()
        .filter(|evidence| {
            evidence.signature_skeleton
                && evidence
                    .source_span_file()
                    .is_some_and(is_restricted_source_path)
        })
        .count();
    result.receipt.truncated_evidence_classes = result
        .omissions
        .iter()
        .map(|entry| entry.class)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    result.receipt.read_back_required = result.diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_str(),
            "atlas-context-stale-source"
                | "atlas-context-missing-source"
                | "atlas-context-invalid-source"
        )
    });
    result.receipt.follow_up_queries = result.omissions.len();
    result.receipt.load_profile = load_profile(&result.projection.evidence);
    for _ in 0..16 {
        control.checkpoint()?;
        let bytes = serde_json::to_vec(result)
            .map_err(|error| {
                AtlasError::Invariant(format!("context serialization failed: {error}"))
            })?
            .len();
        if result.receipt.serialized_bytes == bytes {
            return Ok(());
        }
        result.receipt.serialized_bytes = bytes;
    }
    Err(AtlasError::Invariant(
        "context serialized byte accounting did not converge".into(),
    ))
}

fn represented_files(result: &ContextResult) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for evidence in &result.projection.evidence {
        if let Some(node) = &evidence.node {
            files.insert(node.file.clone());
        }
        if let Some(edge) = &evidence.edge
            && let Some(site) = &edge.site
        {
            files.insert(site.file.clone());
        }
        if let Some(path) = &evidence.path {
            files.extend(path.nodes.iter().map(|node| node.file.clone()));
            files.extend(
                path.hops
                    .iter()
                    .filter_map(|hop| hop.edge.site.as_ref().map(|site| site.file.clone())),
            );
        }
        if let Some(boundary) = &evidence.runtime_boundary {
            files.insert(boundary.source.file.clone());
            files.insert(boundary.site.file.clone());
        }
        if let Some(source) = &evidence.source {
            files.insert(source.file.clone());
        }
    }
    files
}

impl ProjectedEvidence {
    fn source_span_file(&self) -> Option<&str> {
        self.node
            .as_ref()
            .map(|node| node.file.as_str())
            .or_else(|| {
                self.edge
                    .as_ref()
                    .and_then(|edge| edge.site.as_ref())
                    .map(|site| site.file.as_str())
            })
            .or_else(|| {
                self.runtime_boundary
                    .as_ref()
                    .map(|boundary| boundary.site.file.as_str())
            })
    }
}

fn source_body_admitted(candidate: &EvidenceCandidate, intent: &QueryIntent) -> bool {
    let Some(span) = candidate.source_span.as_ref() else {
        return true;
    };
    if !is_restricted_source_path(&span.file) {
        return true;
    }
    intent.paths.iter().any(|path| path == &span.file)
        || (candidate.class == EvidenceClass::NamedSymbol && candidate.required)
        || matches!(
            candidate.class,
            EvidenceClass::PrimarySpine | EvidenceClass::ImpactPath | EvidenceClass::BoundarySite
        )
}

fn is_restricted_source_path(file: &str) -> bool {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    let path = Path::new(&normalized);
    let restricted_component = path.components().any(|component| {
        let Component::Normal(value) = component else {
            return false;
        };
        matches!(
            value.to_str(),
            Some("test" | "tests" | "generated" | "gen" | "vendor" | "vendored" | "third_party")
        )
    });
    let restricted_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.ends_with("_test.rs")
                || name.ends_with(".generated.rs")
                || name.ends_with("_generated.rs")
        });
    restricted_component || restricted_name
}

fn source_slice(
    code_root: &Path,
    recorded_files: &BTreeMap<String, String>,
    span: &EvidenceSpan,
    max_lines: usize,
) -> Result<SourceSlice, ContextDiagnostic> {
    let normalized =
        normalize_affected_path(code_root, Path::new(&span.file)).map_err(|error| {
            source_diagnostic(
                "atlas-context-invalid-source",
                &span.file,
                format!("source path is unsafe: {error}"),
            )
        })?;
    let expected = recorded_files.get(&normalized).ok_or_else(|| {
        source_diagnostic(
            "atlas-context-missing-source",
            &span.file,
            "source file has no hash in the selected graph generation",
        )
    })?;
    let bytes = std::fs::read(code_root.join(&normalized)).map_err(|error| {
        source_diagnostic(
            "atlas-context-missing-source",
            &span.file,
            format!("cannot read source file: {error}"),
        )
    })?;
    let found = blake3::hash(&bytes).to_hex().to_string();
    if &found != expected {
        return Err(source_diagnostic(
            "atlas-context-stale-source",
            &span.file,
            "source bytes do not match the selected graph generation",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        source_diagnostic(
            "atlas-context-invalid-source",
            &span.file,
            format!("source file is not UTF-8: {error}"),
        )
    })?;
    let lines = text.lines().collect::<Vec<_>>();
    let (line_start, line_end) =
        span_window(lines.len(), span.line_start, span.line_end, max_lines);
    let text = if line_start == 0 {
        String::new()
    } else {
        lines[(line_start - 1)..line_end].join("\n")
    };
    Ok(SourceSlice {
        file: normalized,
        line_start,
        line_end,
        text,
        source_hash: expected.clone(),
    })
}

fn span_window(total_lines: usize, start: usize, end: usize, max_lines: usize) -> (usize, usize) {
    if total_lines == 0 || max_lines == 0 {
        return (0, 0);
    }
    let start = start.clamp(1, total_lines);
    let end = end
        .clamp(start, total_lines)
        .min(start.saturating_add(max_lines - 1));
    let required = end - start + 1;
    let before = ((max_lines.saturating_sub(required)) / 2).min(start - 1);
    let mut line_start = start - before;
    let line_end = (line_start + max_lines - 1).min(total_lines);
    line_start = line_start
        .saturating_sub(max_lines - (line_end - line_start + 1))
        .max(1);
    (line_start, line_end)
}

fn source_diagnostic(code: &str, file: &str, message: impl Into<String>) -> ContextDiagnostic {
    ContextDiagnostic {
        code: code.into(),
        message: message.into(),
        file: Some(file.into()),
    }
}

fn omitted_candidate(candidate: &EvidenceCandidate, reason: OmissionReason) -> OmittedCandidate {
    OmittedCandidate {
        class: candidate.class,
        reason,
        score: candidate.score,
        id: candidate.id.clone(),
    }
}

fn merge_omissions(
    retrieval: &RetrievalCandidateSet,
    plan: &EvidencePriorityPlan,
    omitted: Vec<OmittedCandidate>,
) -> Vec<OmissionEntry> {
    let mut grouped = BTreeMap::<(EvidenceClass, OmissionReason), Vec<OmittedCandidate>>::new();
    for entry in omitted {
        grouped
            .entry((entry.class, entry.reason))
            .or_default()
            .push(entry);
    }
    let mut result = grouped
        .into_iter()
        .filter_map(|((class, reason), mut entries)| {
            entries.sort_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then_with(|| left.id.cmp(&right.id))
            });
            let highest = entries.first()?;
            Some(OmissionEntry {
                class,
                reason,
                count: entries.len(),
                highest_score: highest.score,
                highest_candidate: highest.id.clone(),
                continuation: continuation_query(
                    retrieval,
                    plan,
                    if reason == OmissionReason::RetrievalCap {
                        retrieval
                            .candidates
                            .last()
                            .map(|candidate| candidate.id.as_str())
                    } else {
                        continuation_predecessor(retrieval, highest.id.as_str())
                    },
                    reason,
                ),
            })
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        left.reason
            .cmp(&right.reason)
            .then_with(|| left.class.cmp(&right.class))
    });
    result
}

fn continuation_predecessor<'a>(
    retrieval: &'a RetrievalCandidateSet,
    omitted_id: &str,
) -> Option<&'a str> {
    retrieval
        .candidates
        .iter()
        .position(|candidate| candidate.id == omitted_id)
        .and_then(|position| position.checked_sub(1))
        .and_then(|position| retrieval.candidates.get(position))
        .map(|candidate| candidate.id.as_str())
        .or(Some("START"))
}

fn expand_omission(entry: &OmissionEntry) -> impl Iterator<Item = OmittedCandidate> + '_ {
    (0..entry.count).map(|_| OmittedCandidate {
        class: entry.class,
        reason: entry.reason,
        score: entry.highest_score,
        id: entry.highest_candidate.clone(),
    })
}

fn continuation_query(
    retrieval: &RetrievalCandidateSet,
    plan: &EvidencePriorityPlan,
    after: Option<&str>,
    reason: OmissionReason,
) -> ContinuationQuery {
    let mut argv = vec![
        "agent-spec".into(),
        "atlas".into(),
        "context".into(),
        retrieval.intent.query.clone(),
        "--profile".into(),
        profile_name(retrieval.intent.profile).into(),
        "--expect-graph".into(),
        retrieval.graph_fingerprint.clone(),
    ];
    if let Some(after) = after {
        argv.extend(["--after".into(), after.into()]);
    }
    if reason == OmissionReason::BelowRelevance {
        argv.extend(["--min-score".into(), "0".into()]);
    }
    if reason == OmissionReason::ByteCeiling {
        argv.extend([
            "--max-bytes".into(),
            plan.limits
                .max_serialized_bytes
                .saturating_mul(2)
                .min(MAX_CONTEXT_BYTES)
                .to_string(),
        ]);
    }
    ContinuationQuery { argv }
}

fn profile_name(profile: ContextProfile) -> &'static str {
    match profile {
        ContextProfile::Symbol => "symbol",
        ContextProfile::Flow => "flow",
        ContextProfile::Architecture => "architecture",
        ContextProfile::Impact => "impact",
    }
}

fn omission_count(omissions: &[OmissionEntry], reason: OmissionReason) -> usize {
    omissions
        .iter()
        .filter(|entry| entry.reason == reason)
        .map(|entry| entry.count)
        .sum()
}

fn load_profile(evidence: &[ProjectedEvidence]) -> QueryLoadProfile {
    let paths = evidence.iter().filter(|entry| entry.path.is_some()).count();
    let sources = evidence
        .iter()
        .filter(|entry| entry.source.is_some())
        .count();
    match (paths >= 3, sources >= 4) {
        (true, true) => QueryLoadProfile::Mixed,
        (true, false) => QueryLoadProfile::Traversal,
        (false, true) => QueryLoadProfile::SourceHeavy,
        (false, false) => QueryLoadProfile::Light,
    }
}

fn canonicalize_diagnostics(diagnostics: &mut Vec<ContextDiagnostic>) {
    diagnostics.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    diagnostics.dedup();
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn compile_context(
    code_root: &Path,
    graph_dir: &Path,
    query: &str,
    options: &ContextOptions,
) -> Result<ContextResult, AtlasError> {
    if !options.frozen {
        crate::refresh(code_root, graph_dir, &QueryOptions { frozen: false })?;
    }
    let snapshot = pin_context_snapshot(graph_dir)?;
    let intent = parse_query_intent(query, options.profile);
    let control = ContextExecutionControl::unlimited();
    let retrieval =
        retrieve_context_pinned(code_root, graph_dir, &snapshot, &intent, options, &control)?;
    project_context_controlled(code_root, &retrieval, options, &control)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{
        Capability, EdgeConfidence, EdgeResolution, EdgeSite, GraphIdentity, LayerState,
        LayerStatus, PathConfidence, PathDirection, PathHop, Provenance, SCHEMA_VERSION,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rust-atlas-context-{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn node(id: &str, symbol: &str, kind: NodeKind, file: &str, line: usize) -> Node {
        Node {
            id: id.into(),
            symbol: symbol.into(),
            kind,
            file: file.into(),
            line_start: line,
            line_end: line,
            visibility: "pub".into(),
            signature: format!("pub fn {}()", symbol.rsplit("::").next().unwrap_or(symbol)),
            doc: None,
            cfg: None,
        }
    }

    fn edge(from: &str, to: &str, kind: EdgeKind, line: usize) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            target_text: Some(to.into()),
            resolution: EdgeResolution::Resolved,
            kind,
            provenance: Provenance::Scip,
            site: Some(EdgeSite {
                file: "src/lib.rs".into(),
                line_start: line,
                column_start: 1,
                line_end: line,
                column_end: 8,
            }),
            extractor: None,
            dispatch: None,
            confidence: Some(EdgeConfidence::Exact),
            candidates: Vec::new(),
            evidence: Some("fixture".into()),
            generic: false,
        }
    }

    fn fixture() -> (PathBuf, Meta, QueryIndex, AtlasStatus) {
        let code = temp_dir("fixture");
        fs::create_dir_all(code.join("src")).unwrap();
        let source = [
            "pub trait Handler {}",
            "pub struct Service;",
            "pub fn entry() { worker(); }",
            "pub fn worker() {}",
            "pub fn sibling() {}",
            "pub mod api {}",
        ]
        .join("\n");
        fs::write(code.join("src/lib.rs"), source.as_bytes()).unwrap();
        let nodes = vec![
            node("handler", "demo::Handler", NodeKind::Trait, "src/lib.rs", 1),
            node(
                "service",
                "demo::Service",
                NodeKind::Struct,
                "src/lib.rs",
                2,
            ),
            node("entry", "demo::entry", NodeKind::Fn, "src/lib.rs", 3),
            node("worker", "demo::worker", NodeKind::Fn, "src/lib.rs", 4),
            node("sibling", "demo::sibling", NodeKind::Fn, "src/lib.rs", 5),
            node("api", "demo::api", NodeKind::Module, "src/lib.rs", 6),
        ];
        let edges = vec![
            edge("entry", "worker", EdgeKind::Calls, 3),
            edge("entry", "sibling", EdgeKind::References, 3),
            edge("service", "handler", EdgeKind::ImplsTrait, 2),
            edge("api", "entry", EdgeKind::Contains, 6),
        ];
        let index = QueryIndex::from_test_parts("context-fixture", nodes, edges);
        let mut files = BTreeMap::new();
        files.insert(
            "src/lib.rs".into(),
            blake3::hash(source.as_bytes()).to_hex().to_string(),
        );
        let meta = Meta {
            schema_version: SCHEMA_VERSION,
            package: "demo".into(),
            packages: vec!["demo".into()],
            roots: vec!["src/lib.rs".into()],
            capability: Capability::default(),
            files,
            graph_fingerprint: "context-fixture".into(),
        };
        let identity = GraphIdentity {
            repository_root: code.display().to_string(),
            git_common_dir: None,
            worktree_root: code.display().to_string(),
            graph_root: code.join("graph").display().to_string(),
            toolchain: "test".into(),
        };
        let layer = LayerStatus {
            state: LayerState::Fresh,
            extractor: None,
            recorded_fingerprint: None,
            current_fingerprint: None,
            recorded_source_fingerprint: None,
            current_source_fingerprint: None,
            stale_files: Vec::new(),
            diagnostics: Vec::new(),
        };
        let status = AtlasStatus {
            live: crate::live::LiveRuntimeStatus::new(crate::live::LiveRuntimeState::Unavailable),
            generation: Some("g-context".into()),
            graph_fingerprint: "context-fixture".into(),
            recorded_identity: identity.clone(),
            current_identity: identity,
            worktree_mismatch: None,
            syn: layer.clone(),
            scip: layer.clone(),
            mir: LayerStatus {
                state: LayerState::Unavailable,
                ..layer
            },
        };
        (code, meta, index, status)
    }

    fn retrieval(profile: ContextProfile, query: &str) -> (PathBuf, RetrievalCandidateSet) {
        let (code, meta, index, status) = fixture();
        let intent = parse_query_intent(query, profile);
        let options = ContextOptions {
            profile,
            frozen: true,
            ..ContextOptions::default()
        };
        let retrieval =
            retrieve_context_index(&code, &meta, &index, &status, &intent, &options).unwrap();
        (code, retrieval)
    }

    #[test]
    fn test_atlas_context_intent_parses_identifiers_paths_relations_and_profiles() {
        let intent = parse_query_intent(
            "crate::api::serve src/api.rs calls crate::api::serve ???",
            ContextProfile::Flow,
        );
        assert_eq!(intent.profile, ContextProfile::Flow);
        assert_eq!(intent.identifiers, vec!["crate::api::serve"]);
        assert_eq!(intent.paths, vec!["src/api.rs"]);
        assert_eq!(intent.relations, vec![ContextRelation::Calls]);
        assert_eq!(intent.unrecognized, vec!["???"]);
    }

    #[test]
    fn test_atlas_context_profiles_have_deterministic_priority_and_limits() {
        let mut serialized = Vec::new();
        for profile in [
            ContextProfile::Symbol,
            ContextProfile::Flow,
            ContextProfile::Architecture,
            ContextProfile::Impact,
        ] {
            let options = ContextOptions {
                profile,
                ..ContextOptions::default()
            };
            let left = evidence_priority_plan(profile, &options).unwrap();
            let right = evidence_priority_plan(profile, &options).unwrap();
            assert_eq!(left, right);
            assert_eq!(left.class_order.first(), Some(&EvidenceClass::NamedSymbol));
            assert!(left.limits.max_serialized_bytes <= MAX_CONTEXT_BYTES);
            serialized.push(serde_json::to_vec(&left).unwrap());
        }
        assert_eq!(serialized.iter().collect::<BTreeSet<_>>().len(), 4);
    }

    #[test]
    fn test_atlas_context_retrieval_returns_scored_candidate_supergraph() {
        let (code, retrieval) = retrieval(ContextProfile::Flow, "demo::entry demo::worker");
        assert_eq!(retrieval.eligible_candidates, retrieval.candidates.len());
        assert!(retrieval.candidates.iter().all(|candidate| {
            candidate.id.starts_with("ev-")
                && candidate.score > 0
                && !candidate.scoring_reasons.is_empty()
        }));
        assert!(retrieval.candidates.iter().any(|candidate| {
            candidate.class == EvidenceClass::PrimarySpine && candidate.path.is_some()
        }));
        assert!(retrieval.candidates.iter().any(|candidate| {
            candidate.class == EvidenceClass::AlternativePath
                || candidate.class == EvidenceClass::AdjacentStructure
        }));
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_impact_projects_reverse_path_source() {
        let (code, retrieval) = retrieval(ContextProfile::Impact, "demo::worker");
        let result = project_context(
            &code,
            &retrieval,
            &ContextOptions {
                profile: ContextProfile::Impact,
                ..ContextOptions::default()
            },
        )
        .unwrap();
        assert!(result.projection.evidence.iter().any(|evidence| {
            evidence.class == EvidenceClass::ImpactPath
                && evidence
                    .node
                    .as_ref()
                    .is_some_and(|node| node.id == "entry")
                && evidence.source.is_some()
        }));
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_relevance_gate_precedes_byte_budget() {
        let (code, retrieval) = retrieval(ContextProfile::Symbol, "demo::entry");
        let options = ContextOptions {
            profile: ContextProfile::Symbol,
            min_score: Some(700),
            max_serialized_bytes: Some(15_000),
            ..ContextOptions::default()
        };
        let result = project_context(&code, &retrieval, &options).unwrap();
        assert!(result.receipt.serialized_bytes < result.receipt.limits.max_serialized_bytes);
        assert!(result.receipt.projection.below_relevance_omitted > 0);
        assert!(
            result
                .omissions
                .iter()
                .any(|entry| entry.reason == OmissionReason::BelowRelevance)
        );
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_projects_verified_symbol_and_edge_site_spans() {
        let (code, retrieval) = retrieval(ContextProfile::Flow, "demo::entry demo::worker");
        let result = project_context(
            &code,
            &retrieval,
            &ContextOptions {
                profile: ContextProfile::Flow,
                ..ContextOptions::default()
            },
        )
        .unwrap();
        let sources = result
            .projection
            .evidence
            .iter()
            .filter_map(|evidence| evidence.source.as_ref())
            .collect::<Vec<_>>();
        assert!(!sources.is_empty());
        assert!(sources.iter().all(|source| {
            source.file == "src/lib.rs"
                && source.line_start >= 1
                && source.line_end <= 6
                && source.source_hash == retrieval.recorded_files["src/lib.rs"]
        }));
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_restricted_source_requires_name_or_spine() {
        let (code, mut meta, _, status) = fixture();
        let restricted = [
            ("tests/integration.rs", "demo::test_helper"),
            ("generated/adapter.rs", "demo::generated_adapter"),
            ("vendor/dependency.rs", "demo::vendor_dependency"),
        ];
        for &(file, _) in &restricted {
            let path = code.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let source = format!("pub fn {}() {{}}\n", file.replace(['/', '.'], "_"));
            fs::write(&path, source.as_bytes()).unwrap();
            meta.files.insert(
                file.into(),
                blake3::hash(source.as_bytes()).to_hex().to_string(),
            );
        }
        let nodes = vec![
            node("entry", "demo::entry", NodeKind::Fn, "src/lib.rs", 3),
            node("worker", "demo::worker", NodeKind::Fn, "src/lib.rs", 4),
            node(
                "test-helper",
                "demo::test_helper",
                NodeKind::Fn,
                "tests/integration.rs",
                1,
            ),
            node(
                "generated-adapter",
                "demo::generated_adapter",
                NodeKind::Fn,
                "generated/adapter.rs",
                1,
            ),
            node(
                "vendor-dependency",
                "demo::vendor_dependency",
                NodeKind::Fn,
                "vendor/dependency.rs",
                1,
            ),
        ];
        let edges = vec![
            edge("entry", "test-helper", EdgeKind::Calls, 3),
            edge("test-helper", "worker", EdgeKind::Calls, 1),
            edge("entry", "generated-adapter", EdgeKind::References, 3),
            edge("entry", "vendor-dependency", EdgeKind::References, 3),
        ];
        let index = QueryIndex::from_test_parts("context-fixture", nodes, edges);

        let symbol_intent = parse_query_intent("demo::entry", ContextProfile::Symbol);
        let symbol_options = ContextOptions::default();
        let symbol_retrieval = retrieve_context_index(
            &code,
            &meta,
            &index,
            &status,
            &symbol_intent,
            &symbol_options,
        )
        .unwrap();
        let symbol = project_context(&code, &symbol_retrieval, &symbol_options).unwrap();
        let incidental = symbol
            .projection
            .evidence
            .iter()
            .filter(|evidence| {
                evidence.node.as_ref().is_some_and(|node| {
                    restricted
                        .iter()
                        .any(|(file, _)| node.file.as_str() == *file)
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(incidental.len(), 3);
        assert!(
            incidental
                .iter()
                .all(|evidence| evidence.source.is_none() && evidence.signature_skeleton)
        );
        assert_eq!(symbol.receipt.projection.policy_skeletonized, 3);

        let path_intent = parse_query_intent("tests/integration.rs", ContextProfile::Symbol);
        let path_retrieval =
            retrieve_context_index(&code, &meta, &index, &status, &path_intent, &symbol_options)
                .unwrap();
        let named = project_context(&code, &path_retrieval, &symbol_options).unwrap();
        assert!(named.projection.evidence.iter().any(|evidence| {
            evidence
                .node
                .as_ref()
                .is_some_and(|node| node.id == "test-helper")
                && evidence.source.is_some()
                && !evidence.signature_skeleton
        }));

        let flow_intent = parse_query_intent("demo::entry demo::worker", ContextProfile::Flow);
        let flow_options = ContextOptions {
            profile: ContextProfile::Flow,
            ..ContextOptions::default()
        };
        let flow_retrieval =
            retrieve_context_index(&code, &meta, &index, &status, &flow_intent, &flow_options)
                .unwrap();
        let flow = project_context(&code, &flow_retrieval, &flow_options).unwrap();
        assert!(flow.projection.evidence.iter().any(|evidence| {
            evidence.class == EvidenceClass::PrimarySpine
                && evidence
                    .node
                    .as_ref()
                    .is_some_and(|node| node.id == "test-helper")
                && evidence.source.is_some()
        }));
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_stale_source_is_typed_and_never_projected() {
        let (code, retrieval) = retrieval(ContextProfile::Symbol, "demo::entry");
        fs::write(code.join("src/lib.rs"), "pub fn changed() {}\n").unwrap();
        let result = project_context(&code, &retrieval, &ContextOptions::default()).unwrap();
        assert!(
            result
                .projection
                .evidence
                .iter()
                .all(|evidence| evidence.source.is_none())
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "atlas-context-stale-source" })
        );
        assert!(result.receipt.read_back_required);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_byte_pruning_preserves_required_evidence() {
        let (code, retrieval) = retrieval(ContextProfile::Flow, "demo::entry demo::worker");
        let required = retrieval
            .candidates
            .iter()
            .filter(|candidate| candidate.required)
            .map(|candidate| candidate.id.clone())
            .collect::<BTreeSet<_>>();
        let result = project_context(
            &code,
            &retrieval,
            &ContextOptions {
                profile: ContextProfile::Flow,
                max_serialized_bytes: Some(12_000),
                ..ContextOptions::default()
            },
        )
        .unwrap();
        let retained = result
            .projection
            .evidence
            .iter()
            .map(|evidence| evidence.id.clone())
            .collect::<BTreeSet<_>>();
        assert!(required.is_subset(&retained));
        assert!(result.receipt.serialized_bytes <= 12_000);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_required_evidence_overflow_is_typed() {
        let (code, retrieval) = retrieval(ContextProfile::Flow, "demo::entry demo::worker");
        let error = project_context(
            &code,
            &retrieval,
            &ContextOptions {
                profile: ContextProfile::Flow,
                max_serialized_bytes: Some(MIN_CONTEXT_BYTES),
                ..ContextOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, AtlasError::ContextRequiredBudget { .. }));
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_omission_manifest_has_stable_continuations() {
        let (code, retrieval) = retrieval(ContextProfile::Symbol, "demo::entry");
        let options = ContextOptions {
            profile: ContextProfile::Symbol,
            min_score: Some(700),
            ..ContextOptions::default()
        };
        let left = project_context(&code, &retrieval, &options).unwrap();
        let right = project_context(&code, &retrieval, &options).unwrap();
        assert_eq!(left.omissions, right.omissions);
        assert!(left.omissions.iter().all(|entry| {
            entry.count > 0
                && !entry.highest_candidate.is_empty()
                && entry.continuation.argv.contains(&"--expect-graph".into())
                && entry.continuation.argv.contains(&"--after".into())
        }));
        let omission = left
            .omissions
            .iter()
            .find(|entry| entry.reason == OmissionReason::BelowRelevance)
            .unwrap();
        let after = omission
            .continuation
            .argv
            .windows(2)
            .find(|pair| pair[0] == "--after")
            .map(|pair| pair[1].clone())
            .unwrap();
        let continued = project_context(
            &code,
            &retrieval,
            &ContextOptions {
                profile: ContextProfile::Symbol,
                min_score: Some(0),
                after: Some(after),
                expected_graph_fingerprint: Some(retrieval.graph_fingerprint.clone()),
                ..ContextOptions::default()
            },
        )
        .unwrap();
        assert!(
            continued
                .projection
                .evidence
                .iter()
                .any(|evidence| evidence.id == omission.highest_candidate)
        );
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_continuation_rejects_graph_fingerprint_change() {
        let code = temp_dir("fingerprint-mismatch");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname='context-mismatch'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn entry() {}\n").unwrap();
        let graph = code.join("graph");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();
        let options = ContextOptions {
            expected_graph_fingerprint: Some("old-fingerprint".into()),
            ..ContextOptions::default()
        };
        let error = compile_context(&code, &graph, "entry", &options).unwrap_err();
        assert!(matches!(error, AtlasError::ContextGraphMismatch { .. }));
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_pinned_session_survives_writer_publish() {
        let code = temp_dir("pinned-session");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname='pinned-context'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn entry() {}\n").unwrap();
        let graph = code.join("graph");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();

        let pinned = pin_context_snapshot(&graph).unwrap();
        let generation_a = pinned.generation().unwrap().to_string();
        let graph_a = pinned.graph_fingerprint().to_string();
        fs::write(
            code.join("src/lib.rs"),
            "pub fn entry() {}\npub fn later() {}\n",
        )
        .unwrap();
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();
        let current = crate::graph_snapshot(&graph).unwrap();
        assert_ne!(current.generation.as_deref(), Some(generation_a.as_str()));

        let options = ContextOptions {
            frozen: true,
            ..ContextOptions::default()
        };
        let intent = parse_query_intent("entry", ContextProfile::Symbol);
        let control = ContextExecutionControl::unlimited();
        let retrieval =
            retrieve_context_pinned(&code, &graph, &pinned, &intent, &options, &control).unwrap();
        let result = project_context_controlled(&code, &retrieval, &options, &control).unwrap();
        assert_eq!(result.receipt.graph_fingerprint, graph_a);
        assert!(pinned.snapshot.data_dir.is_dir());

        let generation_a_dir = pinned.snapshot.data_dir.clone();
        drop(pinned);
        let writer = crate::locking::WriterLease::try_acquire(&graph).unwrap();
        crate::generation::safe_reclaim(&graph, &writer).unwrap();
        assert!(!generation_a_dir.exists());
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_control_cancels_projection_at_checkpoint() {
        let (code, retrieval) = retrieval(ContextProfile::Symbol, "entry");
        let control = ContextExecutionControl::unlimited();
        control.cancel();

        let error =
            project_context_controlled(&code, &retrieval, &ContextOptions::default(), &control)
                .unwrap_err();

        assert!(matches!(error, AtlasError::QueryCancelled));
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_retrieval_reports_ambiguous_and_no_match() {
        let (code, mut meta, _, status) = fixture();
        let nodes = vec![
            node("left", "demo::left::run", NodeKind::Fn, "src/lib.rs", 3),
            node("right", "demo::right::run", NodeKind::Fn, "src/lib.rs", 4),
        ];
        let index = QueryIndex::from_test_parts("context-fixture", nodes, Vec::new());
        meta.graph_fingerprint = "context-fixture".into();
        let options = ContextOptions::default();
        let ambiguous = retrieve_context_index(
            &code,
            &meta,
            &index,
            &status,
            &parse_query_intent("run", ContextProfile::Symbol),
            &options,
        )
        .unwrap();
        assert_eq!(
            ambiguous
                .candidates
                .iter()
                .filter(|candidate| candidate.required)
                .count(),
            2
        );
        assert!(
            ambiguous
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "atlas-context-ambiguous-identifier" })
        );
        let missing = retrieve_context_index(
            &code,
            &meta,
            &index,
            &status,
            &parse_query_intent("missing", ContextProfile::Symbol),
            &options,
        )
        .unwrap();
        assert!(
            missing
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "atlas-context-no-match")
        );
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_retrieval_hard_cap_continuation_is_stable() {
        let (code, mut meta, _, status) = fixture();
        let nodes = (0..300)
            .map(|index| {
                node(
                    &format!("item-{index:03}"),
                    &format!("demo::item_{index:03}"),
                    NodeKind::Fn,
                    "src/lib.rs",
                    3,
                )
            })
            .collect::<Vec<_>>();
        let index = QueryIndex::from_test_parts("context-fixture", nodes, Vec::new());
        meta.graph_fingerprint = "context-fixture".into();
        let intent = parse_query_intent("item", ContextProfile::Symbol);
        let options = ContextOptions::default();
        let first =
            retrieve_context_index(&code, &meta, &index, &status, &intent, &options).unwrap();
        assert_eq!(first.total_candidates, 300);
        assert_eq!(first.candidates.len(), 256);
        assert_eq!(first.hard_cap_omitted, 44);
        let cursor = first.candidates.last().unwrap().id.clone();
        let second = retrieve_context_index(
            &code,
            &meta,
            &index,
            &status,
            &intent,
            &ContextOptions {
                after: Some(cursor),
                expected_graph_fingerprint: Some("context-fixture".into()),
                ..ContextOptions::default()
            },
        )
        .unwrap();
        assert_eq!(second.after_cursor_omitted, 256);
        assert_eq!(second.candidates.len(), 44);
        assert_eq!(second.hard_cap_omitted, 0);
        let first_ids = first
            .candidates
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(
            second
                .candidates
                .iter()
                .all(|candidate| !first_ids.contains(candidate.id.as_str()))
        );
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn test_atlas_context_receipt_separates_retrieval_and_projection_loss() {
        let (code, retrieval) = retrieval(ContextProfile::Symbol, "demo::entry");
        let result = project_context(
            &code,
            &retrieval,
            &ContextOptions {
                min_score: Some(700),
                ..ContextOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            result.receipt.retrieval.eligible,
            retrieval.eligible_candidates
        );
        assert_eq!(
            result.receipt.retrieval.returned,
            retrieval.candidates.len()
        );
        assert!(result.receipt.projection.retained < result.receipt.retrieval.returned);
        assert_eq!(
            result.receipt.serialized_bytes,
            serde_json::to_vec(&result).unwrap().len()
        );
        assert!(result.receipt.follow_up_queries > 0);
        fs::remove_dir_all(code).ok();
    }

    #[test]
    fn path_payload_types_remain_serializable() {
        let path = GraphPath {
            nodes: vec![node("a", "demo::a", NodeKind::Fn, "src/lib.rs", 1)],
            hops: vec![PathHop {
                edge: edge("a", "b", EdgeKind::Calls, 1),
                chosen_target: "b".into(),
                candidate: false,
                direction: PathDirection::Forward,
            }],
            confidence: PathConfidence::Exact,
        };
        assert!(!serde_json::to_vec(&path).unwrap().is_empty());
    }
}
