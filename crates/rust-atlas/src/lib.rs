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

pub const SCHEMA_VERSION: u32 = 3;

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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub target_text: Option<String>,
    pub resolution: EdgeResolution,
    pub kind: EdgeKind,
    pub provenance: Provenance,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub schema_version: u32,
    pub package: String,
    pub packages: Vec<String>,
    pub roots: Vec<String>,
    pub capability: Capability,
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    pub full: bool,
    pub scip_index: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildReport {
    pub rebuilt: Vec<String>,
    pub removed: Vec<String>,
    pub unparsed: Vec<String>,
    pub capability: Capability,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub node: Node,
    pub edges_out: Vec<Edge>,
    pub edges_in: Vec<Edge>,
    pub stale: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreeOutline {
    pub tree: serde_json::Value,
    pub stale: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeReport {
    pub symbol: String,
    pub edges: Vec<Edge>,
    pub stale: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    pub frozen: bool,
}

/// Build (or incrementally refresh) the graph for `code_root` into `graph_dir`.
pub fn build(
    code_root: &Path,
    graph_dir: &Path,
    opts: &BuildOptions,
) -> Result<BuildReport, AtlasError> {
    // `cargo metadata` reports absolute, canonical paths; canonicalize the root
    // so the file walk and layout share one path space (otherwise `--code .`
    // yields relative walk paths that never match the absolute target dirs).
    let code_root = &std::fs::canonicalize(code_root).map_err(io_err)?;
    let layout = ProjectLayout::discover(code_root)?;
    let shards_dir = graph_dir.join("shards");
    std::fs::create_dir_all(&shards_dir).map_err(io_err)?;

    let old_meta = read_meta(graph_dir).ok();
    let old_files = old_meta
        .as_ref()
        .map(|m| m.files.clone())
        .unwrap_or_default();

    let mut files = BTreeMap::new();
    let mut rebuilt = Vec::new();
    let mut unparsed = Vec::new();
    for path in walk_rs_files(code_root) {
        let rel = rel_path(code_root, &path);
        let bytes = std::fs::read(&path).map_err(io_err)?;
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
            let _ = std::fs::remove_file(shards_dir.join(shard_file_name(rel)));
            removed.push(rel.clone());
        }
    }

    resolve_syn_edges(&shards_dir, &files)?;

    let mut capability = Capability::default();
    if let Some(index_path) = &opts.scip_index {
        let tool = overlay_scip(&shards_dir, index_path, &files)?;
        capability.scip = true;
        capability.scip_tool = tool;
    } else {
        remove_scip_edges(&shards_dir, &files)?;
    }
    validate_graph(&shards_dir, &files)?;

    let meta = Meta {
        schema_version: SCHEMA_VERSION,
        package: layout.graph_root.clone(),
        packages: layout.packages.clone(),
        roots: layout.roots.clone(),
        capability: capability.clone(),
        files,
    };
    write_json(&graph_dir.join("meta.json"), &meta)?;
    Ok(BuildReport {
        rebuilt,
        removed,
        unparsed,
        capability,
    })
}

/// Report stale shard source files (content hash mismatch, deleted, or new).
pub fn check(code_root: &Path, graph_dir: &Path) -> Result<Vec<String>, AtlasError> {
    // Match `build`'s path space so staleness keys line up for any `code_root`
    // form (relative, `.`, or with symlinks).
    let code_root = &std::fs::canonicalize(code_root).map_err(io_err)?;
    let meta = read_meta(graph_dir)?;
    let mut stale = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for path in walk_rs_files(code_root) {
        let rel = rel_path(code_root, &path);
        let bytes = std::fs::read(&path).map_err(io_err)?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        match meta.files.get(&rel) {
            Some(recorded) if *recorded == hash => {}
            _ => {
                stale.insert(rel.clone());
            }
        }
        seen.insert(rel);
    }
    for rel in meta.files.keys() {
        if !seen.contains(rel) {
            stale.insert(rel.clone());
        }
    }
    Ok(stale.into_iter().collect())
}

/// Node facts plus adjacent edges for a canonical symbol path.
pub fn query(
    code_root: &Path,
    graph_dir: &Path,
    symbol: &str,
    opts: &QueryOptions,
) -> Result<QueryResult, AtlasError> {
    let stale = refresh(code_root, graph_dir, opts)?;
    let (_, shards) = load_graph(graph_dir)?;
    let matches: Vec<&Node> = shards
        .iter()
        .flat_map(|s| s.nodes.iter())
        .filter(|node| node.id == symbol || node.symbol == symbol)
        .collect();
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
    let mut edges_out = BTreeSet::new();
    let mut edges_in = BTreeSet::new();
    for edge in shards.iter().flat_map(|s| s.edges.iter()) {
        if edge.from == node.id {
            edges_out.insert(edge.clone());
        }
        if edge.to == node.id {
            edges_in.insert(edge.clone());
        }
    }
    Ok(QueryResult {
        node,
        edges_out: edges_out.into_iter().collect(),
        edges_in: edges_in.into_iter().collect(),
        stale,
    })
}

/// Deterministic module outline of the whole graph.
pub fn tree(
    code_root: &Path,
    graph_dir: &Path,
    opts: &QueryOptions,
) -> Result<TreeOutline, AtlasError> {
    let stale = refresh(code_root, graph_dir, opts)?;
    let (meta, shards) = load_graph(graph_dir)?;
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
    Ok(TreeOutline { tree, stale })
}

/// Incoming reference/call edges for a symbol.
///
/// `References`/`Calls` edges are produced only by the semantic (SCIP) overlay;
/// the syn baseline emits none. Without a `--scip` index this therefore returns
/// an empty edge set for every symbol — that is "no semantic layer", not "no
/// references". See `overlay_scip`.
pub fn refs(
    code_root: &Path,
    graph_dir: &Path,
    symbol: &str,
    opts: &QueryOptions,
) -> Result<EdgeReport, AtlasError> {
    let stale = refresh(code_root, graph_dir, opts)?;
    let (_, shards) = load_graph(graph_dir)?;
    if !shards
        .iter()
        .flat_map(|s| s.nodes.iter())
        .any(|node| node.id == symbol || node.symbol == symbol)
    {
        return Err(AtlasError::UnknownSymbol {
            symbol: symbol.to_string(),
        });
    }
    let edges: BTreeSet<Edge> = shards
        .iter()
        .flat_map(|s| s.edges.iter())
        .filter(|e| {
            matches!(e.kind, EdgeKind::References | EdgeKind::Calls)
                && shards
                    .iter()
                    .flat_map(|shard| &shard.nodes)
                    .any(|node| node.id == e.to && (node.id == symbol || node.symbol == symbol))
        })
        .cloned()
        .collect();
    Ok(EdgeReport {
        symbol: symbol.to_string(),
        edges: edges.into_iter().collect(),
        stale,
    })
}

/// Impl relations touching a trait or type name.
pub fn impls(
    code_root: &Path,
    graph_dir: &Path,
    name: &str,
    opts: &QueryOptions,
) -> Result<EdgeReport, AtlasError> {
    let stale = refresh(code_root, graph_dir, opts)?;
    let (_, shards) = load_graph(graph_dir)?;
    let matching_ids: BTreeSet<&str> = shards
        .iter()
        .flat_map(|shard| &shard.nodes)
        .filter(|node| node.symbol == name || node.symbol.ends_with(&format!("::{name}")))
        .map(|node| node.id.as_str())
        .collect();
    // Also match edges whose target is still unresolved: the trait/type name
    // survives in `target_text` (or `to`) even when the syn layer could not map
    // it to a node id. Without this, `impls <Trait>` returns nothing for any
    // trait referenced by an imported bare name.
    let text_matches = |value: &str| value == name || value.ends_with(&format!("::{name}"));
    let edges: BTreeSet<Edge> = shards
        .iter()
        .flat_map(|s| s.edges.iter())
        .filter(|e| {
            matches!(e.kind, EdgeKind::ImplsTrait | EdgeKind::ImplFor)
                && (matching_ids.contains(e.to.as_str())
                    || matching_ids.contains(e.from.as_str())
                    || e.target_text.as_deref().is_some_and(text_matches)
                    || text_matches(&e.to))
        })
        .cloned()
        .collect();
    Ok(EdgeReport {
        symbol: name.to_string(),
        edges: edges.into_iter().collect(),
        stale,
    })
}

/// Load every shard plus meta (internal + MCP convenience).
pub fn load_graph(graph_dir: &Path) -> Result<(Meta, Vec<Shard>), AtlasError> {
    let meta = read_meta(graph_dir)?;
    let mut shards = Vec::new();
    for rel in meta.files.keys() {
        shards.push(read_shard(&graph_dir.join("shards"), rel)?);
    }
    Ok((meta, shards))
}

// ── internals ───────────────────────────────────────────────────────

fn io_err(error: std::io::Error) -> AtlasError {
    AtlasError::Io(error.to_string())
}

fn read_meta(graph_dir: &Path) -> Result<Meta, AtlasError> {
    let path = graph_dir.join("meta.json");
    let text = std::fs::read_to_string(&path).map_err(|_| AtlasError::MissingGraph {
        graph_dir: graph_dir.display().to_string(),
    })?;
    serde_json::from_str(&text).map_err(|e| AtlasError::Io(e.to_string()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AtlasError> {
    let mut text =
        serde_json::to_string_pretty(value).map_err(|e| AtlasError::Io(e.to_string()))?;
    text.push('\n');
    std::fs::write(path, text).map_err(io_err)
}

fn shard_file_name(rel: &str) -> String {
    let escaped = rel
        .replace('%', "%25")
        .replace('/', "%2F")
        .replace('\\', "%5C");
    format!("{escaped}.json")
}

fn write_shard(shards_dir: &Path, shard: &Shard) -> Result<(), AtlasError> {
    write_json(&shards_dir.join(shard_file_name(&shard.file)), shard)
}

fn read_shard(shards_dir: &Path, rel: &str) -> Result<Shard, AtlasError> {
    let path = shards_dir.join(shard_file_name(rel));
    let text = std::fs::read_to_string(&path).map_err(io_err)?;
    serde_json::from_str(&text).map_err(|e| AtlasError::Io(e.to_string()))
}

fn resolve_syn_edges(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
) -> Result<(), AtlasError> {
    let mut shards = BTreeMap::new();
    let mut symbols: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Bare-name index: last path segment -> set of fully-qualified symbols that
    // end in `::<segment>`. Lets us resolve `use`-imported bare names (e.g. a
    // trait referenced as `SpecLinter` whose symbol is
    // `crate::spec_lint::SpecLinter`) when there is exactly one candidate.
    let mut by_last: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for rel in files.keys() {
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
        shards.insert(rel.clone(), shard);
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

    for (rel, shard) in &mut shards {
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
                }
                _ => match resolve_bare(&target_text) {
                    Some(id) => {
                        edge.to = id;
                        edge.resolution = EdgeResolution::Resolved;
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
                    }
                },
            }
        }
        shard.edges.sort();
        shard.edges.dedup();
        if shard.edges != before {
            write_shard(shards_dir, shard)?;
        }
        let _ = rel;
    }
    Ok(())
}

fn validate_graph(shards_dir: &Path, files: &BTreeMap<String, String>) -> Result<(), AtlasError> {
    let mut node_ids = BTreeSet::new();
    let mut shards = Vec::new();
    for rel in files.keys() {
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
        shards.push(shard);
    }
    for shard in &shards {
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
    Ok(())
}

fn refresh(
    code_root: &Path,
    graph_dir: &Path,
    opts: &QueryOptions,
) -> Result<Vec<String>, AtlasError> {
    let stale = check(code_root, graph_dir)?;
    if stale.is_empty() {
        return Ok(Vec::new());
    }
    if opts.frozen {
        return Ok(stale);
    }
    build(code_root, graph_dir, &BuildOptions::default())?;
    Ok(Vec::new())
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

#[derive(Debug, Clone)]
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

fn cargo_metadata(manifest: &Path) -> Result<CargoMetadata, AtlasError> {
    let current_dir = manifest.parent().unwrap_or_else(|| Path::new("."));
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(manifest)
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

fn nested_workspace_manifests(code_root: &Path) -> Vec<PathBuf> {
    let root_manifest = code_root.join("Cargo.toml");
    let excludes = workspace_excludes(code_root);
    let mut manifests: Vec<PathBuf> = ignore::WalkBuilder::new(code_root)
        .hidden(true)
        .git_ignore(true)
        .build()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path != &root_manifest
                && path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml")
                && !path
                    .components()
                    .any(|component| component.as_os_str() == "target")
                // Respect the root workspace's `exclude`: a nested workspace that
                // the root deliberately detaches (e.g. test fixtures) must not be
                // pulled back into the graph.
                && !excludes
                    .iter()
                    .any(|dir| path.starts_with(dir))
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
    fn discover(code_root: &Path) -> Result<Self, AtlasError> {
        let manifest = code_root.join("Cargo.toml");
        let root_metadata = cargo_metadata(&manifest)?;
        let root_workspace = root_metadata.workspace_root.clone();
        let mut metadata_sets = vec![root_metadata];
        let mut known_manifests: BTreeSet<PathBuf> = metadata_sets[0]
            .packages
            .iter()
            .map(|package| package.manifest_path.clone())
            .collect();
        for nested_manifest in nested_workspace_manifests(code_root) {
            if known_manifests.contains(&nested_manifest) {
                continue;
            }
            let metadata = cargo_metadata(&nested_manifest)?;
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
                    });
                }
                ctx.edges.push(Edge {
                    from: impl_id.clone(),
                    to: self_target.clone(),
                    target_text: Some(self_target),
                    resolution: EdgeResolution::Unresolved,
                    kind: EdgeKind::ImplFor,
                    provenance: Provenance::Syn,
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

#[derive(Deserialize)]
struct ScipIndex {
    #[serde(default)]
    metadata: Option<ScipMetadata>,
    #[serde(default)]
    documents: Vec<ScipDocument>,
}

#[derive(Deserialize)]
struct ScipMetadata {
    #[serde(default)]
    tool_info: Option<ScipToolInfo>,
}

#[derive(Deserialize)]
struct ScipToolInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
}

#[derive(Deserialize)]
struct ScipDocument {
    relative_path: String,
    #[serde(default)]
    occurrences: Vec<ScipOccurrence>,
}

#[derive(Deserialize)]
struct ScipOccurrence {
    symbol: String,
    #[serde(default)]
    symbol_roles: u32,
    range: Vec<u32>,
}

/// Overlay resolved `references` edges from a SCIP JSON index onto the shards.
fn overlay_scip(
    shards_dir: &Path,
    index_path: &Path,
    files: &BTreeMap<String, String>,
) -> Result<Option<String>, AtlasError> {
    let text = std::fs::read_to_string(index_path)
        .map_err(|e| AtlasError::Scip(format!("cannot read {}: {e}", index_path.display())))?;
    let index: ScipIndex =
        serde_json::from_str(&text).map_err(|e| AtlasError::Scip(e.to_string()))?;

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

    // pass 1: definitions
    let mut defs: BTreeMap<String, String> = BTreeMap::new();
    for doc in &index.documents {
        let Some(shard) = shards.get(&doc.relative_path) else {
            continue;
        };
        for occ in &doc.occurrences {
            if occ.symbol_roles & 1 == 1
                && let Some(line) = occ.range.first()
                && let Some(node_id) = containing_node(shard, *line as usize + 1)
            {
                defs.insert(occ.symbol.clone(), node_id);
            }
        }
    }
    // pass 2: references
    for doc in &index.documents {
        let rel = doc.relative_path.clone();
        for occ in &doc.occurrences {
            if occ.symbol_roles & 1 == 1 {
                continue;
            }
            let Some(target) = defs.get(&occ.symbol) else {
                continue;
            };
            let Some(shard) = shards.get(&rel) else {
                continue;
            };
            let Some(line) = occ.range.first() else {
                continue;
            };
            let Some(from) = containing_node(shard, *line as usize + 1) else {
                continue;
            };
            if from == *target {
                continue;
            }
            let edge = Edge {
                from,
                to: target.clone(),
                target_text: Some(occ.symbol.clone()),
                resolution: EdgeResolution::Resolved,
                kind: EdgeKind::References,
                provenance: Provenance::Scip,
            };
            if let Some(shard) = shards.get_mut(&rel)
                && !shard.edges.contains(&edge)
            {
                shard.edges.push(edge);
                changed.insert(rel.clone());
            }
        }
    }
    for rel in changed {
        if let Some(shard) = shards.get_mut(&rel) {
            shard.edges.sort();
            shard.edges.dedup();
            write_shard(shards_dir, shard)?;
        }
    }
    Ok(index
        .metadata
        .and_then(|m| m.tool_info)
        .map(|t| format!("{} {}", t.name, t.version)))
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas/basic")
    }

    fn scip_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas/scip/index.json")
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

    fn node<'a>(shards: &'a [Shard], id: &str) -> Option<&'a Node> {
        shards
            .iter()
            .flat_map(|s| s.nodes.iter())
            .find(|node| node.id == id || node.symbol == id)
    }

    fn all_edges(shards: &[Shard]) -> Vec<Edge> {
        shards.iter().flat_map(|s| s.edges.clone()).collect()
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
    fn test_atlas_incremental_rebuild_only_dirty_shards() {
        let (code, graph) = copy_fixture("atlas-incremental");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let snapshot: BTreeMap<String, Vec<u8>> = fs::read_dir(graph.join("shards"))
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
            let now = fs::read(graph.join("shards").join(name)).unwrap();
            if name.contains("service") {
                assert_ne!(&now, bytes, "dirty shard must be rewritten");
            } else {
                assert_eq!(&now, bytes, "untouched shard must stay byte-identical");
            }
        }
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_rebuilds_unchanged_file_when_module_path_changes() {
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
    fn test_atlas_frozen_query_reports_stale_warning() {
        let (code, graph) = copy_fixture("atlas-frozen");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let service = code.join("src/service.rs");
        let mut text = fs::read_to_string(&service).unwrap();
        text.push_str("\npub fn extra() -> usize {\n    2\n}\n");
        fs::write(&service, text).unwrap();

        let shard_bytes: Vec<Vec<u8>> = fs::read_dir(graph.join("shards"))
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

        let shard_bytes_after: Vec<Vec<u8>> = fs::read_dir(graph.join("shards"))
            .unwrap()
            .map(|e| fs::read(e.unwrap().path()).unwrap())
            .collect();
        assert_eq!(shard_bytes, shard_bytes_after, "frozen must not rewrite");
        fs::remove_dir_all(code.parent().unwrap()).ok();
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
    fn test_atlas_build_without_scip_removes_previous_overlay() {
        let (code, graph) = copy_fixture("atlas-scip-remove-overlay");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
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
}
