# Rust Atlas Roadmap Wave 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the roadmap's first wave: reproducible Rust Agent evaluation, evidence-complete Atlas edges with an indexed search surface, and worktree-aware layered freshness consumed consistently by CLI, MCP, bindings, and lifecycle.

**Architecture:** Keep `rust-atlas` as the Rust-specific deterministic graph provider and keep per-file JSON shards canonical. Add focused `edge`, `index`, and `status` modules inside the crate while retaining the existing public API wrappers; add the benchmark compiler to the agent-spec binary because it evaluates Agent use rather than extracting Rust facts. Derived indexes and status artifacts are rebuilt from canonical graph metadata and never become KLL truth.

**Tech Stack:** Rust stable, Cargo workspace, serde/serde_json, blake3, syn, SCIP protobuf, clap, shell opt-in runner, agent-spec lifecycle and requirements gates.

## Global Constraints

- `rust-atlas` MUST NOT depend on the `agent-spec` binary crate.
- Default tests, build scripts, and CI MUST NOT invoke a network or a real model.
- Real Agent execution requires an explicit `ATLAS_EVAL_AGENT_COMMAND`.
- `SCHEMA_VERSION` increases from 4 to 5; old graph schemas are rejected with `atlas-schema-mismatch` and rebuilt, not migrated.
- `serde(default)` supports optional fields only within the current schema version.
- JSON shards remain the canonical graph store; `query-index.json` is derived and atomically replaceable.
- Query-index schema and graph fingerprint mismatches MUST fail with a rebuild diagnostic.
- Search limit is `1..=200`, default `20`, with deterministic ranking and byte-stable ordering.
- Graph identity and freshness MUST distinguish syn, SCIP, and MIR and MUST reject definitive queries against another worktree's graph.
- KLL requirements and Task Contracts MUST remain byte-identical during graph build, query, binding, and lifecycle verification.
- Every production change follows red-green-refactor and introduces no `unsafe`, `unwrap`, or `expect` in `rust-atlas` production code.

---

### Task 1: Governance And Execution DAG

**Files:**
- Create: `knowledge/requirements/req-atlas-agent-evaluation.md`
- Create: `knowledge/requirements/req-atlas-edge-evidence-index.md`
- Create: `knowledge/requirements/req-atlas-worktree-freshness.md`
- Create: `knowledge/requirements/req-atlas-scip-semantic.md`
- Create: `specs/task-atlas-agent-evaluation.spec.md`
- Create: `specs/task-atlas-edge-evidence-index.spec.md`
- Create: `specs/task-atlas-worktree-layered-freshness.spec.md`
- Modify: `specs/task-atlas-scip-semantic.spec.md`
- Modify: `docs/atlas-roadmap.md`

**Interfaces:**
- Consumes: `REQ-RUST-ATLAS`, `REQ-INTENT-CODE-LINKER`, and delivered Atlas contracts.
- Produces: one ready work unit per new requirement and the ordered contract chain `evaluation || edge-index -> worktree-freshness`.

- [x] **Step 1: Validate every Task Contract**

Run:

```bash
agent-spec lint specs/task-atlas-agent-evaluation.spec.md specs/task-atlas-edge-evidence-index.spec.md specs/task-atlas-worktree-layered-freshness.spec.md specs/task-atlas-scip-semantic.spec.md --min-score 0.7
```

Expected: all four specs parse and score 100%; no error-level finding.

- [x] **Step 2: Validate KLL and requirement topology**

Run:

```bash
agent-spec lint-knowledge --knowledge knowledge --gate
agent-spec requirements graph --knowledge knowledge --format json --gate
agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
```

Expected: all commands exit 0; each of the four Atlas requirements has exactly one satisfying spec and no error diagnostic.

- [ ] **Step 3: Commit governance inputs**

```bash
git add docs/atlas-roadmap.md docs/superpowers/plans/2026-07-20-atlas-roadmap-wave1.md knowledge/requirements/req-atlas-*.md specs/task-atlas-*.spec.md
git commit -m "docs(atlas): define roadmap wave one contracts"
```

### Task 2: Evaluation Corpus And Typed Validator

**Files:**
- Create: `src/atlas_eval.rs`
- Create: `benchmarks/atlas/corpus.json`
- Modify: `src/main.rs`
- Test: `src/atlas_eval.rs`

**Interfaces:**
- Consumes: JSON corpus schema id `agent-spec/atlas-eval/corpus-v1`.
- Produces: `pub fn load_corpus(path: &Path) -> Result<Corpus, EvalError>` and `pub fn compile_plan(corpus: &Corpus) -> Result<RunPlan, EvalError>`.

- [ ] **Step 1: Write failing corpus and planning tests**

Add these exact tests to `src/atlas_eval.rs`:

```rust
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
    assert_eq!(compile_plan(&corpus).unwrap_err().code(), "atlas-eval-duplicate-case");
}

#[test]
fn test_atlas_eval_rejects_too_few_trials() {
    let corpus = valid_corpus(2);
    assert_eq!(compile_plan(&corpus).unwrap_err().code(), "atlas-eval-trials");
}
```

- [ ] **Step 2: Run tests and observe the missing module failure**

Run: `cargo test test_atlas_eval_ -- --nocapture`

Expected: FAIL because `atlas_eval`, corpus types, and plan compiler do not exist.

- [ ] **Step 3: Implement typed corpus validation and paired run planning**

Define these public types and functions:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corpus { pub schema: String, pub model: String, pub prompt: String, pub cases: Vec<Case> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    pub size: WorkspaceSize,
    pub task_class: TaskClass,
    pub repository: String,
    pub revision: String,
    pub trials_per_arm: u32,
    pub rubric: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arm { Atlas, Baseline }

pub fn load_corpus(path: &Path) -> Result<Corpus, EvalError>;
pub fn compile_plan(corpus: &Corpus) -> Result<RunPlan, EvalError>;
```

Validation rejects the wrong schema, duplicate/empty ids, empty revisions, empty rubrics, and fewer than three trials. Generate runs in corpus order, then `Atlas`, `Baseline`, and ascending trial order, copying model, prompt, repository, revision, permissions, and cache condition into every run.

- [ ] **Step 4: Run focused tests**

Run: `cargo test test_atlas_eval_ -- --nocapture`

Expected: the three planning tests pass.

- [ ] **Step 5: Commit typed evaluator core**

```bash
git add src/atlas_eval.rs src/main.rs benchmarks/atlas/corpus.json
git commit -m "feat(atlas): add deterministic evaluation corpus compiler"
```

### Task 3: Evaluation Receipts, Summary, CLI, And Opt-In Runner

**Files:**
- Modify: `src/atlas_eval.rs`
- Modify: `src/main.rs`
- Create: `scripts/atlas-eval/run-opt-in.sh`
- Create: `docs/atlas-evaluation.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: `RunPlan` JSON and newline-delimited or array `RunReceipt` JSON.
- Produces: `pub fn summarize(receipts: &[RunReceipt]) -> Result<EvalSummary, EvalError>` and CLI `agent-spec atlas benchmark validate|plan|summarize`.

- [ ] **Step 1: Write failing receipt and atomic output tests**

```rust
#[test]
fn test_atlas_eval_summary_rejects_missing_correctness() {
    let receipts = vec![receipt(None)];
    assert_eq!(summarize(&receipts).unwrap_err().code(), "atlas-eval-receipt");
}

#[test]
fn test_atlas_eval_plan_writes_atomic_output() {
    let dir = temp_dir("atlas-eval-out");
    let out = dir.join("plan.json");
    write_json_atomic(&out, &compile_plan(&valid_corpus(3)).unwrap()).unwrap();
    let parsed: RunPlan = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert!(!parsed.runs.is_empty());
    assert!(!dir.join("plan.json.tmp").exists());
}
```

- [ ] **Step 2: Run tests and confirm red state**

Run: `cargo test test_atlas_eval_ -- --nocapture`

Expected: FAIL because receipt summarization and atomic writing do not exist.

- [ ] **Step 3: Implement summaries and CLI actions**

Add typed `Correctness`, `RunReceipt`, `MetricSummary`, and `EvalSummary`. `summarize` rejects any missing correctness before calculating median and median absolute deviation for file reads, graph calls, tool calls, duration, context size, and optional cost. Add nested clap enums:

```rust
Benchmark { #[command(subcommand)] action: AtlasBenchmarkCommands }

enum AtlasBenchmarkCommands {
    Validate { #[arg(long)] corpus: PathBuf },
    Plan { #[arg(long)] corpus: PathBuf, #[arg(long)] out: Option<PathBuf> },
    Summarize { #[arg(long)] receipts: PathBuf, #[arg(long)] out: Option<PathBuf> },
}
```

When `--out` is present, atomically write JSON and keep stdout empty. On validation failure return an error before printing anything.

- [ ] **Step 4: Implement and test the opt-in shell boundary**

Create an executable script whose first operation is:

```bash
if [[ -z "${ATLAS_EVAL_AGENT_COMMAND:-}" ]]; then
  printf '%s\n' 'atlas-eval-agent-command: set ATLAS_EVAL_AGENT_COMMAND explicitly' >&2
  exit 2
fi
```

Add `test_atlas_eval_opt_in_runner_requires_command` that launches the script with the variable removed and asserts exit code 2 and no child receipt.

- [ ] **Step 5: Verify evaluator commands and docs**

Run:

```bash
cargo test test_atlas_eval_ -- --nocapture
cargo run -- atlas benchmark validate --corpus benchmarks/atlas/corpus.json
bash scripts/docs-lint.sh
```

Expected: tests pass, validation emits valid JSON, and docs gates have zero errors.

- [ ] **Step 6: Commit E0**

```bash
git add src/atlas_eval.rs src/main.rs benchmarks/atlas scripts/atlas-eval docs/atlas-evaluation.md README.md AGENTS.md skills/agent-spec-tool-first/SKILL.md CHANGELOG.md
git commit -m "feat(atlas): add reproducible agent evaluation baseline"
```

### Task 4: Evidence-Complete Edge Schema

**Files:**
- Modify: `crates/rust-atlas/src/lib.rs`
- Test: `crates/rust-atlas/src/lib.rs`

**Interfaces:**
- Consumes: existing `Edge`, syn extraction, and SCIP overlay.
- Produces: schema v5 `EdgeSite`, `ExtractorIdentity`, `DispatchKind`, `EdgeConfidence`, and evidence fields.

- [ ] **Step 1: Write the failing confidence invariant test**

```rust
#[test]
fn test_atlas_rejects_exact_confidence_with_multiple_candidates() {
    let mut edge = edge("a", "b", EdgeKind::Calls);
    edge.confidence = Some(EdgeConfidence::Exact);
    edge.candidates = vec!["b".into(), "c".into()];
    let error = validate_edges([&edge]).unwrap_err().to_string();
    assert!(error.contains("confidence"));
    assert!(error.contains("2 candidates"));
}
```

- [ ] **Step 2: Run the test and confirm missing fields/types**

Run: `cargo test -p rust-atlas test_atlas_rejects_exact_confidence_with_multiple_candidates`

Expected: FAIL to compile because evidence types and fields do not exist.

- [ ] **Step 3: Add schema v5 evidence types and invariant**

```rust
pub const SCHEMA_VERSION: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EdgeSite { pub file: String, pub line_start: usize, pub column_start: usize, pub line_end: usize, pub column_end: usize }

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExtractorIdentity { pub name: String, pub version: Option<String> }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchKind { Static, Trait, Generic, Closure, FunctionPointer, Channel, Macro }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeConfidence { Exact, BoundedCandidates, Heuristic }
```

Add `#[serde(default)]` to every new edge field. Update every edge constructor explicitly: syn resolved facts use exact confidence and `extractor=syn`; unresolved facts do not claim exact confidence. Include `site` in derived ordering and deduplication.

- [ ] **Step 4: Run crate tests and repair every constructor**

Run: `cargo test -p rust-atlas`

Expected: all existing and new crate tests pass under schema v5.

- [ ] **Step 5: Commit edge schema**

```bash
git add crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): preserve edge evidence in schema v5"
```

### Task 5: SCIP Occurrence Evidence

**Files:**
- Modify: `crates/rust-atlas/src/lib.rs`
- Test: `crates/rust-atlas/src/lib.rs`
- Modify: `fixtures/atlas/**`

**Interfaces:**
- Consumes: normalized SCIP occurrence range and symbol information.
- Produces: SCIP `calls`, `uses-type`, and `references` edges with repository-relative 1-based `EdgeSite`, extractor identity, confidence, and evidence.

- [ ] **Step 1: Write the failing occurrence evidence test**

```rust
#[test]
fn test_scip_edges_preserve_occurrence_site_and_evidence() {
    let (_code, graph) = build_scip_fixture();
    let (_, shards) = load_graph(&graph).unwrap();
    let edge = shards.iter().flat_map(|s| &s.edges)
        .find(|edge| edge.provenance == Provenance::Scip && edge.kind == EdgeKind::Calls)
        .unwrap();
    let site = edge.site.as_ref().unwrap();
    assert!(site.line_start >= 1 && site.column_start >= 1);
    assert_eq!(edge.extractor.as_ref().unwrap().name, "rust-analyzer-scip");
    assert_eq!(edge.confidence, Some(EdgeConfidence::Exact));
    assert!(edge.evidence.as_deref().is_some_and(|value| value.contains("occurrence")));
}
```

- [ ] **Step 2: Run the test and confirm fields are empty**

Run: `cargo test -p rust-atlas test_scip_edges_preserve_occurrence_site_and_evidence`

Expected: FAIL because the overlay does not populate evidence.

- [ ] **Step 3: Populate evidence at the SCIP normalization boundary**

Convert SCIP zero-based positions once:

```rust
fn edge_site(file: &str, range: &ScipRange) -> EdgeSite {
    EdgeSite {
        file: file.to_string(),
        line_start: range.start_line + 1,
        column_start: range.start_column + 1,
        line_end: range.end_line + 1,
        column_end: range.end_column + 1,
    }
}
```

Use exact confidence only for one resolved target. Preserve multiple candidates with `BoundedCandidates` and record an analyzer-specific occurrence reason.

- [ ] **Step 4: Run SCIP and full crate tests**

Run: `cargo test -p rust-atlas`

Expected: all rust-atlas tests pass.

- [ ] **Step 5: Commit SCIP evidence**

```bash
git add crates/rust-atlas/src/lib.rs fixtures/atlas
git commit -m "feat(atlas): retain SCIP occurrence evidence"
```

### Task 6: Derived Query Index

**Files:**
- Create: `crates/rust-atlas/src/index.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Test: `crates/rust-atlas/src/index.rs`

**Interfaces:**
- Consumes: validated `Meta` and `Shard` values.
- Produces: `pub fn rebuild_query_index(graph_dir: &Path, meta: &Meta, shards: &[Shard]) -> Result<QueryIndex, AtlasError>` and `pub fn load_query_index(graph_dir: &Path, meta: &Meta) -> Result<QueryIndex, AtlasError>`.

- [ ] **Step 1: Write failing index build and stale tests**

```rust
#[test]
fn test_atlas_build_writes_current_query_index() {
    let (_code, graph) = build_basic_fixture();
    let index: QueryIndex = read_json(&graph.join("query-index.json")).unwrap();
    assert_eq!(index.schema_version, SCHEMA_VERSION);
    assert!(!index.nodes.is_empty());
    assert!(!index.outgoing.is_empty());
}

#[test]
fn test_atlas_search_rejects_graph_fingerprint_mismatch() {
    let (_code, graph) = build_basic_fixture();
    rewrite_index_fingerprint(&graph, "wrong");
    let error = search(&code, &graph, "Store", &SearchOptions::default()).unwrap_err();
    assert!(error.to_string().contains("atlas-query-index-stale"));
}
```

- [ ] **Step 2: Run tests and observe missing index**

Run: `cargo test -p rust-atlas query_index -- --nocapture`

Expected: FAIL because `query-index.json` and index APIs do not exist.

- [ ] **Step 3: Implement canonical fingerprint and index tables**

Define `QueryIndex` with `schema_version`, `graph_fingerprint`, sorted node/edge tables, and BTreeMap locators for id, symbol, file, incoming, and outgoing edges. Compute the fingerprint from canonical JSON containing schema, package, roots, capability, and `meta.files`. Write through a same-directory temporary file followed by `rename`.

- [ ] **Step 4: Integrate index rebuild after graph validation**

After `validate_graph`, load the current shards, compute the new meta, atomically write `meta.json`, and then atomically write `query-index.json`. A failed index write returns an error and never leaves a partially serialized index.

- [ ] **Step 5: Run index and crate tests**

Run: `cargo test -p rust-atlas`

Expected: all tests pass, including schema mismatch and fingerprint mismatch.

- [ ] **Step 6: Commit index**

```bash
git add crates/rust-atlas/src/index.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): add derived graph query index"
```

### Task 7: Deterministic Search Library, CLI, And MCP

**Files:**
- Modify: `crates/rust-atlas/src/index.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Modify: `src/main.rs`
- Modify: `src/spec_mcp/tools.rs`
- Test: `crates/rust-atlas/src/index.rs`
- Test: `src/main.rs`

**Interfaces:**
- Consumes: current `QueryIndex`.
- Produces: `pub fn search(code_root: &Path, graph_dir: &Path, query: &str, opts: &SearchOptions) -> Result<SearchResult, AtlasError>`, CLI `atlas search`, and MCP `atlas_search`.

- [ ] **Step 1: Write failing ranking and limit tests**

```rust
#[test]
fn test_atlas_search_orders_exact_suffix_segment_and_fuzzy_matches() {
    let result = search_fixture("mem_store");
    let kinds: Vec<_> = result.matches.iter().map(|hit| hit.match_kind).collect();
    assert_eq!(kinds, vec![MatchKind::ExactId, MatchKind::ExactSymbol, MatchKind::QualifiedSuffix, MatchKind::SegmentedIdentifier, MatchKind::NormalizedSubstring]);
    assert_eq!(serde_json::to_vec(&result).unwrap(), serde_json::to_vec(&search_fixture("mem_store")).unwrap());
}

#[test]
fn test_atlas_search_rejects_limit_outside_range() {
    for limit in [0, 201] {
        let error = validate_search_limit(limit).unwrap_err();
        assert!(error.to_string().contains("atlas-search-limit"));
    }
}
```

- [ ] **Step 2: Run search tests and confirm red state**

Run: `cargo test -p rust-atlas test_atlas_search_ -- --nocapture`

Expected: FAIL because search types and ranking do not exist.

- [ ] **Step 3: Implement fixed ranking and stable tie breaking**

Rank exact id, exact symbol, case-insensitive exact, qualified suffix, segmented identifier, and normalized substring in that order. Within a rank sort by symbol, file, line, and id. Return `match_kind`, numeric score, full node, graph fingerprint, stale files, and applied limit.

- [ ] **Step 4: Add CLI and MCP wrappers**

CLI shape:

```rust
Search {
    query: String,
    #[arg(long, default_value_t = 20)] limit: usize,
    #[arg(long, default_value = ".")] code: PathBuf,
    #[arg(long, default_value = ".agent-spec/graph")] graph: PathBuf,
    #[arg(long)] frozen: bool,
}
```

MCP schema accepts `query`, optional `limit`, optional `code`, and optional `graph`; it calls the same library API and serializes the same result.

- [ ] **Step 5: Verify crate, CLI, and MCP tests**

Run:

```bash
cargo test -p rust-atlas
cargo test test_mcp_atlas -- --nocapture
cargo test test_atlas_search -- --nocapture
```

Expected: all commands pass.

- [ ] **Step 6: Commit B1**

```bash
git add crates/rust-atlas/src/index.rs crates/rust-atlas/src/lib.rs src/main.rs src/spec_mcp/tools.rs
git commit -m "feat(atlas): add deterministic indexed symbol search"
```

### Task 8: Graph Identity And Layered Status

**Files:**
- Create: `crates/rust-atlas/src/status.rs`
- Modify: `crates/rust-atlas/src/lib.rs`
- Test: `crates/rust-atlas/src/status.rs`

**Interfaces:**
- Consumes: canonical code root, graph root, Cargo source set, stored SCIP fingerprint, and `rustc -Vv`.
- Produces: `GraphIdentity`, `LayerState`, `LayerStatus`, `AtlasStatus`, `pub fn status(code_root: &Path, graph_dir: &Path) -> Result<AtlasStatus, AtlasError>`.

- [ ] **Step 1: Write failing layer and no-git identity tests**

```rust
#[test]
fn test_atlas_status_reports_fresh_syn_scip_and_unavailable_mir() {
    let (code, graph) = build_current_scip_fixture();
    let status = crate::status(&code, &graph).unwrap();
    assert_eq!(status.syn.state, LayerState::Fresh);
    assert_eq!(status.scip.state, LayerState::Fresh);
    assert_eq!(status.mir.state, LayerState::Unavailable);
}

#[test]
fn test_atlas_identity_falls_back_outside_git() {
    let (code, graph) = build_non_git_fixture();
    let status = crate::status(&code, &graph).unwrap();
    assert_eq!(status.current_identity.worktree_root, canonical(&code));
    assert_eq!(status.current_identity.git_common_dir, None);
}
```

- [ ] **Step 2: Run status tests and confirm types are missing**

Run:

```bash
cargo test -p rust-atlas test_atlas_status_ -- --nocapture
cargo test -p rust-atlas test_atlas_identity_ -- --nocapture
```

Expected: FAIL because status and identity types do not exist.

- [ ] **Step 3: Implement identity capture and layered status**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphIdentity {
    pub repository_root: String,
    pub git_common_dir: Option<String>,
    pub worktree_root: String,
    pub graph_root: String,
    pub toolchain: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerState { Fresh, Stale, Unavailable }
```

Use `git rev-parse --show-toplevel` and `--git-common-dir` when available; canonical code root is the no-git fallback. Capture `rustc -Vv` exactly. Status reports recorded and current identity, mismatch diagnostic, separate layer fingerprints, and syn stale files.

- [ ] **Step 4: Persist explicit SCIP source-set fingerprint**

On explicit `build --scip`, hash the canonical sorted `meta.files` map and store it with the index fingerprint. An automatic refresh that reuses the old index retains the prior source-set fingerprint so status reports SCIP stale after source edits.

- [ ] **Step 5: Run status and crate tests**

Run: `cargo test -p rust-atlas`

Expected: all tests pass.

- [ ] **Step 6: Commit layered status**

```bash
git add crates/rust-atlas/src/status.rs crates/rust-atlas/src/lib.rs
git commit -m "feat(atlas): add graph identity and layered freshness"
```

### Task 9: Worktree Rejection And Shared Freshness Consumers

**Files:**
- Modify: `crates/rust-atlas/src/lib.rs`
- Modify: `src/main.rs`
- Modify: `src/spec_mcp/tools.rs`
- Modify: `src/spec_knowledge/code_graph.rs`
- Modify: `src/spec_verify/atlas_symbols.rs`
- Test: all modified Rust modules

**Interfaces:**
- Consumes: `rust_atlas::status`.
- Produces: `AtlasError::WorktreeMismatch`, CLI `atlas status`, shared status embedded in all query results, and one provider fingerprint contract.

- [ ] **Step 1: Write failing worktree and binding tests**

```rust
#[test]
fn test_atlas_rejects_borrowed_worktree_graph() {
    let fixture = two_worktrees_with_one_graph();
    let error = query(&fixture.second, &fixture.graph, fixture.symbol(), &QueryOptions::default()).unwrap_err();
    assert!(matches!(error, AtlasError::WorktreeMismatch { .. }));
}

#[test]
fn test_code_bindings_block_on_stale_semantic_layer() {
    let result = build_bindings_with_stale_scip_only_symbol();
    assert!(result.unwrap_err().to_string().contains("atlas-stale"));
    assert!(!bindings_path().exists());
}
```

- [ ] **Step 2: Run tests and confirm current false-authority behavior**

Run:

```bash
cargo test test_atlas_rejects_borrowed_worktree_graph -- --nocapture
cargo test test_code_bindings_block_on_stale_semantic_layer -- --nocapture
```

Expected: FAIL because queries and bindings currently accept the graph.

- [ ] **Step 3: Route every consumer through shared status**

Before definitive query/index access, reject identity mismatch. `atlas status` itself returns successfully with both identities and the mismatch diagnostic. Replace provider-local file-hash freshness interpretation with `rust_atlas::status`; provider fingerprint is blake3 over canonical JSON containing schema, identity, toolchain, source set, and layer fingerprints.

- [ ] **Step 4: Preserve schema-mismatch precedence**

Ensure `read_meta` validates schema before status computes identity or freshness. Extend `test_atlas_rejects_mismatched_schema_version` to call status, query, and provider binding and assert `atlas-schema-mismatch`, never `atlas-stale`.

- [ ] **Step 5: Verify all consumers**

Run:

```bash
cargo test -p rust-atlas
cargo test atlas_symbols -- --nocapture
cargo test code_bindings -- --nocapture
cargo test test_mcp_atlas -- --nocapture
```

Expected: all tests pass and all result shapes carry the same serialized `AtlasStatus`.

- [ ] **Step 6: Commit D1**

```bash
git add crates/rust-atlas/src src/main.rs src/spec_mcp/tools.rs src/spec_knowledge/code_graph.rs src/spec_verify/atlas_symbols.rs
git commit -m "feat(atlas): enforce worktree-aware layered freshness"
```

### Task 10: Wave-One Documentation And Completion Gates

**Files:**
- Modify: `docs/atlas-roadmap.md`
- Modify: `docs/atlas-evaluation.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `skills/agent-spec-tool-first/SKILL.md`
- Modify: `CHANGELOG.md`
- Modify: `.agent-spec/wiki/**`

**Interfaces:**
- Consumes: final CLI/MCP APIs and lifecycle evidence.
- Produces: current user guidance, live wiki source trace, and replayable completion evidence for all three requirements.

- [ ] **Step 1: Update docs from actual help output**

Run `cargo run -- atlas --help`, `cargo run -- atlas benchmark --help`, `cargo run -- atlas search --help`, and `cargo run -- atlas status --help`. Copy only commands and options that exist into README, AGENTS, skill guidance, evaluation docs, and wiki pages.

- [ ] **Step 2: Run deterministic code gates**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all commands exit 0 with no warning promoted to an error.

- [ ] **Step 3: Run documentation and governance gates**

```bash
bash scripts/docs-lint.sh
agent-spec lint-knowledge --knowledge knowledge --gate
agent-spec requirements graph --knowledge knowledge --format json --gate
agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
agent-spec wiki check --code . --wiki .agent-spec/wiki
```

Expected: commands exit 0; external docs tools may be skipped only outside CI and must be reported.

- [ ] **Step 4: Run each active contract lifecycle**

```bash
agent-spec lifecycle specs/task-atlas-agent-evaluation.spec.md --code . --format json
agent-spec lifecycle specs/task-atlas-edge-evidence-index.spec.md --code . --format json
agent-spec lifecycle specs/task-atlas-worktree-layered-freshness.spec.md --code . --format json
```

Expected: every scenario is `pass`; no `skip`, `fail`, or `uncertain` verdict.

- [ ] **Step 5: Prove requirement replay and trace**

```bash
agent-spec requirements replay REQ-ATLAS-AGENT-EVALUATION --format text
agent-spec requirements replay REQ-ATLAS-EDGE-EVIDENCE-INDEX --format text
agent-spec requirements replay REQ-ATLAS-WORKTREE-FRESHNESS --format text
agent-spec requirements trace-graph REQ-ATLAS-WORKTREE-FRESHNESS --format mermaid
```

Expected: each replay reaches its work unit, spec scenarios, test selectors, latest passing lifecycle run, code targets, branch/worktree, and VCS reference.

- [ ] **Step 6: Commit verified wave one**

```bash
git add docs README.md AGENTS.md skills CHANGELOG.md .agent-spec/wiki knowledge specs
git commit -m "docs(atlas): publish verified roadmap wave one"
```

## Self-Review Record

- Spec coverage: Tasks 2-3 cover every E0 clause and six contract scenarios; Tasks 4-7 cover every A2/B1 clause and seven scenarios; Tasks 8-9 cover every D1 clause and six scenarios; Task 10 covers lifecycle, replay, docs, and roadmap completion rules.
- Placeholder scan: every implementation step names concrete types, behavior, commands, and expected evidence. Roadmap work outside wave one is assigned to later contracts rather than left as an unspecified step in this plan.
- Type consistency: `EdgeSite`, `ExtractorIdentity`, `DispatchKind`, `EdgeConfidence`, `QueryIndex`, `SearchOptions`, `GraphIdentity`, `LayerState`, and `AtlasStatus` are introduced before their consumers; CLI and MCP wrappers call the same library entry points.
