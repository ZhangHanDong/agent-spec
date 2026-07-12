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

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum AtlasError {
    #[error("atlas-missing-graph: no graph at {graph_dir}; run `atlas build` first")]
    MissingGraph { graph_dir: String },
    #[error("atlas-unknown-symbol: `{symbol}` is not in the graph")]
    UnknownSymbol { symbol: String },
    #[error("atlas-io: {0}")]
    Io(String),
    #[error("atlas-scip: {0}")]
    Scip(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    Crate,
    Module,
    Struct,
    Enum,
    Trait,
    Fn,
    Impl,
    TypeAlias,
    Const,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub visibility: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
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
    let package = package_name(code_root);
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
        let dirty = opts.full || old_files.get(&rel) != Some(&hash);
        if dirty {
            let source = String::from_utf8_lossy(&bytes);
            let shard = extract_shard(&package, &rel, &hash, &source);
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

    let mut capability = Capability::default();
    if let Some(index_path) = &opts.scip_index {
        let tool = overlay_scip(&shards_dir, index_path, &files)?;
        capability.scip = true;
        capability.scip_tool = tool;
    }

    let meta = Meta {
        schema_version: SCHEMA_VERSION,
        package,
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
    let node = shards
        .iter()
        .flat_map(|s| s.nodes.iter())
        .find(|n| n.id == symbol)
        .cloned()
        .ok_or_else(|| AtlasError::UnknownSymbol {
            symbol: symbol.to_string(),
        })?;
    let mut edges_out = BTreeSet::new();
    let mut edges_in = BTreeSet::new();
    for edge in shards.iter().flat_map(|s| s.edges.iter()) {
        if edge.from == symbol {
            edges_out.insert(edge.clone());
        }
        if edge.to == symbol {
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
    let tree = render(&meta.package, &kinds, &children, &mut visited);
    Ok(TreeOutline { tree, stale })
}

/// Incoming reference/call edges for a symbol.
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
        .any(|n| n.id == symbol)
    {
        return Err(AtlasError::UnknownSymbol {
            symbol: symbol.to_string(),
        });
    }
    let edges: BTreeSet<Edge> = shards
        .iter()
        .flat_map(|s| s.edges.iter())
        .filter(|e| e.to == symbol && matches!(e.kind, EdgeKind::References | EdgeKind::Calls))
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
    let matches_name = |id: &str| id == name || id.ends_with(&format!("::{name}"));
    let edges: BTreeSet<Edge> = shards
        .iter()
        .flat_map(|s| s.edges.iter())
        .filter(|e| {
            matches!(e.kind, EdgeKind::ImplsTrait | EdgeKind::ImplFor)
                && (matches_name(&e.to) || matches_name(&e.from))
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
    format!("{}.json", rel.replace(['/', '\\'], "__"))
}

fn write_shard(shards_dir: &Path, shard: &Shard) -> Result<(), AtlasError> {
    write_json(&shards_dir.join(shard_file_name(&shard.file)), shard)
}

fn read_shard(shards_dir: &Path, rel: &str) -> Result<Shard, AtlasError> {
    let path = shards_dir.join(shard_file_name(rel));
    let text = std::fs::read_to_string(&path).map_err(io_err)?;
    serde_json::from_str(&text).map_err(|e| AtlasError::Io(e.to_string()))
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

fn package_name(code_root: &Path) -> String {
    let manifest = code_root.join("Cargo.toml");
    if let Ok(text) = std::fs::read_to_string(manifest) {
        let mut in_package = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_package = line == "[package]";
                continue;
            }
            if in_package
                && let Some(rest) = line.strip_prefix("name")
                && let Some(value) = rest.trim_start().strip_prefix('=')
            {
                return value.trim().trim_matches('"').replace('-', "_");
            }
        }
    }
    code_root
        .file_name()
        .map(|n| n.to_string_lossy().replace('-', "_"))
        .unwrap_or_else(|| "crate".to_string())
}

fn walk_rs_files(code_root: &Path) -> Vec<PathBuf> {
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

fn module_segments(rel: &str) -> Vec<String> {
    let trimmed = rel.strip_prefix("src/").unwrap_or(rel);
    let no_ext = trimmed.strip_suffix(".rs").unwrap_or(trimmed);
    let mut segs: Vec<String> = no_ext.split('/').map(str::to_string).collect();
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
    rel: &'a str,
    source_lines: Vec<&'a str>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    impl_counter: usize,
}

impl ExtractCtx<'_> {
    fn line_of(&self, span: proc_macro2::Span) -> (usize, usize) {
        (span.start().line, span.end().line)
    }

    fn signature_at(&self, line_start: usize) -> String {
        self.source_lines
            .get(line_start.saturating_sub(1))
            .map(|l| l.trim().trim_end_matches('{').trim().to_string())
            .unwrap_or_default()
    }

    fn push_node(&mut self, id: &str, kind: NodeKind, span: proc_macro2::Span, vis: String) {
        let (line_start, line_end) = self.line_of(span);
        self.nodes.push(Node {
            id: id.to_string(),
            kind,
            file: self.rel.to_string(),
            line_start,
            line_end,
            visibility: vis,
            signature: self.signature_at(line_start),
        });
    }

    fn push_contains(&mut self, from: &str, to: &str) {
        self.edges.push(Edge {
            from: from.to_string(),
            to: to.to_string(),
            kind: EdgeKind::Contains,
            provenance: Provenance::Syn,
        });
    }
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
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn type_text(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => path_text(&p.path),
        other => quote_tokens(other),
    }
}

fn quote_tokens<T: syn::spanned::Spanned + std::fmt::Debug>(t: &T) -> String {
    format!("{t:?}").chars().take(48).collect()
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
    if !raw.contains("::") && local_names.contains(raw) {
        return format!("{module_id}::{raw}");
    }
    raw.to_string()
}

fn extract_shard(package: &str, rel: &str, hash: &str, source: &str) -> Shard {
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
        package,
        rel,
        source_lines: source.lines().collect(),
        nodes: Vec::new(),
        edges: Vec::new(),
        impl_counter: 0,
    };

    // crate + module chain (deduped at load time across shards)
    let segs = module_segments(rel);
    ctx.nodes.push(Node {
        id: package.to_string(),
        kind: NodeKind::Crate,
        file: rel.to_string(),
        line_start: 1,
        line_end: 1,
        visibility: "pub".to_string(),
        signature: format!("crate {package}"),
    });
    let mut module_id = package.to_string();
    for seg in &segs {
        let child = format!("{module_id}::{seg}");
        ctx.nodes.push(Node {
            id: child.clone(),
            kind: NodeKind::Module,
            file: rel.to_string(),
            line_start: 1,
            line_end: ctx.source_lines.len().max(1),
            visibility: "pub".to_string(),
            signature: format!("mod {seg}"),
        });
        ctx.push_contains(&module_id, &child);
        module_id = child;
    }

    extract_items(&mut ctx, &parsed.items, &module_id);

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

fn extract_items(ctx: &mut ExtractCtx<'_>, items: &[syn::Item], module_id: &str) {
    use syn::Item;
    use syn::spanned::Spanned;

    let local_names: BTreeSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(i) => Some(i.ident.to_string()),
            Item::Enum(i) => Some(i.ident.to_string()),
            Item::Trait(i) => Some(i.ident.to_string()),
            Item::Fn(i) => Some(i.sig.ident.to_string()),
            Item::Type(i) => Some(i.ident.to_string()),
            Item::Const(i) => Some(i.ident.to_string()),
            _ => None,
        })
        .collect();

    for item in items {
        match item {
            Item::Struct(i) => {
                let id = format!("{module_id}::{}", i.ident);
                ctx.push_node(&id, NodeKind::Struct, i.span(), vis_string(&i.vis));
                ctx.push_contains(module_id, &id);
            }
            Item::Enum(i) => {
                let id = format!("{module_id}::{}", i.ident);
                ctx.push_node(&id, NodeKind::Enum, i.span(), vis_string(&i.vis));
                ctx.push_contains(module_id, &id);
            }
            Item::Trait(i) => {
                let id = format!("{module_id}::{}", i.ident);
                ctx.push_node(&id, NodeKind::Trait, i.span(), vis_string(&i.vis));
                ctx.push_contains(module_id, &id);
                for ti in &i.items {
                    if let syn::TraitItem::Fn(f) = ti {
                        let fid = format!("{id}::{}", f.sig.ident);
                        ctx.push_node(&fid, NodeKind::Fn, f.span(), "pub".to_string());
                        ctx.push_contains(&id, &fid);
                    }
                }
            }
            Item::Fn(i) => {
                let id = format!("{module_id}::{}", i.sig.ident);
                ctx.push_node(&id, NodeKind::Fn, i.span(), vis_string(&i.vis));
                ctx.push_contains(module_id, &id);
            }
            Item::Type(i) => {
                let id = format!("{module_id}::{}", i.ident);
                ctx.push_node(&id, NodeKind::TypeAlias, i.span(), vis_string(&i.vis));
                ctx.push_contains(module_id, &id);
            }
            Item::Const(i) => {
                let id = format!("{module_id}::{}", i.ident);
                ctx.push_node(&id, NodeKind::Const, i.span(), vis_string(&i.vis));
                ctx.push_contains(module_id, &id);
            }
            Item::Macro(i) => {
                if let Some(ident) = &i.ident {
                    // macro_rules! export at crate root by convention
                    let id = format!("{}::{ident}", ctx.package);
                    ctx.push_node(&id, NodeKind::Macro, i.span(), "pub".to_string());
                    ctx.push_contains(ctx.package, &id);
                }
            }
            Item::Impl(i) => {
                ctx.impl_counter += 1;
                let self_ty =
                    resolve_path(&type_text(&i.self_ty), module_id, ctx.package, &local_names);
                let (impl_id, trait_id) = match &i.trait_ {
                    Some((_, path, _)) => {
                        let trait_id =
                            resolve_path(&path_text(path), module_id, ctx.package, &local_names);
                        (
                            format!("{module_id}::impl {trait_id} for {self_ty}"),
                            Some(trait_id),
                        )
                    }
                    None => (format!("{module_id}::impl {self_ty}"), None),
                };
                ctx.push_node(&impl_id, NodeKind::Impl, i.span(), "private".to_string());
                ctx.push_contains(module_id, &impl_id);
                if let Some(trait_id) = trait_id {
                    ctx.edges.push(Edge {
                        from: impl_id.clone(),
                        to: trait_id,
                        kind: EdgeKind::ImplsTrait,
                        provenance: Provenance::Syn,
                    });
                }
                ctx.edges.push(Edge {
                    from: impl_id.clone(),
                    to: self_ty,
                    kind: EdgeKind::ImplFor,
                    provenance: Provenance::Syn,
                });
                for ii in &i.items {
                    if let syn::ImplItem::Fn(f) = ii {
                        let fid = format!("{impl_id}::{}", f.sig.ident);
                        ctx.push_node(&fid, NodeKind::Fn, f.span(), vis_string(&f.vis));
                        ctx.push_contains(&impl_id, &fid);
                    }
                }
            }
            Item::Mod(i) => {
                if let Some((_, nested)) = &i.content {
                    let child = format!("{module_id}::{}", i.ident);
                    ctx.push_node(&child, NodeKind::Module, i.span(), vis_string(&i.vis));
                    ctx.push_contains(module_id, &child);
                    extract_items(ctx, nested, &child);
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
    // recompute scip edges from scratch
    for shard in shards.values_mut() {
        shard.edges.retain(|e| e.provenance != Provenance::Scip);
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
    let mut changed: BTreeSet<String> = BTreeSet::new();
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
            .find(|n| n.id == id)
    }

    fn all_edges(shards: &[Shard]) -> Vec<Edge> {
        shards.iter().flat_map(|s| s.edges.clone()).collect()
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

        assert!(node(&shards, "atlas_basic").is_some(), "crate node");
        assert!(node(&shards, "atlas_basic::store").is_some(), "module node");
        assert!(node(&shards, "atlas_basic::store::Kind").is_some());
        assert!(node(&shards, "atlas_basic::store::Store").is_some());
        assert!(node(&shards, "atlas_basic::store::LIMIT").is_some());
        assert!(node(&shards, "atlas_basic::store::Alias").is_some());
        assert!(node(&shards, "atlas_basic::mk_store").is_some(), "macro");
        assert!(node(&shards, "atlas_basic::open_default").is_some());

        let edges = all_edges(&shards);
        assert!(edges.iter().any(|e| e.kind == EdgeKind::Contains
            && e.from == "atlas_basic"
            && e.to == "atlas_basic::store"
            && e.provenance == Provenance::Syn));
        assert!(edges.iter().any(|e| e.kind == EdgeKind::Contains
            && e.from == "atlas_basic::store"
            && e.to == "atlas_basic::store::MemStore"));
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
        assert!(edges.iter().any(|e| e.kind == EdgeKind::ImplsTrait
            && e.from == impl_node.id
            && e.to == "atlas_basic::store::Store"
            && e.provenance == Provenance::Syn));
        assert!(edges.iter().any(|e| e.kind == EdgeKind::ImplFor
            && e.from == impl_node.id
            && e.to == "atlas_basic::store::MemStore"
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
            result
                .edges_in
                .iter()
                .any(|e| e.kind == EdgeKind::Contains && e.from == "atlas_basic::store")
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
                && e.from == "atlas_basic::service::run"
                && e.to == "atlas_basic::store::MemStore"),
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
}
