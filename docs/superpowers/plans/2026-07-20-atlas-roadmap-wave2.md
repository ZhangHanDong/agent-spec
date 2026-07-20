# Rust Atlas Roadmap Wave 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver deterministic, bounded `atlas explore`, flow, impact, and affected queries on the schema-v6 Atlas index, plus the offline E1 measurements needed to evaluate their Agent value without changing the default MCP surface.

**Architecture:** Keep JSON shards canonical and load the existing validated `QueryIndex` through one shared traversal layer. Flow and impact expose typed graph paths; affected maps safe path inputs into impact seeds; explore composes those primitives with per-file hash-checked excerpts and deterministic pruning. CLI and opt-in MCP remain thin wrappers over the library, while evaluation receipts add query-payload and follow-up metrics.

**Tech Stack:** Rust 2024, serde/serde_json, blake3, clap, the existing `rust-atlas` crate and agent-spec lifecycle/KLL tooling. No new runtime dependency is required.

## Global Constraints

- Graph schema MUST remain v6; this wave adds no persisted graph format and no database.
- JSON shards remain canonical; `.agent-spec/graph/query-index.json` remains derived and atomically rebuildable.
- Atlas query code MUST NOT invoke an LLM, access the network, or evaluate shell strings.
- Every result MUST use shared `AtlasStatus`, reject worktree mismatch, and preserve `stale == status.syn.stale_files`.
- Frozen stale facts MAY be returned with status, but a source excerpt MUST require a matching per-file blake3 hash.
- Traversal MUST be deterministic, cycle-safe, bounded by depth and expansion limits, and honest about truncation.
- An edge hop MUST preserve site, extractor, dispatch, confidence, candidates, evidence, provenance, and resolution.
- Affected output MUST NOT infer test selectors or coverage from filenames; C1 owns test-obligation linking.
- `atlas_explore` MUST remain absent from default MCP discovery until real opt-in E1 A/B evidence approves a change.
- `compact` limits are 8 seeds, 32 nodes, 48 edges, 8 paths, 4 excerpts, 20 lines per excerpt, and 16,000 serialized JSON bytes.
- `deep` limits are 16 seeds, 96 nodes, 160 edges, 20 paths, 12 excerpts, 40 lines per excerpt, and 24,000 serialized JSON bytes.
- Flow defaults are max depth 8, max expansions 2,000, and max paths 8; impact depth is 1..=8, default 3, with default max nodes 200.
- No task may weaken an existing Wave 1 lifecycle selector, query-index error precedence, schema gate, or authority gate.

---

### Task 1: Accept The Wave 2 Requirement And Contract

**Files:**
- Create: `knowledge/requirements/req-atlas-explore-flow-impact.md`
- Create: `specs/task-atlas-explore-flow-impact.spec.md`
- Modify: `docs/atlas-roadmap.md`
- Create: `docs/superpowers/plans/2026-07-20-atlas-roadmap-wave2.md`

**Interfaces:**
- Consumes: delivered `REQ-ATLAS-AGENT-EVALUATION`, `REQ-ATLAS-EDGE-EVIDENCE-INDEX`, and `REQ-ATLAS-WORKTREE-FRESHNESS`.
- Produces: `WU-REQ-ATLAS-EXPLORE-FLOW-IMPACT`, batch 4, and one active Task Contract with twenty-two bound scenarios.

- [ ] **Step 1: Build the current workspace CLI**

```bash
cargo build --workspace
```

Expected: the baseline workspace builds and provides `target/debug/agent-spec` in a fresh worktree.

- [ ] **Step 2: Verify the Task Contract quality gate**

```bash
target/debug/agent-spec lint specs/task-atlas-explore-flow-impact.spec.md --min-score 0.7
```

Expected: quality score 1.0, no error-level lint issue, and every scenario has a concrete selector.

- [ ] **Step 3: Verify KLL and graph gates**

```bash
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected: all commands exit 0; the plan contains one ready work unit in batch 4 and coverage points only to `specs/task-atlas-explore-flow-impact.spec.md`.

- [ ] **Step 4: Prove the roadmap does not claim delivery early**

```bash
rg -n "Wave 2 accepted contract，尚未交付|真实 Agent A/B 仍为 opt-in" docs/atlas-roadmap.md
```

Expected: B2/B3/B4 remain “尚未交付”, and E1 explicitly lacks a real A/B conclusion.

- [ ] **Step 5: Commit the governance baseline**

```bash
git add knowledge/requirements/req-atlas-explore-flow-impact.md specs/task-atlas-explore-flow-impact.spec.md docs/atlas-roadmap.md docs/superpowers/plans/2026-07-20-atlas-roadmap-wave2.md
git commit -m "docs(atlas): accept explore flow impact wave"
```

### Task 2: Introduce Shared Deterministic Traversal Contracts

**Files:**
- Create: `crates/rust-atlas/src/traversal.rs`
- Modify: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Consumes: `QueryIndex::{matching_nodes,incoming_edges,outgoing_edges}`, `Edge`, `Node`, `AtlasStatus`.
- Produces: public `GraphPath`, `PathHop`, `PathConfidence`, `FlowState`, `TraversalLimits`; crate-private `EndpointResolution`, `PathEnumeration`, `resolve_endpoint`, `enumerate_paths`, `confidence_cost`.

- [ ] **Step 1: Write the failing shared-contract test**

Add this test to `traversal.rs`:

```rust
#[test]
fn test_atlas_query_surfaces_share_traversal_contract() {
    let hop = PathHop {
        edge: edge("a", "b", EdgeConfidence::Exact),
        chosen_target: "b".into(),
        candidate: false,
    };
    let path = GraphPath {
        nodes: vec![node("a"), node("b")],
        hops: vec![hop],
        confidence: PathConfidence::Exact,
    };
    let value = serde_json::to_value(path).unwrap();
    assert_eq!(value["hops"][0]["chosen_target"], "b");
    assert_eq!(value["confidence"], "exact");
    assert_eq!(TraversalLimits::flow_default().max_expansions, 2_000);
}
```

- [ ] **Step 2: Run the test and observe the missing types**

```bash
cargo test -p rust-atlas test_atlas_query_surfaces_share_traversal_contract -- --nocapture
```

Expected: compilation fails because `PathHop`, `GraphPath`, `PathConfidence`, and `TraversalLimits` do not exist.

- [ ] **Step 3: Add the typed traversal surface**

Implement these exact public shapes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathConfidence { Exact, BoundedCandidates, Heuristic }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathHop {
    pub edge: Edge,
    pub chosen_target: String,
    pub candidate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphPath {
    pub nodes: Vec<Node>,
    pub hops: Vec<PathHop>,
    pub confidence: PathConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlowState {
    Found,
    NoPath,
    CapabilityUnavailable,
    AmbiguousEndpoint,
    UnknownEndpoint,
    Truncated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraversalLimits {
    pub max_depth: usize,
    pub max_expansions: usize,
    pub max_paths: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EndpointResolution {
    Found(Node),
    Unknown,
    Ambiguous(Vec<Node>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathEnumeration {
    pub paths: Vec<GraphPath>,
    pub highest_confidence: Option<GraphPath>,
    pub expansions: usize,
    pub truncated: bool,
}
```

`TraversalLimits::flow_default()` returns `{8, 2_000, 8}`. Add this error for zero limits and depth above 32, then re-export the public types from `lib.rs`:

```rust
#[error("atlas-traversal-limit: {detail}")]
TraversalLimit { detail: String },
```

`resolve_endpoint(index, value)` checks exact id/symbol before sorted suffix matches. `enumerate_paths(index, start, end, limits)` returns `Result<PathEnumeration, AtlasError>`. Sort complete paths by `(hop count, total confidence cost, canonical node/hop signature)`. `paths` is bounded by `max_paths`, while `highest_confidence` retains the global best discovered path separately so the mandatory shortest and highest-confidence fields remain truthful even when `max_paths == 1`. `confidence_cost` maps exact and implicit resolved edges to 0, chosen bounded candidates or `BoundedCandidates` to 10, and `Heuristic` plus otherwise-unresolved edges to 100. A path's `PathConfidence` is its worst hop class.

- [ ] **Step 4: Add deterministic path-enumeration tests**

Cover candidate expansion without mutating the edge:

```rust
#[test]
fn candidate_neighbors_keep_original_edge_and_sorted_targets() {
    let mut candidate = edge("a", "unresolved", EdgeConfidence::BoundedCandidates);
    candidate.resolution = EdgeResolution::Unresolved;
    candidate.candidates = vec!["c".into(), "b".into()];
    let graph = index(&[node("a"), node("b"), node("c")], &[candidate]);
    let paths = enumerate_paths(&graph, "a", Some("c"), TraversalLimits::flow_default()).unwrap();
    assert_eq!(paths.paths[0].hops[0].chosen_target, "c");
    assert!(paths.paths[0].hops[0].candidate);
    assert_eq!(paths.paths[0].hops[0].edge.resolution, EdgeResolution::Unresolved);
}
```

Also test cycle termination, canonical tie ordering, and exact/bounded/heuristic costs `0/10/100`.

- [ ] **Step 5: Run traversal and crate tests**

```bash
cargo test -p rust-atlas traversal -- --nocapture
cargo test -p rust-atlas
```

Expected: all traversal tests and the full crate suite pass.

- [ ] **Step 6: Commit shared traversal**

```bash
git add crates/rust-atlas/src/traversal.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): add bounded traversal contracts"
```

### Task 3: Implement From-To And Through Flow

**Files:**
- Create: `crates/rust-atlas/src/flow.rs`
- Modify: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Consumes: `indexed_query_state`, `enumerate_paths`, `TraversalLimits`, `QueryOptions`.
- Produces: `FlowQuery`, `FlowOptions`, `FlowEndpoint`, `FlowDiagnostic`, `FlowResult`, `flow(code_root, graph_dir, query, options)`.

- [ ] **Step 1: Write failing flow-selection tests**

```rust
#[test]
fn test_atlas_flow_returns_shortest_and_highest_confidence_paths() {
    let fixture = flow_fixture_with_two_hop_bounded_and_three_hop_exact_paths();
    let result = flow_index(&fixture.index, FlowQuery::Between {
        from: "a".into(), to: "z".into()
    }, FlowOptions::default(), &fixture.status);
    assert_eq!(result.state, FlowState::Found);
    assert_eq!(result.shortest.as_ref().unwrap().hops.len(), 2);
    assert_eq!(result.highest_confidence.as_ref().unwrap().hops.len(), 3);
    assert_eq!(result.highest_confidence.as_ref().unwrap().confidence, PathConfidence::Exact);
}
```

```rust
#[test]
fn test_atlas_flow_preserves_bounded_candidate_alternatives() {
    let result = through_candidate_fixture();
    let targets = result.alternatives.iter()
        .map(|path| path.hops.last().unwrap().chosen_target.as_str())
        .collect::<Vec<_>>();
    assert_eq!(targets, vec!["crate::B", "crate::C"]);
    for path in &result.alternatives {
        assert_eq!(path.nodes[0].id, "crate::Caller");
        assert_eq!(path.nodes[1].id, "crate::Through");
        assert_eq!(path.hops.len(), 2);
        assert_eq!(path.hops[0].chosen_target, "crate::Through");
        assert!(path.hops[1].candidate);
        assert_eq!(path.hops[1].edge.resolution, EdgeResolution::Unresolved);
    }
}
```

- [ ] **Step 2: Verify the flow tests fail**

```bash
cargo test -p rust-atlas test_atlas_flow_ -- --nocapture
```

Expected: compilation fails because `flow.rs` and its public API are absent.

- [ ] **Step 3: Implement exact flow result types**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowQuery {
    Between { from: String, to: String },
    Through { symbol: String },
}

#[derive(Debug, Clone)]
pub struct FlowOptions {
    pub limits: TraversalLimits,
    pub frozen: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowEndpoint {
    pub query: String,
    pub selected: Option<Node>,
    pub candidates: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowResult {
    pub schema: String,
    pub state: FlowState,
    pub endpoints: Vec<FlowEndpoint>,
    pub shortest: Option<GraphPath>,
    pub highest_confidence: Option<GraphPath>,
    pub alternatives: Vec<GraphPath>,
    pub expansions: usize,
    pub truncated: bool,
    pub diagnostics: Vec<FlowDiagnostic>,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}
```

`FlowOptions::default()` uses `TraversalLimits::flow_default()` with `frozen: false`. Use schema `agent-spec/rust-atlas/flow-v1`. Resolve exact id/symbol first and suffix candidates second. More than one candidate produces `AmbiguousEndpoint`; no candidate produces `UnknownEndpoint`.

- [ ] **Step 4: Implement endpoint and outcome precedence**

Resolve exact node id or symbol first, then sorted suffix candidates. Return `UnknownEndpoint` for zero candidates and `AmbiguousEndpoint` for multiple candidates before traversal. For between-flow: return `Found` whenever a complete path exists, including a syn-only path while SCIP is unavailable; otherwise return `Truncated` if expansion ended early; otherwise return `CapabilityUnavailable` when SCIP is unavailable and a semantic path cannot be disproved; otherwise return `NoPath`. MIR availability does not affect this Wave 2 verdict. Through-flow joins each sorted incoming resolved/candidate hop to each sorted outgoing hop, removes cyclic duplicates, then truncates to `max_paths`.

- [ ] **Step 5: Add the negative outcome test**

```rust
#[test]
fn test_atlas_flow_distinguishes_no_path_unavailable_and_truncated() {
    assert_eq!(complete_disconnected_flow().state, FlowState::NoPath);
    assert_eq!(syn_only_unprovable_flow().state, FlowState::CapabilityUnavailable);
    let limited = expansion_limited_flow();
    assert_eq!(limited.state, FlowState::Truncated);
    assert!(limited.truncated);
}
```

Also add the endpoint/capability contract:

```rust
#[test]
fn test_atlas_flow_handles_ambiguous_endpoints_and_syn_paths_without_scip() {
    assert_eq!(unknown_endpoint_flow().state, FlowState::UnknownEndpoint);
    let ambiguous = ambiguous_suffix_flow();
    assert_eq!(ambiguous.state, FlowState::AmbiguousEndpoint);
    assert_eq!(candidate_ids(&ambiguous), vec!["crate::a::run", "crate::b::run"]);

    let found = syn_path_without_scip_flow();
    assert_eq!(found.state, FlowState::Found);
    assert!(found.shortest.is_some());
}
```

- [ ] **Step 6: Run and commit flow**

```bash
cargo test -p rust-atlas test_atlas_flow_ -- --nocapture
cargo test -p rust-atlas
git add crates/rust-atlas/src/flow.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): add explainable flow queries"
```

### Task 4: Implement Explainable Reverse Impact

**Files:**
- Create: `crates/rust-atlas/src/impact.rs`
- Modify: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Consumes: shared `PathHop`, `GraphPath`, `QueryIndex`, `indexed_query_state`.
- Produces: `ImpactOptions`, `ImpactEntry`, `ImpactDiagnostic`, `ImpactResult`, `impact` and crate-private `impact_index`.

- [ ] **Step 1: Write failing distance and containment tests**

```rust
#[test]
fn test_atlas_impact_returns_distance_and_explanation_paths() {
    let result = impact_fixture_with_converging_dependents();
    let shared = result.affected.iter().find(|entry| entry.node.id == "shared").unwrap();
    assert_eq!(shared.distance, 2);
    assert_eq!(shared.path.nodes.first().unwrap().id, result.seed.id);
    assert_eq!(shared.path.nodes.last().unwrap().id, "shared");
    assert_eq!(result.affected.iter().filter(|entry| entry.node.id == "shared").count(), 1);
}

#[test]
fn test_atlas_impact_container_expansion_avoids_sibling_explosion() {
    assert!(!leaf_impact().affected.iter().any(|entry| entry.node.id == "sibling"));
    assert_eq!(container_impact().affected.iter()
        .find(|entry| entry.node.id == "member").unwrap().distance, 0);
}
```

- [ ] **Step 2: Run the tests to verify the missing module**

```bash
cargo test -p rust-atlas test_atlas_impact_ -- --nocapture
```

Expected: compilation fails for missing impact API.

- [ ] **Step 3: Implement impact types and traversal**

```rust
#[derive(Debug, Clone)]
pub struct ImpactOptions {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub frozen: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactEntry {
    pub node: Node,
    pub distance: usize,
    pub path: GraphPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImpactDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactResult {
    pub schema: String,
    pub seed: Node,
    pub affected: Vec<ImpactEntry>,
    pub truncated: bool,
    pub diagnostics: Vec<ImpactDiagnostic>,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}
```

`ImpactOptions::default()` is `{ max_depth: 3, max_nodes: 200, frozen: false }`. Use schema `agent-spec/rust-atlas/impact-v1`. Reject depth outside 1..=8 and zero max nodes with `AtlasError::TraversalLimit { detail }`. Traverse incoming `Calls`, `References`, `UsesType`, `ImplsTrait`, and `ImplFor`. For a container seed/current node, traverse outgoing `Contains` at the same distance. Never traverse incoming `Contains` from a leaf. Keep the minimum-distance path; tie-break by serialized hop signature.

- [ ] **Step 4: Add truncation and deterministic-order tests**

Assert `max_nodes` never overshoots, `truncated` is true when a queued dependent remains, all `affected` entries sort by `(distance, node.id)`, and repeated JSON is byte-identical.

- [ ] **Step 5: Run and commit impact**

```bash
cargo test -p rust-atlas test_atlas_impact_ -- --nocapture
cargo test -p rust-atlas
git add crates/rust-atlas/src/impact.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): add explainable impact traversal"
```

### Task 5: Map Safe File Inputs To Affected Code

**Files:**
- Create: `crates/rust-atlas/src/affected.rs`
- Modify: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Consumes: `QueryIndex.file`, `impact_index`, `ImpactOptions`, canonical code root.
- Produces: `AffectedOptions`, `AffectedSeed`, `AffectedResult`, `affected_paths`, crate-private `normalize_affected_path`.

- [ ] **Step 1: Write failing path-normalization test**

```rust
#[test]
fn test_atlas_affected_normalizes_repo_relative_dot_and_absolute_paths() {
    let fixture = affected_fixture();
    let relative = affected_paths(&fixture.code, &fixture.graph, &["src/lib.rs".into()], &opts()).unwrap();
    let dotted = affected_paths(&fixture.code, &fixture.graph, &["./src/lib.rs".into()], &opts()).unwrap();
    let absolute = affected_paths(&fixture.code, &fixture.graph, &[fixture.code.join("src/lib.rs")], &opts()).unwrap();
    assert_eq!(serde_json::to_vec(&relative).unwrap(), serde_json::to_vec(&dotted).unwrap());
    assert_eq!(serde_json::to_vec(&relative).unwrap(), serde_json::to_vec(&absolute).unwrap());
    assert!(affected_paths(&fixture.code, &fixture.graph, &["../escape.rs".into()], &opts())
        .unwrap_err().to_string().contains("atlas-affected-path"));
    assert!(affected_paths(&fixture.code, &fixture.graph, &[fixture.out_of_root.clone()], &opts())
        .unwrap_err().to_string().contains("atlas-affected-path"));
    assert!(affected_paths(&fixture.code, &fixture.graph, &[fixture.escaping_symlink.clone()], &opts())
        .unwrap_err().to_string().contains("atlas-affected-path"));
}
```

- [ ] **Step 2: Verify the test fails**

```bash
cargo test -p rust-atlas test_atlas_affected_normalizes_repo_relative_dot_and_absolute_paths -- --nocapture
```

Expected: compilation fails because `affected_paths` is undefined.

- [ ] **Step 3: Implement safe normalization and result types**

Normalize components lexically so deleted changed files remain representable. Reject `ParentDir`, absolute paths outside canonical root, and any existing symlink whose canonical target escapes root. Convert accepted separators to `/` before index lookup.

Add this exact library error:

```rust
#[error("atlas-affected-path: `{path}`: {detail}")]
AffectedPath { path: String, detail: String },
```

```rust
#[derive(Debug, Clone, Default)]
pub struct AffectedOptions { pub impact: ImpactOptions }

#[derive(Debug, Clone, Serialize)]
pub struct AffectedSeed { pub file: String, pub nodes: Vec<Node> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffectedDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedResult {
    pub schema: String,
    pub files: Vec<String>,
    pub seeds: Vec<AffectedSeed>,
    pub affected: Vec<ImpactEntry>,
    pub truncated: bool,
    pub diagnostics: Vec<AffectedDiagnostic>,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}
```

Use schema `agent-spec/rust-atlas/affected-v1`; merge duplicate impact entries by minimum `(distance, path signature)`.

- [ ] **Step 4: Prove no test-name inference exists**

```rust
#[test]
fn test_atlas_affected_does_not_infer_tests_from_filenames() {
    let value = serde_json::to_value(affected_fixture_with_test_named_file()).unwrap();
    assert!(value.to_string().contains("tests/feature_test.rs"));
    assert!(value.get("test_selectors").is_none());
    assert!(value.get("coverage").is_none());
    assert!(!value.to_string().contains("inferred_test"));
}
```

- [ ] **Step 5: Run and commit affected library support**

```bash
cargo test -p rust-atlas test_atlas_affected_ -- --nocapture
cargo test -p rust-atlas
git add crates/rust-atlas/src/affected.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): map changed files to affected code"
```

### Task 6: Compose Explore Seeds And Hash-Checked Excerpts

**Files:**
- Create: `crates/rust-atlas/src/explore.rs`
- Modify: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Consumes: `QueryIndex::search_nodes`, flow/impact internals, `Meta.files`, `AtlasStatus`.
- Produces: `ExploreProfile`, `ExploreOptions`, `ExploreBudget`, `BudgetUsage`, `ExploreNode`, `SourceExcerpt`, `ExploreDiagnostic`, `ExploreResult`, `explore`.

- [ ] **Step 1: Write the failing stale-excerpt test**

```rust
#[test]
fn test_atlas_explore_omits_excerpt_when_selected_source_hash_is_stale() {
    let fixture = built_explore_fixture();
    std::fs::write(fixture.code.join("src/lib.rs"), "pub fn changed() {}\n").unwrap();
    let result = explore(&fixture.code, &fixture.graph, "entry", &ExploreOptions {
        profile: ExploreProfile::Compact,
        frozen: true,
    }).unwrap();
    assert!(result.nodes.iter().any(|node| node.node.file == "src/lib.rs"));
    assert!(!result.excerpts.iter().any(|excerpt| excerpt.file == "src/lib.rs"));
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "atlas-excerpt-stale-source" && diagnostic.file.as_deref() == Some("src/lib.rs")
    }));
}
```

- [ ] **Step 2: Run and verify the missing explore API**

```bash
cargo test -p rust-atlas test_atlas_explore_omits_excerpt_when_selected_source_hash_is_stale -- --nocapture
```

Expected: compilation fails for missing `explore` and profile types.

- [ ] **Step 3: Implement deterministic query tokenization and seeding**

Tokenize only ASCII alphanumeric, `_`, `:`, `.`, `/`, and `-`; trim punctuation, deduplicate while preserving first occurrence, and cap terms at 32. Resolve exact repository paths through `QueryIndex.file`; rank identifier hits by existing `MatchKind`, then symbol/file/line/id. Stop at the profile seed limit and emit `atlas-explore-no-match` when no path or symbol seed exists.

- [ ] **Step 4: Implement per-file verified excerpts**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExploreProfile { Compact, Deep }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreOptions {
    pub profile: ExploreProfile,
    pub frozen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExploreBudget {
    pub max_seeds: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_paths: usize,
    pub max_excerpts: usize,
    pub max_excerpt_lines: usize,
    pub max_serialized_bytes: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BudgetUsage {
    pub seeds: usize,
    pub nodes: usize,
    pub edges: usize,
    pub paths: usize,
    pub excerpts: usize,
    pub serialized_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExploreNode {
    pub node: Node,
    pub seed: bool,
    pub spine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceExcerpt {
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub text: String,
    pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExploreDiagnostic {
    pub code: String,
    pub message: String,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExploreResult {
    pub schema: String,
    pub query: String,
    pub profile: ExploreProfile,
    pub limits: ExploreBudget,
    pub usage: BudgetUsage,
    pub seeds: Vec<Node>,
    pub nodes: Vec<ExploreNode>,
    pub edges: Vec<Edge>,
    pub primary_paths: Vec<GraphPath>,
    pub alternative_paths: Vec<GraphPath>,
    pub impact: Vec<ImpactEntry>,
    pub excerpts: Vec<SourceExcerpt>,
    pub truncated: bool,
    pub truncation_reasons: Vec<String>,
    pub diagnostics: Vec<ExploreDiagnostic>,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}
```

For each selected node, normalize its repository-relative path with the same root/symlink checks as affected, read bytes, compare blake3 with `Meta.files[file]`, decode UTF-8, and take a line-bounded window around `node.line_start..=node.line_end`. Emit `atlas-excerpt-stale-source`, `atlas-excerpt-missing-source`, `atlas-excerpt-unsafe-source`, or `atlas-excerpt-invalid-utf8` as appropriate; every such diagnostic omits the excerpt. Add `test_atlas_explore_rejects_missing_and_escaping_excerpt_sources` with separate missing, out-of-root, and escaping-symlink nodes; assert all excerpts are omitted and diagnostics sort by `(file, code, message)`.

- [ ] **Step 5: Compose unpruned relationships, paths, and impact summary**

Populate `ExploreResult` with schema `agent-spec/rust-atlas/explore-v1`, query/profile, ranked seeds, deduplicated nodes/edges, pairwise primary spines, bounded alternatives, impact entries, excerpts, diagnostics, shared status, and stale mirror. Preserve full `Edge` values. Build primary paths in `(left seed rank, right seed rank, path signature)` order up to `max_paths`; only then fill remaining path slots with alternatives. If more primary candidates exist, mark `truncated` with `primary-paths-count`; primary paths admitted to the result are never removed by byte pruning. `usage.paths` is `primary_paths.len() + alternative_paths.len()`. Add `test_atlas_explore_composes_ranked_context_and_relationships` to assert one fixture returns the path and identifier seeds, verified excerpts, caller/callee/implementation edges, primary and bounded paths, and reverse impact in byte-identical order. Add `test_atlas_explore_ranks_query_terms_and_reports_no_match` to bind token deduplication, suffix ambiguity ordering, and the typed `atlas-explore-no-match` diagnostic.

Add `test_atlas_query_surfaces_reject_worktree_mismatch` after all four library entry points exist. Build an index under one worktree identity, query it from a same-content alternate worktree, and assert `explore`, `flow`, `impact`, and `affected_paths` each return `atlas-worktree-mismatch` before returning a result or reading source excerpts.

Extend `test_atlas_query_surfaces_share_traversal_contract` from Task 2 with representative `FlowResult`, `ImpactResult`, `AffectedResult`, and `ExploreResult` values. Assert each serialized path uses the same `GraphPath`/`PathHop` shape, each preserves the complete original `Edge`, and each top-level `stale` equals `status.syn.stale_files`; do not leave the selector as a type-only smoke test.

- [ ] **Step 6: Run and commit source-safe explore composition**

```bash
cargo test -p rust-atlas test_atlas_explore_omits_excerpt_when_selected_source_hash_is_stale -- --nocapture
cargo test -p rust-atlas test_atlas_explore_composes_ranked_context_and_relationships -- --nocapture
cargo test -p rust-atlas test_atlas_explore_ranks_query_terms_and_reports_no_match -- --nocapture
cargo test -p rust-atlas test_atlas_explore_rejects_missing_and_escaping_excerpt_sources -- --nocapture
cargo test -p rust-atlas test_atlas_query_surfaces_reject_worktree_mismatch -- --nocapture
cargo test -p rust-atlas explore -- --nocapture
git add crates/rust-atlas/src/explore.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): compose source-safe explore context"
```

### Task 7: Enforce Compact And Deep Serialized Budgets

**Files:**
- Modify: `crates/rust-atlas/src/explore.rs`

**Interfaces:**
- Consumes: unpruned `ExploreResult` from Task 6.
- Produces: `ExploreBudget::compact/deep`, `finalize_budget -> Result<ExploreResult, AtlasError>`, exact `BudgetUsage`, deterministic truncation reasons, and typed failure for unshrinkable mandatory evidence.

- [ ] **Step 1: Write failing profile and pruning tests**

```rust
#[test]
fn test_atlas_explore_compact_and_deep_budgets_are_deterministic() {
    for profile in [ExploreProfile::Compact, ExploreProfile::Deep] {
        let left = oversized_explore(profile);
        let right = oversized_explore(profile);
        let left_bytes = serde_json::to_vec(&left).unwrap();
        let right_bytes = serde_json::to_vec(&right).unwrap();
        assert_eq!(left_bytes, right_bytes);
        assert!(left_bytes.len() <= profile.budget().max_serialized_bytes);
        assert_eq!(left.usage.serialized_bytes, left_bytes.len());
    }
}

#[test]
fn test_atlas_explore_prunes_optional_sections_in_fixed_order() {
    let snapshots = single_optional_item_pruning_snapshots();
    assert_eq!(snapshots.iter().map(optional_counts).collect::<Vec<_>>(), vec![
        (1, 1, 1, 1),
        (0, 1, 1, 1),
        (0, 0, 1, 1),
        (0, 0, 0, 1),
        (0, 0, 0, 0),
    ]);
    assert_eq!(snapshots[1].last_reason(), "excerpts");
    assert_eq!(snapshots[2].last_reason(), "alternative-paths");
    assert_eq!(snapshots[3].last_reason(), "off-spine-edges");
    assert_eq!(snapshots[4].last_reason(), "off-spine-nodes");

    let required = &snapshots[0];
    for snapshot in &snapshots[1..] {
        assert_eq!(snapshot.seeds, required.seeds);
        assert_eq!(snapshot.status, required.status);
        assert_eq!(snapshot.diagnostics, required.diagnostics);
        assert_eq!(snapshot.primary_paths, required.primary_paths);
    }
}

#[test]
fn test_atlas_explore_rejects_unshrinkable_required_payload() {
    let error = finalize_budget(required_payload_larger_than_compact_limit()).unwrap_err();
    assert!(error.to_string().contains("atlas-explore-budget"));
}
```

The test-only `optional_counts` tuple is `(excerpts.len(), alternative_paths.len(), edges not referenced by any primary-path hop, nodes where spine == false and seed == false)`. Build the five fixtures from the same required fields and one item in each optional category, changing only the byte cap so each next category must be removed.

- [ ] **Step 2: Verify tests fail for unbounded output**

```bash
cargo test -p rust-atlas test_atlas_explore_compact_and_deep_budgets_are_deterministic -- --nocapture
cargo test -p rust-atlas test_atlas_explore_prunes_optional_sections_in_fixed_order -- --nocapture
```

Expected: at least one result exceeds its byte limit and pruning reasons are absent.

- [ ] **Step 3: Implement exact profile budgets**

```rust
pub struct ExploreBudget {
    pub max_seeds: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_paths: usize,
    pub max_excerpts: usize,
    pub max_excerpt_lines: usize,
    pub max_serialized_bytes: usize,
}
```

Return exact compact values `(8,32,48,8,4,20,16_000)` and deep values `(16,96,160,20,12,40,24_000)`.

- [ ] **Step 4: Implement stable pruning to a fixed point**

First enforce count caps from deterministic tails. Then serialize compact JSON, set `usage.serialized_bytes`, serialize again, and repeat until the value is stable. If over the byte cap, remove one item from the fixed tail order: excerpt, alternative path, off-spine edge, off-spine node. Record each category once in `truncation_reasons`. If required seed/status/diagnostic/primary-spine bytes alone exceed the profile, return this error instead of corrupting the required result:

```rust
#[error("atlas-explore-budget: required payload is {required_bytes} bytes, profile limit is {max_bytes}")]
ExploreBudget { required_bytes: usize, max_bytes: usize },
```

- [ ] **Step 5: Run and commit bounded explore**

```bash
cargo test -p rust-atlas test_atlas_explore_compact_and_deep_budgets_are_deterministic -- --nocapture
cargo test -p rust-atlas test_atlas_explore_prunes_optional_sections_in_fixed_order -- --nocapture
cargo test -p rust-atlas test_atlas_explore_rejects_unshrinkable_required_payload -- --nocapture
cargo test -p rust-atlas
git add crates/rust-atlas/src/explore.rs
git commit -m "feat(atlas): enforce deterministic explore budgets"
```

### Task 8: Add CLI Explore Flow Impact And Affected Modes

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `rust_atlas::{explore,flow,impact,affected_paths}`.
- Produces: `agent-spec atlas explore|flow|impact|affected`; crate-private `AffectedCliInput`, `AffectedInputMode`, `GitDiffRequest`, `resolve_affected_input_mode`, `resolve_affected_inputs`, `git_diff_request`, `run_git_name_only`, and writer/dependency-injected `cmd_atlas_affected_with_io`.

- [ ] **Step 1: Write failing clap and input-mode tests**

```rust
#[test]
fn test_atlas_affected_cli_rejects_conflicting_input_modes() {
    for input in [
        AffectedCliInput::new(vec![PathBuf::from("src/lib.rs")]).staged(),
        AffectedCliInput::new(vec![]).stdin().commit("HEAD~1..HEAD"),
    ] {
        let mut stdin = PanicOnRead;
        let mut git = |_: &GitDiffRequest| -> anyhow::Result<Vec<u8>> {
            panic!("Git must not run before affected mode validation")
        };
        let mut stdout = Vec::new();
        let error = cmd_atlas_affected_with_io(
            Path::new("/unused"), Path::new("/unused-graph"), input, &AffectedOptions::default(),
            &mut stdin, &mut git, &mut stdout,
        ).unwrap_err().to_string();
        assert!(error.contains("atlas-affected-input-mode"));
        assert!(stdout.is_empty());
    }
}
```

Add `test_atlas_explore_flow_impact_cli_parse_contract` to parse `explore QUERY --profile compact|deep --frozen`, paired `flow --from/--to`, `flow --through`, `impact SYMBOL --depth`, and all five affected modes, asserting every parsed field. Include invalid profile, half-paired flow, missing impact symbol, and zero affected mode cases; all must fail before any I/O helper is called.

- [ ] **Step 2: Verify tests fail before command variants exist**

```bash
cargo test test_atlas_affected_cli_rejects_conflicting_input_modes -- --nocapture
```

Expected: compilation or clap parsing fails for missing variants.

- [ ] **Step 3: Add exact CLI variants**

Add `Explore`, `Flow`, `Impact`, and `Affected` to `AtlasCommands`. `Flow` accepts either both `--from/--to` or only `--through`. `Affected` accepts positional `paths`, `--stdin`, `--staged`, `--worktree`, or `--commit` and validates exactly one mode before stdin or process access.

Define `AffectedCliInput` with exactly `paths: Vec<PathBuf>`, `stdin: bool`, `staged: bool`, `worktree: bool`, and `commit: Option<String>`. `cmd_atlas_affected_with_io(code, graph, input, options, stdin, run_git, stdout)` first calls the pure mode resolver, then obtains paths, then calls the library, and writes only a successful result. The production command supplies locked stdin/stdout and a closure around `run_git_name_only`; tests supply panic-on-access stdin/Git dependencies and a `Vec<u8>` writer.

- [ ] **Step 4: Implement argv-only Git inputs**

Represent the program and argv first as:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct GitDiffRequest {
    program: &'static str,
    args: Vec<OsString>,
}
```

Use `std::process::Command::new(request.program).args(request.args)` with these exact argv shapes:

```text
git -C <code> diff --name-only --cached --
git -C <code> diff --name-only --
git -C <code> diff --name-only <range> --
```

Reject an empty range or a range beginning with `-`. Split stdout by lines, reject NUL, sort/deduplicate through affected normalization, and return `anyhow!("atlas-affected-git: ...")` on nonzero status. Mode count failures use `anyhow!("atlas-affected-input-mode: ...")`. Never use `sh -c`.

Add `test_atlas_affected_cli_covers_all_vcs_modes_and_failures`. Assert staged, worktree, and commit requests equal the exact argv above and `program == "git"`; explicit/stdin never construct a request; zero mode and option-like revision fail before spawning; a temporary non-repository root proves Git nonzero becomes `atlas-affected-git` and leaves the writer-backed stdout empty.

- [ ] **Step 5: Print budgeted explore without reformat inflation**

Use the finalized compact `serde_json::to_vec` bytes for explore and append one newline. Other results continue through existing pretty JSON output. A command error writes no normal stdout.

- [ ] **Step 6: Run CLI tests and actual help**

```bash
cargo test test_atlas_affected_cli_rejects_conflicting_input_modes -- --nocapture
cargo test test_atlas_affected_cli_covers_all_vcs_modes_and_failures -- --nocapture
cargo test test_atlas_explore_flow_impact_cli_parse_contract -- --nocapture
cargo run --quiet -- atlas explore --help
cargo run --quiet -- atlas flow --help
cargo run --quiet -- atlas impact --help
cargo run --quiet -- atlas affected --help
```

Expected: tests pass and help lists only implemented flags and mutual-exclusion rules.

- [ ] **Step 7: Commit CLI surfaces**

```bash
git add src/main.rs
git commit -m "feat(atlas): expose explore flow impact commands"
```

### Task 9: Add Opt-In Frozen MCP Explore

**Files:**
- Modify: `src/spec_mcp/tools.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `rust_atlas::explore` with `frozen: true`.
- Produces: opt-in `atlas_explore` tool with `query` and `profile`; no default discovery change.

- [ ] **Step 1: Write the failing discovery and parity test**

```rust
#[test]
fn test_atlas_explore_mcp_is_hidden_by_default_and_opt_in() {
    with_atlas_explore_tool(false, || {
        assert!(tool_specs().as_array().unwrap().iter().all(|tool| tool["name"] != "atlas_explore"));
    });
    with_atlas_explore_tool(true, || {
        let ctx = ctx();
        let tool = tool_specs().as_array().unwrap().iter()
            .find(|tool| tool["name"] == "atlas_explore").unwrap();
        assert_eq!(tool["inputSchema"]["required"], serde_json::json!(["query"]));
        let actual = dispatch("atlas_explore", &serde_json::json!({"query":"run","profile":"compact"}), &ctx).unwrap();
        let expected = serde_json::to_value(rust_atlas::explore(
            &ctx.code,
            &ctx.code.join(".agent-spec/graph"),
            "run",
            &rust_atlas::ExploreOptions { profile: rust_atlas::ExploreProfile::Compact, frozen: true },
        ).unwrap()).unwrap();
        assert_eq!(actual, expected);
    });
}
```

- [ ] **Step 2: Verify the tool is absent**

```bash
cargo test test_atlas_explore_mcp_is_hidden_by_default_and_opt_in -- --nocapture
```

Expected: test fails because the opt-in helper/tool does not exist.

- [ ] **Step 3: Implement opt-in discovery and thin dispatch**

Mirror the existing thread-local test override pattern for search. In production, enable only when `AGENT_SPEC_MCP_ATLAS_EXPLORE == "1"`. The input schema allows `profile` enum `compact|deep`, defaults to compact, rejects other strings with `atlas-explore-profile`, and always calls the library in frozen mode.

- [ ] **Step 4: Run MCP and default-surface regression tests**

```bash
cargo test test_atlas_explore_mcp_is_hidden_by_default_and_opt_in -- --nocapture
cargo test spec_mcp -- --nocapture
cargo test test_mcp_atlas_search_returns_the_frozen_library_result -- --nocapture
```

Expected: explore parity passes, search behavior remains unchanged, and default tools/list still omits both opt-in tools.

- [ ] **Step 5: Commit MCP integration**

```bash
git add src/spec_mcp/tools.rs src/main.rs
git commit -m "feat(atlas): add opt-in explore mcp tool"
```

### Task 10: Extend Offline E1 Query Metrics

**Files:**
- Modify: `src/atlas_eval.rs`
- Modify: `docs/atlas-evaluation.md`
- Modify: `benchmarks/atlas/corpus.json`
- Modify: `scripts/atlas-eval/run-opt-in.sh`

**Interfaces:**
- Consumes: existing `RunReceipt`, `MetricsSummary`, paired run plan.
- Produces: four backward-compatible receipt metrics and median/MAD summaries; still no default real-Agent execution.

- [ ] **Step 1: Write the failing backward-compatible metric test**

```rust
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
    for metric in [&baseline.response_bytes, &baseline.read_back_calls,
        &baseline.follow_up_queries, &baseline.truncated_queries]
    {
        assert_metric(metric, 0.0, 0.0, 1);
    }
    let legacy: RunReceipt = serde_json::from_str(LEGACY_RECEIPT_JSON).unwrap();
    assert_eq!((legacy.response_bytes, legacy.read_back_calls, legacy.follow_up_queries, legacy.truncated_queries), (0, 0, 0, 0));
}
```

- [ ] **Step 2: Run and verify missing fields**

```bash
cargo test test_atlas_eval_receipts_measure_explore_readback_and_response_bytes -- --nocapture
```

Expected: compilation fails because the receipt and summary fields are absent.

- [ ] **Step 3: Add serde-default receipt and summary fields**

Add these exact receipt fields; all counts and byte values are `u64`:

```rust
#[serde(default)] pub response_bytes: u64,
#[serde(default)] pub read_back_calls: u64,
#[serde(default)] pub follow_up_queries: u64,
#[serde(default)] pub truncated_queries: u64,
```

Add matching non-optional `MetricSummary` fields to `MetricsSummary` and feed them through aggregate and every per-arm summarization. The test helper `assert_metric(value, median, mad, samples)` asserts all three fields. Keep correctness validation first.

- [ ] **Step 4: Harden the opt-in receipt boundary**

Update the documented external agent receipt contract to emit the four fields. The runner remains a byte-preserving executable boundary and MUST NOT fabricate them; `summarize` supplies zero only for genuinely legacy JSON that omits them.

- [ ] **Step 5: Update corpus rubrics without claiming results**

For the existing flow and impact cases, add rubric items requiring truncation disclosure and evidence-path completeness. Keep revisions pinned, both arms present, and three trials. Do not add benchmark percentages or a checked-in passing A/B summary.

- [ ] **Step 6: Run evaluator tests and commit**

```bash
cargo test atlas_eval -- --nocapture
env -u ATLAS_EVAL_AGENT_COMMAND bash scripts/atlas-eval/run-opt-in.sh /nonexistent /tmp/unused
```

Expected: evaluator tests pass; the runner exits 2 before any model call when `ATLAS_EVAL_AGENT_COMMAND` is unset.

```bash
git add src/atlas_eval.rs docs/atlas-evaluation.md benchmarks/atlas/corpus.json scripts/atlas-eval/run-opt-in.sh
git commit -m "feat(atlas): measure query payload and followups"
```

### Task 11: Publish, Dogfood, And Prove Wave 2

**Files:**
- Modify: `docs/atlas-roadmap.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `CHANGELOG.md`
- Modify: `.agent-spec/wiki/architecture/atlas.md`
- Modify: `.agent-spec/wiki/concepts/atlas-authority.md`
- Modify: `.agent-spec/wiki/_index.md`
- Modify: `.agent-spec/wiki/_meta.json`

**Interfaces:**
- Consumes: final local CLI help and all Wave 2 library/MCP behavior.
- Produces: user guidance, maintained live-wiki context, passing lifecycle, and complete requirement replay evidence.

- [ ] **Step 1: Update docs from actual command help**

Run the four new `--help` commands and document only flags that exist. Explain compact/deep budgets, FlowState meanings, affected input modes, no filename-based test inference, source-hash excerpt rule, and MCP opt-in. Mark B2/B3/B4 delivered only after code gates pass; keep real E1 A/B pending.

- [ ] **Step 2: Update the live wiki from owned sources**

Add the new Rust modules and Contract to Atlas architecture/concept pages with repo-relative `source_files`. Refresh wiki index/meta through current wiki commands; do not hand-edit generated fingerprints.

- [ ] **Step 3: Run deterministic code and docs gates**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/docs-lint.sh
target/debug/agent-spec lint-knowledge --knowledge knowledge --gate
target/debug/agent-spec requirements graph --knowledge knowledge --format json --gate
target/debug/agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
target/debug/agent-spec wiki check --code . --wiki .agent-spec/wiki
```

Expected: all commands exit 0. Harper may be skipped only outside CI and must be recorded; markdownlint and lychee must run.

- [ ] **Step 4: Build a fresh dogfood graph and resolve Contract symbols**

```bash
cargo build --workspace
target/debug/agent-spec atlas build --code . --graph .agent-spec/graph --full
target/debug/agent-spec atlas search rust_atlas::traversal --code . --graph .agent-spec/graph --frozen
target/debug/agent-spec atlas search rust_atlas::flow --code . --graph .agent-spec/graph --frozen
target/debug/agent-spec atlas search rust_atlas::impact --code . --graph .agent-spec/graph --frozen
target/debug/agent-spec atlas search rust_atlas::affected --code . --graph .agent-spec/graph --frozen
target/debug/agent-spec atlas search rust_atlas::explore --code . --graph .agent-spec/graph --frozen
```

Expected: each Contract symbol resolves to one canonical module node and status reports fresh syn authority.

- [ ] **Step 5: Record the actual worktree execution manifest**

Create ignored `.agent-spec/worktrees.json` with one entry:

```json
{
  "version": 1,
  "entries": [{
    "work_unit_id": "WU-REQ-ATLAS-EXPLORE-FLOW-IMPACT",
    "requirement_id": "REQ-ATLAS-EXPLORE-FLOW-IMPACT",
    "batch": 4,
    "base_branch": "main",
    "branch": "feat/atlas-roadmap-wave2",
    "path": "/Users/zhangalex/Work/Projects/FW/rust-agents/agent-spec-worktrees/atlas-roadmap-wave2",
    "spec_path": "specs/task-atlas-explore-flow-impact.spec.md",
    "depends_on": ["REQ-ATLAS-AGENT-EVALUATION", "REQ-ATLAS-EDGE-EVIDENCE-INDEX", "REQ-ATLAS-WORKTREE-FRESHNESS"]
  }],
  "diagnostics": []
}
```

- [ ] **Step 6: Commit tracked Wave 2 documentation before lifecycle**

```bash
git add docs README.md AGENTS.md skills CHANGELOG.md .agent-spec/wiki knowledge specs
git commit -m "docs(atlas): publish explore flow impact wave"
```

Expected: tracked worktree is clean; ignored graph/worktree/run artifacts remain local.

- [ ] **Step 7: Run the active lifecycle against the committed graph**

```bash
target/debug/agent-spec lifecycle specs/task-atlas-explore-flow-impact.spec.md --code . --run-log-dir .agent-spec/runs --format json
```

Expected: all twenty-two scenarios pass; there are no skip, uncertain, pending-review, or failure verdicts.

- [ ] **Step 8: Prove complete replay and Mermaid chain**

```bash
target/debug/agent-spec requirements replay REQ-ATLAS-EXPLORE-FLOW-IMPACT --format text
target/debug/agent-spec requirements trace-graph REQ-ATLAS-EXPLORE-FLOW-IMPACT --format mermaid
```

Expected: every record reaches requirement, batch-4 work unit, Contract scenario, selector, real Atlas code targets, actual Wave 2 worktree, branch, and latest Git commit. Mermaid edges form a continuous scenario-to-VCS chain.

- [ ] **Step 9: Run an independent final review**

Review the complete Wave 2 diff from the Task 1 governance commit. Findings must lead by severity and cover output-bound enforcement, traversal outcome precedence, source-path safety, Git argv safety, no test inference, MCP default stability, and E1 honesty. Resolve every Important or blocking finding before marking the wave complete.

## Self-Review Record

- Spec coverage: Tasks 2-3 cover shared traversal, endpoint ambiguity, capability precedence, and all flow clauses; Tasks 4-5 cover impact, containment, affected normalization, and test honesty; Tasks 6-7 cover ranked context composition, source safety, worktree authority, evidence retention, profiles, pruning, and unshrinkable budget failure; Task 8 covers CLI grammar and every VCS mode/failure; Task 9 covers opt-in MCP; Task 10 covers E1 metrics; Task 11 covers delivery claims and dogfood evidence.
- Completeness scan: every implementation step is concrete; every selector and roadmap clause has an owner. Real A/B execution remains an explicit non-claim rather than an unfinished implementation step.
- Type consistency: `GraphPath`, `PathHop`, `FlowState`, `TraversalLimits`, `FlowResult`, `ImpactResult`, `AffectedResult`, and `ExploreResult` are introduced before consumers. All surfaces use `AtlasStatus` and the same stale mirror.
- Reference-project boundary: codegraph contributes tested invariants for bounded output, reverse impact, containment behavior, path normalization, and A/B method; no TypeScript runtime, SQLite store, daemon, installer, or polyglot parser is copied.
