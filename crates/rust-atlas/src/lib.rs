//! rust-atlas: an incrementally invalidated project graph for Rust code.
//!
//! One schema serves all phases: nodes are symbols, edges carry a
//! `provenance` (`syn` | `scip` | `mir`), and the graph persists as
//! per-source-file JSON shards under a graph directory with blake3 content
//! hashes for staleness detection. The syn layer is the stable-toolchain
//! baseline; a SCIP index (rust-analyzer, JSON form) optionally overlays
//! resolved cross-file `references` edges. Extraction is read-only over the
//! analyzed code and performs no network or LLM calls.

#![warn(clippy::all)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use quote::ToTokens;
use serde::{Deserialize, Serialize};

mod affected;
mod context;
mod dynamic_dispatch;
mod explore;
mod flow;
mod generation;
mod impact;
mod incremental;
mod index;
mod input_plan;
pub mod live;
pub mod locking;
mod mir;
mod runtime_boundary;
pub mod scope;
mod status;
pub mod sync;
mod traversal;
pub mod watch;

pub use affected::{
    AffectedDiagnostic, AffectedOptions, AffectedResult, AffectedSeed, affected_paths,
};
pub use context::{
    ContextDiagnostic, ContextLimits, ContextOptions, ContextProfile, ContextProjection,
    ContextRelation, ContextResult, ContinuationQuery, EvidenceCandidate, EvidenceClass,
    EvidencePriorityPlan, EvidenceSpan, OmissionEntry, OmissionReason, ProjectedEvidence,
    ProjectionReceipt, QueryIntent, QueryLoadProfile, QueryReceipt, RetrievalCandidateSet,
    RetrievalReceipt, SourceSlice, compile_context, evidence_priority_plan, parse_query_intent,
    project_context, retrieve_context,
};
pub use explore::{
    BudgetUsage, ExploreBudget, ExploreDiagnostic, ExploreNode, ExploreOptions, ExploreProfile,
    ExploreResult, SourceExcerpt, explore,
};
pub use flow::{FlowDiagnostic, FlowEndpoint, FlowOptions, FlowQuery, FlowResult, flow};
pub use generation::GraphSnapshot;
pub use impact::{ImpactDiagnostic, ImpactEntry, ImpactOptions, ImpactResult, impact};
use index::write_json_atomic;
pub use index::{QueryIndex, canonical_graph_fingerprint, load_query_index, rebuild_query_index};
pub use input_plan::InputPlanState;
pub use runtime_boundary::{
    RuntimeBoundaryAuthority, RuntimeBoundaryHint, RuntimeBoundaryMechanism,
};
pub use status::{AtlasStatus, GraphIdentity, LayerState, LayerStatus, require_authority, status};
pub use traversal::{
    FlowState, GraphPath, PathConfidence, PathDirection, PathHop, TraversalLimits,
};

pub const SCHEMA_VERSION: u32 = 6;

/// Resolve the immutable graph snapshot selected by `CURRENT.json`.
///
/// Legacy graphs without a generation pointer resolve to the graph root with
/// `generation: None` until the next successful build migrates them.
pub fn graph_snapshot(graph_dir: &Path) -> Result<GraphSnapshot, AtlasError> {
    generation::resolve_snapshot(graph_dir)
}

#[derive(Debug, thiserror::Error)]
pub enum AtlasError {
    #[error("atlas-missing-graph: no graph at {graph_dir}; run `atlas build` first")]
    MissingGraph { graph_dir: String },
    #[error("atlas-unknown-symbol: `{symbol}` is not in the graph")]
    UnknownSymbol { symbol: String },
    #[error("atlas-ambiguous-symbol: `{symbol}` has {declarations} declarations; query by node id")]
    AmbiguousSymbol { symbol: String, declarations: usize },
    #[error("atlas-io: {0}")]
    Io(String),
    #[error("atlas-scip: {0}")]
    Scip(String),
    #[error("atlas-cargo: {0}")]
    Cargo(String),
    #[error("atlas-invariant: {0}")]
    Invariant(String),
    #[error(
        "atlas-schema-mismatch: graph schema v{found} != binary v{expected}; \
         the graph was built by a different atlas version — rebuild with `atlas build`"
    )]
    SchemaMismatch { found: u32, expected: u32 },
    #[error(
        "atlas-worktree-mismatch: graph was built in {recorded}; current worktree is {current}"
    )]
    WorktreeMismatch { recorded: String, current: String },
    #[error("atlas-stale: {detail}")]
    StaleAuthority { detail: String },
    #[error(
        "atlas-query-index-missing: no query index at {index_path}; \
         rebuild with `atlas build`"
    )]
    QueryIndexMissing { index_path: String },
    #[error(
        "atlas-query-index-schema: query index schema v{found} != binary v{expected}; \
         rebuild with `atlas build`"
    )]
    QueryIndexSchema { found: u32, expected: u32 },
    #[error(
        "atlas-query-index-stale: query index fingerprint {found} != graph fingerprint {expected}; \
         rebuild with `atlas build`"
    )]
    QueryIndexStale { found: String, expected: String },
    #[error("atlas-query-index-corrupt: {detail}; rebuild with `atlas build`")]
    QueryIndexCorrupt { detail: String },
    #[error("atlas-search-limit: {limit} is outside the supported range 1..=200")]
    SearchLimit { limit: usize },
    #[error("atlas-traversal-limit: {detail}")]
    TraversalLimit { detail: String },
    #[error("atlas-cancelled: build cancelled before publication")]
    Cancelled,
    #[error("atlas-resource-limit: {resource} requires {required} bytes, limit is {limit}")]
    ResourceLimit {
        resource: String,
        required: usize,
        limit: usize,
    },
    #[error("atlas-orphan-queue: {detail}")]
    OrphanQueue { detail: String },
    #[error("atlas-live-state: {detail}")]
    LiveState { detail: String },
    #[error("atlas-writer-busy: graph writer is already active for {graph_root}")]
    WriterBusy { graph_root: String },
    #[error("atlas-watcher: {detail}")]
    Watcher { detail: String },
    #[error("atlas-daemon-busy: daemon is already active for {graph_root}")]
    DaemonBusy { graph_root: String },
    #[error("atlas-affected-path: `{path}`: {detail}")]
    AffectedPath { path: String, detail: String },
    #[error(
        "atlas-explore-budget: required payload is {required_bytes} bytes, profile limit is {max_bytes}"
    )]
    ExploreBudget {
        required_bytes: usize,
        max_bytes: usize,
    },
    #[error(
        "atlas-explore-budget: required {resource} count is {required}, profile limit is {max}"
    )]
    ExploreCountBudget {
        resource: String,
        required: usize,
        max: usize,
    },
    #[error("atlas-context-limit: {detail}")]
    ContextLimit { detail: String },
    #[error(
        "atlas-context-graph-mismatch: continuation expects graph {expected}, current graph is {found}"
    )]
    ContextGraphMismatch { expected: String, found: String },
    #[error("atlas-context-cursor: evidence id `{cursor}` is not in the selected candidate set")]
    ContextCursor { cursor: String },
    #[error(
        "atlas-context-required-budget: required evidence needs {required_bytes} bytes, limit is {max_bytes}"
    )]
    ContextRequiredBudget {
        required_bytes: usize,
        max_bytes: usize,
    },
    #[error(
        "atlas-context-required-candidate-cap: required evidence count {required} exceeds retrieval cap {max}"
    )]
    ContextRequiredCandidateCap { required: usize, max: usize },
    #[error(
        "atlas-context-required-source-cap: required source span count {required} exceeds profile cap {max}"
    )]
    ContextRequiredSourceCap { required: usize, max: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    Crate,
    Module,
    Struct,
    Enum,
    Union,
    Trait,
    TraitAlias,
    Fn,
    Impl,
    TypeAlias,
    Const,
    Static,
    Macro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    Contains,
    ImplsTrait,
    ImplFor,
    References,
    Calls,
    UsesType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    Syn,
    Scip,
    Mir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeResolution {
    Resolved,
    Unresolved,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EdgeSite {
    pub file: String,
    pub line_start: usize,
    pub column_start: usize,
    pub line_end: usize,
    pub column_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExtractorIdentity {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchKind {
    Static,
    Trait,
    Generic,
    Closure,
    FunctionPointer,
    Channel,
    Macro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeConfidence {
    Exact,
    BoundedCandidates,
    Heuristic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub symbol: String,
    pub kind: NodeKind,
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub visibility: String,
    pub signature: String,
    pub doc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfg: Option<CfgSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CfgSummary {
    pub basic_blocks: usize,
    pub edges: usize,
    pub exits: usize,
    pub loop_headers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub target_text: Option<String>,
    pub resolution: EdgeResolution,
    pub kind: EdgeKind,
    pub provenance: Provenance,
    #[serde(default)]
    pub site: Option<EdgeSite>,
    #[serde(default)]
    pub extractor: Option<ExtractorIdentity>,
    #[serde(default)]
    pub dispatch: Option<DispatchKind>,
    #[serde(default)]
    pub confidence: Option<EdgeConfidence>,
    #[serde(default)]
    pub candidates: Vec<String>,
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub generic: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shard {
    pub file: String,
    pub hash: String,
    pub unparsed: Option<String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub scip: bool,
    pub scip_tool: Option<String>,
    /// Absolute path of the SCIP index last overlaid, so an incremental
    /// `refresh` can re-overlay instead of purging the semantic layer.
    #[serde(default)]
    pub scip_index: Option<String>,
    /// blake3 of that index file at overlay time (staleness signal).
    #[serde(default)]
    pub scip_fingerprint: Option<String>,
    /// Source-set fingerprint captured when the current SCIP index was
    /// explicitly overlaid. Automatic refreshes intentionally retain it.
    pub scip_source_fingerprint: Option<String>,
    /// Whether a MIR overlay was applied to this graph snapshot.
    #[serde(default)]
    pub mir: bool,
    /// Extractor identity declared by the MIR overlay producer.
    #[serde(default)]
    pub mir_tool: Option<String>,
    /// Absolute path of the MIR overlay last consumed.
    #[serde(default)]
    pub mir_overlay: Option<String>,
    /// blake3 of the MIR overlay artifact at overlay time.
    #[serde(default)]
    pub mir_fingerprint: Option<String>,
    /// Source-set fingerprint declared by the current MIR overlay.
    #[serde(default)]
    pub mir_source_fingerprint: Option<String>,
    /// Whether the bounded whole-graph dynamic-dispatch pass was enabled.
    #[serde(default)]
    pub dynamic_dispatch: bool,
    /// Number of inferred bounded-candidate edges in this graph snapshot.
    #[serde(default)]
    pub dynamic_dispatch_edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub schema_version: u32,
    pub package: String,
    pub packages: Vec<String>,
    pub roots: Vec<String>,
    pub capability: Capability,
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub graph_fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub full: bool,
    pub scip_index: Option<PathBuf>,
    pub dynamic_dispatch: bool,
    pub features: Vec<String>,
    pub target: Option<String>,
    pub cfg: Vec<String>,
    pub frontier_limit: usize,
    pub batch_size: usize,
    pub working_byte_limit: usize,
    pub cancellation: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            full: false,
            scip_index: None,
            dynamic_dispatch: false,
            features: Vec::new(),
            target: None,
            cfg: Vec::new(),
            frontier_limit: 2048,
            batch_size: 256,
            working_byte_limit: 512 * 1024 * 1024,
            cancellation: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MirBuildOptions {
    pub overlay: Option<PathBuf>,
    pub driver: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildReport {
    pub generation: String,
    pub input_plan: InputPlanState,
    pub rebuilt: Vec<String>,
    pub removed: Vec<String>,
    pub unparsed: Vec<String>,
    pub capability: Capability,
    pub diagnostics: Vec<BuildDiagnostic>,
    pub touched_shards: Vec<String>,
    pub resolved_shards: usize,
    pub validated_shards: usize,
    pub resolved_edge_delta: isize,
    pub unresolved_edge_delta: isize,
    pub working_bytes: usize,
    pub fallback_reason: Option<String>,
    pub orphans_recovered: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub node: Node,
    pub edges_out: Vec<Edge>,
    pub edges_in: Vec<Edge>,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreeOutline {
    pub tree: serde_json::Value,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeReport {
    pub symbol: String,
    pub edges: Vec<Edge>,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatchKind {
    ExactId,
    ExactSymbol,
    CaseInsensitiveExact,
    QualifiedSuffix,
    SegmentedIdentifier,
    NormalizedSubstring,
}

impl MatchKind {
    pub(crate) fn rank(self) -> u8 {
        match self {
            Self::ExactId => 0,
            Self::ExactSymbol => 1,
            Self::CaseInsensitiveExact => 2,
            Self::QualifiedSuffix => 3,
            Self::SegmentedIdentifier => 4,
            Self::NormalizedSubstring => 5,
        }
    }

    pub(crate) fn score(self) -> u16 {
        match self {
            Self::ExactId => 600,
            Self::ExactSymbol => 500,
            Self::CaseInsensitiveExact => 400,
            Self::QualifiedSuffix => 300,
            Self::SegmentedIdentifier => 200,
            Self::NormalizedSubstring => 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    pub match_kind: MatchKind,
    pub score: u16,
    pub node: Node,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    pub matches: Vec<SearchHit>,
    pub graph_fingerprint: String,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    pub frozen: bool,
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: usize,
    pub frozen: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 20,
            frozen: false,
        }
    }
}

pub fn validate_search_limit(limit: usize) -> Result<(), AtlasError> {
    if (1..=200).contains(&limit) {
        Ok(())
    } else {
        Err(AtlasError::SearchLimit { limit })
    }
}

/// Build (or incrementally refresh) the graph for `code_root` into `graph_dir`.
pub fn build(
    code_root: &Path,
    graph_dir: &Path,
    opts: &BuildOptions,
) -> Result<BuildReport, AtlasError> {
    build_with_meta(code_root, graph_dir, opts, None, false, None)
}

#[cfg(any(feature = "mir", test))]
pub fn build_with_mir(
    code_root: &Path,
    graph_dir: &Path,
    opts: &BuildOptions,
    mir: &MirBuildOptions,
) -> Result<BuildReport, AtlasError> {
    build_with_meta(code_root, graph_dir, opts, None, false, Some(mir))
}

fn build_with_meta(
    code_root: &Path,
    graph_dir: &Path,
    opts: &BuildOptions,
    known_meta: Option<Meta>,
    retain_semantic_authority_fingerprints: bool,
    mir_options: Option<&MirBuildOptions>,
) -> Result<BuildReport, AtlasError> {
    let writer = locking::WriterLease::try_acquire(graph_dir)?;
    build_with_meta_locked(
        code_root,
        graph_dir,
        opts,
        known_meta,
        retain_semantic_authority_fingerprints,
        mir_options,
        &writer,
    )
}

pub(crate) fn build_with_meta_locked(
    code_root: &Path,
    graph_dir: &Path,
    opts: &BuildOptions,
    known_meta: Option<Meta>,
    retain_semantic_authority_fingerprints: bool,
    mir_options: Option<&MirBuildOptions>,
    writer: &locking::WriterLease,
) -> Result<BuildReport, AtlasError> {
    writer.assert_graph(graph_dir)?;
    // `cargo metadata` reports absolute, canonical paths; canonicalize the root
    // so the file walk and layout share one path space (otherwise `--code .`
    // yields relative walk paths that never match the absolute target dirs).
    let code_root = &std::fs::canonicalize(code_root).map_err(io_err)?;
    validate_build_options(opts)?;
    check_cancelled(opts)?;
    let base = generation::resolve_optional_snapshot(graph_dir)?;
    let known_meta = known_meta.or_else(|| {
        base.as_ref()
            .and_then(|snapshot| read_meta_at(&snapshot.data_dir).ok())
    });
    let scope = scope::AtlasScope::discover(code_root, graph_dir)?;
    let identity = status::capture_identity(code_root, graph_dir)?;
    let input_key = input_plan::InputPlanKey::capture(&scope, opts, &identity.toolchain)?;
    let input_fingerprint = input_key.fingerprint()?;
    let prior_orphan = incremental::load_orphan(
        graph_dir,
        base.as_ref()
            .and_then(|snapshot| snapshot.generation.as_deref()),
    )?;
    let cached_plan = base
        .as_ref()
        .and_then(|snapshot| input_plan::load(&snapshot.data_dir, &input_fingerprint));
    let (layout, input_plan_state) = match cached_plan {
        Some(plan) => (
            ProjectLayout::from_input_plan(code_root, &plan),
            InputPlanState::Hit,
        ),
        None => (
            ProjectLayout::discover(code_root, opts, &scope)?,
            InputPlanState::Miss,
        ),
    };
    let input_artifact = layout.to_input_plan(input_fingerprint.clone(), &input_key);
    let current_files = status::source_hashes_with_scope(code_root, &scope)?;
    if !opts.full
        && input_plan_state == InputPlanState::Hit
        && prior_orphan.is_none()
        && base.as_ref().is_some_and(|snapshot| {
            known_meta.as_ref().is_some_and(|meta| {
                graph_is_unchanged(snapshot, meta, &layout, &current_files, opts, mir_options)
            })
        })
    {
        let meta = known_meta.unwrap_or_else(|| unreachable!("checked above"));
        let snapshot = base.unwrap_or_else(|| unreachable!("checked above"));
        let mut report = BuildReport {
            generation: snapshot.generation.unwrap_or_default(),
            input_plan: InputPlanState::Hit,
            rebuilt: Vec::new(),
            removed: Vec::new(),
            unparsed: Vec::new(),
            capability: meta.capability,
            diagnostics: Vec::new(),
            touched_shards: Vec::new(),
            resolved_shards: 0,
            validated_shards: 0,
            resolved_edge_delta: 0,
            unresolved_edge_delta: 0,
            working_bytes: 0,
            fallback_reason: None,
            orphans_recovered: 0,
        };
        append_reclamation_diagnostics(graph_dir, writer, &mut report);
        return Ok(report);
    }
    let transaction = generation::GenerationTransaction::begin(graph_dir, base.as_ref())?;
    input_plan::write(transaction.data_dir(), &input_artifact)?;
    let (mut report, staged_meta) = build_staged(BuildStagedContext {
        code_root,
        scope,
        graph_root: graph_dir,
        graph_dir: transaction.data_dir(),
        layout,
        opts,
        known_meta,
        input_plan_state,
        prior_orphan,
        base_generation: base
            .as_ref()
            .and_then(|snapshot| snapshot.generation.clone()),
        retain_semantic_authority_fingerprints,
        mir_options,
    })?;
    check_cancelled(opts)?;
    let snapshot = transaction.publish(
        &staged_meta.graph_fingerprint,
        &staged_meta.capability,
        Some(&input_fingerprint),
    )?;
    report.generation = snapshot.generation.unwrap_or_default();
    if let Err(error) = incremental::clear_orphan(graph_dir) {
        let recovery = incremental::rebase_orphan_after_commit(graph_dir, &report.generation)
            .map(|()| "queue rebased for the next recovery build".to_string())
            .unwrap_or_else(|rebase| format!("queue rebase also failed: {rebase}"));
        report.diagnostics.push(BuildDiagnostic {
            code: "incremental-maintenance-failed".to_string(),
            severity: "warning".to_string(),
            message: format!(
                "committed generation retained; cannot clear orphan queue: {error}; {recovery}"
            ),
        });
    }
    append_reclamation_diagnostics(graph_dir, writer, &mut report);
    Ok(report)
}

fn append_reclamation_diagnostics(
    graph_dir: &Path,
    writer: &locking::WriterLease,
    report: &mut BuildReport,
) {
    match generation::safe_reclaim(graph_dir, writer) {
        Ok(reclaim) => {
            report
                .diagnostics
                .extend(
                    reclaim
                        .diagnostics
                        .into_iter()
                        .map(|message| BuildDiagnostic {
                            code: "generation-maintenance-warning".to_string(),
                            severity: "warning".to_string(),
                            message,
                        }),
                );
        }
        Err(error) => report.diagnostics.push(BuildDiagnostic {
            code: "generation-maintenance-failed".to_string(),
            severity: "warning".to_string(),
            message: format!("committed generation retained; safe reclamation failed: {error}"),
        }),
    }
}

struct BuildStagedContext<'a> {
    code_root: &'a Path,
    scope: scope::AtlasScope,
    graph_root: &'a Path,
    graph_dir: &'a Path,
    layout: ProjectLayout,
    opts: &'a BuildOptions,
    known_meta: Option<Meta>,
    input_plan_state: InputPlanState,
    prior_orphan: Option<incremental::OrphanQueue>,
    base_generation: Option<String>,
    retain_semantic_authority_fingerprints: bool,
    mir_options: Option<&'a MirBuildOptions>,
}

fn build_staged(context: BuildStagedContext<'_>) -> Result<(BuildReport, Meta), AtlasError> {
    let BuildStagedContext {
        code_root,
        scope,
        graph_root,
        graph_dir,
        layout,
        opts,
        known_meta,
        input_plan_state,
        prior_orphan,
        base_generation,
        retain_semantic_authority_fingerprints,
        mir_options,
    } = context;
    let identity = status::capture_identity(code_root, graph_root)?;
    let shards_dir = graph_dir.join("shards");
    std::fs::create_dir_all(&shards_dir).map_err(io_err)?;

    let old_meta = known_meta.or_else(|| read_meta(graph_dir).ok());
    let old_files = old_meta
        .as_ref()
        .map(|m| m.files.clone())
        .unwrap_or_default();
    let retained_scip_source_fingerprint = old_meta
        .as_ref()
        .and_then(|meta| meta.capability.scip_source_fingerprint.clone());
    let retained_scip_fingerprint = old_meta
        .as_ref()
        .and_then(|meta| meta.capability.scip_fingerprint.clone());
    let retained_mir_source_fingerprint = old_meta
        .as_ref()
        .and_then(|meta| meta.capability.mir_source_fingerprint.clone());
    let retained_mir_fingerprint = old_meta
        .as_ref()
        .and_then(|meta| meta.capability.mir_fingerprint.clone());

    let mut files = BTreeMap::new();
    let mut rebuilt = Vec::new();
    let mut unparsed = Vec::new();
    let mut old_changed_shards = BTreeMap::new();
    let mut working_bytes: usize = 0;
    for (position, path) in scope.source_files().into_iter().enumerate() {
        if position % opts.batch_size == 0 {
            check_cancelled(opts)?;
        }
        let rel = rel_path(code_root, &path);
        let file_bytes = std::fs::metadata(&path)
            .map_err(io_err)?
            .len()
            .try_into()
            .map_err(|_| AtlasError::ResourceLimit {
                resource: "source working set".to_string(),
                required: usize::MAX,
                limit: opts.working_byte_limit,
            })?;
        let required = working_bytes.saturating_add(file_bytes);
        if required > opts.working_byte_limit {
            return Err(AtlasError::ResourceLimit {
                resource: "source working set".to_string(),
                required,
                limit: opts.working_byte_limit,
            });
        }
        let bytes = std::fs::read(&path).map_err(io_err)?;
        let required = working_bytes.saturating_add(bytes.len());
        if required > opts.working_byte_limit {
            return Err(AtlasError::ResourceLimit {
                resource: "source working set".to_string(),
                required,
                limit: opts.working_byte_limit,
            });
        }
        working_bytes = required;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        let unit = layout.source_unit(&path).ok_or_else(|| {
            AtlasError::Cargo(format!("{} is not owned by a Cargo target", path.display()))
        })?;
        let layout_changed = old_files.get(&rel) == Some(&hash)
            && match read_shard(&shards_dir, &rel) {
                Ok(shard) if shard.unparsed.is_some() => false,
                Ok(shard) => !shard.nodes.iter().any(|node| node.id == unit.node_id),
                Err(_) => true,
            };
        let dirty = opts.full || old_files.get(&rel) != Some(&hash) || layout_changed;
        if dirty {
            if old_files.contains_key(&rel)
                && let Ok(shard) = read_shard(&shards_dir, &rel)
            {
                old_changed_shards.insert(rel.clone(), shard);
            }
            let source = String::from_utf8_lossy(&bytes);
            let shard = extract_shard(&unit, &rel, &hash, &source);
            if shard.unparsed.is_some() {
                unparsed.push(rel.clone());
            }
            write_shard(&shards_dir, &shard)?;
            rebuilt.push(rel.clone());
        } else if let Ok(shard) = read_shard(&shards_dir, &rel)
            && shard.unparsed.is_some()
        {
            unparsed.push(rel.clone());
        }
        files.insert(rel, hash);
    }

    let mut removed = Vec::new();
    for rel in old_files.keys() {
        if !files.contains_key(rel) {
            if let Ok(shard) = read_shard(&shards_dir, rel) {
                old_changed_shards.insert(rel.clone(), shard);
            }
            let _ = std::fs::remove_file(shards_dir.join(shard_file_name(rel)));
            removed.push(rel.clone());
        }
    }

    working_bytes = admit_working_bytes(
        working_bytes,
        serialized_shard_bytes(&shards_dir, &files)?,
        opts,
        "serialized graph working set",
    )?;

    let input_plan_changed = old_meta.is_some() && input_plan_state == InputPlanState::Miss;
    let capability_change_reason = old_meta.as_ref().and_then(|meta| {
        if meta.capability.dynamic_dispatch != opts.dynamic_dispatch {
            Some("dynamic-dispatch-full-recompute")
        } else if !capability_matches_request(&meta.capability, opts, mir_options) {
            Some("semantic-overlay-change")
        } else {
            None
        }
    });
    let frontier = incremental::plan_frontier(incremental::FrontierRequest {
        shards_dir: &shards_dir,
        files: &files,
        old_changed_shards: &old_changed_shards,
        rebuilt: &rebuilt,
        removed: &removed,
        prior: prior_orphan.as_ref(),
        full: opts.full || old_meta.is_none(),
        input_plan_changed,
        capability_change_reason,
        opts,
    })?;
    let orphans_recovered = prior_orphan
        .as_ref()
        .map_or(0, |queue| queue.affected_files.len());
    if !frontier.files.is_empty() {
        let reason = if prior_orphan.is_some() && rebuilt.is_empty() && removed.is_empty() {
            "orphan-recovery"
        } else if let Some(reason) = &frontier.fallback_reason {
            reason
        } else if old_meta.is_none() || opts.full {
            "full-build"
        } else {
            "source-change"
        };
        let queue = incremental::OrphanQueue::new(
            base_generation,
            status::source_fingerprint(&files)?,
            frontier.files.clone(),
            frontier.changed_symbols.clone(),
            reason.to_string(),
        )?;
        incremental::write_orphan(graph_root, &queue)?;
    }

    check_cancelled(opts)?;
    dynamic_dispatch::remove(&shards_dir, &files)?;
    let before_resolution = resolution_counts(&shards_dir, &frontier.files, opts)?;
    resolve_syn_edges(&shards_dir, &files, &frontier.files, opts)?;
    let after_resolution = resolution_counts(&shards_dir, &frontier.files, opts)?;

    let mut capability = Capability::default();
    check_cancelled(opts)?;
    if let Some(index_path) = &opts.scip_index {
        working_bytes = admit_working_bytes(
            working_bytes,
            serialized_file_bytes(index_path)?,
            opts,
            "SCIP overlay working set",
        )?;
        let (tool, overlaid_scip_fingerprint) = overlay_scip(&shards_dir, index_path, &files)?;
        capability.scip = true;
        capability.scip_tool = tool;
        // Record the index (absolute) + its fingerprint so `refresh` can
        // re-overlay after edits instead of silently dropping the semantic layer.
        let abs = std::fs::canonicalize(index_path).unwrap_or_else(|_| index_path.clone());
        capability.scip_index = Some(abs.to_string_lossy().into_owned());
        capability.scip_fingerprint = if retain_semantic_authority_fingerprints {
            retained_scip_fingerprint
        } else {
            Some(overlaid_scip_fingerprint)
        };
        capability.scip_source_fingerprint = if retain_semantic_authority_fingerprints {
            retained_scip_source_fingerprint
        } else {
            Some(status::source_fingerprint(&files)?)
        };
    } else {
        remove_scip_edges(&shards_dir, &files)?;
    }
    let mut diagnostics = Vec::new();
    check_cancelled(opts)?;
    remove_mir_facts(&shards_dir, &files, opts)?;
    if let Some(options) = mir_options {
        if let Some(overlay) = &options.overlay {
            working_bytes = admit_working_bytes(
                working_bytes,
                serialized_file_bytes(overlay)?,
                opts,
                "MIR overlay working set",
            )?;
        }
        match mir::prepare_and_overlay(
            code_root,
            graph_dir,
            &shards_dir,
            &files,
            options,
            retain_semantic_authority_fingerprints,
        ) {
            Ok(Some(applied)) => match commit_shard_generation(&shards_dir, &applied.shards) {
                Ok(()) => {
                    capability.mir = true;
                    capability.mir_tool = Some(applied.tool);
                    capability.mir_overlay = Some(applied.overlay_path);
                    capability.mir_fingerprint = if retain_semantic_authority_fingerprints {
                        retained_mir_fingerprint
                    } else {
                        Some(applied.overlay_fingerprint)
                    };
                    capability.mir_source_fingerprint = if retain_semantic_authority_fingerprints {
                        retained_mir_source_fingerprint
                    } else {
                        Some(applied.source_fingerprint)
                    };
                }
                Err(error @ AtlasError::Invariant(_)) => return Err(error),
                Err(error) => diagnostics.push(BuildDiagnostic {
                    code: "mir-extraction-failed".to_string(),
                    severity: "warning".to_string(),
                    message: format!("cannot commit MIR shard generation: {error}"),
                }),
            },
            Ok(None) => {}
            Err(message) => diagnostics.push(BuildDiagnostic {
                code: "mir-extraction-failed".to_string(),
                severity: "warning".to_string(),
                message,
            }),
        }
    }
    if opts.dynamic_dispatch {
        check_cancelled(opts)?;
        let outcome =
            dynamic_dispatch::enrich(&shards_dir, &files, dynamic_dispatch::MAX_CANDIDATES)?;
        capability.dynamic_dispatch = true;
        capability.dynamic_dispatch_edges = outcome.edges_added;
        diagnostics.extend(outcome.diagnostics);
    }
    check_cancelled(opts)?;
    working_bytes = admit_working_bytes(
        working_bytes,
        serialized_shard_bytes(&shards_dir, &files)?,
        opts,
        "serialized graph working set",
    )?;
    let validation_files =
        if opts.scip_index.is_some() || mir_options.is_some() || opts.dynamic_dispatch {
            files.keys().cloned().collect::<Vec<_>>()
        } else {
            frontier.files.clone()
        };
    validate_graph_frontier(&shards_dir, &files, &validation_files, opts)?;

    let mut meta = Meta {
        schema_version: SCHEMA_VERSION,
        package: layout.graph_root.clone(),
        packages: layout.packages.clone(),
        roots: layout.roots.clone(),
        capability: capability.clone(),
        files,
        graph_fingerprint: String::new(),
    };
    let mut shards = Vec::with_capacity(meta.files.len());
    for batch in meta
        .files
        .keys()
        .collect::<Vec<_>>()
        .chunks(opts.batch_size)
    {
        check_cancelled(opts)?;
        for rel in batch {
            shards.push(read_shard(&shards_dir, rel)?);
        }
    }
    meta.graph_fingerprint = graph_fingerprint_with_identity(&meta, &shards, &identity)?;
    write_persisted_meta(&graph_dir.join("meta.json"), &meta, &identity)?;
    index::rebuild_query_index_at(graph_dir, &meta, &shards)?;
    let touched_shards = frontier
        .files
        .iter()
        .chain(removed.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok((
        BuildReport {
            generation: String::new(),
            input_plan: input_plan_state,
            rebuilt,
            removed,
            unparsed,
            capability,
            diagnostics,
            touched_shards,
            resolved_shards: frontier.files.len(),
            validated_shards: validation_files.len(),
            resolved_edge_delta: after_resolution.0 as isize - before_resolution.0 as isize,
            unresolved_edge_delta: after_resolution.1 as isize - before_resolution.1 as isize,
            working_bytes,
            fallback_reason: frontier.fallback_reason,
            orphans_recovered,
        },
        meta,
    ))
}

/// Report stale shard source files (content hash mismatch, deleted, or new).
pub fn check(code_root: &Path, graph_dir: &Path) -> Result<Vec<String>, AtlasError> {
    Ok(status(code_root, graph_dir)?.syn.stale_files)
}

/// Node facts plus adjacent edges for a canonical symbol path.
pub fn query(
    code_root: &Path,
    graph_dir: &Path,
    symbol: &str,
    opts: &QueryOptions,
) -> Result<QueryResult, AtlasError> {
    let (_meta, index, mut status) = indexed_query_state(code_root, graph_dir, opts)?;
    let matches = index.matching_nodes(symbol);
    let node = match matches.as_slice() {
        [] => {
            return Err(AtlasError::UnknownSymbol {
                symbol: symbol.to_string(),
            });
        }
        [node] => (*node).clone(),
        many => {
            return Err(AtlasError::AmbiguousSymbol {
                symbol: symbol.to_string(),
                declarations: many.len(),
            });
        }
    };
    let edges_out = preferred_query_edges(
        index
            .outgoing_edges([node.id.as_str()])
            .into_iter()
            .cloned()
            .collect(),
    );
    let edges_in = preferred_query_edges(
        index
            .incoming_edges([node.id.as_str()])
            .into_iter()
            .cloned()
            .collect(),
    );
    let mut represented = edge_represented_files(&index, edges_out.iter().chain(&edges_in));
    represented.insert(node.file.clone());
    status::scope_live_status(&mut status, represented);
    Ok(QueryResult {
        edges_out,
        edges_in,
        node,
        stale: status.syn.stale_files.clone(),
        status,
    })
}

pub(crate) fn preferred_query_edges(mut edges: Vec<Edge>) -> Vec<Edge> {
    let mut highest: BTreeMap<(String, String), Provenance> = BTreeMap::new();
    for edge in &edges {
        if is_precedence_relation(edge) {
            highest
                .entry((edge.from.clone(), edge.to.clone()))
                .and_modify(|current| *current = (*current).max(edge.provenance))
                .or_insert(edge.provenance);
        }
    }
    edges.retain(|edge| {
        !is_precedence_relation(edge)
            || highest.get(&(edge.from.clone(), edge.to.clone())) == Some(&edge.provenance)
    });
    edges.sort();
    edges.dedup();
    edges
}

fn is_precedence_relation(edge: &Edge) -> bool {
    matches!(edge.kind, EdgeKind::Calls | EdgeKind::References)
        && edge.resolution == EdgeResolution::Resolved
        && edge.candidates.is_empty()
}

/// Deterministic indexed symbol search with fixed match precedence.
pub fn search(
    code_root: &Path,
    graph_dir: &Path,
    query: &str,
    opts: &SearchOptions,
) -> Result<SearchResult, AtlasError> {
    validate_search_limit(opts.limit)?;
    let (meta, index, mut status) = indexed_query_state(
        code_root,
        graph_dir,
        &QueryOptions {
            frozen: opts.frozen,
        },
    )?;
    let mut matches = index.search_nodes(query);
    matches.truncate(opts.limit);
    status::scope_live_status(&mut status, matches.iter().map(|hit| hit.node.file.clone()));
    Ok(SearchResult {
        matches,
        graph_fingerprint: meta.graph_fingerprint,
        stale: status.syn.stale_files.clone(),
        status,
        limit: opts.limit,
    })
}

/// Deterministic module outline of the whole graph.
pub fn tree(
    code_root: &Path,
    graph_dir: &Path,
    opts: &QueryOptions,
) -> Result<TreeOutline, AtlasError> {
    let (meta, status, snapshot) = refresh(code_root, graph_dir, opts)?;
    let shards = load_shards(&snapshot.data_dir, &meta)?;
    let mut kinds = BTreeMap::new();
    let mut children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for shard in &shards {
        for node in &shard.nodes {
            kinds.entry(node.id.clone()).or_insert(node.kind);
        }
        for edge in &shard.edges {
            if edge.kind == EdgeKind::Contains {
                children
                    .entry(edge.from.clone())
                    .or_default()
                    .insert(edge.to.clone());
            }
        }
    }
    fn render(
        id: &str,
        kinds: &BTreeMap<String, NodeKind>,
        children: &BTreeMap<String, BTreeSet<String>>,
        visited: &mut BTreeSet<String>,
    ) -> serde_json::Value {
        if !visited.insert(id.to_string()) {
            return serde_json::json!({ "id": id, "cycle": true });
        }
        let kids: Vec<serde_json::Value> = children
            .get(id)
            .into_iter()
            .flatten()
            .map(|c| render(c, kinds, children, visited))
            .collect();
        let kind = kinds
            .get(id)
            .map(|k| serde_json::to_value(k).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);
        if kids.is_empty() {
            serde_json::json!({ "id": id, "kind": kind })
        } else {
            serde_json::json!({ "id": id, "kind": kind, "children": kids })
        }
    }
    let mut visited = BTreeSet::new();
    let roots: Vec<serde_json::Value> = meta
        .roots
        .iter()
        .map(|root| render(root, &kinds, &children, &mut visited))
        .collect();
    let tree = if roots.len() == 1 {
        roots.into_iter().next().unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::json!({ "id": meta.package, "kind": "workspace", "children": roots })
    };
    Ok(TreeOutline {
        tree,
        stale: status.syn.stale_files.clone(),
        status,
    })
}

/// Incoming reference/call edges for a symbol.
///
/// `References`/`Calls` edges come from semantic overlays. The derived query
/// index projects the highest available provenance for each source-target
/// relation while canonical shards retain every evidence layer.
pub fn refs(
    code_root: &Path,
    graph_dir: &Path,
    symbol: &str,
    opts: &QueryOptions,
) -> Result<EdgeReport, AtlasError> {
    let (_meta, index, mut status) = indexed_query_state(code_root, graph_dir, opts)?;
    let matches = index.matching_nodes(symbol);
    if matches.is_empty() {
        return Err(AtlasError::UnknownSymbol {
            symbol: symbol.to_string(),
        });
    }
    let edges: BTreeSet<Edge> = index
        .incoming_edges(matches.iter().map(|node| node.id.as_str()))
        .into_iter()
        .filter(|e| matches!(e.kind, EdgeKind::References | EdgeKind::Calls))
        .cloned()
        .collect();
    let edges = edges.into_iter().collect::<Vec<_>>();
    let mut represented = edge_represented_files(&index, &edges);
    represented.extend(matches.iter().map(|node| node.file.clone()));
    status::scope_live_status(&mut status, represented);
    Ok(EdgeReport {
        symbol: symbol.to_string(),
        edges,
        stale: status.syn.stale_files.clone(),
        status,
    })
}

/// Impl relations touching a trait or type name.
pub fn impls(
    code_root: &Path,
    graph_dir: &Path,
    name: &str,
    opts: &QueryOptions,
) -> Result<EdgeReport, AtlasError> {
    let (_meta, index, mut status) = indexed_query_state(code_root, graph_dir, opts)?;
    let matching_ids: BTreeSet<&str> = index
        .nodes_with_symbol_suffix(name)
        .into_iter()
        .map(|node| node.id.as_str())
        .collect();
    // Also match edges whose target is still unresolved: the trait/type name
    // survives in `target_text` (or `to`) even when the syn layer could not map
    // it to a node id. Without this, `impls <Trait>` returns nothing for any
    // trait referenced by an imported bare name.
    let text_matches = |value: &str| value == name || value.ends_with(&format!("::{name}"));
    let edges: BTreeSet<Edge> = index
        .edges
        .iter()
        .filter(|e| {
            matches!(e.kind, EdgeKind::ImplsTrait | EdgeKind::ImplFor)
                && (matching_ids.contains(e.to.as_str())
                    || matching_ids.contains(e.from.as_str())
                    || e.target_text.as_deref().is_some_and(text_matches)
                    || text_matches(&e.to))
        })
        .cloned()
        .collect();
    let edges = edges.into_iter().collect::<Vec<_>>();
    let represented = edge_represented_files(&index, &edges);
    status::scope_live_status(&mut status, represented);
    Ok(EdgeReport {
        symbol: name.to_string(),
        edges,
        stale: status.syn.stale_files.clone(),
        status,
    })
}

fn edge_represented_files<'a>(
    index: &QueryIndex,
    edges: impl IntoIterator<Item = &'a Edge>,
) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for edge in edges {
        if let Some(site) = &edge.site {
            files.insert(site.file.clone());
        }
        for node_id in std::iter::once(edge.from.as_str())
            .chain(std::iter::once(edge.to.as_str()))
            .chain(edge.candidates.iter().map(String::as_str))
        {
            if let Some(node) = index.node_by_id(node_id) {
                files.insert(node.file.clone());
            }
        }
    }
    files
}

/// Load every shard plus meta (internal + MCP convenience).
pub fn load_graph(graph_dir: &Path) -> Result<(Meta, Vec<Shard>), AtlasError> {
    let snapshot = generation::resolve_snapshot(graph_dir)?;
    let meta = read_meta_at(&snapshot.data_dir)?;
    let shards = load_shards(&snapshot.data_dir, &meta)?;
    Ok((meta, shards))
}

fn load_shards(graph_dir: &Path, meta: &Meta) -> Result<Vec<Shard>, AtlasError> {
    let mut shards = Vec::new();
    for rel in meta.files.keys() {
        shards.push(read_shard(&graph_dir.join("shards"), rel)?);
    }
    Ok(shards)
}

// ── internals ───────────────────────────────────────────────────────

fn io_err(error: std::io::Error) -> AtlasError {
    AtlasError::Io(error.to_string())
}

fn validate_build_options(opts: &BuildOptions) -> Result<(), AtlasError> {
    if opts.frontier_limit == 0 {
        return Err(AtlasError::TraversalLimit {
            detail: "frontier_limit must be greater than zero".to_string(),
        });
    }
    if opts.batch_size == 0 {
        return Err(AtlasError::TraversalLimit {
            detail: "batch_size must be greater than zero".to_string(),
        });
    }
    if opts.working_byte_limit == 0 {
        return Err(AtlasError::TraversalLimit {
            detail: "working_byte_limit must be greater than zero".to_string(),
        });
    }
    Ok(())
}

fn check_cancelled(opts: &BuildOptions) -> Result<(), AtlasError> {
    if opts
        .cancellation
        .as_ref()
        .is_some_and(|token| token.load(std::sync::atomic::Ordering::Acquire))
    {
        Err(AtlasError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
std::thread_local! {
    static TEST_CANCEL_AFTER_RESOLUTION_BATCHES: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
fn set_test_cancel_after_resolution_batches(value: Option<usize>) {
    TEST_CANCEL_AFTER_RESOLUTION_BATCHES.with(|remaining| remaining.set(value));
}

fn resolution_batch_completed(opts: &BuildOptions) {
    #[cfg(test)]
    TEST_CANCEL_AFTER_RESOLUTION_BATCHES.with(|remaining| {
        let Some(count) = remaining.get() else {
            return;
        };
        if count <= 1 {
            remaining.set(None);
            if let Some(token) = &opts.cancellation {
                token.store(true, std::sync::atomic::Ordering::Release);
            }
        } else {
            remaining.set(Some(count - 1));
        }
    });
    #[cfg(not(test))]
    let _ = opts;
}

fn admit_working_bytes(
    current: usize,
    required: usize,
    opts: &BuildOptions,
    resource: &str,
) -> Result<usize, AtlasError> {
    if required > opts.working_byte_limit {
        Err(AtlasError::ResourceLimit {
            resource: resource.to_string(),
            required,
            limit: opts.working_byte_limit,
        })
    } else {
        Ok(current.max(required))
    }
}

fn serialized_file_bytes(path: &Path) -> Result<usize, AtlasError> {
    std::fs::metadata(path)
        .map_err(io_err)?
        .len()
        .try_into()
        .map_err(|_| AtlasError::ResourceLimit {
            resource: "serialized artifact working set".to_string(),
            required: usize::MAX,
            limit: usize::MAX,
        })
}

fn serialized_shard_bytes(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
) -> Result<usize, AtlasError> {
    files.keys().try_fold(0_usize, |total, rel| {
        Ok(total.saturating_add(serialized_file_bytes(
            &shards_dir.join(shard_file_name(rel)),
        )?))
    })
}

fn graph_is_unchanged(
    snapshot: &GraphSnapshot,
    meta: &Meta,
    layout: &ProjectLayout,
    current_files: &BTreeMap<String, String>,
    opts: &BuildOptions,
    mir_options: Option<&MirBuildOptions>,
) -> bool {
    meta.files == *current_files
        && meta.package == layout.graph_root
        && meta.packages == layout.packages
        && meta.roots == layout.roots
        && capability_matches_request(&meta.capability, opts, mir_options)
        && generation::artifacts_match_manifest(snapshot)
        && index::load_query_index_at(&snapshot.data_dir, meta).is_ok()
}

fn capability_matches_request(
    capability: &Capability,
    opts: &BuildOptions,
    mir_options: Option<&MirBuildOptions>,
) -> bool {
    if capability.dynamic_dispatch != opts.dynamic_dispatch {
        return false;
    }
    match &opts.scip_index {
        Some(path) => {
            let Ok(bytes) = std::fs::read(path) else {
                return false;
            };
            if !capability.scip
                || capability.scip_fingerprint.as_deref()
                    != Some(blake3::hash(&bytes).to_hex().as_ref())
            {
                return false;
            }
        }
        None if capability.scip => return false,
        None => {}
    }
    match mir_options {
        Some(MirBuildOptions {
            overlay: Some(path),
            driver: None,
        }) => {
            let Ok(bytes) = std::fs::read(path) else {
                return false;
            };
            capability.mir
                && capability.mir_fingerprint.as_deref()
                    == Some(blake3::hash(&bytes).to_hex().as_ref())
        }
        Some(MirBuildOptions {
            overlay: None,
            driver: Some(_),
        }) => false,
        Some(_) => !capability.mir,
        None => !capability.mir,
    }
}

fn syn_extractor() -> ExtractorIdentity {
    ExtractorIdentity {
        name: "syn".to_string(),
        version: None,
    }
}

fn scip_extractor(tool: Option<&str>) -> ExtractorIdentity {
    ExtractorIdentity {
        name: "rust-analyzer-scip".to_string(),
        version: tool.map(str::to_string),
    }
}

#[derive(Deserialize)]
struct MetaHeader {
    schema_version: u32,
}

#[derive(Deserialize)]
pub(crate) struct PersistedMeta {
    #[serde(flatten)]
    meta: Meta,
    identity: GraphIdentity,
}

#[derive(Serialize)]
struct PersistedMetaRef<'a> {
    #[serde(flatten)]
    meta: &'a Meta,
    identity: &'a GraphIdentity,
}

#[derive(Serialize)]
struct GraphFingerprint<'a> {
    graph: String,
    identity: &'a GraphIdentity,
}

fn read_meta(graph_dir: &Path) -> Result<Meta, AtlasError> {
    Ok(read_persisted_meta(graph_dir)?.meta)
}

#[cfg(test)]
pub(crate) fn active_data_dir(graph_dir: &Path) -> PathBuf {
    generation::resolve_snapshot(graph_dir)
        .unwrap_or_else(|error| panic!("cannot resolve active test generation: {error}"))
        .data_dir
}

fn read_persisted_meta(graph_dir: &Path) -> Result<PersistedMeta, AtlasError> {
    let snapshot = generation::resolve_snapshot(graph_dir)?;
    read_persisted_meta_at(&snapshot.data_dir)
}

fn read_meta_at(data_dir: &Path) -> Result<Meta, AtlasError> {
    Ok(read_persisted_meta_at(data_dir)?.meta)
}

fn read_persisted_meta_at(data_dir: &Path) -> Result<PersistedMeta, AtlasError> {
    #[cfg(test)]
    META_READ_COUNT.with(|count| count.set(count.get() + 1));
    let path = data_dir.join("meta.json");
    let text = std::fs::read_to_string(&path).map_err(|_| AtlasError::MissingGraph {
        graph_dir: data_dir.display().to_string(),
    })?;
    // Reject a graph written by a different atlas version. Without this, a
    // stale-schema graph passes `check` (which only diffs file hashes) while
    // `query` silently misreads or fails on the changed shard format — a false
    // green. `build` reads meta via `.ok()`, so a mismatch there degrades to a
    // full rebuild rather than an error.
    let header: MetaHeader =
        serde_json::from_str(&text).map_err(|e| AtlasError::Io(e.to_string()))?;
    if header.schema_version != SCHEMA_VERSION {
        return Err(AtlasError::SchemaMismatch {
            found: header.schema_version,
            expected: SCHEMA_VERSION,
        });
    }
    serde_json::from_str(&text).map_err(|e| AtlasError::Io(e.to_string()))
}

fn write_persisted_meta(
    path: &Path,
    meta: &Meta,
    identity: &GraphIdentity,
) -> Result<(), AtlasError> {
    write_json_atomic(path, &PersistedMetaRef { meta, identity })
}

fn graph_fingerprint_with_identity(
    meta: &Meta,
    shards: &[Shard],
    identity: &GraphIdentity,
) -> Result<String, AtlasError> {
    let graph = canonical_graph_fingerprint(meta, shards)?;
    let bytes = serde_json::to_vec(&GraphFingerprint { graph, identity })
        .map_err(|error| AtlasError::Io(error.to_string()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

#[cfg(test)]
std::thread_local! {
    static META_READ_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_meta_read_count() {
    META_READ_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn meta_read_count() -> usize {
    META_READ_COUNT.with(std::cell::Cell::get)
}

fn shard_file_name(rel: &str) -> String {
    let escaped = rel
        .replace('%', "%25")
        .replace('/', "%2F")
        .replace('\\', "%5C");
    format!("{escaped}.json")
}

fn write_shard(shards_dir: &Path, shard: &Shard) -> Result<(), AtlasError> {
    write_json_atomic(&shards_dir.join(shard_file_name(&shard.file)), shard)
}

fn commit_shard_generation(
    shards_dir: &Path,
    shards: &BTreeMap<String, Shard>,
) -> Result<(), AtlasError> {
    commit_shard_generation_with(shards_dir, shards, write_shard)
}

fn commit_shard_generation_with(
    shards_dir: &Path,
    shards: &BTreeMap<String, Shard>,
    mut writer: impl FnMut(&Path, &Shard) -> Result<(), AtlasError>,
) -> Result<(), AtlasError> {
    static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let parent = shards_dir.parent().ok_or_else(|| {
        AtlasError::Invariant(format!(
            "shard directory {} has no parent",
            shards_dir.display()
        ))
    })?;
    let nonce = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let suffix = format!("{}-{nonce}", std::process::id());
    let staging = parent.join(format!(".shards-next-{suffix}"));
    let backup = parent.join(format!(".shards-prev-{suffix}"));
    std::fs::create_dir(&staging).map_err(io_err)?;

    for shard in shards.values() {
        if let Err(error) = writer(&staging, shard) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
    }
    if let Err(error) = std::fs::rename(shards_dir, &backup) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(io_err(error));
    }
    if let Err(error) = std::fs::rename(&staging, shards_dir) {
        let rollback = std::fs::rename(&backup, shards_dir);
        let _ = std::fs::remove_dir_all(&staging);
        return match rollback {
            Ok(()) => Err(io_err(error)),
            Err(rollback_error) => Err(AtlasError::Invariant(format!(
                "cannot activate shard generation ({error}) or restore baseline ({rollback_error}); baseline remains at {}",
                backup.display()
            ))),
        };
    }
    let _ = std::fs::remove_dir_all(&backup);
    Ok(())
}

fn read_shard(shards_dir: &Path, rel: &str) -> Result<Shard, AtlasError> {
    let path = shards_dir.join(shard_file_name(rel));
    let text = std::fs::read_to_string(&path).map_err(io_err)?;
    serde_json::from_str(&text).map_err(|e| AtlasError::Io(e.to_string()))
}

fn resolve_syn_edges(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
    frontier: &[String],
    opts: &BuildOptions,
) -> Result<(), AtlasError> {
    let mut symbols: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Bare-name index: last path segment -> set of fully-qualified symbols that
    // end in `::<segment>`. Lets us resolve `use`-imported bare names (e.g. a
    // trait referenced as `SpecLinter` whose symbol is
    // `crate::spec_lint::SpecLinter`) when there is exactly one candidate.
    let mut by_last: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for batch in files.keys().collect::<Vec<_>>().chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            let shard = read_shard(shards_dir, rel)?;
            for node in &shard.nodes {
                symbols
                    .entry(node.symbol.clone())
                    .or_default()
                    .push(node.id.clone());
                if let Some(last) = node.symbol.rsplit("::").next() {
                    by_last
                        .entry(last.to_string())
                        .or_default()
                        .insert(node.symbol.clone());
                }
            }
        }
    }
    for ids in symbols.values_mut() {
        ids.sort();
        ids.dedup();
    }
    // Resolve a bare (no `::`) target to a unique node id via the last-segment
    // index. Returns None on zero or multiple candidates (never guesses).
    let resolve_bare = |target: &str| -> Option<String> {
        if target.contains("::") {
            return None;
        }
        let candidates = by_last.get(target)?;
        if candidates.len() != 1 {
            return None;
        }
        let symbol = candidates.iter().next()?;
        match symbols.get(symbol).map(Vec::as_slice) {
            Some([id]) => Some(id.clone()),
            _ => None,
        }
    };

    for batch in frontier.chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            if !files.contains_key(rel) {
                continue;
            }
            let mut shard = read_shard(shards_dir, rel)?;
            let before = shard.edges.clone();
            for edge in &mut shard.edges {
                if edge.provenance != Provenance::Syn || edge.kind == EdgeKind::Contains {
                    continue;
                }
                let target_text = edge.target_text.clone().unwrap_or_else(|| edge.to.clone());
                edge.target_text = Some(target_text.clone());
                match symbols.get(&target_text).map(Vec::as_slice) {
                    Some([id]) => {
                        edge.to = id.clone();
                        edge.resolution = EdgeResolution::Resolved;
                        edge.confidence = Some(EdgeConfidence::Exact);
                    }
                    _ => match resolve_bare(&target_text) {
                        Some(id) => {
                            edge.to = id;
                            edge.resolution = EdgeResolution::Resolved;
                            edge.confidence = Some(EdgeConfidence::Exact);
                        }
                        None => {
                            edge.to = target_text.clone();
                            edge.resolution = if target_text.starts_with("std::")
                                || target_text.starts_with("core::")
                                || target_text.starts_with("alloc::")
                            {
                                EdgeResolution::External
                            } else {
                                EdgeResolution::Unresolved
                            };
                            edge.confidence = None;
                        }
                    },
                }
            }
            shard.edges.sort();
            shard.edges.dedup();
            if shard.edges != before {
                write_shard(shards_dir, &shard)?;
            }
        }
        resolution_batch_completed(opts);
    }
    Ok(())
}

fn resolution_counts(
    shards_dir: &Path,
    frontier: &[String],
    opts: &BuildOptions,
) -> Result<(usize, usize), AtlasError> {
    let mut resolved = 0;
    let mut unresolved = 0;
    for batch in frontier.chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            let shard = read_shard(shards_dir, rel)?;
            for edge in shard.edges.iter().filter(|edge| {
                edge.provenance == Provenance::Syn && edge.kind != EdgeKind::Contains
            }) {
                match edge.resolution {
                    EdgeResolution::Resolved => resolved += 1,
                    EdgeResolution::Unresolved => unresolved += 1,
                    EdgeResolution::External => {}
                }
            }
        }
    }
    Ok((resolved, unresolved))
}

fn validate_graph_frontier(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
    frontier: &[String],
    opts: &BuildOptions,
) -> Result<(), AtlasError> {
    let mut node_ids = BTreeSet::new();
    for batch in files.keys().collect::<Vec<_>>().chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            let shard = read_shard(shards_dir, rel)?;
            let mut shard_ids = BTreeSet::new();
            for node in &shard.nodes {
                if !shard_ids.insert(node.id.as_str()) {
                    return Err(AtlasError::Invariant(format!(
                        "duplicate node id `{}` in {}",
                        node.id, shard.file
                    )));
                }
                if !node_ids.insert(node.id.clone()) {
                    return Err(AtlasError::Invariant(format!(
                        "duplicate node id `{}` across graph",
                        node.id
                    )));
                }
            }
        }
    }
    for batch in frontier.chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            if !files.contains_key(rel) {
                continue;
            }
            let shard = read_shard(shards_dir, rel)?;
            validate_edges(shard.edges.iter())?;
            for edge in &shard.edges {
                if !node_ids.contains(&edge.from) {
                    return Err(AtlasError::Invariant(format!(
                        "edge source `{}` does not exist ({})",
                        edge.from, shard.file
                    )));
                }
                if edge.resolution == EdgeResolution::Resolved && !node_ids.contains(&edge.to) {
                    return Err(AtlasError::Invariant(format!(
                        "resolved edge target `{}` does not exist ({})",
                        edge.to, shard.file
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_edges<'a>(edges: impl IntoIterator<Item = &'a Edge>) -> Result<(), AtlasError> {
    for edge in edges {
        if edge.confidence == Some(EdgeConfidence::Exact) && edge.candidates.len() > 1 {
            return Err(AtlasError::Invariant(format!(
                "edge confidence exact cannot represent {} candidates ({} -> {})",
                edge.candidates.len(),
                edge.from,
                edge.to
            )));
        }
    }
    Ok(())
}

fn refresh(
    code_root: &Path,
    graph_dir: &Path,
    opts: &QueryOptions,
) -> Result<(Meta, AtlasStatus, generation::GraphSnapshot), AtlasError> {
    let snapshot = generation::resolve_snapshot(graph_dir)?;
    let persisted = read_persisted_meta_at(&snapshot.data_dir)?;
    let status = status::status_with_meta(
        code_root,
        graph_dir,
        &persisted,
        snapshot.generation.clone(),
    )?;
    status::require_worktree_match(&status)?;
    if status.syn.state == LayerState::Fresh {
        return Ok((persisted.meta, status, snapshot));
    }
    if opts.frozen {
        return Ok((persisted.meta, status, snapshot));
    }
    let (persisted, snapshot) = refresh_stale_graph(code_root, graph_dir, persisted.meta)?;
    let status = status::status_with_meta(
        code_root,
        graph_dir,
        &persisted,
        snapshot.generation.clone(),
    )?;
    status::require_worktree_match(&status)?;
    Ok((persisted.meta, status, snapshot))
}

fn indexed_query_state(
    code_root: &Path,
    graph_dir: &Path,
    opts: &QueryOptions,
) -> Result<(Meta, QueryIndex, AtlasStatus), AtlasError> {
    let snapshot = generation::resolve_snapshot(graph_dir)?;
    let persisted = read_persisted_meta_at(&snapshot.data_dir)?;
    let status = status::status_with_meta(
        code_root,
        graph_dir,
        &persisted,
        snapshot.generation.clone(),
    )?;
    status::require_worktree_match(&status)?;
    let index = index::load_query_index_at(&snapshot.data_dir, &persisted.meta)?;
    if status.syn.state == LayerState::Fresh {
        return Ok((persisted.meta, index, status));
    }
    if opts.frozen {
        return Ok((persisted.meta, index, status));
    }
    let (persisted, snapshot) = refresh_stale_graph(code_root, graph_dir, persisted.meta)?;
    let status = status::status_with_meta(
        code_root,
        graph_dir,
        &persisted,
        snapshot.generation.clone(),
    )?;
    status::require_worktree_match(&status)?;
    let index = index::load_query_index_at(&snapshot.data_dir, &persisted.meta)?;
    Ok((persisted.meta, index, status))
}

fn refresh_stale_graph(
    code_root: &Path,
    graph_dir: &Path,
    meta: Meta,
) -> Result<(PersistedMeta, generation::GraphSnapshot), AtlasError> {
    // Keep the SCIP layer alive across incremental refreshes: re-overlay the
    // index recorded in the prior build if it still exists. A vanished index
    // falls back to syn (scip edges purged, capability cleared).
    let scip_index = meta
        .capability
        .scip_index
        .clone()
        .map(PathBuf::from)
        .filter(|p| p.exists());
    let mir_options = meta
        .capability
        .mir_overlay
        .clone()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .map(|overlay| MirBuildOptions {
            overlay: Some(overlay),
            driver: None,
        });
    let mut build_options = BuildOptions {
        full: false,
        scip_index,
        dynamic_dispatch: meta.capability.dynamic_dispatch,
        ..BuildOptions::default()
    };
    let snapshot = generation::resolve_snapshot(graph_dir)?;
    if snapshot.generation.is_some() {
        if !generation::artifacts_match_manifest(&snapshot) {
            return Err(AtlasError::Invariant(
                "committed generation artifact integrity check failed; run `atlas build`"
                    .to_string(),
            ));
        }
        let plan = input_plan::load_committed(&snapshot.data_dir).ok_or_else(|| {
            AtlasError::Invariant(
                "committed generation input plan is missing or incompatible; run `atlas build`"
                    .to_string(),
            )
        })?;
        plan.apply_build_inputs(&mut build_options);
    }
    build_with_meta(
        code_root,
        graph_dir,
        &build_options,
        Some(meta),
        true,
        mir_options.as_ref(),
    )?;
    let snapshot = generation::resolve_snapshot(graph_dir)?;
    let persisted = read_persisted_meta_at(&snapshot.data_dir)?;
    Ok((persisted, snapshot))
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    workspace_root: PathBuf,
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    manifest_path: PathBuf,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TargetLayout {
    crate_name: String,
    package_dir: PathBuf,
    src_path: PathBuf,
    module_dir: PathBuf,
    priority: u8,
}

#[derive(Debug)]
struct ProjectLayout {
    graph_root: String,
    packages: Vec<String>,
    roots: Vec<String>,
    targets: Vec<TargetLayout>,
    units: BTreeMap<PathBuf, SourceUnit>,
    code_root: PathBuf,
}

#[derive(Debug, Clone)]
struct SourceUnit {
    package: String,
    modules: Vec<String>,
    root_path: PathBuf,
    node_id: String,
    parent_id: Option<String>,
    crate_id: String,
}

fn rust_name(name: &str) -> String {
    name.replace('-', "_")
}

fn cargo_metadata(manifest: &Path, opts: &BuildOptions) -> Result<CargoMetadata, AtlasError> {
    let current_dir = manifest.parent().unwrap_or_else(|| Path::new("."));
    let mut command = Command::new("cargo");
    command
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(manifest);
    if !opts.features.is_empty() {
        command.arg("--features").arg(opts.features.join(","));
    }
    if let Some(target) = &opts.target {
        command.arg("--filter-platform").arg(target);
    }
    let output = command
        .current_dir(current_dir)
        .output()
        .map_err(|error| AtlasError::Cargo(format!("cannot run cargo metadata: {error}")))?;
    if !output.status.success() {
        return Err(AtlasError::Cargo(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| AtlasError::Cargo(format!("invalid cargo metadata: {error}")))
}

/// Directories excluded by the root workspace's `[workspace] exclude = [...]`.
/// Hand-parsed (no toml dep) to mirror the crude `[package]` reader; tolerant of
/// single- and multi-line arrays.
fn workspace_excludes(code_root: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(code_root.join("Cargo.toml")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // Find `exclude` key, then the `[ ... ]` span that follows it.
    if let Some(key) = text.find("exclude")
        && let Some(open_rel) = text[key..].find('[')
    {
        let start = key + open_rel + 1;
        if let Some(close_rel) = text[start..].find(']') {
            for piece in text[start..start + close_rel].split(',') {
                let entry = piece.trim().trim_matches(['"', '\'']).trim();
                if !entry.is_empty() {
                    out.push(code_root.join(entry));
                }
            }
        }
    }
    out
}

fn nested_workspace_manifests(code_root: &Path, scope: &scope::AtlasScope) -> Vec<PathBuf> {
    let root_manifest = code_root.join("Cargo.toml");
    let mut manifests: Vec<PathBuf> = scope
        .cargo_manifest_files()
        .into_iter()
        .filter(|path| {
            path != &root_manifest
                && std::fs::read_to_string(path)
                    .is_ok_and(|text| text.lines().any(|line| line.trim() == "[workspace]"))
        })
        .collect();
    manifests.sort();
    manifests
}

fn path_attribute(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        let syn::Meta::NameValue(value) = &attr.meta else {
            return None;
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(path),
            ..
        }) = &value.value
        else {
            return None;
        };
        Some(path.value())
    })
}

fn discover_module_file(
    path: &Path,
    root_path: &Path,
    package: &str,
    modules: &[String],
    units: &mut BTreeMap<PathBuf, SourceUnit>,
    visited: &mut BTreeSet<(PathBuf, PathBuf)>,
) {
    let visit_key = (root_path.to_path_buf(), path.to_path_buf());
    if !visited.insert(visit_key) {
        return;
    }
    units
        .entry(path.to_path_buf())
        .or_insert_with(|| SourceUnit {
            package: package.to_string(),
            modules: modules.to_vec(),
            root_path: root_path.to_path_buf(),
            node_id: String::new(),
            parent_id: None,
            crate_id: String::new(),
        });
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(parsed) = syn::parse_file(&source) else {
        return;
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let module_dir = match path.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "main" | "mod") | None => parent.to_path_buf(),
        Some(stem) => parent.join(stem),
    };
    discover_module_items(
        &parsed.items,
        (&module_dir, parent),
        root_path,
        package,
        modules,
        units,
        visited,
    );
}

fn discover_module_items(
    items: &[syn::Item],
    dirs: (&Path, &Path),
    root_path: &Path,
    package: &str,
    modules: &[String],
    units: &mut BTreeMap<PathBuf, SourceUnit>,
    visited: &mut BTreeSet<(PathBuf, PathBuf)>,
) {
    let (module_dir, path_attr_dir) = dirs;
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let mut child_modules = modules.to_vec();
        child_modules.push(module.ident.to_string());
        if let Some((_, nested)) = &module.content {
            discover_module_items(
                nested,
                (
                    &module_dir.join(module.ident.to_string()),
                    &module_dir.join(module.ident.to_string()),
                ),
                root_path,
                package,
                &child_modules,
                units,
                visited,
            );
            continue;
        }
        let child_path = if let Some(path) = path_attribute(&module.attrs) {
            path_attr_dir.join(path)
        } else {
            let direct = module_dir.join(format!("{}.rs", module.ident));
            if direct.is_file() {
                direct
            } else {
                module_dir.join(module.ident.to_string()).join("mod.rs")
            }
        };
        if child_path.is_file() {
            discover_module_file(
                &child_path,
                root_path,
                package,
                &child_modules,
                units,
                visited,
            );
        }
    }
}

fn structural_node_id(code_root: &Path, path: &Path, unit: &SourceUnit) -> String {
    let symbol = if unit.modules.is_empty() {
        unit.package.clone()
    } else {
        format!("{}::{}", unit.package, unit.modules.join("::"))
    };
    let root = rel_path(code_root, &unit.root_path);
    let file = rel_path(code_root, path);
    let digest = blake3::hash(format!("{root}\0{file}\0{symbol}").as_bytes())
        .to_hex()
        .to_string();
    format!("{symbol}#module-{}", &digest[..12])
}

fn source_units(code_root: &Path, targets: &[TargetLayout]) -> BTreeMap<PathBuf, SourceUnit> {
    let mut units = BTreeMap::new();
    let mut visited = BTreeSet::new();
    for target in targets {
        discover_module_file(
            &target.src_path,
            &target.src_path,
            &target.crate_name,
            &[],
            &mut units,
            &mut visited,
        );
    }
    for (path, unit) in &mut units {
        unit.node_id = structural_node_id(code_root, path, unit);
    }
    let ids: BTreeMap<(PathBuf, Vec<String>), String> = units
        .values()
        .map(|unit| {
            (
                (unit.root_path.clone(), unit.modules.clone()),
                unit.node_id.clone(),
            )
        })
        .collect();
    for unit in units.values_mut() {
        unit.crate_id = ids
            .get(&(unit.root_path.clone(), Vec::new()))
            .cloned()
            .unwrap_or_else(|| unit.node_id.clone());
        if !unit.modules.is_empty() {
            let mut parents = unit.modules.clone();
            parents.pop();
            unit.parent_id = ids.get(&(unit.root_path.clone(), parents)).cloned();
        }
    }
    units
}

impl ProjectLayout {
    fn discover(
        code_root: &Path,
        opts: &BuildOptions,
        scope: &scope::AtlasScope,
    ) -> Result<Self, AtlasError> {
        let manifest = code_root.join("Cargo.toml");
        let root_metadata = cargo_metadata(&manifest, opts)?;
        let root_workspace = root_metadata.workspace_root.clone();
        let mut metadata_sets = vec![root_metadata];
        let mut known_manifests: BTreeSet<PathBuf> = metadata_sets[0]
            .packages
            .iter()
            .map(|package| package.manifest_path.clone())
            .collect();
        for nested_manifest in nested_workspace_manifests(code_root, scope) {
            if known_manifests.contains(&nested_manifest) {
                continue;
            }
            let metadata = cargo_metadata(&nested_manifest, opts)?;
            known_manifests.extend(
                metadata
                    .packages
                    .iter()
                    .map(|package| package.manifest_path.clone()),
            );
            metadata_sets.push(metadata);
        }
        let mut targets = Vec::new();
        let mut packages = BTreeSet::new();
        for metadata in &metadata_sets {
            let members: BTreeSet<&str> = metadata
                .workspace_members
                .iter()
                .map(String::as_str)
                .collect();
            for package in metadata
                .packages
                .iter()
                .filter(|package| members.contains(package.id.as_str()))
            {
                let package_dir = package
                    .manifest_path
                    .parent()
                    .unwrap_or(&metadata.workspace_root)
                    .to_path_buf();
                for target in &package.targets {
                    // Build scripts (`build.rs`) compile to a `custom-build`
                    // target named `build-script-build`; it is not a navigable
                    // crate namespace, so keep it out of the graph.
                    if target.kind.iter().any(|kind| kind == "custom-build") {
                        continue;
                    }
                    let crate_name = rust_name(&target.name);
                    packages.insert(crate_name.clone());
                    let priority = if target
                        .kind
                        .iter()
                        .any(|kind| kind == "lib" || kind == "proc-macro")
                    {
                        0
                    } else if target.kind.iter().any(|kind| kind == "bin") {
                        1
                    } else {
                        2
                    };
                    targets.push(TargetLayout {
                        crate_name,
                        package_dir: package_dir.clone(),
                        module_dir: target
                            .src_path
                            .parent()
                            .unwrap_or(&package_dir)
                            .to_path_buf(),
                        src_path: target.src_path.clone(),
                        priority,
                    });
                }
            }
        }
        targets.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| {
                    right
                        .module_dir
                        .components()
                        .count()
                        .cmp(&left.module_dir.components().count())
                })
                .then_with(|| left.crate_name.cmp(&right.crate_name))
        });
        let packages: Vec<String> = packages.into_iter().collect();
        if targets.is_empty() {
            return Err(AtlasError::Cargo(format!(
                "{} has no Rust targets",
                manifest.display()
            )));
        }
        let graph_root = if packages.len() == 1 {
            packages[0].clone()
        } else {
            root_workspace
                .file_name()
                .map(|name| rust_name(&name.to_string_lossy()))
                .unwrap_or_else(|| "workspace".to_string())
        };
        let units = source_units(code_root, &targets);
        let roots: Vec<String> = units
            .values()
            .filter(|unit| unit.modules.is_empty())
            .map(|unit| unit.node_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Self {
            graph_root,
            packages,
            roots,
            targets,
            units,
            code_root: code_root.to_path_buf(),
        })
    }

    fn from_input_plan(code_root: &Path, plan: &input_plan::CargoInputPlan) -> Self {
        let units = source_units(code_root, &plan.targets);
        let roots = units
            .values()
            .filter(|unit| unit.modules.is_empty())
            .map(|unit| unit.node_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            graph_root: plan.graph_root.clone(),
            packages: plan.packages.clone(),
            roots,
            targets: plan.targets.clone(),
            units,
            code_root: code_root.to_path_buf(),
        }
    }

    fn to_input_plan(
        &self,
        fingerprint: String,
        key: &input_plan::InputPlanKey,
    ) -> input_plan::CargoInputPlan {
        input_plan::CargoInputPlan::new(
            fingerprint,
            self.graph_root.clone(),
            self.packages.clone(),
            self.targets.clone(),
            key,
        )
    }

    fn source_unit(&self, path: &Path) -> Option<SourceUnit> {
        if let Some(unit) = self.units.get(path) {
            return Some(unit.clone());
        }
        if let Some(target) = self.targets.iter().find(|target| target.src_path == path) {
            let mut unit = SourceUnit {
                package: target.crate_name.clone(),
                modules: Vec::new(),
                root_path: target.src_path.clone(),
                node_id: String::new(),
                parent_id: None,
                crate_id: String::new(),
            };
            unit.node_id = structural_node_id(&self.code_root, path, &unit);
            unit.crate_id = unit.node_id.clone();
            return Some(unit);
        }
        let target = self
            .targets
            .iter()
            .filter(|target| path.starts_with(&target.module_dir))
            .min_by_key(|target| {
                (
                    std::cmp::Reverse(target.module_dir.components().count()),
                    target.priority,
                )
            })
            .or_else(|| {
                self.targets
                    .iter()
                    .filter(|target| path.starts_with(&target.package_dir))
                    .min_by_key(|target| {
                        (
                            std::cmp::Reverse(target.package_dir.components().count()),
                            target.priority,
                        )
                    })
            })?;
        let relative = path
            .strip_prefix(&target.module_dir)
            .or_else(|_| path.strip_prefix(&target.package_dir))
            .ok()?;
        let mut unit = SourceUnit {
            package: target.crate_name.clone(),
            modules: module_segments(relative),
            root_path: target.src_path.clone(),
            node_id: String::new(),
            parent_id: None,
            crate_id: String::new(),
        };
        unit.node_id = structural_node_id(&self.code_root, path, &unit);
        let root_unit = SourceUnit {
            package: target.crate_name.clone(),
            modules: Vec::new(),
            root_path: target.src_path.clone(),
            node_id: String::new(),
            parent_id: None,
            crate_id: String::new(),
        };
        unit.crate_id = structural_node_id(&self.code_root, &target.src_path, &root_unit);
        Some(unit)
    }
}

#[cfg(test)]
fn walk_rs_files(code_root: &Path) -> Vec<PathBuf> {
    // Skip files under workspace-excluded directories so build and check agree
    // on the same file set and excluded crates never enter the graph (not even
    // refiled under the host crate via the package-dir fallback).
    let excludes = workspace_excludes(code_root);
    let mut files: Vec<PathBuf> = ignore::WalkBuilder::new(code_root)
        .hidden(true)
        .git_ignore(true)
        .build()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.is_file()
                && path.extension().and_then(|e| e.to_str()) == Some("rs")
                && !path.components().any(|c| c.as_os_str() == "target")
                && !excludes.iter().any(|dir| path.starts_with(dir))
        })
        .collect();
    files.sort();
    files
}

fn rel_path(code_root: &Path, path: &Path) -> String {
    path.strip_prefix(code_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn module_segments(relative: &Path) -> Vec<String> {
    let portable = relative.to_string_lossy().replace('\\', "/");
    let no_ext = portable.strip_suffix(".rs").unwrap_or(&portable);
    let mut segs: Vec<String> = no_ext.split('/').map(rust_name).collect();
    if matches!(
        segs.last().map(String::as_str),
        Some("lib") | Some("main") | Some("mod")
    ) {
        segs.pop();
    }
    segs
}

struct ExtractCtx<'a> {
    package: &'a str,
    crate_id: &'a str,
    rel: &'a str,
    source_lines: Vec<&'a str>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    declaration_counts: BTreeMap<String, usize>,
    cfg_context: Vec<String>,
}

impl ExtractCtx<'_> {
    fn line_of(&self, span: proc_macro2::Span) -> (usize, usize) {
        (span.start().line, span.end().line)
    }

    fn push_declaration(
        &mut self,
        symbol: &str,
        kind: NodeKind,
        span: proc_macro2::Span,
        vis: String,
        signature: String,
        attrs: &[syn::Attribute],
    ) -> String {
        let (line_start, line_end) = self.line_of(span);
        let base_id = declaration_id(self.rel, symbol, &signature, attrs, &self.cfg_context);
        let count = self.declaration_counts.entry(base_id.clone()).or_default();
        *count += 1;
        let id = if *count == 1 {
            base_id
        } else {
            format!("{base_id}~{count}")
        };
        self.nodes.push(Node {
            id: id.clone(),
            symbol: symbol.to_string(),
            kind,
            file: self.rel.to_string(),
            line_start,
            line_end,
            visibility: vis,
            signature,
            doc: doc_first_line(attrs),
            cfg: None,
        });
        id
    }

    fn push_structural_node(
        &mut self,
        id: &str,
        symbol: &str,
        kind: NodeKind,
        line_end: usize,
        visibility: String,
        signature: String,
    ) {
        self.nodes.push(Node {
            id: id.to_string(),
            symbol: symbol.to_string(),
            kind,
            file: self.rel.to_string(),
            line_start: 1,
            line_end,
            visibility,
            signature,
            doc: None,
            cfg: None,
        });
    }

    fn push_contains(&mut self, from: &str, to: &str) {
        self.edges.push(Edge {
            from: from.to_string(),
            to: to.to_string(),
            target_text: None,
            resolution: EdgeResolution::Resolved,
            kind: EdgeKind::Contains,
            provenance: Provenance::Syn,
            site: None,
            extractor: Some(syn_extractor()),
            dispatch: None,
            confidence: Some(EdgeConfidence::Exact),
            candidates: Vec::new(),
            evidence: None,
            generic: false,
        });
    }
}

fn normalized_tokens(value: &impl ToTokens) -> String {
    value
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Declaration-head signature (`pub struct Foo<T> where T: Bound`) without the
/// field/variant body, so struct/enum/union signatures stay compact instead of
/// embedding the whole type.
fn head_signature(
    vis: &syn::Visibility,
    keyword: &str,
    ident: &syn::Ident,
    generics: &syn::Generics,
) -> String {
    let (_, ty_generics, where_clause) = generics.split_for_impl();
    let mut parts: Vec<String> = Vec::new();
    let vis_s = normalized_tokens(vis);
    if !vis_s.is_empty() {
        parts.push(vis_s);
    }
    parts.push(keyword.to_string());
    parts.push(ident.to_string());
    let gen_s = normalized_tokens(&ty_generics);
    if !gen_s.is_empty() {
        parts.push(gen_s.replace(" :: ", "::"));
    }
    let where_s = normalized_tokens(&where_clause);
    if !where_s.is_empty() {
        parts.push(where_s.replace(" :: ", "::"));
    }
    parts.join(" ")
}

fn doc_first_line(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("doc") {
            return None;
        }
        let syn::Meta::NameValue(value) = &attr.meta else {
            return None;
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(text),
            ..
        }) = &value.value
        else {
            return None;
        };
        text.value()
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_string)
    })
}

fn cfg_identity(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        .map(normalized_tokens)
        .collect()
}

fn declaration_id(
    rel: &str,
    symbol: &str,
    signature: &str,
    attrs: &[syn::Attribute],
    cfg_context: &[String],
) -> String {
    let cfg = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        .map(normalized_tokens)
        .chain(cfg_context.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");
    let digest = blake3::hash(format!("{rel}\0{symbol}\0{signature}\0{cfg}").as_bytes())
        .to_hex()
        .to_string();
    format!("{symbol}#{}", &digest[..12])
}

fn vis_string(vis: &syn::Visibility) -> String {
    match vis {
        syn::Visibility::Public(_) => "pub".to_string(),
        syn::Visibility::Restricted(r) => format!(
            "pub({})",
            r.path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
        ),
        syn::Visibility::Inherited => "private".to_string(),
    }
}

fn path_text(path: &syn::Path) -> String {
    normalized_tokens(path).replace(" :: ", "::")
}

fn path_base_text(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn type_text(ty: &syn::Type) -> String {
    normalized_tokens(ty).replace(" :: ", "::")
}

/// Resolve a possibly-bare path against the local module's item names.
fn resolve_path(
    raw: &str,
    module_id: &str,
    package: &str,
    local_names: &BTreeSet<String>,
) -> String {
    if let Some(rest) = raw.strip_prefix("crate::") {
        return format!("{package}::{rest}");
    }
    if let Some(rest) = raw.strip_prefix("self::") {
        return format!("{module_id}::{rest}");
    }
    if let Some(mut rest) = raw.strip_prefix("super::") {
        let mut parent = module_id.to_string();
        while let Some(next) = rest.strip_prefix("super::") {
            parent = parent
                .rsplit_once("::")
                .map(|(parent, _)| parent.to_string())
                .unwrap_or_else(|| package.to_string());
            rest = next;
        }
        parent = parent
            .rsplit_once("::")
            .map(|(parent, _)| parent.to_string())
            .unwrap_or_else(|| package.to_string());
        return format!("{parent}::{rest}");
    }
    let first = raw.split("::").next().unwrap_or(raw);
    if local_names.contains(first) {
        return format!("{module_id}::{raw}");
    }
    raw.to_string()
}

fn extract_shard(unit: &SourceUnit, rel: &str, hash: &str, source: &str) -> Shard {
    let parsed = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => {
            return Shard {
                file: rel.to_string(),
                hash: hash.to_string(),
                unparsed: Some(format!("{rel}: {error}")),
                nodes: Vec::new(),
                edges: Vec::new(),
            };
        }
    };

    let mut ctx = ExtractCtx {
        package: &unit.package,
        crate_id: &unit.crate_id,
        rel,
        source_lines: source.lines().collect(),
        nodes: Vec::new(),
        edges: Vec::new(),
        declaration_counts: BTreeMap::new(),
        cfg_context: Vec::new(),
    };

    let package = &unit.package;
    let segs = &unit.modules;
    let module_id = if segs.is_empty() {
        ctx.push_structural_node(
            &unit.node_id,
            package,
            NodeKind::Crate,
            ctx.source_lines.len().max(1),
            "pub".to_string(),
            format!("crate {package}"),
        );
        package.to_string()
    } else {
        let module_id = format!("{package}::{}", segs.join("::"));
        let name = segs.last().map(String::as_str).unwrap_or(package);
        ctx.push_structural_node(
            &unit.node_id,
            &module_id,
            NodeKind::Module,
            ctx.source_lines.len().max(1),
            "pub".to_string(),
            format!("mod {name}"),
        );
        if let Some(parent_id) = &unit.parent_id {
            ctx.push_contains(parent_id, &unit.node_id);
        }
        module_id
    };

    extract_items(&mut ctx, &parsed.items, &module_id, &unit.node_id);

    let mut edges: Vec<Edge> = std::mem::take(&mut ctx.edges);
    edges.sort();
    edges.dedup();
    Shard {
        file: rel.to_string(),
        hash: hash.to_string(),
        unparsed: None,
        nodes: ctx.nodes,
        edges,
    }
}

fn extract_items(
    ctx: &mut ExtractCtx<'_>,
    items: &[syn::Item],
    module_symbol: &str,
    container_id: &str,
) {
    use syn::Item;
    use syn::spanned::Spanned;

    let local_names: BTreeSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(i) => Some(i.ident.to_string()),
            Item::Enum(i) => Some(i.ident.to_string()),
            Item::Union(i) => Some(i.ident.to_string()),
            Item::Trait(i) => Some(i.ident.to_string()),
            Item::TraitAlias(i) => Some(i.ident.to_string()),
            Item::Fn(i) => Some(i.sig.ident.to_string()),
            Item::Type(i) => Some(i.ident.to_string()),
            Item::Const(i) => Some(i.ident.to_string()),
            Item::Static(i) => Some(i.ident.to_string()),
            _ => None,
        })
        .collect();

    for item in items {
        match item {
            Item::Struct(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::Struct,
                    i.span(),
                    vis_string(&i.vis),
                    head_signature(&i.vis, "struct", &i.ident, &i.generics),
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::Enum(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::Enum,
                    i.span(),
                    vis_string(&i.vis),
                    head_signature(&i.vis, "enum", &i.ident, &i.generics),
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::Union(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::Union,
                    i.span(),
                    vis_string(&i.vis),
                    head_signature(&i.vis, "union", &i.ident, &i.generics),
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::Static(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let vis = &i.vis;
                let ident = &i.ident;
                let ty = &i.ty;
                let signature = normalized_tokens(&quote::quote!(#vis static #ident : #ty))
                    .replace(" :: ", "::");
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::Static,
                    i.span(),
                    vis_string(&i.vis),
                    signature,
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::TraitAlias(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let vis = &i.vis;
                let ident = &i.ident;
                let generics = &i.generics;
                let bounds = &i.bounds;
                let signature =
                    normalized_tokens(&quote::quote!(#vis trait #ident #generics = #bounds))
                        .replace(" :: ", "::");
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::TraitAlias,
                    i.span(),
                    vis_string(&i.vis),
                    signature,
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::Trait(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let mut declaration = i.clone();
                declaration.attrs.clear();
                declaration.items.clear();
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::Trait,
                    i.span(),
                    vis_string(&i.vis),
                    normalized_tokens(&declaration),
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
                for ti in &i.items {
                    let (member, kind, span, signature, attrs) = match ti {
                        syn::TraitItem::Fn(f) => (
                            f.sig.ident.to_string(),
                            NodeKind::Fn,
                            f.span(),
                            normalized_tokens(&f.sig).replace(" :: ", "::"),
                            &f.attrs,
                        ),
                        syn::TraitItem::Const(c) => {
                            let ident = &c.ident;
                            let ty = &c.ty;
                            (
                                c.ident.to_string(),
                                NodeKind::Const,
                                c.span(),
                                normalized_tokens(&quote::quote!(const #ident : #ty))
                                    .replace(" :: ", "::"),
                                &c.attrs,
                            )
                        }
                        syn::TraitItem::Type(t) => {
                            let ident = &t.ident;
                            (
                                t.ident.to_string(),
                                NodeKind::TypeAlias,
                                t.span(),
                                normalized_tokens(&quote::quote!(type #ident)),
                                &t.attrs,
                            )
                        }
                        _ => continue,
                    };
                    let member_symbol = format!("{symbol}::{member}");
                    let mid = ctx.push_declaration(
                        &member_symbol,
                        kind,
                        span,
                        "pub".to_string(),
                        signature,
                        attrs,
                    );
                    ctx.push_contains(&id, &mid);
                }
            }
            Item::Fn(i) => {
                let symbol = format!("{module_symbol}::{}", i.sig.ident);
                let vis = &i.vis;
                let sig = &i.sig;
                let signature = normalized_tokens(&quote::quote!(#vis #sig));
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::Fn,
                    i.span(),
                    vis_string(&i.vis),
                    signature,
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::Type(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let mut declaration = i.clone();
                declaration.attrs.clear();
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::TypeAlias,
                    i.span(),
                    vis_string(&i.vis),
                    normalized_tokens(&declaration),
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::Const(i) => {
                let symbol = format!("{module_symbol}::{}", i.ident);
                let mut declaration = i.clone();
                declaration.attrs.clear();
                let id = ctx.push_declaration(
                    &symbol,
                    NodeKind::Const,
                    i.span(),
                    vis_string(&i.vis),
                    normalized_tokens(&declaration),
                    &i.attrs,
                );
                ctx.push_contains(container_id, &id);
            }
            Item::Macro(i) => {
                if let Some(ident) = &i.ident {
                    // macro_rules! export at crate root by convention
                    let symbol = format!("{}::{ident}", ctx.package);
                    let signature = format!("macro_rules! {ident}");
                    let id = ctx.push_declaration(
                        &symbol,
                        NodeKind::Macro,
                        i.span(),
                        "pub".to_string(),
                        signature,
                        &i.attrs,
                    );
                    ctx.push_contains(ctx.crate_id, &id);
                }
            }
            Item::Impl(i) => {
                let self_ty = resolve_path(
                    &type_text(&i.self_ty),
                    module_symbol,
                    ctx.package,
                    &local_names,
                );
                let self_target = match i.self_ty.as_ref() {
                    syn::Type::Path(path) => resolve_path(
                        &path_base_text(&path.path),
                        module_symbol,
                        ctx.package,
                        &local_names,
                    ),
                    _ => self_ty.clone(),
                };
                let (impl_symbol, trait_id) = match &i.trait_ {
                    Some((_, path, _)) => {
                        let trait_id = resolve_path(
                            &path_base_text(path),
                            module_symbol,
                            ctx.package,
                            &local_names,
                        );
                        let trait_display = resolve_path(
                            &path_text(path),
                            module_symbol,
                            ctx.package,
                            &local_names,
                        );
                        (
                            format!("{module_symbol}::impl {trait_display} for {self_ty}"),
                            Some(trait_id),
                        )
                    }
                    None => (format!("{module_symbol}::impl {self_ty}"), None),
                };
                let mut declaration = i.clone();
                declaration.attrs.clear();
                declaration.items.clear();
                let impl_id = ctx.push_declaration(
                    &impl_symbol,
                    NodeKind::Impl,
                    i.span(),
                    "private".to_string(),
                    normalized_tokens(&declaration),
                    &i.attrs,
                );
                ctx.push_contains(container_id, &impl_id);
                if let Some(trait_id) = trait_id {
                    ctx.edges.push(Edge {
                        from: impl_id.clone(),
                        to: trait_id.clone(),
                        target_text: Some(trait_id),
                        resolution: EdgeResolution::Unresolved,
                        kind: EdgeKind::ImplsTrait,
                        provenance: Provenance::Syn,
                        site: None,
                        extractor: Some(syn_extractor()),
                        dispatch: None,
                        confidence: None,
                        candidates: Vec::new(),
                        evidence: None,
                        generic: false,
                    });
                }
                ctx.edges.push(Edge {
                    from: impl_id.clone(),
                    to: self_target.clone(),
                    target_text: Some(self_target),
                    resolution: EdgeResolution::Unresolved,
                    kind: EdgeKind::ImplFor,
                    provenance: Provenance::Syn,
                    site: None,
                    extractor: Some(syn_extractor()),
                    dispatch: None,
                    confidence: None,
                    candidates: Vec::new(),
                    evidence: None,
                    generic: false,
                });
                for ii in &i.items {
                    let (member, kind, span, vis, signature, attrs) = match ii {
                        syn::ImplItem::Fn(f) => {
                            let vis = &f.vis;
                            let sig = &f.sig;
                            (
                                f.sig.ident.to_string(),
                                NodeKind::Fn,
                                f.span(),
                                vis_string(&f.vis),
                                normalized_tokens(&quote::quote!(#vis #sig)).replace(" :: ", "::"),
                                &f.attrs,
                            )
                        }
                        syn::ImplItem::Const(c) => {
                            let vis = &c.vis;
                            let ident = &c.ident;
                            let ty = &c.ty;
                            (
                                c.ident.to_string(),
                                NodeKind::Const,
                                c.span(),
                                vis_string(&c.vis),
                                normalized_tokens(&quote::quote!(#vis const #ident : #ty))
                                    .replace(" :: ", "::"),
                                &c.attrs,
                            )
                        }
                        syn::ImplItem::Type(t) => {
                            let vis = &t.vis;
                            let ident = &t.ident;
                            let aliased = &t.ty;
                            (
                                t.ident.to_string(),
                                NodeKind::TypeAlias,
                                t.span(),
                                vis_string(&t.vis),
                                normalized_tokens(&quote::quote!(#vis type #ident = #aliased))
                                    .replace(" :: ", "::"),
                                &t.attrs,
                            )
                        }
                        _ => continue,
                    };
                    let member_symbol = format!("{impl_symbol}::{member}");
                    let mid =
                        ctx.push_declaration(&member_symbol, kind, span, vis, signature, attrs);
                    ctx.push_contains(&impl_id, &mid);
                }
            }
            Item::Mod(i) => {
                if let Some((_, nested)) = &i.content {
                    let child = format!("{module_symbol}::{}", i.ident);
                    let vis = &i.vis;
                    let ident = &i.ident;
                    let child_id = ctx.push_declaration(
                        &child,
                        NodeKind::Module,
                        i.span(),
                        vis_string(&i.vis),
                        normalized_tokens(&quote::quote!(#vis mod #ident)),
                        &i.attrs,
                    );
                    ctx.push_contains(container_id, &child_id);
                    let context_len = ctx.cfg_context.len();
                    ctx.cfg_context.extend(cfg_identity(&i.attrs));
                    extract_items(ctx, nested, &child, &child_id);
                    ctx.cfg_context.truncate(context_len);
                }
            }
            _ => {}
        }
    }
}

// ── SCIP overlay ────────────────────────────────────────────────────

// ── JSON form (hand-authored fixtures; kept for backward compatibility) ──

#[derive(Deserialize)]
struct ScipJsonIndex {
    #[serde(default)]
    metadata: Option<ScipJsonMetadata>,
    #[serde(default)]
    documents: Vec<ScipJsonDocument>,
}

#[derive(Deserialize)]
struct ScipJsonMetadata {
    #[serde(default)]
    tool_info: Option<ScipJsonToolInfo>,
}

#[derive(Deserialize)]
struct ScipJsonToolInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
}

#[derive(Deserialize)]
struct ScipJsonDocument {
    relative_path: String,
    #[serde(default)]
    occurrences: Vec<ScipJsonOccurrence>,
}

#[derive(Deserialize)]
struct ScipJsonOccurrence {
    symbol: String,
    #[serde(default)]
    symbol_roles: u32,
    range: Vec<u32>,
}

// ── Neutral model (both JSON and protobuf decode into this) ──

/// How a SCIP target symbol projects onto an atlas edge.
///
/// `Trait` and `DataType` both yield `UsesType` for ordinary references, but
/// are kept distinct so `impl` headers can tell the implemented trait from the
/// implementing type (`ImplsTrait` vs `ImplFor`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScipKind {
    Call,
    Trait,
    DataType,
    Other,
}

struct ScipModel {
    tool: Option<String>,
    documents: Vec<ScipDoc>,
    /// SCIP symbol string → classified kind (from `SymbolInformation.kind`).
    /// Empty for JSON indexes, which carry occurrences only.
    kinds: BTreeMap<String, ScipKind>,
}

struct ScipDoc {
    relative_path: String,
    occurrences: Vec<ScipOcc>,
}

struct ScipOcc {
    symbol: String,
    is_definition: bool,
    site: EdgeSite,
}

impl ScipOcc {
    fn edge_kind(&self, kinds: &BTreeMap<String, ScipKind>) -> EdgeKind {
        match kinds.get(&self.symbol) {
            Some(ScipKind::Call) => EdgeKind::Calls,
            Some(ScipKind::Trait) | Some(ScipKind::DataType) => EdgeKind::UsesType,
            _ => EdgeKind::References,
        }
    }
}

/// Dispatch on content: a UTF-8 payload whose first non-space byte is `{` is the
/// hand-authored JSON form; anything else is decoded as protobuf (the only shape
/// `rust-analyzer scip` emits).
fn load_scip_with_fingerprint(index_path: &Path) -> Result<(ScipModel, String), AtlasError> {
    let bytes = std::fs::read(index_path)
        .map_err(|e| AtlasError::Scip(format!("cannot read {}: {e}", index_path.display())))?;
    let fingerprint = blake3::hash(&bytes).to_hex().to_string();
    let model = parse_scip(bytes)?;
    Ok((model, fingerprint))
}

#[cfg(test)]
fn load_scip(index_path: &Path) -> Result<ScipModel, AtlasError> {
    load_scip_with_fingerprint(index_path).map(|(model, _)| model)
}

fn parse_scip(bytes: Vec<u8>) -> Result<ScipModel, AtlasError> {
    let looks_json = bytes.iter().find(|b| !b.is_ascii_whitespace()).copied() == Some(b'{');
    if looks_json {
        let text = String::from_utf8(bytes)
            .map_err(|e| AtlasError::Scip(format!("index is not valid UTF-8 JSON: {e}")))?;
        load_scip_json(&text)
    } else {
        load_scip_protobuf(&bytes)
    }
}

fn load_scip_json(text: &str) -> Result<ScipModel, AtlasError> {
    let index: ScipJsonIndex =
        serde_json::from_str(text).map_err(|e| AtlasError::Scip(e.to_string()))?;
    let documents = index
        .documents
        .into_iter()
        .map(|doc| {
            let relative_path = doc.relative_path;
            let occurrences = doc
                .occurrences
                .into_iter()
                .filter_map(|occ| {
                    let site = normalize_scip_site(&relative_path, &occ.range)?;
                    Some(ScipOcc {
                        symbol: occ.symbol,
                        is_definition: occ.symbol_roles & 1 == 1,
                        site,
                    })
                })
                .collect();
            ScipDoc {
                relative_path,
                occurrences,
            }
        })
        .collect();
    let tool = index
        .metadata
        .and_then(|m| m.tool_info)
        .map(|t| format!("{} {}", t.name, t.version));
    Ok(ScipModel {
        tool,
        documents,
        kinds: BTreeMap::new(),
    })
}

fn load_scip_protobuf(bytes: &[u8]) -> Result<ScipModel, AtlasError> {
    use protobuf::Message;
    let index = scip::types::Index::parse_from_bytes(bytes)
        .map_err(|e| AtlasError::Scip(format!("cannot decode protobuf index: {e}")))?;

    let mut kinds = BTreeMap::new();
    let mut record_kind = |sym: &str, raw: scip::types::symbol_information::Kind| {
        kinds.insert(sym.to_string(), classify_scip_kind(raw));
    };
    for sym in &index.external_symbols {
        record_kind(&sym.symbol, sym.kind.enum_value_or_default());
    }
    for doc in &index.documents {
        for sym in &doc.symbols {
            record_kind(&sym.symbol, sym.kind.enum_value_or_default());
        }
    }

    let documents = index
        .documents
        .iter()
        .map(|doc| {
            let relative_path = doc.relative_path.clone();
            let occurrences = doc
                .occurrences
                .iter()
                .filter_map(|occ| {
                    let range = occ
                        .range
                        .iter()
                        .map(|value| u32::try_from(*value).ok())
                        .collect::<Option<Vec<_>>>()?;
                    normalize_scip_site(&relative_path, &range).map(|site| ScipOcc {
                        symbol: occ.symbol.clone(),
                        is_definition: occ.symbol_roles & 1 == 1,
                        site,
                    })
                })
                .collect();
            ScipDoc {
                relative_path,
                occurrences,
            }
        })
        .collect();

    let tool = index
        .metadata
        .as_ref()
        .and_then(|m| m.tool_info.as_ref())
        .map(|t| format!("{} {}", t.name, t.version));

    Ok(ScipModel {
        tool,
        documents,
        kinds,
    })
}

fn classify_scip_kind(kind: scip::types::symbol_information::Kind) -> ScipKind {
    use scip::types::symbol_information::Kind as K;
    match kind {
        K::Method | K::TraitMethod | K::StaticMethod | K::Function | K::Macro => ScipKind::Call,
        K::Trait => ScipKind::Trait,
        K::Struct
        | K::Enum
        | K::Union
        | K::TypeAlias
        | K::Class
        | K::Interface
        | K::TypeParameter => ScipKind::DataType,
        _ => ScipKind::Other,
    }
}

fn normalize_scip_site(file: &str, range: &[u32]) -> Option<EdgeSite> {
    fn one_based(value: u32) -> Option<usize> {
        usize::try_from(value).ok()?.checked_add(1)
    }

    let (start_line, start_column, end_line, end_column) = match range {
        [start_line, start_column, end_column] => {
            (*start_line, *start_column, *start_line, *end_column)
        }
        [start_line, start_column, end_line, end_column] => {
            (*start_line, *start_column, *end_line, *end_column)
        }
        _ => return None,
    };
    Some(EdgeSite {
        file: file.to_string(),
        line_start: one_based(start_line)?,
        column_start: one_based(start_column)?,
        line_end: one_based(end_line)?,
        column_end: one_based(end_column)?,
    })
}

fn scip_occurrence_evidence(site: &EdgeSite, symbol: &str, candidates: usize) -> String {
    let resolution = if candidates > 1 {
        format!("multiple candidates ({candidates})")
    } else {
        "one target".to_string()
    };
    format!(
        "rust-analyzer-scip occurrence at {}:{}:{}-{}:{} for `{symbol}`: {resolution}",
        site.file, site.line_start, site.column_start, site.line_end, site.column_end
    )
}

/// Overlay semantic edges from a SCIP index (protobuf or JSON) onto the shards.
///
/// Adds `Provenance::Scip` edges only; the syn baseline is never mutated, so
/// `remove_scip_edges` reverts cleanly. Reference occurrences become
/// `Calls`/`UsesType`/`References` by target-symbol kind, and `impl` headers
/// resolve `ImplsTrait`/`ImplFor` to the canonical trait/type node (or
/// `External` when the target is defined outside this workspace).
fn overlay_scip(
    shards_dir: &Path,
    index_path: &Path,
    files: &BTreeMap<String, String>,
) -> Result<(Option<String>, String), AtlasError> {
    fn push_edge(
        shards: &mut BTreeMap<String, Shard>,
        changed: &mut BTreeSet<String>,
        rel: &str,
        edge: Edge,
    ) {
        if let Some(shard) = shards.get_mut(rel)
            && !shard.edges.contains(&edge)
        {
            shard.edges.push(edge);
            changed.insert(rel.to_string());
        }
    }

    let (model, fingerprint) = load_scip_with_fingerprint(index_path)?;

    let mut shards: BTreeMap<String, Shard> = BTreeMap::new();
    for rel in files.keys() {
        shards.insert(rel.clone(), read_shard(shards_dir, rel)?);
    }
    // Recompute SCIP edges from scratch, including shards that only lose edges.
    let mut changed: BTreeSet<String> = BTreeSet::new();
    for (rel, shard) in &mut shards {
        let old_len = shard.edges.len();
        shard.edges.retain(|e| e.provenance != Provenance::Scip);
        if shard.edges.len() != old_len {
            changed.insert(rel.clone());
        }
    }

    let containing_node = |shard: &Shard, line: usize| -> Option<String> {
        shard
            .nodes
            .iter()
            .filter(|n| {
                n.line_start <= line
                    && line <= n.line_end
                    && n.kind != NodeKind::Crate
                    && n.kind != NodeKind::Module
            })
            .min_by_key(|n| n.line_end - n.line_start)
            .map(|n| n.id.clone())
    };

    // pass 1: definition occurrences → canonical atlas node id
    let mut defs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for doc in &model.documents {
        let Some(shard) = shards.get(&doc.relative_path) else {
            continue;
        };
        for occ in &doc.occurrences {
            if occ.is_definition
                && let Some(node_id) = containing_node(shard, occ.site.line_start)
            {
                defs.entry(occ.symbol.clone()).or_default().insert(node_id);
            }
        }
    }

    // pass 2: reference occurrences → Calls / UsesType / References (resolved only)
    for doc in &model.documents {
        let rel = doc.relative_path.clone();
        for occ in &doc.occurrences {
            if occ.is_definition {
                continue;
            }
            let from = {
                let Some(shard) = shards.get(&rel) else {
                    continue;
                };
                match containing_node(shard, occ.site.line_start) {
                    Some(from) => from,
                    None => continue,
                }
            };
            let (to, resolution, confidence, candidates) = match defs.get(&occ.symbol) {
                Some(ids) if ids.len() == 1 => {
                    let Some(target) = ids.iter().next() else {
                        continue;
                    };
                    (
                        target.clone(),
                        EdgeResolution::Resolved,
                        EdgeConfidence::Exact,
                        Vec::new(),
                    )
                }
                Some(ids) => (
                    occ.symbol.clone(),
                    EdgeResolution::Unresolved,
                    EdgeConfidence::BoundedCandidates,
                    ids.iter().cloned().collect(),
                ),
                None => continue,
            };
            if from == to {
                continue;
            }
            let edge = Edge {
                from,
                to,
                target_text: Some(occ.symbol.clone()),
                resolution,
                kind: occ.edge_kind(&model.kinds),
                provenance: Provenance::Scip,
                site: Some(occ.site.clone()),
                extractor: Some(scip_extractor(model.tool.as_deref())),
                dispatch: None,
                confidence: Some(confidence),
                evidence: Some(scip_occurrence_evidence(
                    &occ.site,
                    &occ.symbol,
                    candidates.len(),
                )),
                candidates,
                generic: false,
            };
            push_edge(&mut shards, &mut changed, &rel, edge);
        }
    }

    // pass 3: impl headers → resolved ImplsTrait / ImplFor.
    //
    // rust-analyzer 1.92 leaves SCIP `relationships` empty, so recover the
    // implemented trait and implementing type from the reference occurrences on
    // the impl node's own declaration line (`impl Trait for Type`): the
    // trait-kinded target yields ImplsTrait, the data-type target yields ImplFor.
    // Targets in the graph resolve to a node id; targets defined elsewhere are
    // marked External (honest, and skipped by `validate_graph`'s endpoint check).
    let impl_sites: Vec<(String, String, usize)> = shards
        .iter()
        .flat_map(|(rel, shard)| {
            shard
                .nodes
                .iter()
                .filter(|n| n.kind == NodeKind::Impl)
                .map(move |n| (rel.clone(), n.id.clone(), n.line_start))
        })
        .collect();
    for (rel, impl_id, line_start) in impl_sites {
        let Some(doc) = model.documents.iter().find(|d| d.relative_path == rel) else {
            continue;
        };
        for occ in &doc.occurrences {
            if occ.is_definition || occ.site.line_start != line_start {
                continue;
            }
            let kind = match model.kinds.get(&occ.symbol) {
                Some(ScipKind::Trait) => EdgeKind::ImplsTrait,
                Some(ScipKind::DataType) => EdgeKind::ImplFor,
                _ => continue,
            };
            let (to, resolution, confidence, candidates) = match defs.get(&occ.symbol) {
                Some(ids) if ids.len() == 1 => {
                    let Some(node_id) = ids.iter().next() else {
                        continue;
                    };
                    (
                        node_id.clone(),
                        EdgeResolution::Resolved,
                        EdgeConfidence::Exact,
                        Vec::new(),
                    )
                }
                Some(ids) => (
                    occ.symbol.clone(),
                    EdgeResolution::Unresolved,
                    EdgeConfidence::BoundedCandidates,
                    ids.iter().cloned().collect(),
                ),
                None => (
                    occ.symbol.clone(),
                    EdgeResolution::External,
                    EdgeConfidence::Exact,
                    Vec::new(),
                ),
            };
            if to == impl_id {
                continue;
            }
            let edge = Edge {
                from: impl_id.clone(),
                to,
                target_text: Some(occ.symbol.clone()),
                resolution,
                kind,
                provenance: Provenance::Scip,
                site: Some(occ.site.clone()),
                extractor: Some(scip_extractor(model.tool.as_deref())),
                dispatch: None,
                confidence: Some(confidence),
                evidence: Some(scip_occurrence_evidence(
                    &occ.site,
                    &occ.symbol,
                    candidates.len(),
                )),
                candidates,
                generic: false,
            };
            push_edge(&mut shards, &mut changed, &rel, edge);
        }
    }

    for rel in changed {
        if let Some(shard) = shards.get_mut(&rel) {
            shard.edges.sort();
            shard.edges.dedup();
            write_shard(shards_dir, shard)?;
        }
    }
    Ok((model.tool, fingerprint))
}

/// Generate a SCIP index by invoking `rust-analyzer scip`, writing it to
/// `out_path`. Returns the output path on success.
///
/// Never panics: a missing binary, a non-zero exit, or a missing output all map
/// to an actionable `AtlasError::Scip`. This is opt-in tooling — the syn baseline
/// never depends on it.
pub fn generate_scip(
    code_root: &Path,
    out_path: &Path,
    ra_cmd: &str,
) -> Result<PathBuf, AtlasError> {
    let code_root = std::fs::canonicalize(code_root).map_err(io_err)?;
    // rust-analyzer writes `index.scip` into its cwd; run it inside the code
    // root, then relocate the artifact to `out_path`.
    let output = std::process::Command::new(ra_cmd)
        .arg("scip")
        .arg(".")
        .current_dir(&code_root)
        .output()
        .map_err(|e| {
            AtlasError::Scip(format!(
                "cannot run rust-analyzer (`{ra_cmd} scip .`): {e}. \
                 Install it (`rustup component add rust-analyzer`) or pass a valid --ra <path>."
            ))
        })?;
    if !output.status.success() {
        return Err(AtlasError::Scip(format!(
            "rust-analyzer scip failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let produced = code_root.join("index.scip");
    if !produced.exists() {
        return Err(AtlasError::Scip(
            "rust-analyzer scip exited 0 but produced no index.scip".to_string(),
        ));
    }
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(io_err)?;
    }
    // `rename` fails across filesystems; fall back to copy + remove.
    if std::fs::rename(&produced, out_path).is_err() {
        std::fs::copy(&produced, out_path).map_err(io_err)?;
        let _ = std::fs::remove_file(&produced);
    }
    Ok(out_path.to_path_buf())
}

fn remove_scip_edges(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
) -> Result<(), AtlasError> {
    for rel in files.keys() {
        let mut shard = read_shard(shards_dir, rel)?;
        let old_len = shard.edges.len();
        shard
            .edges
            .retain(|edge| edge.provenance != Provenance::Scip);
        if shard.edges.len() != old_len {
            write_shard(shards_dir, &shard)?;
        }
    }
    Ok(())
}

fn remove_mir_facts(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
    opts: &BuildOptions,
) -> Result<(), AtlasError> {
    for batch in files.keys().collect::<Vec<_>>().chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            let mut shard = read_shard(shards_dir, rel)?;
            let old_edges = shard.edges.len();
            shard
                .edges
                .retain(|edge| edge.provenance != Provenance::Mir);
            let mut changed = shard.edges.len() != old_edges;
            for node in &mut shard.nodes {
                changed |= node.cfg.take().is_some();
            }
            if changed {
                write_shard(shards_dir, &shard)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    type QueryIndexMutation = fn(&mut serde_json::Value);
    type QueryIndexErrorMatcher = fn(&AtlasError) -> bool;
    type LowLevelQuery = fn(&Path, &Path, &QueryOptions) -> Result<(), AtlasError>;
    type QueryIndexSetup = fn(&Path);
    type QueryIndexCase = (&'static str, QueryIndexSetup, QueryIndexErrorMatcher);
    type BorrowedQueryIndexCase = (&'static str, QueryIndexSetup);

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas/basic")
    }

    fn scip_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas/scip/index.json")
    }

    /// Real `rust-analyzer scip` output (protobuf) for the atlas-basic fixture.
    fn scip_protobuf_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas/scip/index.scip")
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Copy the fixture crate into a temp dir so tests can mutate freely.
    fn copy_fixture(name: &str) -> (PathBuf, PathBuf) {
        let base = temp_dir(name);
        let code = base.join("code");
        fs::create_dir_all(code.join("src")).unwrap();
        for rel in ["Cargo.toml", "src/lib.rs", "src/store.rs", "src/service.rs"] {
            fs::copy(fixture_root().join(rel), code.join(rel)).unwrap();
        }
        let graph = base.join("graph");
        (code, graph)
    }

    fn copy_incremental_fixture(name: &str) -> (PathBuf, PathBuf) {
        let base = temp_dir(name);
        let code = base.join("code");
        fs::create_dir_all(code.join("src")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/atlas/incremental-hardening");
        for rel in [
            "Cargo.toml",
            "src/lib.rs",
            "src/api.rs",
            "src/a.rs",
            "src/b.rs",
            "src/c.rs",
        ] {
            fs::copy(fixture.join(rel), code.join(rel)).unwrap();
        }
        (code, base.join("graph"))
    }

    #[test]
    fn test_atlas_live_scope_is_shared_by_build_and_watcher() {
        let base = temp_dir("atlas-live-shared-scope");
        let code = base.join("code");
        let graph = code.join(".agent-spec/graph");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::create_dir_all(code.join(".cargo")).unwrap();
        fs::create_dir_all(code.join("target/generated")).unwrap();
        fs::create_dir_all(code.join("ignored")).unwrap();
        fs::create_dir_all(code.join("excluded/src")).unwrap();
        fs::create_dir_all(graph.join(".runtime")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[workspace]\nexclude = [\"excluded\"]\n",
        )
        .unwrap();
        fs::write(code.join("Cargo.lock"), "# lock\n").unwrap();
        fs::write(code.join(".cargo/config.toml"), "[build]\n").unwrap();
        fs::write(code.join(".gitignore"), "ignored/\n").unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn live() {}\n").unwrap();
        fs::write(code.join("target/generated/hidden.rs"), "fn hidden() {}\n").unwrap();
        fs::write(code.join("ignored/hidden.rs"), "fn hidden() {}\n").unwrap();
        fs::write(code.join("excluded/src/lib.rs"), "fn excluded() {}\n").unwrap();
        fs::write(graph.join(".runtime/Cargo.toml"), "[workspace]\n").unwrap();
        let outside = base.join("outside.rs");
        fs::write(&outside, "fn outside() {}\n").unwrap();
        let code = fs::canonicalize(code).unwrap();
        let graph = code.join(".agent-spec/graph");
        let outside = fs::canonicalize(outside).unwrap();

        let scope = crate::scope::AtlasScope::discover(&code, &graph).unwrap();
        assert_eq!(
            scope
                .source_files()
                .iter()
                .map(|path| rel_path(&code, path))
                .collect::<Vec<_>>(),
            vec!["src/lib.rs"]
        );
        assert_eq!(
            crate::input_plan::input_hashes(&scope)
                .unwrap()
                .keys()
                .filter_map(|path| Path::new(path).strip_prefix(&code).ok())
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .collect::<Vec<_>>(),
            vec![".cargo/config.toml", "Cargo.lock", "Cargo.toml"]
        );
        assert_eq!(
            scope.classify(&code.join("src/lib.rs")).unwrap(),
            crate::scope::ScopeEntryKind::RustSource
        );
        assert_eq!(
            scope.classify(&code.join("Cargo.toml")).unwrap(),
            crate::scope::ScopeEntryKind::CargoInput
        );
        for rejected in [
            code.join("target/generated/hidden.rs"),
            code.join("ignored/hidden.rs"),
            code.join("excluded/src/lib.rs"),
            graph.join(".runtime/Cargo.toml"),
            outside,
        ] {
            assert_eq!(
                scope.classify(&rejected).unwrap(),
                crate::scope::ScopeEntryKind::Ignored,
                "{}",
                rejected.display()
            );
        }
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_atlas_live_status_scopes_pending_paths_to_query() {
        let (code, graph) = copy_fixture("atlas-live-query-scope");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let journal = crate::live::PendingJournal::open(&graph).unwrap();
        for (sequence, path) in ["Cargo.toml", "src/lib.rs", "src/store.rs"]
            .into_iter()
            .enumerate()
        {
            journal.record(path, sequence as u64 + 1).unwrap();
        }
        let mut runtime =
            crate::live::LiveRuntimeStatus::new(crate::live::LiveRuntimeState::Pending);
        runtime.pending_paths = vec![
            "Cargo.toml".to_string(),
            "src/lib.rs".to_string(),
            "src/store.rs".to_string(),
        ];
        runtime.store(&graph).unwrap();

        let global = status(&code, &graph).unwrap();
        assert_eq!(
            global.live.pending_paths,
            vec!["Cargo.toml", "src/lib.rs", "src/store.rs"]
        );

        let queried = query(
            &code,
            &graph,
            "atlas_basic::open_default",
            &QueryOptions { frozen: true },
        )
        .unwrap();
        assert_eq!(queried.status.live.pending_paths, vec!["src/lib.rs"]);

        let flowed = flow(
            &code,
            &graph,
            FlowQuery::Through {
                symbol: "atlas_basic::service::run".to_string(),
            },
            &FlowOptions {
                frozen: true,
                ..FlowOptions::default()
            },
        )
        .unwrap();
        let flow_files = flowed
            .endpoints
            .iter()
            .flat_map(|endpoint| endpoint.selected.iter().chain(endpoint.candidates.iter()))
            .chain(
                flowed
                    .alternatives
                    .iter()
                    .flat_map(|path| path.nodes.iter()),
            )
            .map(|node| node.file.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            flowed.status.live.pending_paths,
            global
                .live
                .pending_paths
                .iter()
                .filter(|path| flow_files.contains(*path))
                .cloned()
                .collect::<Vec<_>>()
        );
        assert!(
            !flowed
                .status
                .live
                .pending_paths
                .contains(&"Cargo.toml".to_string())
        );

        let impacted = impact(
            &code,
            &graph,
            "atlas_basic::service::run",
            &ImpactOptions {
                frozen: true,
                ..ImpactOptions::default()
            },
        )
        .unwrap();
        let impact_files = std::iter::once(&impacted.seed)
            .chain(impacted.affected.iter().map(|entry| &entry.node))
            .map(|node| node.file.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            impacted.status.live.pending_paths,
            global
                .live
                .pending_paths
                .iter()
                .filter(|path| impact_files.contains(*path))
                .cloned()
                .collect::<Vec<_>>()
        );
        assert!(
            !impacted
                .status
                .live
                .pending_paths
                .contains(&"Cargo.toml".to_string())
        );
        fs::remove_dir_all(graph.parent().unwrap()).ok();
    }

    fn node<'a>(shards: &'a [Shard], id: &str) -> Option<&'a Node> {
        shards
            .iter()
            .flat_map(|s| s.nodes.iter())
            .find(|node| node.id == id || node.symbol == id)
    }

    fn all_edges(shards: &[Shard]) -> Vec<Edge> {
        shards.iter().flat_map(|s| s.edges.clone()).collect()
    }

    fn source_tree_snapshot(code: &Path) -> BTreeMap<String, Vec<u8>> {
        walk_rs_files(code)
            .into_iter()
            .map(|path| (rel_path(code, &path), fs::read(path).unwrap()))
            .collect()
    }

    fn init_git_repository(code: &Path) {
        for args in [
            ["init"].as_slice(),
            ["add", "."].as_slice(),
            [
                "-c",
                "user.name=Atlas Test",
                "-c",
                "user.email=atlas@example.test",
                "commit",
                "-m",
                "initial",
            ]
            .as_slice(),
        ] {
            let output = Command::new("git")
                .args(args)
                .current_dir(code)
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
        }
    }

    fn file_tree_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn collect(root: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
            let mut entries: Vec<_> = fs::read_dir(directory)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    collect(root, &path, files);
                } else {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    files.insert(relative, fs::read(path).unwrap());
                }
            }
        }

        let mut files = BTreeMap::new();
        collect(root, root, &mut files);
        files
    }

    fn rewrite_query_index(path: &Path, update: QueryIndexMutation) {
        let mut index: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        update(&mut index);
        fs::write(path, serde_json::to_vec_pretty(&index).unwrap()).unwrap();
    }

    fn remove_query_index(path: &Path) {
        fs::remove_file(path).unwrap();
    }

    fn downgrade_query_index_schema(path: &Path) {
        rewrite_query_index(path, |index| {
            index["schema_version"] = serde_json::json!(SCHEMA_VERSION - 1);
        });
    }

    fn stale_query_index_fingerprint(path: &Path) {
        rewrite_query_index(path, |index| {
            index["graph_fingerprint"] = serde_json::json!("wrong");
        });
    }

    fn corrupt_query_index(path: &Path) {
        rewrite_query_index(path, |index| {
            index["nodes"] = serde_json::json!("not a node table");
        });
    }

    fn rebuild_test_index(
        graph: &Path,
        update: impl FnOnce(&mut Vec<Shard>),
    ) -> (Meta, Vec<Shard>) {
        let (meta, mut shards) = load_graph(graph).unwrap();
        update(&mut shards);
        rebuild_query_index(graph, &meta, &shards).unwrap();
        (meta, shards)
    }

    fn assert_scip_occurrence_metadata(edge: &Edge, expected_site: EdgeSite) {
        let expected_location = format!(
            "{}:{}:{}-{}:{}",
            expected_site.file,
            expected_site.line_start,
            expected_site.column_start,
            expected_site.line_end,
            expected_site.column_end
        );
        assert_eq!(edge.site.as_ref(), Some(&expected_site));
        assert_eq!(
            edge.extractor
                .as_ref()
                .map(|extractor| extractor.name.as_str()),
            Some("rust-analyzer-scip")
        );
        assert_eq!(edge.confidence, Some(EdgeConfidence::Exact));
        assert!(edge.evidence.as_deref().is_some_and(
            |evidence| evidence.contains("occurrence") && evidence.contains(&expected_location)
        ));
    }

    fn edge(from: &str, to: &str, kind: EdgeKind) -> Edge {
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            target_text: None,
            resolution: EdgeResolution::Resolved,
            kind,
            provenance: Provenance::Syn,
            site: None,
            extractor: Some(syn_extractor()),
            dispatch: None,
            confidence: Some(EdgeConfidence::Exact),
            candidates: Vec::new(),
            evidence: None,
            generic: false,
        }
    }

    #[test]
    fn test_atlas_rejects_exact_confidence_with_multiple_candidates() {
        let mut edge = edge("a", "b", EdgeKind::Calls);
        edge.confidence = Some(EdgeConfidence::Exact);
        edge.candidates = vec!["b".into(), "c".into()];
        let error = validate_edges([&edge]).unwrap_err().to_string();
        assert!(error.contains("confidence"));
        assert!(error.contains("2 candidates"));
    }

    /// Write a standalone crate (detached `[workspace]`) with the given source
    /// files, returning `(code_dir, graph_dir)`.
    fn scratch_crate(name: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let base = temp_dir(name);
        let code = base.join("code");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n"
            ),
        )
        .unwrap();
        for (rel, contents) in files {
            let path = code.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
        (code, base.join("graph"))
    }

    fn kinds_by_symbol(shards: &[Shard]) -> BTreeMap<String, NodeKind> {
        shards
            .iter()
            .flat_map(|s| s.nodes.iter())
            .map(|n| (n.symbol.clone(), n.kind))
            .collect()
    }

    // Finding A: a trait implemented through its `use`-imported bare name (in a
    // different module) must resolve, so `impls <Trait>` returns its impls.
    #[test]
    fn test_atlas_resolves_bare_imported_trait_impls() {
        let (code, graph) = scratch_crate(
            "atlas_bare_trait",
            &[
                ("src/lib.rs", "pub mod traits;\npub mod imps;\n"),
                ("src/traits.rs", "pub trait Linter { fn lint(&self); }\n"),
                (
                    "src/imps.rs",
                    "use crate::traits::Linter;\n\
                     pub struct A;\npub struct B;\n\
                     impl Linter for A { fn lint(&self) {} }\n\
                     impl Linter for B { fn lint(&self) {} }\n",
                ),
            ],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();

        let report = impls(&code, &graph, "Linter", &QueryOptions::default()).unwrap();
        let impls_trait = report
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ImplsTrait)
            .count();
        assert_eq!(impls_trait, 2, "both impls must surface: {report:?}");

        let (_, shards) = load_graph(&graph).unwrap();
        let resolved = all_edges(&shards)
            .into_iter()
            .filter(|e| e.kind == EdgeKind::ImplsTrait && e.resolution == EdgeResolution::Resolved)
            .count();
        assert_eq!(
            resolved, 2,
            "bare imported trait name must resolve to a node id"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    // Finding C: static, union, and associated const/type must be nodes.
    #[test]
    fn test_atlas_extracts_static_union_and_associated_items() {
        let (code, graph) = scratch_crate(
            "atlas_kinds",
            &[(
                "src/lib.rs",
                "pub static GREETING: &str = \"hi\";\n\
                 pub union U { a: u32, b: u32 }\n\
                 pub trait T { type Out; const N: usize; fn f(&self); }\n\
                 pub struct S;\n\
                 impl T for S { type Out = i32; const N: usize = 3; fn f(&self) {} }\n",
            )],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let kinds = kinds_by_symbol(&shards);
        assert_eq!(kinds.get("atlas_kinds::GREETING"), Some(&NodeKind::Static));
        assert_eq!(kinds.get("atlas_kinds::U"), Some(&NodeKind::Union));
        assert_eq!(kinds.get("atlas_kinds::T::Out"), Some(&NodeKind::TypeAlias));
        assert_eq!(kinds.get("atlas_kinds::T::N"), Some(&NodeKind::Const));
        let impl_assoc_type = shards.iter().flat_map(|s| &s.nodes).any(|n| {
            n.kind == NodeKind::TypeAlias
                && n.symbol.contains("::impl ")
                && n.symbol.ends_with("::Out")
        });
        let impl_assoc_const = shards.iter().flat_map(|s| &s.nodes).any(|n| {
            n.kind == NodeKind::Const && n.symbol.contains("::impl ") && n.symbol.ends_with("::N")
        });
        assert!(impl_assoc_type, "impl associated type must be a node");
        assert!(impl_assoc_const, "impl associated const must be a node");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    // Finding D: struct/enum signatures are the declaration head, no body.
    #[test]
    fn test_atlas_struct_signature_is_declaration_head() {
        let (code, graph) = scratch_crate(
            "atlas_sig",
            &[(
                "src/lib.rs",
                "pub struct Big { pub a: u32, pub b: u32, pub c: String, pub d: Vec<u8> }\n\
                 pub enum E { A, B(u32), C { x: u8 } }\n",
            )],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let big = shards
            .iter()
            .flat_map(|s| &s.nodes)
            .find(|n| n.symbol == "atlas_sig::Big")
            .unwrap();
        assert_eq!(big.signature, "pub struct Big");
        assert!(
            !big.signature.contains("a :") && !big.signature.contains('{'),
            "fields must not be in the signature: {}",
            big.signature
        );
        let e = shards
            .iter()
            .flat_map(|s| &s.nodes)
            .find(|n| n.symbol == "atlas_sig::E")
            .unwrap();
        assert_eq!(e.signature, "pub enum E");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    // Finding B: crates the root workspace `exclude`s must not enter the graph,
    // not even refiled under the host crate's namespace.
    #[test]
    fn test_atlas_respects_workspace_exclude() {
        let base = temp_dir("atlas_exclude");
        let code = base.join("code");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::create_dir_all(code.join("fixture/src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"host\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [workspace]\nexclude = [\"fixture\"]\n",
        )
        .unwrap();
        fs::write(code.join("src/lib.rs"), "pub struct Host;\n").unwrap();
        fs::write(
            code.join("fixture/Cargo.toml"),
            "[package]\nname = \"excluded_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        fs::write(
            code.join("fixture/src/lib.rs"),
            "pub struct ShouldNotAppear;\n",
        )
        .unwrap();

        let graph = base.join("graph");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, shards) = load_graph(&graph).unwrap();
        assert!(
            !meta.packages.contains(&"excluded_fixture".to_string()),
            "excluded crate must not be a package: {:?}",
            meta.packages
        );
        assert!(
            !shards
                .iter()
                .flat_map(|s| &s.nodes)
                .any(|n| n.symbol.contains("ShouldNotAppear")),
            "excluded crate symbols must not appear anywhere"
        );
        assert!(
            shards
                .iter()
                .flat_map(|s| &s.nodes)
                .any(|n| n.symbol == "host::Host"),
            "host crate must still be indexed"
        );
        fs::remove_dir_all(base).ok();
    }

    // Finding F: a non-canonical `code_root` (relative, `.`, or containing `..`)
    // must work — `cargo metadata` yields absolute canonical paths, so the root
    // is canonicalized before the file walk (otherwise the walk paths never
    // match the target dirs and every file errors "not owned by a Cargo target").
    #[test]
    fn test_atlas_build_accepts_noncanonical_code_root() {
        let (code, graph) =
            scratch_crate("atlas_noncanon", &[("src/lib.rs", "pub struct Widget;\n")]);
        let noncanon = code.join("..").join(code.file_name().unwrap());
        assert!(noncanon.to_string_lossy().contains(".."));
        build(&noncanon, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            shards
                .iter()
                .flat_map(|s| &s.nodes)
                .any(|n| n.symbol == "atlas_noncanon::Widget"),
            "non-canonical code_root must still index the crate"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_uses_cargo_workspace_targets_for_symbol_paths() {
        let base = temp_dir("atlas-cargo-workspace");
        let code = base.join("code");
        let graph = base.join("graph");
        fs::create_dir_all(code.join("crates/a-b/src")).unwrap();
        fs::create_dir_all(code.join("crates/a-b/src/nested")).unwrap();
        fs::create_dir_all(code.join("crates/a-b/fuzz/fuzz_targets")).unwrap();
        fs::create_dir_all(code.join("crates/a-b/fuzz/fuzz_targets/shared")).unwrap();
        fs::create_dir_all(code.join("tools/helper/src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a-b\", \"tools/helper\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        fs::write(
            code.join("crates/a-b/Cargo.toml"),
            "[package]\nname = \"a-b\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("crates/a-b/src/lib.rs"),
            "mod find_protoc;\n#[path = \"nested/actual.rs\"]\npub mod public_api;\npub fn root() {}\n",
        )
        .unwrap();
        fs::write(
            code.join("crates/a-b/src/nested/actual.rs"),
            "pub fn api() {}\n",
        )
        .unwrap();
        fs::write(
            code.join("crates/a-b/src/find_protoc.rs"),
            "pub fn find_protoc() {}\n",
        )
        .unwrap();
        fs::write(
            code.join("crates/a-b/fuzz/Cargo.toml"),
            "[package]\nname = \"a-b-fuzz\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\nmembers = [\".\"]\n\n[[bin]]\nname = \"render_all\"\npath = \"fuzz_targets/render_all.rs\"\n",
        )
        .unwrap();
        fs::write(
            code.join("crates/a-b/fuzz/fuzz_targets/render_all.rs"),
            "#[path = \"shared/helper.rs\"]\nmod helper;\npub fn fuzz_one() {}\nfn main() {}\n",
        )
        .unwrap();
        fs::write(
            code.join("crates/a-b/fuzz/fuzz_targets/shared/helper.rs"),
            "pub fn assist() {}\n",
        )
        .unwrap();
        fs::write(
            code.join("tools/helper/Cargo.toml"),
            "[package]\nname = \"helper\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(code.join("tools/helper/src/lib.rs"), "pub struct Tool;\n").unwrap();

        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, shards) = load_graph(&graph).unwrap();
        assert_eq!(meta.packages, vec!["a_b", "helper", "render_all"]);
        assert!(node(&shards, "a_b::find_protoc::find_protoc").is_some());
        assert!(node(&shards, "a_b::public_api::api").is_some());
        assert!(node(&shards, "a_b::nested::actual::api").is_none());
        assert!(node(&shards, "helper::Tool").is_some());
        assert!(node(&shards, "render_all::fuzz_one").is_some());
        assert!(node(&shards, "render_all::helper::assist").is_some());
        assert!(
            shards
                .iter()
                .flat_map(|shard| &shard.nodes)
                .all(|node| !node.id.contains("::crates::") && !node.id.contains("::src::"))
        );
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_atlas_declaration_ids_are_unique_and_stable_for_impl_variants() {
        let (code, graph) = copy_fixture("atlas-unique-impl-ids");
        fs::write(
            code.join("src/variants.rs"),
            r#"
pub trait Receiver {}
pub struct Gateway<T>(T);
impl<T: Clone> Receiver for Gateway<T> {}
impl<T: Copy> Receiver for Gateway<T> {}
#[cfg(unix)]
impl<T: Send> Receiver for Gateway<T> {}
impl<T> Gateway<T> { fn first(&self) {} }
impl<T> Gateway<T> { fn second(&self) {} }
"#,
        )
        .unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let declarations: Vec<&Node> = shards
            .iter()
            .flat_map(|shard| &shard.nodes)
            .filter(|node| node.kind == NodeKind::Impl && node.file.ends_with("variants.rs"))
            .collect();
        assert_eq!(declarations.len(), 5);
        assert_eq!(
            declarations
                .iter()
                .map(|node| node.id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            5
        );
        assert!(declarations.iter().all(|node| node.id.contains('#')));
        assert!(declarations.iter().any(|node| node.id.ends_with("~2")));

        let first_ids: BTreeSet<String> = declarations.iter().map(|node| node.id.clone()).collect();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let second_ids: BTreeSet<String> = shards
            .iter()
            .flat_map(|shard| &shard.nodes)
            .filter(|node| node.kind == NodeKind::Impl && node.file.ends_with("variants.rs"))
            .map(|node| node.id.clone())
            .collect();
        assert_eq!(first_ids, second_ids);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_cfg_module_identity_is_inherited_by_children() {
        let (code, graph) = copy_fixture("atlas-cfg-modules");
        fs::write(
            code.join("src/platform.rs"),
            r#"
#[cfg(target_os = "linux")]
mod imp { pub fn sample() {} }
#[cfg(target_os = "macos")]
mod imp { pub fn sample() {} }
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod imp { pub fn sample() {} }
"#,
        )
        .unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        for symbol in [
            "atlas_basic::platform::imp",
            "atlas_basic::platform::imp::sample",
        ] {
            let ids: BTreeSet<&str> = shards
                .iter()
                .flat_map(|shard| &shard.nodes)
                .filter(|node| node.symbol == symbol)
                .map(|node| node.id.as_str())
                .collect();
            assert_eq!(ids.len(), 3, "{symbol}: {ids:?}");
        }
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_signature_comes_from_ast_and_doc_is_separate() {
        let (code, graph) = copy_fixture("atlas-ast-signature");
        fs::write(
            code.join("src/documented.rs"),
            r#"
/// Finds the configured compiler.
/// More details are deliberately separate.
#[cfg(any(unix, windows))]
#[inline]
pub fn find_protoc<T: AsRef<str>>(name: T) -> Option<String>
where
    T: Clone,
{
    Some(name.as_ref().to_owned())
}
"#,
        )
        .unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let function = node(&shards, "atlas_basic::documented::find_protoc").unwrap();
        assert!(function.signature.starts_with("pub fn find_protoc"));
        assert!(function.signature.contains("where T : Clone"));
        assert!(!function.signature.contains("///"));
        assert!(!function.signature.contains("#["));
        assert_eq!(
            function.doc.as_deref(),
            Some("Finds the configured compiler.")
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_marks_unresolved_and_external_edge_targets() {
        let (code, graph) = copy_fixture("atlas-edge-resolution");
        fs::write(
            code.join("src/targets.rs"),
            r#"
pub struct Local;
impl MissingTrait for Local {}
impl std::fmt::Display for Local {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { todo!() }
}
"#,
        )
        .unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let edges = all_edges(&shards);
        assert!(edges.iter().any(|edge| {
            edge.kind == EdgeKind::ImplsTrait
                && edge.target_text.as_deref() == Some("MissingTrait")
                && edge.to == "MissingTrait"
                && edge.resolution == EdgeResolution::Unresolved
        }));
        assert!(edges.iter().any(|edge| {
            edge.kind == EdgeKind::ImplsTrait
                && edge.target_text.as_deref() == Some("std::fmt::Display")
                && edge.resolution == EdgeResolution::External
        }));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_builds_module_tree_and_symbol_nodes() {
        let (code, graph) = copy_fixture("atlas-build-basic");
        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert!(report.unparsed.is_empty());

        let (meta, shards) = load_graph(&graph).unwrap();
        assert_eq!(meta.schema_version, SCHEMA_VERSION);
        assert_eq!(meta.package, "atlas_basic");
        assert_eq!(meta.files.len(), 3, "one hash per source file: {meta:?}");
        for hash in meta.files.values() {
            assert_eq!(hash.len(), 64, "blake3 hex hash");
        }

        let store = node(&shards, "atlas_basic::store::MemStore").unwrap();
        assert_eq!(store.kind, NodeKind::Struct);
        assert!(store.file.ends_with("src/store.rs"));
        assert!(store.line_start > 0 && store.line_end >= store.line_start);
        assert_eq!(store.visibility, "pub");

        let crate_node = node(&shards, "atlas_basic").unwrap();
        let store_module = node(&shards, "atlas_basic::store").unwrap();
        assert!(node(&shards, "atlas_basic::store::Kind").is_some());
        assert!(node(&shards, "atlas_basic::store::Store").is_some());
        assert!(node(&shards, "atlas_basic::store::LIMIT").is_some());
        assert!(node(&shards, "atlas_basic::store::Alias").is_some());
        assert!(node(&shards, "atlas_basic::mk_store").is_some(), "macro");
        assert!(node(&shards, "atlas_basic::open_default").is_some());

        let edges = all_edges(&shards);
        assert!(edges.iter().any(|e| e.kind == EdgeKind::Contains
            && e.from == crate_node.id
            && e.to == store_module.id
            && e.provenance == Provenance::Syn));
        assert!(edges.iter().any(|e| e.kind == EdgeKind::Contains
            && e.from == store_module.id
            && e.to == store.id));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_builds_impl_edges() {
        let (code, graph) = copy_fixture("atlas-build-impls");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let edges = all_edges(&shards);
        let impl_node = shards
            .iter()
            .flat_map(|s| s.nodes.iter())
            .find(|n| n.kind == NodeKind::Impl)
            .unwrap();
        let store_trait = node(&shards, "atlas_basic::store::Store").unwrap();
        let mem_store = node(&shards, "atlas_basic::store::MemStore").unwrap();
        assert!(edges.iter().any(|e| e.kind == EdgeKind::ImplsTrait
            && e.from == impl_node.id
            && e.to == store_trait.id
            && e.resolution == EdgeResolution::Resolved
            && e.provenance == Provenance::Syn));
        assert!(edges.iter().any(|e| e.kind == EdgeKind::ImplFor
            && e.from == impl_node.id
            && e.to == mem_store.id
            && e.resolution == EdgeResolution::Resolved
            && e.provenance == Provenance::Syn));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_records_unparsed_file_diagnostic() {
        let (code, graph) = copy_fixture("atlas-unparsed");
        fs::write(code.join("src/broken.rs"), "pub fn broken( {").unwrap();
        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(report.unparsed.len(), 1);
        assert!(report.unparsed[0].contains("broken.rs"));

        let (_, shards) = load_graph(&graph).unwrap();
        let broken = shards
            .iter()
            .find(|s| s.file.ends_with("broken.rs"))
            .unwrap();
        assert!(broken.unparsed.is_some());
        assert!(broken.nodes.is_empty());
        // the rest of the graph still builds
        assert!(node(&shards, "atlas_basic::store::MemStore").is_some());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_rejects_mismatched_schema_version() {
        let (code, graph) = copy_fixture("atlas-schema-mismatch");
        build(&code, &graph, &BuildOptions::default()).unwrap();

        // Rewrite meta.json to an older, incompatible schema version.
        let meta_path = active_data_dir(&graph).join("meta.json");
        let mut meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        meta["schema_version"] = serde_json::json!(SCHEMA_VERSION - 1);
        fs::write(&meta_path, meta.to_string()).unwrap();

        // Consuming paths must fail loudly, not false-green.
        assert!(matches!(
            load_graph(&graph).unwrap_err(),
            AtlasError::SchemaMismatch { .. }
        ));
        assert!(matches!(
            check(&code, &graph).unwrap_err(),
            AtlasError::SchemaMismatch { .. }
        ));
        assert!(matches!(
            status(&code, &graph).unwrap_err(),
            AtlasError::SchemaMismatch { .. }
        ));
        assert!(matches!(
            query(
                &code,
                &graph,
                "atlas_basic::store::MemStore",
                &QueryOptions::default(),
            )
            .unwrap_err(),
            AtlasError::SchemaMismatch { .. }
        ));

        // build recovers: it reads old meta via `.ok()`, so a mismatch degrades
        // to a full rebuild that restores the current schema.
        build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(load_graph(&graph).unwrap().0.schema_version, SCHEMA_VERSION);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_incremental_rebuild_only_dirty_shards() {
        let (code, graph) = copy_fixture("atlas-incremental");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let snapshot: BTreeMap<String, Vec<u8>> =
            fs::read_dir(active_data_dir(&graph).join("shards"))
                .unwrap()
                .map(|e| e.unwrap().path())
                .map(|p| {
                    (
                        p.file_name().unwrap().to_string_lossy().to_string(),
                        fs::read(&p).unwrap(),
                    )
                })
                .collect();

        let service = code.join("src/service.rs");
        let mut text = fs::read_to_string(&service).unwrap();
        text.push_str("\npub fn extra() -> usize {\n    1\n}\n");
        fs::write(&service, text).unwrap();

        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(report.rebuilt.len(), 1, "{report:?}");
        assert!(report.rebuilt[0].ends_with("service.rs"));

        for (name, bytes) in &snapshot {
            let now = fs::read(active_data_dir(&graph).join("shards").join(name)).unwrap();
            if name.contains("service") {
                assert_ne!(&now, bytes, "dirty shard must be rewritten");
            } else {
                assert_eq!(&now, bytes, "untouched shard must stay byte-identical");
            }
        }
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_input_plan_rebuilds_source_module_ownership() {
        let (code, graph) = copy_fixture("atlas-incremental-module-layout");
        let lib = code.join("src/lib.rs");
        let mut source = fs::read_to_string(&lib).unwrap();
        source.push_str("\nmod actual;\n");
        fs::write(&lib, &source).unwrap();
        fs::write(code.join("src/actual.rs"), "pub fn endpoint() {}\n").unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();

        let source = source.replace("mod actual;", "#[path = \"actual.rs\"]\nmod renamed;");
        fs::write(&lib, source).unwrap();
        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(report.input_plan, InputPlanState::Hit);
        assert_eq!(
            report.rebuilt,
            vec!["src/actual.rs".to_string(), "src/lib.rs".to_string()]
        );
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(node(&shards, "atlas_basic::renamed::endpoint").is_some());
        assert!(node(&shards, "atlas_basic::actual::endpoint").is_none());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_auto_refresh_preserves_input_plan_configuration() {
        let (code, graph) = scratch_crate(
            "atlas_refresh_input_plan",
            &[("src/lib.rs", "pub fn value() -> u32 { 1 }\n")],
        );
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"atlas_refresh_input_plan\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\nalt = []\n\n[workspace]\n",
        )
        .unwrap();
        build(
            &code,
            &graph,
            &BuildOptions {
                features: vec!["alt".to_string()],
                cfg: vec!["atlas_test".to_string()],
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let baseline_plan: serde_json::Value = serde_json::from_slice(
            &fs::read(active_data_dir(&graph).join("input-plan.json")).unwrap(),
        )
        .unwrap();

        fs::write(code.join("src/lib.rs"), "pub fn value() -> u32 { 2 }\n").unwrap();
        query(
            &code,
            &graph,
            "atlas_refresh_input_plan::value",
            &QueryOptions::default(),
        )
        .unwrap();

        let refreshed_plan: serde_json::Value = serde_json::from_slice(
            &fs::read(active_data_dir(&graph).join("input-plan.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            refreshed_plan["fingerprint"], baseline_plan["fingerprint"],
            "automatic refresh must reuse the committed Cargo input configuration"
        );
        assert_eq!(refreshed_plan["features"], serde_json::json!(["alt"]));
        assert_eq!(refreshed_plan["cfg"], serde_json::json!(["atlas_test"]));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_auto_refresh_rejects_corrupt_input_plan_artifact() {
        let (code, graph) = copy_fixture("atlas-refresh-corrupt-input-plan");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let input_plan_path = active_data_dir(&graph).join("input-plan.json");
        let mut input_plan: serde_json::Value =
            serde_json::from_slice(&fs::read(&input_plan_path).unwrap()).unwrap();
        input_plan["features"] = serde_json::json!(["uncommitted-feature"]);
        fs::write(
            &input_plan_path,
            serde_json::to_vec_pretty(&input_plan).unwrap(),
        )
        .unwrap();
        let service = code.join("src/service.rs");
        let mut source = fs::read_to_string(&service).unwrap();
        source.push_str("\npub fn stale_after_plan_corruption() {}\n");
        fs::write(service, source).unwrap();

        let error = query(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions::default(),
        )
        .unwrap_err();
        let AtlasError::Invariant(detail) = error else {
            panic!("expected committed artifact integrity failure");
        };
        assert!(detail.contains("artifact integrity"));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_incremental_frontier_repairs_renamed_target_dependents() {
        let (code, graph) = scratch_crate(
            "atlas_frontier_rename",
            &[
                ("src/lib.rs", "pub mod api;\npub mod client;\n"),
                ("src/api.rs", "pub trait Service {}\n"),
                (
                    "src/client.rs",
                    "use crate::api::Service;\npub struct Client;\nimpl Service for Client {}\n",
                ),
            ],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();
        fs::write(code.join("src/api.rs"), "pub trait ServiceV2 {}\n").unwrap();

        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(
            report.touched_shards,
            vec!["src/api.rs".to_string(), "src/client.rs".to_string()]
        );
        assert_eq!(report.resolved_shards, 2);
        let (_, shards) = load_graph(&graph).unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| {
                edge.kind == EdgeKind::ImplsTrait && edge.target_text.as_deref() == Some("Service")
            })
            .unwrap();
        assert_eq!(edge.resolution, EdgeResolution::Unresolved);
        assert_eq!(edge.to, "Service");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_incremental_frontier_repairs_bare_name_ambiguity() {
        let (code, graph) = scratch_crate(
            "atlas_frontier_ambiguity",
            &[
                ("src/lib.rs", "pub mod api;\npub mod client;\n"),
                ("src/api.rs", "pub trait Service {}\n"),
                (
                    "src/client.rs",
                    "use crate::api::Service;\npub struct Client;\nimpl Service for Client {}\n",
                ),
            ],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, baseline) = load_graph(&graph).unwrap();
        let old_target = all_edges(&baseline)
            .into_iter()
            .find(|edge| edge.kind == EdgeKind::ImplsTrait)
            .unwrap()
            .to;
        fs::write(
            code.join("src/lib.rs"),
            "pub mod api;\npub mod client;\npub mod other;\n",
        )
        .unwrap();
        fs::write(code.join("src/other.rs"), "pub trait Service {}\n").unwrap();

        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert!(report.touched_shards.contains(&"src/client.rs".to_string()));
        let (_, shards) = load_graph(&graph).unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| {
                edge.kind == EdgeKind::ImplsTrait && edge.target_text.as_deref() == Some("Service")
            })
            .unwrap();
        assert_eq!(edge.resolution, EdgeResolution::Unresolved);
        assert_ne!(edge.to, old_target);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_incremental_frontier_overflow_falls_back_completely() {
        let (code, graph) = scratch_crate(
            "atlas_frontier_overflow",
            &[
                (
                    "src/lib.rs",
                    "pub mod api;\npub mod a;\npub mod b;\npub mod c;\n",
                ),
                ("src/api.rs", "pub trait Service {}\n"),
                (
                    "src/a.rs",
                    "use crate::api::Service; pub struct A; impl Service for A {}\n",
                ),
                (
                    "src/b.rs",
                    "use crate::api::Service; pub struct B; impl Service for B {}\n",
                ),
                (
                    "src/c.rs",
                    "use crate::api::Service; pub struct C; impl Service for C {}\n",
                ),
            ],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();
        fs::write(code.join("src/api.rs"), "pub trait ServiceV2 {}\n").unwrap();
        let report = build(
            &code,
            &graph,
            &BuildOptions {
                frontier_limit: 2,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            report.fallback_reason.as_deref(),
            Some("dependency-frontier-overflow")
        );
        assert_eq!(report.resolved_shards, 5);
        assert_eq!(report.validated_shards, 5);
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            all_edges(&shards)
                .iter()
                .filter(|edge| edge.kind == EdgeKind::ImplsTrait)
                .all(|edge| edge.resolution == EdgeResolution::Unresolved)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_zero_change_fast_path_is_byte_and_counter_inert() {
        let (code, graph) = copy_fixture("atlas-zero-change-fast-path");
        let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
        let bytes = file_tree_snapshot(&graph);

        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(report.input_plan, InputPlanState::Hit);
        assert_eq!(report.generation, baseline.generation);
        assert!(report.touched_shards.is_empty());
        assert_eq!(report.resolved_shards, 0);
        assert_eq!(report.validated_shards, 0);
        assert_eq!(report.resolved_edge_delta, 0);
        assert_eq!(report.unresolved_edge_delta, 0);
        assert_eq!(report.working_bytes, 0);
        assert_eq!(file_tree_snapshot(&graph), bytes);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    fn persist_test_orphan(
        code: &Path,
        graph: &Path,
        affected_files: Vec<String>,
        changed_symbols: Vec<String>,
    ) {
        let snapshot = graph_snapshot(graph).unwrap();
        let fingerprint =
            status::source_fingerprint(&status::source_hashes(code).unwrap()).unwrap();
        let queue = incremental::OrphanQueue::new(
            snapshot.generation,
            fingerprint,
            affected_files,
            changed_symbols,
            "source-change".to_string(),
        )
        .unwrap();
        incremental::write_orphan(graph, &queue).unwrap();
    }

    #[test]
    fn test_atlas_zero_change_build_recovers_interrupted_orphans() {
        let (code, graph) = scratch_crate(
            "atlas_orphan_recovery",
            &[
                ("src/lib.rs", "pub mod api;\npub mod client;\n"),
                ("src/api.rs", "pub trait Service {}\n"),
                (
                    "src/client.rs",
                    "use crate::api::Service; pub struct Client; impl Service for Client {}\n",
                ),
            ],
        );
        let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
        fs::write(code.join("src/api.rs"), "pub trait ServiceV2 {}\n").unwrap();
        persist_test_orphan(
            &code,
            &graph,
            vec!["src/client.rs".to_string()],
            vec!["Service".to_string()],
        );

        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(report.orphans_recovered, 1);
        assert_ne!(report.generation, baseline.generation);
        assert!(!graph.join("orphans.json").exists());
        let generation = report.generation.clone();
        let zero = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert_eq!(zero.generation, generation);
        assert_eq!(zero.resolved_shards, 0);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_orphan_recovery_consumes_deterministic_unresolved() {
        let (code, graph) = scratch_crate(
            "atlas_orphan_unresolved",
            &[
                ("src/lib.rs", "pub mod api;\npub mod client;\n"),
                ("src/api.rs", "pub trait Service {}\n"),
                (
                    "src/client.rs",
                    "use crate::api::Service; pub struct Client; impl Service for Client {}\n",
                ),
            ],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();
        fs::write(code.join("src/api.rs"), "pub trait Replacement {}\n").unwrap();
        persist_test_orphan(
            &code,
            &graph,
            vec!["src/client.rs".to_string()],
            vec!["Service".to_string()],
        );

        build(&code, &graph, &BuildOptions::default()).unwrap();
        assert!(!graph.join("orphans.json").exists());
        let (_, shards) = load_graph(&graph).unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| edge.kind == EdgeKind::ImplsTrait)
            .unwrap();
        assert_eq!(edge.resolution, EdgeResolution::Unresolved);
        assert_eq!(edge.to, "Service");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_incremental_cancellation_preserves_generation_and_orphans() {
        let (code, graph) = copy_fixture("atlas-incremental-cancel");
        let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
        let pointer = fs::read(graph.join("CURRENT.json")).unwrap();
        let service = code.join("src/service.rs");
        let mut source = fs::read_to_string(&service).unwrap();
        source.push_str("\npub fn cancel_after_batch() {}\n");
        fs::write(service, source).unwrap();
        let token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        set_test_cancel_after_resolution_batches(Some(1));
        let result = build(
            &code,
            &graph,
            &BuildOptions {
                cancellation: Some(token),
                batch_size: 1,
                ..BuildOptions::default()
            },
        );
        set_test_cancel_after_resolution_batches(None);
        let error = result.unwrap_err();
        assert!(matches!(error, AtlasError::Cancelled));
        assert_eq!(
            graph_snapshot(&graph).unwrap().generation,
            Some(baseline.generation)
        );
        assert_eq!(fs::read(graph.join("CURRENT.json")).unwrap(), pointer);
        assert!(graph.join("orphans.json").is_file());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_incremental_working_byte_limit_fails_closed() {
        let (code, graph) = copy_fixture("atlas-incremental-byte-limit");
        let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
        let pointer = fs::read(graph.join("CURRENT.json")).unwrap();
        let service = code.join("src/service.rs");
        let mut source = fs::read_to_string(&service).unwrap();
        source.push_str("\npub fn exceeds_working_set() {}\n");
        fs::write(service, source).unwrap();
        let source_bytes = walk_rs_files(&code)
            .iter()
            .map(|path| fs::metadata(path).unwrap().len() as usize)
            .sum::<usize>();
        let shard_bytes = fs::read_dir(active_data_dir(&graph).join("shards"))
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len() as usize)
            .sum::<usize>();
        assert!(shard_bytes > source_bytes);
        let working_byte_limit = source_bytes + (shard_bytes - source_bytes) / 2;
        let error = build(
            &code,
            &graph,
            &BuildOptions {
                working_byte_limit,
                ..BuildOptions::default()
            },
        )
        .unwrap_err();
        let AtlasError::ResourceLimit {
            resource,
            required,
            limit,
        } = error
        else {
            panic!("expected resource limit");
        };
        assert_eq!(resource, "serialized graph working set");
        assert!(required > limit);
        assert_eq!(limit, working_byte_limit);
        assert_eq!(
            graph_snapshot(&graph).unwrap().generation,
            Some(baseline.generation)
        );
        assert_eq!(fs::read(graph.join("CURRENT.json")).unwrap(), pointer);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_post_commit_orphan_clear_failure_remains_recoverable() {
        let (code, graph) = copy_fixture("atlas-incremental-maintenance");
        let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
        let service = code.join("src/service.rs");
        let mut source = fs::read_to_string(&service).unwrap();
        source.push_str("\npub fn maintenance_recovery() {}\n");
        fs::write(service, source).unwrap();

        incremental::set_test_clear_failure(true);
        let result = build(&code, &graph, &BuildOptions::default());
        incremental::set_test_clear_failure(false);
        let report = result.unwrap();

        assert_ne!(report.generation, baseline.generation);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "incremental-maintenance-failed"
                && diagnostic.severity == "warning"
                && diagnostic.message.contains("queue rebased")
        }));
        assert_eq!(
            graph_snapshot(&graph).unwrap().generation.as_deref(),
            Some(report.generation.as_str())
        );
        assert!(
            incremental::load_orphan(&graph, Some(&report.generation))
                .unwrap()
                .is_some()
        );

        let recovery = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert!(recovery.orphans_recovered > 0);
        assert!(!graph.join("orphans.json").exists());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_reclamation_ambiguity_is_warning_only_after_success() {
        let (code, graph) = copy_fixture("atlas-reclamation-warning");
        let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
        fs::write(graph.join(".runtime/readers"), "not-a-directory").unwrap();

        let report = build(&code, &graph, &BuildOptions::default()).unwrap();

        assert_eq!(report.generation, baseline.generation);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "generation-maintenance-warning"
                && diagnostic.severity == "warning"
                && diagnostic.message.contains("reader lease directory")
        }));
        fs::remove_file(graph.join(".runtime/readers")).unwrap();
        assert_eq!(
            graph_snapshot(&graph).unwrap().generation,
            Some(report.generation)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_incremental_overlay_capability_is_generation_atomic() {
        let (code, graph) = copy_fixture("atlas-incremental-overlay-atomic");
        let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
        let bad_index = code.parent().unwrap().join("bad-scip.json");
        fs::write(&bad_index, "not valid SCIP").unwrap();
        let error = build(
            &code,
            &graph,
            &BuildOptions {
                scip_index: Some(bad_index),
                ..BuildOptions::default()
            },
        );
        assert!(error.is_err());
        let after_failure = status(&code, &graph).unwrap();
        assert_eq!(after_failure.generation, Some(baseline.generation));
        assert_eq!(after_failure.scip.state, LayerState::Unavailable);

        let report = build(
            &code,
            &graph,
            &BuildOptions {
                scip_index: Some(scip_fixture()),
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let result = query(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions::default(),
        )
        .unwrap();
        assert_eq!(
            result.status.generation.as_deref(),
            Some(report.generation.as_str())
        );
        assert_eq!(result.status.scip.state, LayerState::Fresh);
        assert!(report.capability.scip);
        assert_eq!(
            report.fallback_reason.as_deref(),
            Some("semantic-overlay-change")
        );
        assert_eq!(report.touched_shards.len(), 3);
        assert_eq!(report.validated_shards, 3);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    fn incremental_receipt(
        case: &str,
        outcome: &str,
        report: Option<&BuildReport>,
        generation: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "case": case,
            "outcome": outcome,
            "generation": report.map_or(generation, |report| report.generation.as_str()),
            "input_plan": report.map(|report| report.input_plan),
            "touched_shards": report.map_or_else(Vec::new, |report| report.touched_shards.clone()),
            "resolved_edge_delta": report.map_or(0, |report| report.resolved_edge_delta),
            "unresolved_edge_delta": report.map_or(0, |report| report.unresolved_edge_delta),
            "working_bytes": report.map_or(0, |report| report.working_bytes),
            "fallback_reason": report.and_then(|report| report.fallback_reason.as_deref()),
            "orphans_recovered": report.map_or(0, |report| report.orphans_recovered),
        })
    }

    #[test]
    fn test_atlas_incremental_acceptance_matrix_records_receipts() {
        let matrix: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/atlas/incremental-hardening/matrix.json"
        ))
        .unwrap();
        assert_eq!(
            matrix["schema"],
            "rust-atlas/incremental-hardening-matrix-v1"
        );
        let cases = matrix["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 10);
        let mut receipts = Vec::new();

        for case in cases.iter().map(|case| case.as_str().unwrap()) {
            let (code, graph) = copy_incremental_fixture(&format!("atlas-matrix-{case}"));
            let (outcome, report, generation) = if case == "cold" {
                let report = build(&code, &graph, &BuildOptions::default()).unwrap();
                ("pass", Some(report.clone()), report.generation)
            } else {
                let baseline = build(&code, &graph, &BuildOptions::default()).unwrap();
                match case {
                    "zero-change" => {
                        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
                        ("pass", Some(report.clone()), report.generation)
                    }
                    "single-file-edit" => {
                        fs::write(
                            code.join("src/api.rs"),
                            "pub trait Service {}\npub fn changed() {}\n",
                        )
                        .unwrap();
                        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
                        ("pass", Some(report.clone()), report.generation)
                    }
                    "deletion" => {
                        fs::remove_file(code.join("src/c.rs")).unwrap();
                        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
                        ("pass", Some(report.clone()), report.generation)
                    }
                    "manifest-content-edit" => {
                        let manifest = code.join("Cargo.toml");
                        let mut text = fs::read_to_string(&manifest).unwrap();
                        text.push_str("\n# content-addressed cache miss\n");
                        fs::write(manifest, text).unwrap();
                        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
                        assert_eq!(report.input_plan, InputPlanState::Miss);
                        ("pass", Some(report.clone()), report.generation)
                    }
                    "frontier-overflow" => {
                        fs::write(code.join("src/api.rs"), "pub trait Replacement {}\n").unwrap();
                        let report = build(
                            &code,
                            &graph,
                            &BuildOptions {
                                frontier_limit: 2,
                                ..BuildOptions::default()
                            },
                        )
                        .unwrap();
                        assert_eq!(
                            report.fallback_reason.as_deref(),
                            Some("dependency-frontier-overflow")
                        );
                        ("pass", Some(report.clone()), report.generation)
                    }
                    "overlay" => {
                        let report = build(
                            &code,
                            &graph,
                            &BuildOptions {
                                dynamic_dispatch: true,
                                ..BuildOptions::default()
                            },
                        )
                        .unwrap();
                        assert!(report.capability.dynamic_dispatch);
                        ("pass", Some(report.clone()), report.generation)
                    }
                    "cancellation" => {
                        let token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                        let error = build(
                            &code,
                            &graph,
                            &BuildOptions {
                                cancellation: Some(token),
                                ..BuildOptions::default()
                            },
                        )
                        .unwrap_err();
                        assert!(matches!(error, AtlasError::Cancelled));
                        ("cancelled", None, baseline.generation)
                    }
                    "generation-commit-failure" => {
                        let snapshot = graph_snapshot(&graph).unwrap();
                        let (meta, _) = load_graph(&graph).unwrap();
                        let transaction =
                            generation::GenerationTransaction::begin(&graph, Some(&snapshot))
                                .unwrap();
                        index::write_json_atomic(
                            &transaction.data_dir().join("matrix-marker.json"),
                            &"failure",
                        )
                        .unwrap();
                        let error = transaction.publish_with_fault(
                            &meta.graph_fingerprint,
                            &meta.capability,
                            None,
                            generation::PublishFault::FinalRename,
                        );
                        assert!(error.is_err());
                        assert_eq!(
                            graph_snapshot(&graph).unwrap().generation,
                            snapshot.generation
                        );
                        ("commit-failed", None, baseline.generation)
                    }
                    "orphan-recovery" => {
                        persist_test_orphan(
                            &code,
                            &graph,
                            vec!["src/a.rs".to_string()],
                            vec!["Service".to_string()],
                        );
                        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
                        assert_eq!(report.orphans_recovered, 1);
                        ("recovered", Some(report.clone()), report.generation)
                    }
                    _ => unreachable!("matrix schema controls case names"),
                }
            };
            receipts.push(incremental_receipt(
                case,
                outcome,
                report.as_ref(),
                &generation,
            ));
            fs::remove_dir_all(code.parent().unwrap()).ok();
        }

        let required = [
            "case",
            "outcome",
            "generation",
            "input_plan",
            "touched_shards",
            "resolved_edge_delta",
            "unresolved_edge_delta",
            "working_bytes",
            "fallback_reason",
            "orphans_recovered",
        ];
        for receipt in &receipts {
            assert!(
                required.iter().all(|field| receipt.get(field).is_some()),
                "incomplete D2 receipt: {receipt}"
            );
            assert!(!receipt["generation"].as_str().unwrap().is_empty());
        }
        assert_eq!(
            receipts
                .iter()
                .map(|receipt| receipt["case"].as_str().unwrap())
                .collect::<Vec<_>>(),
            cases
                .iter()
                .map(|case| case.as_str().unwrap())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_atlas_frozen_query_reports_stale_warning() {
        let (code, graph) = copy_fixture("atlas-frozen");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let service = code.join("src/service.rs");
        let mut text = fs::read_to_string(&service).unwrap();
        text.push_str("\npub fn extra() -> usize {\n    2\n}\n");
        fs::write(&service, text).unwrap();

        let data_dir = active_data_dir(&graph);
        let shard_bytes: Vec<Vec<u8>> = fs::read_dir(data_dir.join("shards"))
            .unwrap()
            .map(|e| fs::read(e.unwrap().path()).unwrap())
            .collect();

        let result = query(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions { frozen: true },
        )
        .unwrap();
        assert_eq!(result.stale.len(), 1);
        assert!(result.stale[0].ends_with("service.rs"));

        let shard_bytes_after: Vec<Vec<u8>> = fs::read_dir(data_dir.join("shards"))
            .unwrap()
            .map(|e| fs::read(e.unwrap().path()).unwrap())
            .collect();
        assert_eq!(shard_bytes, shard_bytes_after, "frozen must not rewrite");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_low_level_queries_use_index_without_scanning_unrelated_shards() {
        let (code, graph) = copy_fixture("atlas-indexed-low-level-queries");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let source_before = source_tree_snapshot(&code);
        fs::write(
            active_data_dir(&graph)
                .join("shards")
                .join(shard_file_name("src/service.rs")),
            "not valid JSON",
        )
        .unwrap();
        let frozen = QueryOptions { frozen: true };

        let query_result = query(&code, &graph, "atlas_basic::store::MemStore", &frozen).unwrap();
        assert_eq!(query_result.node.symbol, "atlas_basic::store::MemStore");
        assert_eq!(query_result.node.kind, NodeKind::Struct);
        assert_eq!(query_result.node.file, "src/store.rs");
        assert!(query_result.stale.is_empty());
        assert!(
            query_result
                .edges_in
                .iter()
                .any(|edge| { edge.kind == EdgeKind::Contains && edge.to == query_result.node.id })
        );

        let refs_result = refs(&code, &graph, "atlas_basic::store::MemStore", &frozen).unwrap();
        assert_eq!(refs_result.symbol, "atlas_basic::store::MemStore");
        assert!(refs_result.edges.is_empty());
        assert!(refs_result.stale.is_empty());

        let impls_result = impls(&code, &graph, "Store", &frozen).unwrap();
        assert_eq!(impls_result.symbol, "Store");
        assert_eq!(impls_result.edges.len(), 1);
        assert_eq!(impls_result.edges[0].kind, EdgeKind::ImplsTrait);
        assert_eq!(
            impls_result.edges[0].target_text.as_deref(),
            Some("atlas_basic::store::Store")
        );
        assert!(impls_result.stale.is_empty());

        assert_eq!(source_before, source_tree_snapshot(&code));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_low_level_queries_reuse_refresh_meta() {
        let (code, graph) = copy_fixture("atlas-low-level-query-meta-reads");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let operations: [(&str, LowLevelQuery); 4] = [
            ("query", |code, graph, opts| {
                query(code, graph, "atlas_basic::store::MemStore", opts).map(|_| ())
            }),
            ("refs", |code, graph, opts| {
                refs(code, graph, "atlas_basic::store::MemStore", opts).map(|_| ())
            }),
            ("impls", |code, graph, opts| {
                impls(code, graph, "Store", opts).map(|_| ())
            }),
            ("search", |code, graph, opts| {
                search(
                    code,
                    graph,
                    "MemStore",
                    &SearchOptions {
                        limit: 20,
                        frozen: opts.frozen,
                    },
                )
                .map(|_| ())
            }),
        ];

        for (name, operation) in operations {
            reset_meta_read_count();
            operation(&code, &graph, &QueryOptions::default()).unwrap();
            assert_eq!(meta_read_count(), 1, "normal {name}");
        }

        let service = code.join("src/service.rs");
        let mut source = fs::read_to_string(&service).unwrap();
        source.push_str("\n// stale for frozen queries\n");
        fs::write(&service, source).unwrap();
        for (name, operation) in operations {
            reset_meta_read_count();
            operation(&code, &graph, &QueryOptions { frozen: true }).unwrap();
            assert_eq!(meta_read_count(), 1, "frozen {name}");
        }

        reset_meta_read_count();
        query(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions::default(),
        )
        .unwrap();
        assert_eq!(meta_read_count(), 2, "non-frozen stale query");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_rejects_borrowed_worktree_graph() {
        let (code, graph) = copy_fixture("atlas-borrowed-worktree");
        init_git_repository(&code);
        let linked = code.parent().unwrap().join("linked");
        let output = Command::new("git")
            .args(["worktree", "add", "-b", "linked", &linked.to_string_lossy()])
            .current_dir(&code)
            .output()
            .unwrap();
        assert!(output.status.success(), "git worktree add: {output:?}");
        build(&code, &graph, &BuildOptions::default()).unwrap();

        let report = status(&linked, &graph).unwrap();
        let recorded = report.recorded_identity.worktree_root.clone();
        let current = report.current_identity.worktree_root.clone();
        assert_ne!(recorded, current);
        assert!(report.worktree_mismatch.is_some());
        let graph_before = file_tree_snapshot(&graph);

        let operations = [
            tree(&linked, &graph, &QueryOptions::default()).map(|_| ()),
            query(
                &linked,
                &graph,
                "atlas_basic::store::MemStore",
                &QueryOptions::default(),
            )
            .map(|_| ()),
            refs(
                &linked,
                &graph,
                "atlas_basic::store::MemStore",
                &QueryOptions::default(),
            )
            .map(|_| ()),
            impls(&linked, &graph, "Store", &QueryOptions::default()).map(|_| ()),
            search(&linked, &graph, "MemStore", &SearchOptions::default()).map(|_| ()),
        ];
        for result in operations {
            let error = result.unwrap_err();
            assert!(
                matches!(error, AtlasError::WorktreeMismatch { .. }),
                "unexpected error: {error}"
            );
            let diagnostic = error.to_string();
            assert!(diagnostic.contains(&recorded), "{diagnostic}");
            assert!(diagnostic.contains(&current), "{diagnostic}");
        }
        assert_eq!(file_tree_snapshot(&graph), graph_before);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_surfaces_reject_worktree_mismatch() {
        let (code, graph) = copy_fixture("atlas-query-surfaces-worktree");
        init_git_repository(&code);
        let linked = code.parent().unwrap().join("linked-surfaces");
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                "linked-surfaces",
                &linked.to_string_lossy(),
            ])
            .current_dir(&code)
            .output()
            .unwrap();
        assert!(output.status.success(), "git worktree add: {output:?}");
        build(&code, &graph, &BuildOptions::default()).unwrap();

        let operations = [
            explore(&linked, &graph, "MemStore", &ExploreOptions::default()).map(|_| ()),
            flow(
                &linked,
                &graph,
                FlowQuery::Through {
                    symbol: "Store".into(),
                },
                &FlowOptions::default(),
            )
            .map(|_| ()),
            impact(&linked, &graph, "Store", &ImpactOptions::default()).map(|_| ()),
            affected_paths(
                &linked,
                &graph,
                &[PathBuf::from("src/lib.rs")],
                &AffectedOptions::default(),
            )
            .map(|_| ()),
        ];
        for result in operations {
            assert!(matches!(result, Err(AtlasError::WorktreeMismatch { .. })));
        }
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_borrowed_worktree_mismatch_precedes_invalid_query_index() {
        let cases: [BorrowedQueryIndexCase; 2] = [
            ("missing", remove_query_index),
            ("corrupt", corrupt_query_index),
        ];

        for (case, invalidate) in cases {
            let (code, graph) = copy_fixture(&format!("atlas-borrowed-index-{case}"));
            init_git_repository(&code);
            let linked = code.parent().unwrap().join("linked");
            let output = Command::new("git")
                .args(["worktree", "add", "-b", "linked", &linked.to_string_lossy()])
                .current_dir(&code)
                .output()
                .unwrap();
            assert!(output.status.success(), "git worktree add: {output:?}");
            build(&code, &graph, &BuildOptions::default()).unwrap();
            invalidate(&active_data_dir(&graph).join("query-index.json"));
            let graph_before = file_tree_snapshot(&graph);

            let error = query(
                &linked,
                &graph,
                "atlas_basic::store::MemStore",
                &QueryOptions::default(),
            )
            .unwrap_err();
            let AtlasError::WorktreeMismatch { recorded, current } = error else {
                panic!("{case}: expected worktree mismatch, got {error}");
            };
            assert!(recorded.contains("code"), "{case}: {recorded}");
            assert!(current.contains("linked"), "{case}: {current}");
            assert_eq!(file_tree_snapshot(&graph), graph_before, "{case}");
            fs::remove_dir_all(code.parent().unwrap()).ok();
        }
    }

    #[test]
    fn test_atlas_read_results_share_status_and_stale_mirror() {
        let (code, graph) = copy_fixture("atlas-result-status");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();

        let assert_results = |opts: &QueryOptions| {
            let tree = tree(&code, &graph, opts).unwrap();
            let query = query(&code, &graph, "atlas_basic::store::MemStore", opts).unwrap();
            let refs = refs(&code, &graph, "atlas_basic::store::MemStore", opts).unwrap();
            let impls = impls(&code, &graph, "Store", opts).unwrap();
            let search = search(
                &code,
                &graph,
                "MemStore",
                &SearchOptions {
                    limit: 20,
                    frozen: opts.frozen,
                },
            )
            .unwrap();
            for (status, stale) in [
                (&tree.status, &tree.stale),
                (&query.status, &query.stale),
                (&refs.status, &refs.stale),
                (&impls.status, &impls.stale),
                (&search.status, &search.stale),
            ] {
                assert_eq!(status, &tree.status);
                assert_eq!(stale, &status.syn.stale_files);
            }
            tree.status
        };

        let fresh = assert_results(&QueryOptions::default());
        assert_eq!(fresh.syn.state, LayerState::Fresh);
        assert_eq!(fresh.scip.state, LayerState::Fresh);

        let service = code.join("src/service.rs");
        let mut source = fs::read_to_string(&service).unwrap();
        source.push_str("\npub fn status_consistency_edit() {}\n");
        fs::write(&service, source).unwrap();

        let frozen = assert_results(&QueryOptions { frozen: true });
        assert_eq!(frozen.syn.state, LayerState::Stale);
        assert_eq!(frozen.syn.stale_files, vec!["src/service.rs"]);
        assert_eq!(frozen.scip.state, LayerState::Stale);

        let refreshed = assert_results(&QueryOptions::default());
        assert_eq!(refreshed.syn.state, LayerState::Fresh);
        assert!(refreshed.syn.stale_files.is_empty());
        assert_eq!(refreshed.scip.state, LayerState::Stale);
        assert!(
            refreshed
                .scip
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("source-set fingerprint mismatch"))
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_each_read_consumer_recomputes_status_after_refresh() {
        let (code, graph) = copy_fixture("atlas-result-status-refresh-each");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let service = code.join("src/service.rs");
        let mut edit = 0;
        let mut make_stale = || {
            edit += 1;
            let mut source = fs::read_to_string(&service).unwrap();
            source.push_str(&format!("\npub fn refresh_status_{edit}() {{}}\n"));
            fs::write(&service, source).unwrap();
        };
        let assert_refreshed = |status: &AtlasStatus, stale: &[String], consumer: &str| {
            assert_eq!(status.syn.state, LayerState::Fresh, "{consumer}");
            assert_eq!(status.scip.state, LayerState::Stale, "{consumer}");
            assert_eq!(stale, status.syn.stale_files, "{consumer}");
        };

        make_stale();
        let result = tree(&code, &graph, &QueryOptions::default()).unwrap();
        assert_refreshed(&result.status, &result.stale, "tree");

        make_stale();
        let result = query(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions::default(),
        )
        .unwrap();
        assert_refreshed(&result.status, &result.stale, "query");

        make_stale();
        let result = refs(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions::default(),
        )
        .unwrap();
        assert_refreshed(&result.status, &result.stale, "refs");

        make_stale();
        let result = impls(&code, &graph, "Store", &QueryOptions::default()).unwrap();
        assert_refreshed(&result.status, &result.stale, "impls");

        make_stale();
        let result = search(&code, &graph, "MemStore", &SearchOptions::default()).unwrap();
        assert_refreshed(&result.status, &result.stale, "search");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_exact_id_symbol_and_ambiguity() {
        let (code, graph) = copy_fixture("atlas-query-id-symbol-ambiguity");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let target = node(&shards, "atlas_basic::store::MemStore")
            .cloned()
            .unwrap();

        let by_symbol = query(&code, &graph, &target.symbol, &QueryOptions::default()).unwrap();
        let by_id = query(&code, &graph, &target.id, &QueryOptions::default()).unwrap();
        assert_eq!(by_symbol.node, target);
        assert_eq!(by_id.node, target);

        let mut duplicate = target.clone();
        duplicate.id = format!("{}~duplicate", target.id);
        rebuild_test_index(&graph, |shards| {
            shards
                .iter_mut()
                .find(|shard| shard.file == duplicate.file)
                .unwrap()
                .nodes
                .push(duplicate.clone());
        });

        let error = query(&code, &graph, &target.symbol, &QueryOptions::default()).unwrap_err();
        assert!(matches!(
            error,
            AtlasError::AmbiguousSymbol {
                ref symbol,
                declarations: 2
            } if symbol == &target.symbol
        ));
        let by_id = query(&code, &graph, &target.id, &QueryOptions::default()).unwrap();
        assert_eq!(by_id.node, target);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_refs_aggregate_declarations_filter_and_sort() {
        let (code, graph) = copy_fixture("atlas-refs-multi-declaration");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, original_shards) = load_graph(&graph).unwrap();
        let target = node(&original_shards, "atlas_basic::store::MemStore")
            .cloned()
            .unwrap();
        let mut duplicate = target.clone();
        duplicate.id = format!("{}~duplicate", target.id);
        let first_source = node(&original_shards, "atlas_basic::open_default")
            .unwrap()
            .id
            .clone();
        let second_source = node(&original_shards, "atlas_basic::service::run")
            .unwrap()
            .id
            .clone();
        let mut reference = edge(&second_source, &target.id, EdgeKind::References);
        reference.provenance = Provenance::Scip;
        let mut call = edge(&first_source, &duplicate.id, EdgeKind::Calls);
        call.provenance = Provenance::Scip;
        let uses_type = edge(&first_source, &target.id, EdgeKind::UsesType);

        rebuild_test_index(&graph, |shards| {
            let shard = shards
                .iter_mut()
                .find(|shard| shard.file == target.file)
                .unwrap();
            shard.nodes.push(duplicate.clone());
            shard
                .edges
                .extend([reference.clone(), call.clone(), uses_type]);
        });

        let report = refs(&code, &graph, &target.symbol, &QueryOptions::default()).unwrap();
        let expected: Vec<Edge> = BTreeSet::from([reference, call]).into_iter().collect();
        assert_eq!(report.symbol, target.symbol);
        assert_eq!(report.edges, expected);
        assert!(report.edges.windows(2).all(|pair| pair[0] < pair[1]));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_impls_suffix_unresolved_fallback_and_sort() {
        let (code, graph) = copy_fixture("atlas-impls-indexed-behavior");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, original_shards) = load_graph(&graph).unwrap();
        let suffix_match = all_edges(&original_shards)
            .into_iter()
            .find(|edge| edge.kind == EdgeKind::ImplsTrait)
            .unwrap();
        let mut unresolved = edge("impl-unresolved", "not-store", EdgeKind::ImplsTrait);
        unresolved.target_text = Some("external::Store".to_string());
        unresolved.resolution = EdgeResolution::Unresolved;
        unresolved.confidence = None;
        let mut to_fallback = edge("impl-fallback", "external::Store", EdgeKind::ImplsTrait);
        to_fallback.resolution = EdgeResolution::Unresolved;
        to_fallback.confidence = None;
        let excluded = edge(
            "impl-excluded",
            "external::Storehouse",
            EdgeKind::ImplsTrait,
        );

        rebuild_test_index(&graph, |shards| {
            shards[0]
                .edges
                .extend([unresolved.clone(), to_fallback.clone(), excluded]);
        });

        let report = impls(&code, &graph, "Store", &QueryOptions::default()).unwrap();
        let expected: Vec<Edge> = BTreeSet::from([suffix_match, unresolved, to_fallback])
            .into_iter()
            .collect();
        assert_eq!(report.symbol, "Store");
        assert_eq!(report.edges, expected);
        assert!(report.edges.windows(2).all(|pair| pair[0] < pair[1]));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_impls_matches_suffix_node_id_without_text_fallback() {
        let (code, graph) = copy_fixture("atlas-impls-suffix-locator-only");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, original_shards) = load_graph(&graph).unwrap();
        let target = node(&original_shards, "atlas_basic::store::Store")
            .cloned()
            .unwrap();
        let mut locator_only = all_edges(&original_shards)
            .into_iter()
            .find(|edge| edge.kind == EdgeKind::ImplsTrait && edge.to == target.id)
            .unwrap();
        locator_only.target_text = None;

        assert!(target.symbol.ends_with("::Store"));
        assert_ne!(target.symbol, "Store");
        assert!(!locator_only.to.ends_with("::Store"));
        rebuild_test_index(&graph, |shards| {
            for shard in shards.iter_mut() {
                shard.edges.clear();
            }
            shards[0].edges.push(locator_only.clone());
        });

        let report = impls(&code, &graph, "Store", &QueryOptions::default()).unwrap();
        assert_eq!(report.symbol, "Store");
        assert_eq!(report.edges, vec![locator_only]);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_sorts_complete_adjacency_and_is_byte_stable() {
        let (code, graph) = copy_fixture("atlas-query-adjacency-order");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let meta = read_meta(&graph).unwrap();
        let mut index = load_query_index(&graph, &meta).unwrap();
        let target = index
            .matching_nodes("atlas_basic::store::MemStore")
            .into_iter()
            .next()
            .cloned()
            .unwrap();
        let incoming_z = edge("z-incoming", &target.id, EdgeKind::References);
        let outgoing_z = edge(&target.id, "z-outgoing", EdgeKind::Calls);
        let incoming_a = edge("a-incoming", &target.id, EdgeKind::Calls);
        let outgoing_a = edge(&target.id, "a-outgoing", EdgeKind::UsesType);

        let unsorted_edges = vec![
            incoming_z.clone(),
            outgoing_z.clone(),
            incoming_a.clone(),
            outgoing_a.clone(),
        ];
        index = QueryIndex::from_test_parts(
            &index.graph_fingerprint,
            index.nodes.clone(),
            unsorted_edges,
        );
        let sorted_table: Vec<Edge> = index
            .edges
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_ne!(index.edges, sorted_table, "fixture table must be unsorted");
        let index_path = active_data_dir(&graph).join("query-index.json");
        write_json_atomic(&index_path, &index).unwrap();
        let index_bytes = fs::read(&index_path).unwrap();

        let first = query(&code, &graph, &target.id, &QueryOptions::default()).unwrap();
        let expected_in: Vec<Edge> = BTreeSet::from([incoming_z, incoming_a])
            .into_iter()
            .collect();
        let expected_out: Vec<Edge> = BTreeSet::from([outgoing_z, outgoing_a])
            .into_iter()
            .collect();
        assert_eq!(first.edges_in, expected_in);
        assert_eq!(first.edges_out, expected_out);

        let first_json = serde_json::to_vec(&first).unwrap();
        let second = query(&code, &graph, &target.id, &QueryOptions::default()).unwrap();
        assert_eq!(serde_json::to_vec(&second).unwrap(), first_json);
        assert_eq!(fs::read(index_path).unwrap(), index_bytes);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_low_level_queries_propagate_missing_query_index() {
        let (code, graph) = copy_fixture("atlas-low-level-query-index-missing");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        fs::remove_file(active_data_dir(&graph).join("query-index.json")).unwrap();
        let frozen = QueryOptions { frozen: true };

        for result in [
            query(&code, &graph, "atlas_basic::store::MemStore", &frozen).map(|_| ()),
            refs(&code, &graph, "atlas_basic::store::MemStore", &frozen).map(|_| ()),
            impls(&code, &graph, "Store", &frozen).map(|_| ()),
        ] {
            assert!(matches!(result, Err(AtlasError::QueryIndexMissing { .. })));
        }
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_low_level_queries_propagate_invalid_query_index_errors() {
        let cases: [(&str, QueryIndexMutation, QueryIndexErrorMatcher); 3] = [
            (
                "schema",
                |index| index["schema_version"] = serde_json::json!(SCHEMA_VERSION - 1),
                |error| matches!(error, AtlasError::QueryIndexSchema { .. }),
            ),
            (
                "stale",
                |index| index["graph_fingerprint"] = serde_json::json!("wrong"),
                |error| matches!(error, AtlasError::QueryIndexStale { .. }),
            ),
            (
                "corrupt",
                |index| index["nodes"] = serde_json::json!("not a node table"),
                |error| matches!(error, AtlasError::QueryIndexCorrupt { .. }),
            ),
        ];

        for (case, update, expected) in cases {
            let (code, graph) = copy_fixture(&format!("atlas-low-level-query-index-{case}"));
            build(&code, &graph, &BuildOptions::default()).unwrap();
            let index_path = active_data_dir(&graph).join("query-index.json");
            let mut index: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&index_path).unwrap()).unwrap();
            update(&mut index);
            fs::write(&index_path, serde_json::to_vec_pretty(&index).unwrap()).unwrap();
            let frozen = QueryOptions { frozen: true };

            for result in [
                query(&code, &graph, "atlas_basic::store::MemStore", &frozen).map(|_| ()),
                refs(&code, &graph, "atlas_basic::store::MemStore", &frozen).map(|_| ()),
                impls(&code, &graph, "Store", &frozen).map(|_| ()),
            ] {
                let error = result.unwrap_err();
                assert!(expected(&error), "{case}: {error}");
            }
            fs::remove_dir_all(code.parent().unwrap()).ok();
        }
    }

    #[test]
    fn test_atlas_stale_non_frozen_queries_validate_index_before_refresh() {
        let cases: [QueryIndexCase; 4] = [
            ("missing", remove_query_index, |error| {
                matches!(error, AtlasError::QueryIndexMissing { .. })
            }),
            ("schema", downgrade_query_index_schema, |error| {
                matches!(error, AtlasError::QueryIndexSchema { .. })
            }),
            ("stale", stale_query_index_fingerprint, |error| {
                matches!(error, AtlasError::QueryIndexStale { .. })
            }),
            ("corrupt", corrupt_query_index, |error| {
                matches!(error, AtlasError::QueryIndexCorrupt { .. })
            }),
        ];
        let operations: [(&str, LowLevelQuery); 4] = [
            ("query", |code, graph, opts| {
                query(code, graph, "atlas_basic::store::MemStore", opts).map(|_| ())
            }),
            ("refs", |code, graph, opts| {
                refs(code, graph, "atlas_basic::store::MemStore", opts).map(|_| ())
            }),
            ("impls", |code, graph, opts| {
                impls(code, graph, "Store", opts).map(|_| ())
            }),
            ("search", |code, graph, opts| {
                search(
                    code,
                    graph,
                    "MemStore",
                    &SearchOptions {
                        limit: 20,
                        frozen: opts.frozen,
                    },
                )
                .map(|_| ())
            }),
        ];

        for (case, invalidate, expected) in cases {
            for (operation_name, operation) in operations {
                let fixture = format!("atlas-stale-index-{case}-{operation_name}");
                let (code, graph) = copy_fixture(&fixture);
                build(&code, &graph, &BuildOptions::default()).unwrap();

                let service = code.join("src/service.rs");
                let mut source = fs::read_to_string(&service).unwrap();
                source.push_str("\npub fn stale_before_index_validation() {}\n");
                fs::write(&service, source).unwrap();
                invalidate(&active_data_dir(&graph).join("query-index.json"));
                let source_before = source_tree_snapshot(&code);
                let graph_before = file_tree_snapshot(&graph);

                let error = operation(&code, &graph, &QueryOptions::default()).unwrap_err();
                assert!(expected(&error), "{case}/{operation_name}: {error}");
                assert_eq!(source_tree_snapshot(&code), source_before);
                assert_eq!(file_tree_snapshot(&graph), graph_before);
                fs::remove_dir_all(code.parent().unwrap()).ok();
            }
        }
    }

    #[test]
    fn test_atlas_query_returns_symbol_facts() {
        let (code, graph) = copy_fixture("atlas-query");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let result = query(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions::default(),
        )
        .unwrap();
        assert_eq!(result.node.kind, NodeKind::Struct);
        assert!(result.node.file.ends_with("src/store.rs"));
        assert!(result.node.signature.contains("MemStore"));
        assert!(result.stale.is_empty());
        assert!(
            result.edges_in.iter().any(|e| e.kind == EdgeKind::Contains
                && e.from.starts_with("atlas_basic::store#module-"))
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_tree_renders_module_outline() {
        let (code, graph) = copy_fixture("atlas-tree");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let outline = tree(&code, &graph, &QueryOptions::default()).unwrap();
        let rendered = serde_json::to_string(&outline.tree).unwrap();
        // symbols nest under their modules, deterministically ordered
        let store_pos = rendered.find("atlas_basic::store").unwrap();
        let memstore_pos = rendered.find("atlas_basic::store::MemStore").unwrap();
        assert!(store_pos < memstore_pos);
        let a = rendered.find("atlas_basic::service").unwrap();
        assert!(a < store_pos, "service sorts before store");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_unknown_symbol_errors() {
        let (code, graph) = copy_fixture("atlas-unknown");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let err = query(
            &code,
            &graph,
            "atlas_basic::store::Ghost",
            &QueryOptions::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("atlas-unknown-symbol"));
        assert!(err.to_string().contains("Ghost"));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_without_graph_errors() {
        let (code, graph) = copy_fixture("atlas-nograph");
        let err = query(
            &code,
            &graph,
            "atlas_basic::store::MemStore",
            &QueryOptions::default(),
        )
        .unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("atlas build"),
            "must name the first step: {text}"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_ingests_scip_index_reference_edges() {
        let (code, graph) = copy_fixture("atlas-scip");
        let report = build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        assert!(report.capability.scip);

        let (meta, shards) = load_graph(&graph).unwrap();
        assert!(meta.capability.scip);
        let edges = all_edges(&shards);
        assert!(
            edges.iter().any(|e| e.kind == EdgeKind::References
                && e.provenance == Provenance::Scip
                && node(&shards, "atlas_basic::service::run")
                    .is_some_and(|node| e.from == node.id)
                && node(&shards, "atlas_basic::store::MemStore")
                    .is_some_and(|node| e.to == node.id)),
            "cross-file scip reference expected: {edges:?}"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_build_degrades_without_scip() {
        let (code, graph) = copy_fixture("atlas-noscip");
        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert!(!report.capability.scip);
        let (meta, shards) = load_graph(&graph).unwrap();
        assert!(!meta.capability.scip);
        assert!(
            all_edges(&shards)
                .iter()
                .all(|e| e.provenance == Provenance::Syn)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_default_build_excludes_mir() {
        let (code, graph) = copy_fixture("atlas-default-no-mir");
        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["capability"]["mir"], false);
        assert_eq!(value["diagnostics"], serde_json::json!([]));
        assert!(
            all_edges(&load_graph(&graph).unwrap().1)
                .iter()
                .all(|edge| edge.provenance != Provenance::Mir)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_overlay_schema_documents_consumer_contract() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/atlas-schemas/mir-overlay-v1.schema.json"
        ))
        .unwrap();
        assert_eq!(
            schema["properties"]["schema"]["const"],
            "rust-atlas/mir-overlay-v1"
        );
        assert_eq!(
            schema["$defs"]["call"]["properties"]["generic"]["default"],
            false
        );
        assert_eq!(
            schema["$defs"]["cfg"]["required"],
            serde_json::json!(["basic_blocks", "edges", "exits", "loop_headers"])
        );
    }

    #[test]
    fn test_atlas_default_build_removes_previous_mir_overlay() {
        let (code, graph) = copy_fixture("atlas-default-removes-mir");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, shards) = load_graph(&graph).unwrap();
        let caller = node(&shards, "atlas_basic::service::run")
            .unwrap()
            .id
            .clone();
        let callee = node(
            &shards,
            "atlas_basic::store::impl atlas_basic::store::Store for atlas_basic::store::MemStore::get",
        )
        .unwrap()
        .id
        .clone();
        let shards_dir = active_data_dir(&graph).join("shards");
        let mut service = read_shard(&shards_dir, "src/service.rs").unwrap();
        service
            .nodes
            .iter_mut()
            .find(|item| item.id == caller)
            .unwrap()
            .cfg = Some(CfgSummary {
            basic_blocks: 1,
            edges: 0,
            exits: 1,
            loop_headers: 0,
        });
        let mut mir_edge = edge(&caller, &callee, EdgeKind::Calls);
        mir_edge.provenance = Provenance::Mir;
        service.edges.push(mir_edge);
        write_shard(&shards_dir, &service).unwrap();
        assert_eq!(meta.files.len(), 3);

        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            all_edges(&shards)
                .iter()
                .all(|edge| edge.provenance != Provenance::Mir)
        );
        assert!(
            shards
                .iter()
                .flat_map(|shard| &shard.nodes)
                .all(|node| node.cfg.is_none())
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_overlay_adds_calls_edges() {
        let (code, graph) = copy_fixture("atlas-mir-overlay");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        let source_fingerprint = status::source_fingerprint(&meta.files).unwrap();
        let overlay = code.parent().unwrap().join("mir-overlay.json");
        fs::write(
            &overlay,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "rust-atlas/mir-overlay-v1",
                "extractor": {"name": "rustc_public", "version": "nightly-2026-07-10"},
                "source_fingerprint": source_fingerprint,
                "functions": [{
                    "symbol": "atlas_basic::service::run",
                    "cfg": {"basic_blocks": 3, "edges": 2, "exits": 1, "loop_headers": 0},
                    "calls": [{
                        "target": "atlas_basic::store::impl atlas_basic::store::Store for atlas_basic::store::MemStore::get",
                        "site": {"file": "src/service.rs", "line_start": 4, "column_start": 5, "line_end": 4, "column_end": 16},
                        "dispatch": "static",
                        "generic": false,
                        "evidence": "rustc_public MIR terminator Call"
                    }]
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let report = build_with_mir(
            &code,
            &graph,
            &BuildOptions::default(),
            &MirBuildOptions {
                overlay: Some(overlay),
                driver: None,
            },
        )
        .unwrap();
        assert!(report.capability.mir);
        assert!(report.diagnostics.is_empty());

        let (_, shards) = load_graph(&graph).unwrap();
        let caller = node(&shards, "atlas_basic::service::run").unwrap();
        assert_eq!(
            caller.cfg,
            Some(CfgSummary {
                basic_blocks: 3,
                edges: 2,
                exits: 1,
                loop_headers: 0,
            })
        );
        assert!(all_edges(&shards).iter().any(|edge| {
            edge.from == caller.id
                && edge.kind == EdgeKind::Calls
                && edge.provenance == Provenance::Mir
                && edge.confidence == Some(EdgeConfidence::Exact)
        }));
        assert!(
            all_edges(&shards)
                .iter()
                .any(|edge| edge.provenance == Provenance::Syn)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_query_prefers_highest_provenance_edge() {
        let mut syn = edge("caller", "callee", EdgeKind::References);
        syn.provenance = Provenance::Syn;
        let mut scip = edge("caller", "callee", EdgeKind::Calls);
        scip.provenance = Provenance::Scip;
        let mut mir = scip.clone();
        mir.provenance = Provenance::Mir;
        mir.site = Some(EdgeSite {
            file: "src/lib.rs".into(),
            line_start: 3,
            column_start: 5,
            line_end: 3,
            column_end: 12,
        });
        let mut dynamic = scip.clone();
        dynamic.resolution = EdgeResolution::Unresolved;
        dynamic.dispatch = Some(DispatchKind::Trait);
        dynamic.confidence = Some(EdgeConfidence::BoundedCandidates);
        dynamic.candidates = vec!["impl-a".into(), "impl-b".into()];

        let preferred =
            preferred_query_edges(vec![syn.clone(), scip, mir.clone(), dynamic.clone()]);
        assert_eq!(preferred, vec![mir, dynamic]);
        assert_eq!(
            vec![syn].len(),
            1,
            "stored lower evidence remains independent"
        );
    }

    #[test]
    fn test_atlas_status_reports_stale_mir_after_syn_refresh() {
        let (code, graph) = copy_fixture("atlas-mir-status");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        let source_fingerprint = status::source_fingerprint(&meta.files).unwrap();
        let overlay = code.parent().unwrap().join("mir-overlay.json");
        fs::write(
            &overlay,
            serde_json::to_vec(&serde_json::json!({
                "schema": "rust-atlas/mir-overlay-v1",
                "extractor": {"name": "rustc_public", "version": "nightly-test"},
                "source_fingerprint": source_fingerprint,
                "functions": [{
                    "symbol": "atlas_basic::service::run",
                    "cfg": {"basic_blocks": 2, "edges": 1, "exits": 1, "loop_headers": 0},
                    "calls": []
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        build_with_mir(
            &code,
            &graph,
            &BuildOptions::default(),
            &MirBuildOptions {
                overlay: Some(overlay),
                driver: None,
            },
        )
        .unwrap();
        assert_eq!(status(&code, &graph).unwrap().mir.state, LayerState::Fresh);

        let service = code.join("src/service.rs");
        let mut source = fs::read_to_string(&service).unwrap();
        source.push('\n');
        fs::write(&service, source).unwrap();
        let result = query(
            &code,
            &graph,
            "atlas_basic::service::run",
            &QueryOptions::default(),
        )
        .unwrap();
        assert_eq!(result.status.syn.state, LayerState::Fresh);
        assert_eq!(result.status.mir.state, LayerState::Stale);
        assert_eq!(
            result.status.mir.extractor.as_deref(),
            Some("rustc_public nightly-test")
        );
        assert_eq!(
            result.status.mir.recorded_source_fingerprint.as_deref(),
            Some(source_fingerprint.as_str())
        );
        assert!(
            result.status.mir.current_source_fingerprint.is_some()
                && result.status.mir.current_source_fingerprint
                    != result.status.mir.recorded_source_fingerprint
        );
        assert!(result.status.mir.recorded_fingerprint.is_some());
        assert_eq!(
            result.status.mir.recorded_fingerprint,
            result.status.mir.current_fingerprint
        );
        assert!(
            result
                .status
                .mir
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("source fingerprint"))
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_failure_degrades_to_syn_graph() {
        let (code, graph) = copy_fixture("atlas-mir-failure");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        let source_fingerprint = status::source_fingerprint(&meta.files).unwrap();
        let overlay = code.parent().unwrap().join("mir-overlay.json");
        fs::write(
            &overlay,
            serde_json::to_vec(&serde_json::json!({
                "schema": "rust-atlas/mir-overlay-v1",
                "extractor": {"name": "rustc_public", "version": "nightly-test"},
                "source_fingerprint": source_fingerprint,
                "functions": [{
                    "symbol": "atlas_basic::service::run",
                    "cfg": {"basic_blocks": 1, "edges": 0, "exits": 1, "loop_headers": 0},
                    "calls": []
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let options = MirBuildOptions {
            overlay: Some(overlay.clone()),
            driver: None,
        };
        build_with_mir(&code, &graph, &BuildOptions::default(), &options).unwrap();
        assert!(
            load_graph(&graph)
                .unwrap()
                .1
                .iter()
                .any(|shard| { shard.nodes.iter().any(|node| node.cfg.is_some()) })
        );

        fs::write(&overlay, b"{not valid json").unwrap();
        let report = build_with_mir(&code, &graph, &BuildOptions::default(), &options).unwrap();
        assert!(!report.capability.mir);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "mir-extraction-failed");
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            all_edges(&shards)
                .iter()
                .all(|edge| edge.provenance != Provenance::Mir)
        );
        assert!(
            shards
                .iter()
                .flat_map(|shard| &shard.nodes)
                .all(|node| node.cfg.is_none())
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_generic_call_is_not_monomorphized() {
        let (code, graph) = copy_fixture("atlas-mir-generic");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        let source_fingerprint = status::source_fingerprint(&meta.files).unwrap();
        let overlay = code.parent().unwrap().join("mir-overlay.json");
        fs::write(
            &overlay,
            serde_json::to_vec(&serde_json::json!({
                "schema": "rust-atlas/mir-overlay-v1",
                "extractor": {"name": "rustc_public", "version": "nightly-test"},
                "source_fingerprint": source_fingerprint,
                "functions": [{
                    "symbol": "atlas_basic::service::run",
                    "cfg": {"basic_blocks": 2, "edges": 1, "exits": 1, "loop_headers": 0},
                    "calls": [{
                        "target": "atlas_basic::store::Store::get",
                        "site": {"file": "src/service.rs", "line_start": 4, "column_start": 5, "line_end": 4, "column_end": 16},
                        "dispatch": "generic",
                        "generic": true,
                        "evidence": "rustc_public generic MIR call"
                    }]
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        build_with_mir(
            &code,
            &graph,
            &BuildOptions::default(),
            &MirBuildOptions {
                overlay: Some(overlay),
                driver: None,
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let mir_calls = all_edges(&shards)
            .into_iter()
            .filter(|edge| edge.provenance == Provenance::Mir)
            .collect::<Vec<_>>();
        assert_eq!(mir_calls.len(), 1);
        assert!(mir_calls[0].generic);
        assert_eq!(mir_calls[0].dispatch, Some(DispatchKind::Generic));
        assert!(mir_calls[0].candidates.is_empty());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_rejects_schema_without_partial_write() {
        let (code, graph) = copy_fixture("atlas-mir-schema");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        let source_fingerprint = status::source_fingerprint(&meta.files).unwrap();
        let overlay = code.parent().unwrap().join("mir-overlay.json");
        fs::write(
            &overlay,
            serde_json::to_vec(&serde_json::json!({
                "schema": "rust-atlas/mir-overlay-v999",
                "extractor": {"name": "rustc_public", "version": "nightly-test"},
                "source_fingerprint": source_fingerprint,
                "functions": [{
                    "symbol": "atlas_basic::service::run",
                    "cfg": {"basic_blocks": 1, "edges": 0, "exits": 1, "loop_headers": 0},
                    "calls": []
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let report = build_with_mir(
            &code,
            &graph,
            &BuildOptions::default(),
            &MirBuildOptions {
                overlay: Some(overlay),
                driver: None,
            },
        )
        .unwrap();
        assert_eq!(report.diagnostics[0].code, "mir-extraction-failed");
        assert!(report.diagnostics[0].message.contains("mir-overlay-v999"));
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            shards
                .iter()
                .flat_map(|shard| &shard.nodes)
                .all(|node| node.cfg.is_none())
        );
        assert!(
            all_edges(&shards)
                .iter()
                .all(|edge| edge.provenance != Provenance::Mir)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_rejects_non_function_and_inconsistent_generic_facts() {
        let (code, graph) = copy_fixture("atlas-mir-semantic-validation");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        let source_fingerprint = status::source_fingerprint(&meta.files).unwrap();
        let overlay = code.parent().unwrap().join("mir-overlay.json");
        let options = MirBuildOptions {
            overlay: Some(overlay.clone()),
            driver: None,
        };

        fs::write(
            &overlay,
            serde_json::to_vec(&serde_json::json!({
                "schema": "rust-atlas/mir-overlay-v1",
                "extractor": {"name": "rustc_public", "version": "nightly-test"},
                "source_fingerprint": source_fingerprint,
                "functions": [{
                    "symbol": "atlas_basic::service",
                    "cfg": {"basic_blocks": 1, "edges": 0, "exits": 1, "loop_headers": 0},
                    "calls": []
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let report = build_with_mir(&code, &graph, &BuildOptions::default(), &options).unwrap();
        assert_eq!(report.diagnostics[0].code, "mir-extraction-failed");
        assert!(report.diagnostics[0].message.contains("is not a function"));

        fs::write(
            &overlay,
            serde_json::to_vec(&serde_json::json!({
                "schema": "rust-atlas/mir-overlay-v1",
                "extractor": {"name": "rustc_public", "version": "nightly-test"},
                "source_fingerprint": source_fingerprint,
                "functions": [{
                    "symbol": "atlas_basic::service::run",
                    "cfg": {"basic_blocks": 1, "edges": 0, "exits": 1, "loop_headers": 0},
                    "calls": [{
                        "target": "atlas_basic::store::Store::get",
                        "site": {"file": "src/service.rs", "line_start": 4, "column_start": 5, "line_end": 4, "column_end": 16},
                        "dispatch": "static",
                        "generic": true,
                        "evidence": "inconsistent producer fact"
                    }]
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let report = build_with_mir(&code, &graph, &BuildOptions::default(), &options).unwrap();
        assert_eq!(report.diagnostics[0].code, "mir-extraction-failed");
        assert!(report.diagnostics[0].message.contains("generic flag"));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_generation_write_failure_keeps_baseline() {
        let (code, graph) = copy_fixture("atlas-mir-generation-failure");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, mut shards) = load_graph(&graph).unwrap();
        let shards_dir = active_data_dir(&graph).join("shards");
        let baseline = meta
            .files
            .keys()
            .map(|rel| {
                let path = shards_dir.join(shard_file_name(rel));
                (path.clone(), fs::read(path).unwrap())
            })
            .collect::<BTreeMap<_, _>>();
        for shard in &mut shards {
            if let Some(node) = shard
                .nodes
                .iter_mut()
                .find(|node| node.kind == NodeKind::Fn)
            {
                node.cfg = Some(CfgSummary {
                    basic_blocks: 1,
                    edges: 0,
                    exits: 1,
                    loop_headers: 0,
                });
            }
        }
        let updates = shards
            .into_iter()
            .map(|shard| (shard.file.clone(), shard))
            .collect::<BTreeMap<_, _>>();
        let mut writes = 0;
        let error = commit_shard_generation_with(&shards_dir, &updates, |dir, shard| {
            writes += 1;
            if writes == 2 {
                return Err(AtlasError::Io("injected second shard write failure".into()));
            }
            write_shard(dir, shard)
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected second shard"));
        for (path, bytes) in baseline {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_mir_rejects_unknown_nested_wire_fields() {
        let (code, graph) = copy_fixture("atlas-mir-strict-wire");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        let source_fingerprint = status::source_fingerprint(&meta.files).unwrap();
        let overlay = code.parent().unwrap().join("mir-overlay.json");
        let base = serde_json::json!({
            "schema": "rust-atlas/mir-overlay-v1",
            "extractor": {"name": "rustc_public", "version": "nightly-test"},
            "source_fingerprint": source_fingerprint,
            "functions": [{
                "symbol": "atlas_basic::service::run",
                "cfg": {"basic_blocks": 2, "edges": 1, "exits": 1, "loop_headers": 0},
                "calls": [{
                    "target": "atlas_basic::store::impl atlas_basic::store::Store for atlas_basic::store::MemStore::get",
                    "site": {"file": "src/service.rs", "line_start": 4, "column_start": 5, "line_end": 4, "column_end": 16},
                    "dispatch": "static",
                    "generic": false,
                    "evidence": "strict wire test"
                }]
            }]
        });
        for pointer in [
            "/extractor/unknown",
            "/functions/0/cfg/unknown",
            "/functions/0/calls/0/site/unknown",
        ] {
            let mut artifact = base.clone();
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            artifact
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.to_string(), serde_json::json!(true));
            fs::write(&overlay, serde_json::to_vec(&artifact).unwrap()).unwrap();
            let report = build_with_mir(
                &code,
                &graph,
                &BuildOptions::default(),
                &MirBuildOptions {
                    overlay: Some(overlay.clone()),
                    driver: None,
                },
            )
            .unwrap();
            assert_eq!(report.diagnostics[0].code, "mir-extraction-failed");
            assert!(report.diagnostics[0].message.contains("unknown field"));
        }
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    fn dynamic_trait_fixture(name: &str, implementation_count: usize) -> (PathBuf, PathBuf) {
        let mut source = String::from(
            "pub trait Handler { fn handle(&self) -> usize; }\n\
             pub fn dispatch(value: &dyn Handler) -> usize { value.handle() }\n",
        );
        for index in 0..implementation_count {
            source.push_str(&format!(
                "pub struct Handler{index};\nimpl Handler for Handler{index} {{ fn handle(&self) -> usize {{ {index} }} }}\n"
            ));
        }
        scratch_crate(name, &[("src/lib.rs", source.as_str())])
    }

    fn inject_resolved_trait_call(graph: &Path) -> (String, Vec<String>) {
        let (_, shards) = load_graph(graph).unwrap();
        let caller = shards
            .iter()
            .flat_map(|shard| &shard.nodes)
            .find(|node| node.kind == NodeKind::Fn && node.symbol.ends_with("::dispatch"))
            .unwrap()
            .clone();
        let trait_method = shards
            .iter()
            .flat_map(|shard| &shard.nodes)
            .find(|node| node.kind == NodeKind::Fn && node.symbol.ends_with("::Handler::handle"))
            .unwrap()
            .clone();
        let mut candidates = shards
            .iter()
            .flat_map(|shard| &shard.nodes)
            .filter(|node| {
                node.kind == NodeKind::Fn
                    && node.symbol.contains("::impl ")
                    && node.symbol.contains("::Handler for ")
                    && node.symbol.ends_with("::handle")
            })
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        candidates.sort();

        let shards_dir = active_data_dir(graph).join("shards");
        let mut shard = read_shard(&shards_dir, &caller.file).unwrap();
        shard.edges.push(Edge {
            from: caller.id,
            to: trait_method.id,
            target_text: Some(trait_method.symbol),
            resolution: EdgeResolution::Resolved,
            kind: EdgeKind::Calls,
            provenance: Provenance::Scip,
            site: Some(EdgeSite {
                file: caller.file.clone(),
                line_start: caller.line_start,
                column_start: 1,
                line_end: caller.line_start,
                column_end: 2,
            }),
            extractor: Some(scip_extractor(Some("test-anchor"))),
            dispatch: None,
            confidence: Some(EdgeConfidence::Exact),
            candidates: Vec::new(),
            evidence: Some("test resolved trait call anchor".into()),
            generic: false,
        });
        shard.edges.sort();
        shard.edges.dedup();
        write_shard(&shards_dir, &shard).unwrap();
        (caller.symbol, candidates)
    }

    #[test]
    fn test_atlas_dynamic_dispatch_enriches_trait_call_and_flow() {
        let (code, graph) = dynamic_trait_fixture("atlas_dynamic_flow", 2);
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (caller, expected_candidates) = inject_resolved_trait_call(&graph);
        let (meta, _) = load_graph(&graph).unwrap();

        let outcome = dynamic_dispatch::enrich(
            &active_data_dir(&graph).join("shards"),
            &meta.files,
            dynamic_dispatch::MAX_CANDIDATES,
        )
        .unwrap();
        assert_eq!(outcome.edges_added, 1);
        assert!(outcome.diagnostics.is_empty());

        let (meta, shards) = load_graph(&graph).unwrap();
        let edges = all_edges(&shards);
        let inferred = edges
            .iter()
            .find(|edge| dynamic_dispatch::is_dynamic_edge(edge))
            .unwrap();
        assert_eq!(inferred.resolution, EdgeResolution::Unresolved);
        assert_eq!(inferred.dispatch, Some(DispatchKind::Trait));
        assert_eq!(inferred.confidence, Some(EdgeConfidence::BoundedCandidates));
        assert_eq!(inferred.candidates, expected_candidates);
        assert!(edges.iter().any(|edge| {
            edge.from == inferred.from
                && edge.to == inferred.to
                && edge.provenance == Provenance::Scip
                && edge.confidence == Some(EdgeConfidence::Exact)
        }));

        rebuild_query_index(&graph, &meta, &shards).unwrap();
        let target = shards
            .iter()
            .flat_map(|shard| &shard.nodes)
            .find(|node| node.id == inferred.candidates[0])
            .unwrap();
        let result = flow(
            &code,
            &graph,
            FlowQuery::Between {
                from: caller,
                to: target.symbol.clone(),
            },
            &FlowOptions {
                frozen: true,
                ..FlowOptions::default()
            },
        )
        .unwrap();
        assert_eq!(result.state, FlowState::Found);
        assert_eq!(
            result.shortest.unwrap().confidence,
            PathConfidence::BoundedCandidates
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_dynamic_dispatch_is_inert_without_trait_call() {
        let (code, graph) = dynamic_trait_fixture("atlas_dynamic_inert", 2);
        let report = build(
            &code,
            &graph,
            &BuildOptions {
                dynamic_dispatch: true,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        assert!(report.capability.dynamic_dispatch);
        assert_eq!(report.capability.dynamic_dispatch_edges, 0);
        assert!(
            all_edges(&load_graph(&graph).unwrap().1)
                .iter()
                .all(|edge| !dynamic_dispatch::is_dynamic_edge(edge))
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_dynamic_dispatch_fanout_fails_closed() {
        let (code, graph) = dynamic_trait_fixture("atlas_dynamic_fanout", 65);
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, expected_candidates) = inject_resolved_trait_call(&graph);
        assert_eq!(expected_candidates.len(), 65);
        let (meta, _) = load_graph(&graph).unwrap();
        let outcome = dynamic_dispatch::enrich(
            &active_data_dir(&graph).join("shards"),
            &meta.files,
            dynamic_dispatch::MAX_CANDIDATES,
        )
        .unwrap();
        assert_eq!(outcome.edges_added, 0);
        assert_eq!(outcome.diagnostics[0].code, "dynamic-dispatch-truncated");
        assert!(
            all_edges(&load_graph(&graph).unwrap().1)
                .iter()
                .all(|edge| !dynamic_dispatch::is_dynamic_edge(edge))
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_default_build_removes_dynamic_dispatch_edges() {
        let (code, graph) = dynamic_trait_fixture("atlas_dynamic_cleanup", 1);
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (caller, candidates) = inject_resolved_trait_call(&graph);
        let (meta, _) = load_graph(&graph).unwrap();
        dynamic_dispatch::enrich(
            &active_data_dir(&graph).join("shards"),
            &meta.files,
            dynamic_dispatch::MAX_CANDIDATES,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);

        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            all_edges(&shards)
                .iter()
                .all(|edge| !dynamic_dispatch::is_dynamic_edge(edge))
        );
        assert!(node(&shards, &caller).is_some());
        assert!(
            all_edges(&shards)
                .iter()
                .any(|edge| edge.provenance == Provenance::Syn)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_mir_driver_uses_fixed_argv_and_degrades_on_failure() {
        use std::os::unix::fs::PermissionsExt;

        let (code, graph) = copy_fixture("atlas-mir-driver-argv");
        let base = code.parent().unwrap();
        let driver = base.join("mir-driver");
        let argv_log = base.join("argv.log");
        fs::write(
            &driver,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 23\n",
                argv_log.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&driver).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&driver, permissions).unwrap();

        let report = build_with_mir(
            &code,
            &graph,
            &BuildOptions::default(),
            &MirBuildOptions {
                overlay: None,
                driver: Some(driver),
            },
        )
        .unwrap();
        assert_eq!(report.diagnostics[0].code, "mir-extraction-failed");
        let args = fs::read_to_string(&argv_log).unwrap();
        let canonical_code = fs::canonicalize(&code).unwrap();
        let args = args.lines().collect::<Vec<_>>();
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "--code");
        assert_eq!(args[1], canonical_code.to_string_lossy());
        assert_eq!(args[2], "--out");
        let output = Path::new(args[3]);
        assert!(output.starts_with(graph.join(".staging")));
        assert_eq!(
            output.file_name().and_then(|name| name.to_str()),
            Some("mir-overlay.json")
        );
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_atlas_build_without_scip_removes_previous_overlay() {
        let (code, graph) = copy_fixture("atlas-scip-remove-overlay");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            all_edges(&shards)
                .iter()
                .any(|edge| edge.provenance == Provenance::Scip)
        );

        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert!(report.rebuilt.is_empty());
        assert!(!report.capability.scip);
        let (meta, shards) = load_graph(&graph).unwrap();
        assert!(!meta.capability.scip);
        assert!(
            all_edges(&shards)
                .iter()
                .all(|edge| edge.provenance != Provenance::Scip)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    // ── Phase 2: SCIP semantic overlay ──────────────────────────────

    /// Serialize a minimal protobuf SCIP index to `path`. Occurrences are
    /// `(symbol, roles, line0)`; symbols are `(symbol, kind)`.
    fn write_synthetic_scip(
        path: &Path,
        rel: &str,
        occ: &[(&str, u32, u32)],
        syms: &[(&str, scip::types::symbol_information::Kind)],
    ) {
        use protobuf::Message;
        let mut doc = scip::types::Document::new();
        doc.relative_path = rel.to_string();
        for (sym, roles, line0) in occ {
            let mut o = scip::types::Occurrence::new();
            o.symbol = (*sym).to_string();
            o.symbol_roles = *roles as i32;
            o.range = vec![*line0 as i32, 0, 5];
            doc.occurrences.push(o);
        }
        for (sym, kind) in syms {
            let mut si = scip::types::SymbolInformation::new();
            si.symbol = (*sym).to_string();
            si.kind = (*kind).into();
            doc.symbols.push(si);
        }
        let mut index = scip::types::Index::new();
        index.documents.push(doc);
        fs::write(path, index.write_to_bytes().unwrap()).unwrap();
    }

    #[test]
    fn test_overlay_reads_protobuf_index() {
        let (code, graph) = copy_fixture("atlas-scip-pb");
        let report = build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_protobuf_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        assert!(report.capability.scip);
        let (meta, shards) = load_graph(&graph).unwrap();
        assert!(meta.capability.scip);
        assert!(
            all_edges(&shards)
                .iter()
                .any(|e| e.provenance == Provenance::Scip
                    && e.resolution == EdgeResolution::Resolved),
            "expected a resolved scip edge from the protobuf index"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_overlay_still_reads_json_index() {
        let (code, graph) = copy_fixture("atlas-scip-json");
        let report = build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        assert!(report.capability.scip);
        let (_, shards) = load_graph(&graph).unwrap();
        // JSON carries occurrences only → References edges, as before the upgrade.
        assert!(
            all_edges(&shards)
                .iter()
                .any(|e| e.provenance == Provenance::Scip && e.kind == EdgeKind::References),
            "json overlay should still produce reference edges"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_json_references_preserve_full_occurrence_range() {
        let (code, graph) = copy_fixture("atlas-scip-json-occurrence-site");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| {
                edge.provenance == Provenance::Scip
                    && edge.kind == EdgeKind::References
                    && edge.target_text.as_deref()
                        == Some("rust-analyzer cargo atlas-basic 0.1.0 store/MemStore#")
            })
            .unwrap();
        assert_scip_occurrence_metadata(
            &edge,
            EdgeSite {
                file: "src/service.rs".to_string(),
                line_start: 3,
                column_start: 20,
                line_end: 3,
                column_end: 28,
            },
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_loaders_normalize_occurrence_sites() {
        let json = load_scip(&scip_fixture()).unwrap();
        let json_occurrence = json
            .documents
            .iter()
            .find(|document| document.relative_path == "src/service.rs")
            .unwrap()
            .occurrences
            .iter()
            .find(|occurrence| {
                !occurrence.is_definition && occurrence.symbol.ends_with("store/MemStore#")
            })
            .unwrap();
        assert_eq!(
            json_occurrence.site,
            EdgeSite {
                file: "src/service.rs".to_string(),
                line_start: 3,
                column_start: 20,
                line_end: 3,
                column_end: 28,
            }
        );

        let protobuf = load_scip(&scip_protobuf_fixture()).unwrap();
        let protobuf_occurrence = protobuf
            .documents
            .iter()
            .find(|document| document.relative_path == "src/service.rs")
            .unwrap()
            .occurrences
            .iter()
            .find(|occurrence| occurrence.symbol.ends_with("impl#[MemStore][Store]get()."))
            .unwrap();
        assert_eq!(
            protobuf_occurrence.site,
            EdgeSite {
                file: "src/service.rs".to_string(),
                line_start: 4,
                column_start: 11,
                line_end: 4,
                column_end: 14,
            }
        );
    }

    #[test]
    fn test_scip_calls_preserve_occurrence_site_and_evidence() {
        let (code, graph) = copy_fixture("atlas-scip-call-occurrence-site");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_protobuf_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| edge.provenance == Provenance::Scip && edge.kind == EdgeKind::Calls)
            .unwrap();
        assert_scip_occurrence_metadata(
            &edge,
            EdgeSite {
                file: "src/service.rs".to_string(),
                line_start: 4,
                column_start: 11,
                line_end: 4,
                column_end: 14,
            },
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_multiple_definition_candidates_are_not_exact() {
        let (code, graph) = scratch_crate(
            "atlas_scip_multiple_candidates",
            &[
                (
                    "src/lib.rs",
                    "pub mod first;\npub mod second;\npub fn caller() { first::shared(); }\n",
                ),
                ("src/first.rs", "pub fn shared() {}\n"),
                ("src/second.rs", "pub fn shared() {}\n"),
            ],
        );
        let index_path = graph.join("multiple-candidates.json");
        fs::create_dir_all(&graph).unwrap();
        fs::write(
            &index_path,
            r#"{
  "documents": [
    {"relative_path":"src/first.rs","occurrences":[{"symbol":"test shared#","symbol_roles":1,"range":[0,7,0,13]}]},
    {"relative_path":"src/second.rs","occurrences":[{"symbol":"test shared#","symbol_roles":1,"range":[0,7,0,13]}]},
    {"relative_path":"src/lib.rs","occurrences":[{"symbol":"test shared#","symbol_roles":0,"range":[2,24,2,30]}]}
  ]
}"#,
        )
        .unwrap();
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(index_path),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| {
                edge.provenance == Provenance::Scip
                    && edge.target_text.as_deref() == Some("test shared#")
            })
            .unwrap();
        assert_eq!(edge.resolution, EdgeResolution::Unresolved);
        assert_eq!(edge.confidence, Some(EdgeConfidence::BoundedCandidates));
        assert_eq!(edge.candidates.len(), 2);
        assert!(edge.evidence.as_deref().is_some_and(|value| {
            value.contains("occurrence") && value.contains("multiple candidates")
        }));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_preserves_distinct_occurrences_for_same_edge() {
        let (code, graph) = scratch_crate(
            "atlas_scip_distinct_occurrences",
            &[(
                "src/lib.rs",
                "pub fn target() {}\npub fn caller() {\n    target();\n    target();\n}\n",
            )],
        );
        let index_path = graph.join("distinct-occurrences.json");
        fs::create_dir_all(&graph).unwrap();
        fs::write(
            &index_path,
            r#"{
  "documents": [{
    "relative_path": "src/lib.rs",
    "occurrences": [
      {"symbol":"test target().","symbol_roles":1,"range":[0,7,0,13]},
      {"symbol":"test target().","symbol_roles":0,"range":[2,8,3,2]},
      {"symbol":"test target().","symbol_roles":0,"range":[3,4,3,10]}
    ]
  }]
}"#,
        )
        .unwrap();
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(index_path),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let mut edges: Vec<_> = all_edges(&shards)
            .into_iter()
            .filter(|edge| {
                edge.provenance == Provenance::Scip
                    && edge.kind == EdgeKind::References
                    && edge.target_text.as_deref() == Some("test target().")
            })
            .collect();
        edges.sort_by(|left, right| left.site.cmp(&right.site));
        assert_eq!(edges.len(), 2);
        assert_scip_occurrence_metadata(
            &edges[0],
            EdgeSite {
                file: "src/lib.rs".to_string(),
                line_start: 3,
                column_start: 9,
                line_end: 4,
                column_end: 3,
            },
        );
        assert_scip_occurrence_metadata(
            &edges[1],
            EdgeSite {
                file: "src/lib.rs".to_string(),
                line_start: 4,
                column_start: 5,
                line_end: 4,
                column_end: 11,
            },
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_emits_calls_for_method_target() {
        let (code, graph) = copy_fixture("atlas-scip-calls");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_protobuf_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        // `service::run` invokes `MemStore::get()` (an impl Method) → Calls.
        assert!(
            all_edges(&shards)
                .iter()
                .any(|e| e.kind == EdgeKind::Calls && e.provenance == Provenance::Scip),
            "expected a scip Calls edge for the method invocation"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_emits_usestype_for_type_target() {
        let (code, graph) = copy_fixture("atlas-scip-uses");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_protobuf_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let mem = node(&shards, "atlas_basic::store::MemStore").unwrap();
        let run = node(&shards, "atlas_basic::service::run").unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| {
                edge.kind == EdgeKind::UsesType
                    && edge.provenance == Provenance::Scip
                    && edge.from == run.id
                    && edge.to == mem.id
            })
            .unwrap();
        assert_scip_occurrence_metadata(
            &edge,
            EdgeSite {
                file: "src/service.rs".to_string(),
                line_start: 3,
                column_start: 20,
                line_end: 3,
                column_end: 28,
            },
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_resolves_impls_trait_edge() {
        let (code, graph) = copy_fixture("atlas-scip-impls");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_protobuf_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let store = node(&shards, "atlas_basic::store::Store").unwrap();
        let impl_node = shards
            .iter()
            .flat_map(|s| s.nodes.iter())
            .find(|n| n.kind == NodeKind::Impl)
            .unwrap();
        let edge = all_edges(&shards)
            .into_iter()
            .find(|edge| {
                edge.kind == EdgeKind::ImplsTrait
                    && edge.provenance == Provenance::Scip
                    && edge.resolution == EdgeResolution::Resolved
                    && edge.from == impl_node.id
                    && edge.to == store.id
            })
            .unwrap();
        assert_scip_occurrence_metadata(
            &edge,
            EdgeSite {
                file: "src/store.rs".to_string(),
                line_start: 7,
                column_start: 6,
                line_end: 7,
                column_end: 11,
            },
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_external_trait_marked_external() {
        use scip::types::symbol_information::Kind;
        let (code, graph) = scratch_crate(
            "atlas_scip_ext",
            &[(
                "src/lib.rs",
                "pub struct Widget;\nimpl external::Show for Widget {\n    fn show(&self) {}\n}\n",
            )],
        );
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let impl_line = shards
            .iter()
            .flat_map(|s| s.nodes.iter())
            .find(|n| n.kind == NodeKind::Impl)
            .unwrap()
            .line_start;
        // impl header is on source line 2 (1-based) → 0-based occurrence line.
        let occ_line = (impl_line - 1) as u32;
        let index_path = graph.join("synthetic.scip");
        write_synthetic_scip(
            &index_path,
            "src/lib.rs",
            &[
                ("test Widget#", 1, 0),
                ("ext Show#", 0, occ_line),
                ("test Widget#", 0, occ_line),
            ],
            &[("ext Show#", Kind::Trait), ("test Widget#", Kind::Struct)],
        );
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(index_path),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (_, shards) = load_graph(&graph).unwrap();
        let edges = all_edges(&shards);
        assert!(
            edges.iter().any(|e| e.kind == EdgeKind::ImplsTrait
                && e.provenance == Provenance::Scip
                && e.resolution == EdgeResolution::External
                && e.target_text.as_deref() == Some("ext Show#")),
            "external trait should yield an External ImplsTrait edge: {edges:?}"
        );
        let widget = node(&shards, "atlas_scip_ext::Widget").unwrap();
        assert!(
            edges.iter().any(|e| e.kind == EdgeKind::ImplFor
                && e.provenance == Provenance::Scip
                && e.resolution == EdgeResolution::Resolved
                && e.to == widget.id),
            "local type should yield a Resolved ImplFor edge"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_gen_missing_binary_warns() {
        let (code, graph) = copy_fixture("atlas-scip-gen-missing");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let err = generate_scip(
            &code,
            &graph.join("index.scip"),
            "/nonexistent/rust-analyzer-xyz",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("rust-analyzer"),
            "error should mention rust-analyzer: {err}"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_build_without_scip_stays_syn_only() {
        let (code, graph) = copy_fixture("atlas-scip-syn-only");
        let report = build(&code, &graph, &BuildOptions::default()).unwrap();
        assert!(!report.capability.scip);
        let (_, shards) = load_graph(&graph).unwrap();
        assert!(
            all_edges(&shards)
                .iter()
                .all(|e| e.provenance == Provenance::Syn)
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_survives_incremental_refresh() {
        let (code, graph) = copy_fixture("atlas-scip-refresh");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_protobuf_fixture()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let (meta, _) = load_graph(&graph).unwrap();
        assert!(meta.capability.scip);
        assert!(meta.capability.scip_index.is_some());

        // Edit a source file (append only → no line shift for the indexed impl).
        let svc = code.join("src/service.rs");
        let mut src = fs::read_to_string(&svc).unwrap();
        src.push_str("\n// touch\n");
        fs::write(&svc, src).unwrap();

        // A non-frozen query triggers the incremental refresh path.
        impls(&code, &graph, "Store", &QueryOptions { frozen: false }).unwrap();

        let (meta, shards) = load_graph(&graph).unwrap();
        assert!(
            meta.capability.scip,
            "scip capability should survive refresh"
        );
        assert!(
            all_edges(&shards)
                .iter()
                .any(|e| e.provenance == Provenance::Scip),
            "scip edges should survive an incremental refresh"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_scip_missing_index_falls_back_to_syn() {
        let (code, graph) = copy_fixture("atlas-scip-vanish");
        // Overlay from a deletable copy of the index.
        let idx = graph.parent().unwrap().join("index.scip");
        fs::copy(scip_protobuf_fixture(), &idx).unwrap();
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(idx.clone()),
                dynamic_dispatch: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        assert!(load_graph(&graph).unwrap().0.capability.scip);

        // The recorded index disappears.
        fs::remove_file(&idx).unwrap();
        let svc = code.join("src/service.rs");
        let mut src = fs::read_to_string(&svc).unwrap();
        src.push_str("\n// touch\n");
        fs::write(&svc, src).unwrap();
        impls(&code, &graph, "Store", &QueryOptions { frozen: false }).unwrap();

        let (meta, shards) = load_graph(&graph).unwrap();
        assert!(
            !meta.capability.scip,
            "capability should clear when the index vanishes"
        );
        assert!(
            all_edges(&shards)
                .iter()
                .all(|e| e.provenance != Provenance::Scip),
            "scip edges should be purged when the index vanishes"
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }
}
