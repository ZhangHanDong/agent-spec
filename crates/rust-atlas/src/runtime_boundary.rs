use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use proc_macro2::Span;
use quote::ToTokens;
use serde::Serialize;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::flow::FlowQuery;
use crate::traversal::{EndpointResolution, edge_targets, resolve_endpoint};
use crate::{
    AtlasError, AtlasStatus, EdgeConfidence, EdgeKind, EdgeSite, LayerState, Meta, Node, NodeKind,
    Provenance, QueryIndex, TraversalLimits,
};

const MAX_SCAN_NODES: usize = 8;
const MAX_SCAN_BYTES: usize = 200_000;
const MAX_HINTS: usize = 4;
const MAX_CANDIDATES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeBoundaryMechanism {
    AsyncTask,
    Channel,
    CallbackRegistry,
    Reflection,
    FrameworkRoute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeBoundaryAuthority {
    QueryHint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeBoundaryHint {
    pub source: Node,
    pub site: EdgeSite,
    pub mechanism: RuntimeBoundaryMechanism,
    pub form: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub candidate_texts: Vec<String>,
    pub candidates: Vec<Node>,
    pub candidates_truncated: bool,
    pub authority: RuntimeBoundaryAuthority,
    pub confidence: EdgeConfidence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RuntimeBoundaryProjection {
    pub hints: Vec<RuntimeBoundaryHint>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundaryMatch {
    mechanism: RuntimeBoundaryMechanism,
    site: EdgeSite,
    form: String,
    key: Option<String>,
    candidate_texts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateNamespace {
    Callable,
    Type,
}

pub(crate) fn project_runtime_boundaries(
    code_root: &Path,
    meta: &Meta,
    index: &QueryIndex,
    status: &AtlasStatus,
    query: &FlowQuery,
    limits: TraversalLimits,
) -> RuntimeBoundaryProjection {
    let Ok(canonical_root) = std::fs::canonicalize(code_root) else {
        return RuntimeBoundaryProjection::default();
    };
    let (nodes, source_bytes, mut truncated) =
        scan_nodes(&canonical_root, meta, index, status, query, limits);

    let mut hints = Vec::new();
    let node_count = nodes.len();
    for (node_index, node) in nodes.into_iter().enumerate() {
        let Some(bytes) = source_bytes.get(&node.file).and_then(Option::as_ref) else {
            continue;
        };
        let Ok(source) = std::str::from_utf8(bytes) else {
            continue;
        };
        let Ok(matches) = scan_syntax(source, &node.file, &node) else {
            continue;
        };
        for detected in matches {
            if hints.len() == MAX_HINTS {
                truncated = true;
                break;
            }
            let namespace = match detected.mechanism {
                RuntimeBoundaryMechanism::Reflection => CandidateNamespace::Type,
                RuntimeBoundaryMechanism::AsyncTask
                | RuntimeBoundaryMechanism::Channel
                | RuntimeBoundaryMechanism::CallbackRegistry
                | RuntimeBoundaryMechanism::FrameworkRoute => CandidateNamespace::Callable,
            };
            let (candidates, candidates_truncated) =
                resolve_candidates(index, &node, &detected.candidate_texts, namespace);
            hints.push(RuntimeBoundaryHint {
                source: node.clone(),
                site: detected.site,
                mechanism: detected.mechanism,
                form: detected.form,
                key: detected.key,
                candidate_texts: detected.candidate_texts,
                candidates,
                candidates_truncated,
                authority: RuntimeBoundaryAuthority::QueryHint,
                confidence: EdgeConfidence::Heuristic,
            });
        }
        if hints.len() == MAX_HINTS {
            if node_index + 1 < node_count {
                truncated = true;
            }
            break;
        }
    }
    hints.sort_by(|left, right| {
        left.site
            .cmp(&right.site)
            .then_with(|| left.mechanism.cmp(&right.mechanism))
            .then_with(|| left.source.id.cmp(&right.source.id))
    });
    hints.dedup();
    RuntimeBoundaryProjection { hints, truncated }
}

fn scan_nodes(
    canonical_root: &Path,
    meta: &Meta,
    index: &QueryIndex,
    status: &AtlasStatus,
    query: &FlowQuery,
    limits: TraversalLimits,
) -> (Vec<Node>, BTreeMap<String, Option<Vec<u8>>>, bool) {
    let start = match query {
        FlowQuery::Between { from, .. } => from,
        FlowQuery::Through { symbol } => symbol,
    };
    let EndpointResolution::Found(start) = resolve_endpoint(index, start) else {
        return (Vec::new(), BTreeMap::new(), false);
    };

    let mut layer = vec![start.clone()];
    let mut visited = BTreeSet::from([start.id.clone()]);
    let mut reachable = Vec::new();
    let mut source_bytes = BTreeMap::new();
    let mut bytes_read = 0;
    let mut expansions = 0;
    let mut truncated = false;
    let mut depth = 0;
    'frontier: while !layer.is_empty() {
        layer.sort_by(|left, right| left.id.cmp(&right.id));
        layer.dedup_by(|left, right| left.id == right.id);
        let mut next_layer = BTreeMap::new();
        for (position, node) in layer.iter().enumerate() {
            if expansions == limits.max_expansions {
                truncated = true;
                break 'frontier;
            }
            expansions += 1;
            match cache_fresh_source_bytes(
                canonical_root,
                meta,
                node,
                &mut source_bytes,
                &mut bytes_read,
            ) {
                SourceLoad::Fresh => {}
                SourceLoad::Invalid => continue,
                SourceLoad::BudgetExceeded => {
                    truncated = true;
                    break 'frontier;
                }
            }
            if node.kind == NodeKind::Fn {
                reachable.push((depth, node.clone()));
            }
            if matches!(query, FlowQuery::Through { .. }) {
                break 'frontier;
            }
            let neighbors = index
                .outgoing_edges_for(&node.id)
                .filter(|edge| edge_layer_is_fresh(edge.provenance, status))
                .flat_map(|edge| edge_targets(index, edge))
                .map(|(target, _)| target)
                .filter(|target| !visited.contains(&target.id))
                .map(|target| (target.id.clone(), target))
                .collect::<BTreeMap<_, _>>();
            if reachable.len() == MAX_SCAN_NODES {
                truncated |=
                    position + 1 < layer.len() || !neighbors.is_empty() || !next_layer.is_empty();
                break 'frontier;
            }
            if depth == limits.max_depth {
                truncated |= !neighbors.is_empty();
                continue;
            }
            next_layer.extend(neighbors);
        }
        if depth == limits.max_depth {
            break;
        }
        for id in next_layer.keys() {
            visited.insert(id.clone());
        }
        layer = next_layer.into_values().collect();
        depth += 1;
    }
    reachable.sort_by(|(left_depth, left), (right_depth, right)| {
        left_depth
            .cmp(right_depth)
            .then_with(|| left.id.cmp(&right.id))
    });
    (
        reachable.into_iter().map(|(_, node)| node).collect(),
        source_bytes,
        truncated,
    )
}

fn edge_layer_is_fresh(provenance: Provenance, status: &AtlasStatus) -> bool {
    match provenance {
        Provenance::Syn => true,
        Provenance::Scip => status.scip.state == LayerState::Fresh,
        Provenance::Mir => status.mir.state == LayerState::Fresh,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceLoad {
    Fresh,
    Invalid,
    BudgetExceeded,
}

fn cache_fresh_source_bytes(
    canonical_root: &Path,
    meta: &Meta,
    node: &Node,
    cache: &mut BTreeMap<String, Option<Vec<u8>>>,
    bytes_read: &mut usize,
) -> SourceLoad {
    if let Some(bytes) = cache.get(&node.file) {
        return if bytes.is_some() {
            SourceLoad::Fresh
        } else {
            SourceLoad::Invalid
        };
    }
    let relative = Path::new(&node.file);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        cache.insert(node.file.clone(), None);
        return SourceLoad::Invalid;
    }
    let path = canonical_root.join(relative);
    let Ok(canonical) = std::fs::canonicalize(&path) else {
        cache.insert(node.file.clone(), None);
        return SourceLoad::Invalid;
    };
    if !canonical.starts_with(canonical_root) {
        cache.insert(node.file.clone(), None);
        return SourceLoad::Invalid;
    }
    let Ok(file_bytes) = std::fs::metadata(&canonical)
        .ok()
        .and_then(|metadata| usize::try_from(metadata.len()).ok())
        .ok_or(())
    else {
        cache.insert(node.file.clone(), None);
        return SourceLoad::Invalid;
    };
    if bytes_read.saturating_add(file_bytes) > MAX_SCAN_BYTES {
        return SourceLoad::BudgetExceeded;
    }
    let Ok(bytes) = std::fs::read(canonical) else {
        cache.insert(node.file.clone(), None);
        return SourceLoad::Invalid;
    };
    if bytes_read.saturating_add(bytes.len()) > MAX_SCAN_BYTES {
        return SourceLoad::BudgetExceeded;
    }
    *bytes_read += bytes.len();
    let Some(expected) = meta.files.get(&node.file) else {
        cache.insert(node.file.clone(), None);
        return SourceLoad::Invalid;
    };
    let fresh = blake3::hash(&bytes).to_hex().as_str() == expected;
    cache.insert(node.file.clone(), fresh.then_some(bytes));
    if fresh {
        SourceLoad::Fresh
    } else {
        SourceLoad::Invalid
    }
}

fn resolve_candidates(
    index: &QueryIndex,
    source: &Node,
    texts: &[String],
    namespace: CandidateNamespace,
) -> (Vec<Node>, bool) {
    let mut nodes = BTreeMap::new();
    for text in texts {
        let queries = candidate_lookup_queries(index, source, text);
        let exact = queries
            .iter()
            .flat_map(|query| index.matching_nodes(query))
            .filter(|node| node_in_candidate_namespace(node, namespace))
            .map(|node| (node.id.clone(), node))
            .collect::<BTreeMap<_, _>>();
        if !exact.is_empty() {
            for node in exact.into_values() {
                nodes.entry(node.id.clone()).or_insert_with(|| node.clone());
            }
            continue;
        }

        let associated = if namespace == CandidateNamespace::Callable {
            inherent_callable_lookup_queries(index, &queries)
        } else {
            Vec::new()
        };
        let associated_exact = associated
            .iter()
            .flat_map(|query| index.matching_nodes(query))
            .filter(|node| node_in_candidate_namespace(node, namespace))
            .map(|node| (node.id.clone(), node))
            .collect::<BTreeMap<_, _>>();
        if !associated_exact.is_empty() {
            for node in associated_exact.into_values() {
                nodes.entry(node.id.clone()).or_insert_with(|| node.clone());
            }
            continue;
        }

        for query in queries.into_iter().chain(associated) {
            let matches = index.nodes_with_symbol_suffix(&query);
            for node in matches
                .into_iter()
                .filter(|node| node_in_candidate_namespace(node, namespace))
            {
                nodes.entry(node.id.clone()).or_insert_with(|| node.clone());
            }
        }
    }
    if nodes.len() > MAX_CANDIDATES {
        return (Vec::new(), true);
    }
    (nodes.into_values().collect(), false)
}

fn node_in_candidate_namespace(node: &Node, namespace: CandidateNamespace) -> bool {
    match namespace {
        CandidateNamespace::Callable => node.kind == NodeKind::Fn,
        CandidateNamespace::Type => matches!(
            node.kind,
            NodeKind::Struct
                | NodeKind::Enum
                | NodeKind::Union
                | NodeKind::Trait
                | NodeKind::TraitAlias
                | NodeKind::TypeAlias
        ),
    }
}

fn inherent_callable_lookup_queries(index: &QueryIndex, queries: &[String]) -> Vec<String> {
    let mut results = BTreeSet::new();
    for (type_path, member) in queries.iter().filter_map(|query| query.rsplit_once("::")) {
        let type_ids = index
            .matching_nodes(type_path)
            .into_iter()
            .filter(|node| {
                node_in_candidate_namespace(node, CandidateNamespace::Type)
                    && node.kind != NodeKind::Trait
                    && node.kind != NodeKind::TraitAlias
            })
            .map(|node| node.id.as_str())
            .collect::<BTreeSet<_>>();
        for implementation in index.edges.iter().filter(|edge| {
            edge.kind == EdgeKind::ImplFor
                && edge.resolution == crate::EdgeResolution::Resolved
                && type_ids.contains(edge.to.as_str())
        }) {
            for edge in index.outgoing_edges_for(&implementation.from) {
                if edge.kind != EdgeKind::Contains {
                    continue;
                }
                let Some(node) = index.node_by_id(&edge.to) else {
                    continue;
                };
                if node.kind == NodeKind::Fn && node.symbol.rsplit("::").next() == Some(member) {
                    results.insert(node.id.clone());
                }
            }
        }
    }
    results.into_iter().collect()
}

fn candidate_lookup_queries(index: &QueryIndex, source: &Node, text: &str) -> Vec<String> {
    let parsed = syn::parse_str::<syn::ExprPath>(text).ok();
    if let Some(parsed) = parsed.as_ref()
        && parsed.qself.is_some()
    {
        return qualified_self_lookup_queries(index, source, parsed);
    }
    let parsed_type = parsed.is_none().then(|| {
        syn::parse_str::<syn::TypePath>(text)
            .ok()
            .filter(|path| path.qself.is_none())
    });
    let path = parsed
        .as_ref()
        .map(|path| {
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            parsed_type.flatten().map(|path| {
                path.path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_else(|| {
            text.split("::")
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect()
        });
    let Some(first) = path.first().map(String::as_str) else {
        return Vec::new();
    };

    let (container, module, package) = source_symbol_context(index, source);
    let query = match first {
        "crate" => join_symbol(&package, &path[1..]),
        "self" => join_symbol(&module, &path[1..]),
        "Self" => join_symbol(&container, &path[1..]),
        "super" => {
            let levels = path
                .iter()
                .take_while(|segment| *segment == "super")
                .count();
            let mut parent = module.as_str();
            for _ in 0..levels {
                parent = parent
                    .rsplit_once("::")
                    .map(|(parent, _)| parent)
                    .unwrap_or(&package);
            }
            join_symbol(parent, &path[levels..])
        }
        _ => {
            let raw = path.join("::");
            return [join_symbol(&module, &path), raw]
                .into_iter()
                .filter(|query| !query.is_empty())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
        }
    };
    (!query.is_empty()).then_some(query).into_iter().collect()
}

fn qualified_self_lookup_queries(
    index: &QueryIndex,
    source: &Node,
    path: &syn::ExprPath,
) -> Vec<String> {
    let Some(qself) = path.qself.as_ref() else {
        return Vec::new();
    };
    let (_, module, package) = source_symbol_context(index, source);
    let segments = path.path.segments.iter().collect::<Vec<_>>();
    let member = segments
        .get(qself.position..)
        .unwrap_or_default()
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let member = member.join("::");
    if member.is_empty() {
        return Vec::new();
    }
    if type_is_self(&qself.ty)
        && let Some(trait_container) = source_trait_container(index, source)
    {
        if qself.position == 0 {
            return vec![format!("{trait_container}::{member}")];
        }
        return source_relative_path_variants(
            &module,
            &package,
            segments.get(..qself.position).unwrap_or_default(),
        )
        .into_iter()
        .map(|trait_path| format!("{trait_path}::{member}"))
        .collect();
    }
    let self_types = source_relative_type_variants(index, source, &qself.ty);
    let mut implementations = BTreeSet::new();
    if qself.position == 0 {
        for self_type in self_types {
            implementations.insert(format!("impl {self_type}::{member}"));
        }
    } else {
        let trait_paths = source_relative_path_variants(
            &module,
            &package,
            segments.get(..qself.position).unwrap_or_default(),
        );
        for trait_path in trait_paths {
            for self_type in &self_types {
                implementations.insert(format!("impl {trait_path} for {self_type}::{member}"));
            }
        }
    }
    implementations.into_iter().collect()
}

fn type_is_self(ty: &syn::Type) -> bool {
    matches!(
        ty,
        syn::Type::Path(path)
            if path.qself.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].ident == "Self"
    )
}

fn source_relative_type_variants(index: &QueryIndex, source: &Node, ty: &syn::Type) -> Vec<String> {
    let syn::Type::Path(path) = ty else {
        return Vec::new();
    };
    if path.qself.is_some() {
        return Vec::new();
    }
    let (container, module, package) = source_symbol_context(index, source);
    let segments = path.path.segments.iter().collect::<Vec<_>>();
    if segments.len() == 1 && segments[0].ident == "Self" {
        return impl_self_type(&container).into_iter().collect();
    }
    source_relative_path_variants(&module, &package, &segments)
}

fn source_relative_path_variants(
    module: &str,
    package: &str,
    segments: &[&syn::PathSegment],
) -> Vec<String> {
    let Some(first) = segments.first().map(|segment| segment.ident.to_string()) else {
        return Vec::new();
    };
    let raw = path_segments_text(segments);
    let variants = match first.as_str() {
        "crate" => vec![join_symbol_text(
            package,
            &path_segments_text(&segments[1..]),
        )],
        "self" => vec![join_symbol_text(
            module,
            &path_segments_text(&segments[1..]),
        )],
        "super" => {
            let levels = segments
                .iter()
                .take_while(|segment| segment.ident == "super")
                .count();
            let mut parent = module;
            for _ in 0..levels {
                parent = parent
                    .rsplit_once("::")
                    .map(|(parent, _)| parent)
                    .unwrap_or(package);
            }
            vec![join_symbol_text(
                parent,
                &path_segments_text(&segments[levels..]),
            )]
        }
        _ => vec![raw.clone(), join_symbol_text(module, &raw)],
    };
    variants
        .into_iter()
        .filter(|variant| !variant.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn path_segments_text(segments: &[&syn::PathSegment]) -> String {
    segments
        .iter()
        .map(|segment| normalized_tokens(*segment))
        .collect::<Vec<_>>()
        .join("::")
}

fn join_symbol_text(prefix: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}::{suffix}")
    }
}

fn source_symbol_context(index: &QueryIndex, source: &Node) -> (String, String, String) {
    let container = source
        .symbol
        .rsplit_once("::")
        .map(|(container, _)| container)
        .unwrap_or(source.symbol.as_str());
    let implementation_module = container.split_once("::impl ").map(|(module, _)| module);
    let trait_module = source_trait_container(index, source).and_then(|container| {
        container
            .rsplit_once("::")
            .map(|(module, _)| module.to_string())
    });
    let module = implementation_module
        .map(str::to_string)
        .or(trait_module)
        .unwrap_or_else(|| container.to_string());
    let package = source
        .symbol
        .split("::")
        .next()
        .unwrap_or(&module)
        .to_string();
    (container.to_string(), module, package)
}

fn source_trait_container(index: &QueryIndex, source: &Node) -> Option<String> {
    let containers = index
        .incoming_edges_for(&source.id)
        .filter(|edge| edge.kind == EdgeKind::Contains)
        .filter_map(|edge| index.node_by_id(&edge.from))
        .filter(|parent| parent.kind == NodeKind::Trait)
        .map(|parent| parent.symbol.clone())
        .collect::<BTreeSet<_>>();
    (containers.len() == 1)
        .then(|| containers.first().cloned())
        .flatten()
}

fn impl_self_type(container: &str) -> Option<String> {
    let implementation = container.split_once("::impl ")?.1;
    Some(
        implementation
            .rsplit_once(" for ")
            .map(|(_, self_type)| self_type)
            .unwrap_or(implementation)
            .to_string(),
    )
}

fn join_symbol(prefix: &str, suffix: &[String]) -> String {
    if suffix.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}::{}", suffix.join("::"))
    }
}

fn scan_syntax(source: &str, file: &str, node: &Node) -> Result<Vec<BoundaryMatch>, AtlasError> {
    let parsed = syn::parse_file(source).map_err(|error| {
        AtlasError::Io(format!("cannot parse runtime-boundary source: {error}"))
    })?;
    let mut finder = FunctionBodyFinder {
        node,
        bodies: Vec::new(),
    };
    finder.visit_file(&parsed);
    let [body] = finder.bodies.as_slice() else {
        return Ok(Vec::new());
    };
    let mut scanner = BoundaryScanner {
        file,
        matches: Vec::new(),
    };
    scanner.visit_block(body);
    scanner.matches.sort_by(|left, right| {
        left.site
            .cmp(&right.site)
            .then_with(|| left.mechanism.cmp(&right.mechanism))
            .then_with(|| left.form.cmp(&right.form))
    });
    scanner.matches.dedup();
    Ok(scanner.matches)
}

struct FunctionBodyFinder<'node, 'ast> {
    node: &'node Node,
    bodies: Vec<&'ast syn::Block>,
}

impl FunctionBodyFinder<'_, '_> {
    fn matches(&self, span: Span, ident: &syn::Ident, signature: &str) -> bool {
        let name_matches = self
            .node
            .symbol
            .rsplit("::")
            .next()
            .is_some_and(|name| ident == name);
        name_matches
            && self.node.line_start == span.start().line
            && self.node.line_end == span.end().line
            && canonical_signature(&self.node.signature) == canonical_signature(signature)
    }
}

fn canonical_signature(signature: &str) -> String {
    signature
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" :: ", "::")
}

impl<'ast> Visit<'ast> for FunctionBodyFinder<'_, 'ast> {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        let vis = &function.vis;
        let sig = &function.sig;
        let signature = normalized_tokens(&quote::quote!(#vis #sig));
        if self.matches(function.span(), &function.sig.ident, &signature) {
            self.bodies.push(&function.block);
        }
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        let vis = &function.vis;
        let sig = &function.sig;
        let signature = normalized_tokens(&quote::quote!(#vis #sig));
        if self.matches(function.span(), &function.sig.ident, &signature) {
            self.bodies.push(&function.block);
        }
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        let signature = normalized_tokens(&function.sig);
        if self.matches(function.span(), &function.sig.ident, &signature)
            && let Some(body) = &function.default
        {
            self.bodies.push(body);
        }
    }
}

struct BoundaryScanner<'a> {
    file: &'a str,
    matches: Vec<BoundaryMatch>,
}

impl BoundaryScanner<'_> {
    fn push(
        &mut self,
        span: Span,
        mechanism: RuntimeBoundaryMechanism,
        expression: &impl ToTokens,
        key: Option<String>,
        candidate_texts: Vec<String>,
    ) {
        self.matches.push(BoundaryMatch {
            mechanism,
            site: edge_site(self.file, span),
            form: bounded_form(expression),
            key,
            candidate_texts,
        });
    }
}

impl<'ast> Visit<'ast> for BoundaryScanner<'_> {
    fn visit_item_fn(&mut self, _function: &'ast syn::ItemFn) {}

    fn visit_item_impl(&mut self, _implementation: &'ast syn::ItemImpl) {}

    fn visit_item_mod(&mut self, _module: &'ast syn::ItemMod) {}

    fn visit_item_trait(&mut self, _trait: &'ast syn::ItemTrait) {}

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(path) = expression_path(&expression.func)
            && is_async_spawn_path(&path)
        {
            let candidates = expression
                .args
                .first()
                .and_then(candidate_from_expr)
                .into_iter()
                .collect();
            self.push(
                expression.span(),
                RuntimeBoundaryMechanism::AsyncTask,
                expression,
                None,
                candidates,
            );
        }
        visit::visit_expr_call(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        let method = expression.method.to_string();
        if matches!(
            method.as_str(),
            "send" | "try_send" | "blocking_send" | "broadcast"
        ) && channel_receiver(&expression.receiver)
        {
            self.push(
                expression.span(),
                RuntimeBoundaryMechanism::Channel,
                expression,
                None,
                Vec::new(),
            );
        } else if matches!(
            method.as_str(),
            "register" | "register_handler" | "subscribe" | "add_listener" | "set_handler"
        ) && callback_receiver(&expression.receiver)
        {
            let key = expression.args.first().and_then(string_literal);
            let candidates = expression
                .args
                .last()
                .and_then(candidate_from_expr)
                .into_iter()
                .collect();
            self.push(
                expression.span(),
                RuntimeBoundaryMechanism::CallbackRegistry,
                expression,
                key,
                candidates,
            );
        } else if matches!(method.as_str(), "downcast_ref" | "downcast_mut") {
            let candidates = expression
                .turbofish
                .as_ref()
                .into_iter()
                .flat_map(|arguments| arguments.args.iter())
                .filter_map(|argument| match argument {
                    syn::GenericArgument::Type(ty) => Some(normalized_tokens(ty)),
                    _ => None,
                })
                .collect();
            self.push(
                expression.span(),
                RuntimeBoundaryMechanism::Reflection,
                expression,
                None,
                candidates,
            );
        } else if matches!(method.as_str(), "route" | "service")
            && route_receiver(&expression.receiver)
        {
            let key = expression.args.first().and_then(string_literal);
            let candidate = if method == "service" {
                expression.args.first()
            } else {
                expression.args.iter().nth(1)
            };
            let candidates = candidate.and_then(route_candidate).into_iter().collect();
            self.push(
                expression.span(),
                RuntimeBoundaryMechanism::FrameworkRoute,
                expression,
                key,
                candidates,
            );
        }
        visit::visit_expr_method_call(self, expression);
    }
}

fn edge_site(file: &str, span: Span) -> EdgeSite {
    let start = span.start();
    let end = span.end();
    EdgeSite {
        file: file.to_string(),
        line_start: start.line,
        column_start: start.column + 1,
        line_end: end.line,
        column_end: end.column + 1,
    }
}

fn normalized_tokens(value: &impl ToTokens) -> String {
    value
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" :: ", "::")
}

fn bounded_form(value: &impl ToTokens) -> String {
    let value = normalized_tokens(value);
    if value.chars().count() <= 240 {
        return value;
    }
    let mut end = value.len();
    for (count, (index, _)) in value.char_indices().enumerate() {
        if count == 237 {
            end = index;
            break;
        }
    }
    format!("{}...", &value[..end])
}

fn expression_path(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Path(path) => Some(normalized_tokens(&path.path)),
        syn::Expr::Paren(paren) => expression_path(&paren.expr),
        syn::Expr::Group(group) => expression_path(&group.expr),
        _ => None,
    }
}

fn candidate_from_expr(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Call(call) => expression_path(&call.func),
        syn::Expr::Path(path) => Some(normalized_tokens(path)),
        syn::Expr::Paren(paren) => candidate_from_expr(&paren.expr),
        syn::Expr::Group(group) => candidate_from_expr(&group.expr),
        syn::Expr::Reference(reference) => candidate_from_expr(&reference.expr),
        _ => None,
    }
}

fn route_candidate(expression: &syn::Expr) -> Option<String> {
    if let syn::Expr::Call(call) = expression
        && let Some(wrapper) = expression_path(&call.func)
        && matches!(
            wrapper.rsplit("::").next(),
            Some("get" | "post" | "put" | "delete" | "patch" | "any" | "handler")
        )
    {
        return call.args.first().and_then(candidate_from_expr);
    }
    candidate_from_expr(expression)
}

fn string_literal(expression: &syn::Expr) -> Option<String> {
    let syn::Expr::Lit(literal) = expression else {
        return None;
    };
    let syn::Lit::Str(value) = &literal.lit else {
        return None;
    };
    Some(value.value())
}

fn is_async_spawn_path(path: &str) -> bool {
    let segments = path.split("::").collect::<Vec<_>>();
    matches!(segments.last(), Some(&"spawn" | &"spawn_blocking"))
        && segments
            .iter()
            .any(|segment| matches!(*segment, "tokio" | "async_std" | "smol"))
}

fn channel_receiver(receiver: &syn::Expr) -> bool {
    receiver_has_role(
        receiver,
        &["tx", "sender", "channel", "publisher", "broadcaster"],
    )
}

fn callback_receiver(receiver: &syn::Expr) -> bool {
    receiver_has_role(
        receiver,
        &[
            "registry",
            "callbacks",
            "handlers",
            "listeners",
            "subscriptions",
            "bus",
            "dispatcher",
        ],
    )
}

fn route_receiver(receiver: &syn::Expr) -> bool {
    receiver_has_role(receiver, &["router", "routes", "app"])
}

fn receiver_has_role(receiver: &syn::Expr, roles: &[&str]) -> bool {
    match receiver {
        syn::Expr::Path(path) => path
            .path
            .segments
            .iter()
            .any(|segment| identifier_has_role(&segment.ident, roles)),
        syn::Expr::Field(field) => {
            matches!(&field.member, syn::Member::Named(ident) if identifier_has_role(ident, roles))
                || receiver_has_role(&field.base, roles)
        }
        syn::Expr::MethodCall(call) => {
            identifier_has_role(&call.method, roles) || receiver_has_role(&call.receiver, roles)
        }
        syn::Expr::Call(call) => receiver_has_role(&call.func, roles),
        syn::Expr::Index(index) => receiver_has_role(&index.expr, roles),
        syn::Expr::Paren(paren) => receiver_has_role(&paren.expr, roles),
        syn::Expr::Group(group) => receiver_has_role(&group.expr, roles),
        syn::Expr::Reference(reference) => receiver_has_role(&reference.expr, roles),
        syn::Expr::Await(awaited) => receiver_has_role(&awaited.base, roles),
        syn::Expr::Try(tried) => receiver_has_role(&tried.expr, roles),
        syn::Expr::Unary(unary) => receiver_has_role(&unary.expr, roles),
        syn::Expr::Cast(cast) => receiver_has_role(&cast.expr, roles),
        _ => false,
    }
}

fn identifier_has_role(identifier: &syn::Ident, roles: &[&str]) -> bool {
    let identifier = identifier.to_string().to_ascii_lowercase();
    roles.iter().any(|role| {
        identifier == *role
            || identifier
                .strip_suffix(role)
                .is_some_and(|prefix| prefix.ends_with('_'))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn test_runtime_boundary_scanner_detects_rust_mechanisms_and_candidates() {
        let source = r#"
fn worker() {}
fn callback() {}
fn route_handler() {}

fn dispatch(sender: &Sender, registry: &Registry, router: &Router, value: &dyn Any) {
    tokio::spawn(worker());
    sender.send(Message);
    registry.register("save", callback);
    value.downcast_ref::<Message>();
    router.route("/items", get(route_handler));
}
"#;

        let node = parsed_function_node(source, "dispatch", "crate::dispatch");
        let hints = scan_syntax(source, "src/lib.rs", &node).unwrap();
        assert_eq!(
            hints.iter().map(|hint| hint.mechanism).collect::<Vec<_>>(),
            vec![
                RuntimeBoundaryMechanism::AsyncTask,
                RuntimeBoundaryMechanism::Channel,
                RuntimeBoundaryMechanism::CallbackRegistry,
                RuntimeBoundaryMechanism::Reflection,
                RuntimeBoundaryMechanism::FrameworkRoute,
            ]
        );
        assert_eq!(hints[0].candidate_texts, ["worker"]);
        assert_eq!(hints[2].key.as_deref(), Some("save"));
        assert_eq!(hints[2].candidate_texts, ["callback"]);
        assert_eq!(hints[3].candidate_texts, ["Message"]);
        assert_eq!(hints[4].key.as_deref(), Some("/items"));
        assert_eq!(hints[4].candidate_texts, ["route_handler"]);
        assert!(hints.iter().all(|hint| hint.site.file == "src/lib.rs"));
    }

    #[test]
    fn test_runtime_boundary_service_uses_its_single_handler_argument() {
        let source = r#"
fn service_handler() {}

fn dispatch(router: &Router) {
    router.service(get(service_handler));
}
"#;

        let node = parsed_function_node(source, "dispatch", "crate::dispatch");
        let hints = scan_syntax(source, "src/lib.rs", &node).unwrap();

        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].mechanism, RuntimeBoundaryMechanism::FrameworkRoute);
        assert_eq!(hints[0].key, None);
        assert_eq!(hints[0].candidate_texts, ["service_handler"]);
    }

    #[test]
    fn test_runtime_boundary_scanner_ignores_comments_strings_and_unrelated_calls() {
        let source = r#"
fn callback() {}
fn safe(metrics: &Metrics, client: &Client, ctx: &Ctx, syllabus: &Syllabus, sender: Sender) {
    // registry.register("save", callback);
    let documentation = "tokio::spawn(worker()); sender.send(Message);";
    metrics.register("save", callback);
    client.send(Message);
    client.route("/items", callback);
    ctx.send(Message);
    syllabus.register("save", callback);
    lookup("sender").send(Message);
    map["registry"].register("save", callback);
    passthrough(sender, client).send(Message);
}
"#;

        let node = parsed_function_node(source, "safe", "crate::safe");
        let hints = scan_syntax(source, "src/lib.rs", &node).unwrap();

        assert!(hints.is_empty());
    }

    #[test]
    fn test_runtime_boundary_scan_matches_graph_signatures_with_qualified_paths() {
        let code = temp_graph("runtime-boundary-qualified-signature");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"qualified-signature\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            "pub struct Registry; impl Registry { pub fn register(&self, _: &str, _: fn()) {} } pub fn callback() {} pub fn dispatch(registry: &crate::Registry) { registry.register(\"save\", callback); }\n",
        )
        .unwrap();
        let graph = temp_graph("runtime-boundary-qualified-signature-graph");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "qualified_signature::dispatch".to_string(),
                to: "qualified_signature::callback".to_string(),
            },
            &crate::FlowOptions::default(),
        )
        .unwrap();

        assert_eq!(result.runtime_boundaries.len(), 1);
        assert_eq!(
            result.runtime_boundaries[0].mechanism,
            RuntimeBoundaryMechanism::CallbackRegistry
        );

        fs::remove_dir_all(&graph).ok();
        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_scan_binds_sites_to_the_selected_function_ast() {
        let code = temp_graph("runtime-boundary-same-line-functions");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"same-line\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            "pub struct Registry; impl Registry { pub fn register(&self, _: &str, _: fn()) {} } pub fn callback() {} pub fn dispatch(registry: &Registry) { registry.register(\"save\", callback); } pub fn safe() {}\n",
        )
        .unwrap();
        let graph = temp_graph("runtime-boundary-same-line-graph");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "same_line::safe".to_string(),
                to: "same_line::callback".to_string(),
            },
            &crate::FlowOptions::default(),
        )
        .unwrap();

        assert!(result.runtime_boundaries.is_empty());

        fs::remove_dir_all(&graph).ok();
        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_candidate_lookup_canonicalizes_rust_relative_paths() {
        let source = test_node(
            "dispatch",
            "demo::nested::impl demo::nested::Worker::dispatch",
        );
        let nodes = vec![
            source.clone(),
            test_node("root", "demo::root_handler"),
            test_node("parent", "demo::parent_handler"),
            test_node("module", "demo::nested::module_handler"),
            test_node(
                "self",
                "demo::nested::impl demo::nested::Worker::self_handler",
            ),
            test_node(
                "qualified",
                "demo::nested::impl demo::nested::Handler for demo::nested::Worker::qualified_handler",
            ),
            test_node(
                "qualified-trait",
                "demo::nested::Handler::qualified_handler",
            ),
            test_node(
                "qualified-other",
                "demo::nested::impl demo::nested::Handler for demo::nested::Other::qualified_handler",
            ),
            test_node(
                "generic-qualified",
                "demo::nested::impl demo::nested::Handler for Worker < T >::generic_handler",
            ),
        ];
        let index = QueryIndex::from_test_parts("candidate-paths", nodes, Vec::new());
        let qualified_expression =
            syn::parse_str::<syn::Expr>("<Worker as Handler>::qualified_handler").unwrap();
        let qualified_text = candidate_from_expr(&qualified_expression).unwrap();
        assert_eq!(qualified_text, "< Worker as Handler >::qualified_handler");
        let generic_qualified_expression =
            syn::parse_str::<syn::Expr>("<Worker<T> as Handler>::generic_handler").unwrap();
        let generic_qualified_text = candidate_from_expr(&generic_qualified_expression).unwrap();
        assert_eq!(
            generic_qualified_text,
            "< Worker < T > as Handler >::generic_handler"
        );

        let (candidates, truncated) = resolve_candidates(
            &index,
            &source,
            &[
                "crate::root_handler".to_string(),
                "self::module_handler".to_string(),
                "super::parent_handler".to_string(),
                "Self::self_handler".to_string(),
                qualified_text,
                generic_qualified_text,
            ],
            CandidateNamespace::Callable,
        );

        assert!(!truncated);
        assert_eq!(
            candidates
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            [
                "generic-qualified",
                "module",
                "parent",
                "qualified",
                "root",
                "self"
            ]
        );
    }

    #[test]
    fn test_runtime_boundary_candidate_lookup_prefers_source_context() {
        let source = test_node("dispatch", "demo::a::dispatch");
        let index = QueryIndex::from_test_parts(
            "candidate-context",
            vec![
                source.clone(),
                test_node("local", "demo::a::handler"),
                test_node("sibling", "demo::b::handler"),
            ],
            Vec::new(),
        );

        let (candidates, truncated) = resolve_candidates(
            &index,
            &source,
            &["handler".to_string()],
            CandidateNamespace::Callable,
        );

        assert!(!truncated);
        assert_eq!(
            candidates
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["local"]
        );
    }

    #[test]
    fn test_runtime_boundary_candidate_lookup_resolves_inherent_associated_method() {
        let source = test_node("dispatch", "demo::dispatch");
        let mut handler = test_node("handler-type", "demo::Handler");
        handler.kind = NodeKind::Struct;
        let mut implementation = test_node("handler-impl", "demo::impl demo::Handler");
        implementation.kind = NodeKind::Impl;
        let mut impl_for = test_edge("handler-impl", "handler-type");
        impl_for.kind = crate::EdgeKind::ImplFor;
        let mut contains = test_edge("handler-impl", "callback");
        contains.kind = crate::EdgeKind::Contains;
        let index = QueryIndex::from_test_parts(
            "associated-candidate",
            vec![
                source.clone(),
                handler,
                implementation,
                test_node("callback", "demo::impl demo::Handler::callback"),
            ],
            vec![impl_for, contains],
        );

        let (candidates, truncated) = resolve_candidates(
            &index,
            &source,
            &["crate::Handler::callback".to_string()],
            CandidateNamespace::Callable,
        );

        assert!(!truncated);
        assert_eq!(
            candidates
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["callback"]
        );
    }

    #[test]
    fn test_runtime_boundary_generic_inherent_candidate_uses_impl_edges() {
        let source = test_node("dispatch", "demo::dispatch");
        let mut handler = test_node("handler-type", "demo::Handler");
        handler.kind = NodeKind::Struct;
        let mut implementation = test_node("handler-impl", "demo::impl Handler < T >");
        implementation.kind = NodeKind::Impl;
        let callback = test_node("callback", "demo::impl Handler < T >::callback");
        let mut impl_for = test_edge("handler-impl", "handler-type");
        impl_for.kind = crate::EdgeKind::ImplFor;
        let mut contains = test_edge("handler-impl", "callback");
        contains.kind = crate::EdgeKind::Contains;
        let index = QueryIndex::from_test_parts(
            "generic-associated-candidate",
            vec![source.clone(), handler, implementation, callback],
            vec![impl_for, contains],
        );

        let (candidates, truncated) = resolve_candidates(
            &index,
            &source,
            &["crate::Handler::<u8>::callback".to_string()],
            CandidateNamespace::Callable,
        );

        assert!(!truncated);
        assert_eq!(
            candidates
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["callback"]
        );
    }

    #[test]
    fn test_runtime_boundary_reflection_candidate_uses_type_namespace() {
        let source = test_node("dispatch", "demo::dispatch");
        let mut message_type = test_node("message-type", "demo::Message");
        message_type.kind = NodeKind::TypeAlias;
        let message_function = test_node("message-function", "demo::Message");
        let index = QueryIndex::from_test_parts(
            "candidate-namespace",
            vec![source.clone(), message_type, message_function],
            Vec::new(),
        );

        let (candidates, truncated) = resolve_candidates(
            &index,
            &source,
            &["Message".to_string()],
            CandidateNamespace::Type,
        );

        assert!(!truncated);
        assert_eq!(
            candidates
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["message-type"]
        );
    }

    #[test]
    fn test_runtime_boundary_reflection_candidate_resolves_generic_type_declaration() {
        let source = test_node("dispatch", "demo::dispatch");
        let mut message = test_node("message", "demo::Message");
        message.kind = NodeKind::Struct;
        let index = QueryIndex::from_test_parts(
            "generic-reflection",
            vec![source.clone(), message],
            Vec::new(),
        );

        let (candidates, truncated) = resolve_candidates(
            &index,
            &source,
            &["Message < u8 >".to_string()],
            CandidateNamespace::Type,
        );

        assert!(!truncated);
        assert_eq!(
            candidates
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["message"]
        );
    }

    #[test]
    fn test_atlas_flow_resolves_inherent_runtime_candidate_from_built_graph() {
        let code = temp_graph("runtime-boundary-associated-candidate");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"associated-candidate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            r#"
pub struct Registry;
impl Registry { pub fn register(&self, _key: &str, _callback: fn()) {} }
pub struct Handler;
impl Handler { pub fn callback() {} }
pub fn dispatch(registry: &Registry) {
    registry.register("save", crate::Handler::callback);
}
"#,
        )
        .unwrap();
        let graph = temp_graph("runtime-boundary-associated-candidate-graph");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "associated_candidate::dispatch".to_string(),
                to: "associated_candidate::impl associated_candidate::Handler::callback"
                    .to_string(),
            },
            &crate::FlowOptions::default(),
        )
        .unwrap();

        let callback = result
            .runtime_boundaries
            .iter()
            .find(|hint| hint.mechanism == RuntimeBoundaryMechanism::CallbackRegistry)
            .unwrap();
        assert_eq!(
            callback
                .candidates
                .iter()
                .map(|node| node.symbol.as_str())
                .collect::<Vec<_>>(),
            ["associated_candidate::impl associated_candidate::Handler::callback"]
        );

        fs::remove_dir_all(&graph).ok();
        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_atlas_flow_reports_query_time_runtime_boundary_without_mutating_graph() {
        let code = fixture_root();
        let graph = temp_graph("runtime-boundary-flow");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();
        let before = graph_bytes(&graph);

        let query = crate::FlowQuery::Between {
            from: "atlas_runtime_boundaries::dispatch".to_string(),
            to: "atlas_runtime_boundaries::callback_handler".to_string(),
        };
        let first =
            crate::flow(&code, &graph, query.clone(), &crate::FlowOptions::default()).unwrap();
        let second = crate::flow(&code, &graph, query, &crate::FlowOptions::default()).unwrap();

        assert_eq!(first.state, crate::FlowState::CapabilityUnavailable);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        let callback = first
            .runtime_boundaries
            .iter()
            .find(|hint| hint.mechanism == RuntimeBoundaryMechanism::CallbackRegistry)
            .unwrap();
        assert_eq!(callback.authority, RuntimeBoundaryAuthority::QueryHint);
        assert_eq!(callback.confidence, crate::EdgeConfidence::Heuristic);
        assert_eq!(callback.key.as_deref(), Some("save"));
        assert_eq!(callback.candidate_texts, ["callback_handler"]);
        assert_eq!(
            callback
                .candidates
                .iter()
                .map(|node| node.symbol.as_str())
                .collect::<Vec<_>>(),
            ["atlas_runtime_boundaries::callback_handler"]
        );
        assert!(first.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "atlas-flow-runtime-boundary"
                && diagnostic.message.contains("query hint")
        }));
        assert_eq!(before, graph_bytes(&graph));

        fs::remove_dir_all(&graph).ok();
    }

    #[test]
    fn test_atlas_flow_suppresses_runtime_hints_for_connected_paths() {
        let code = fixture_root();
        let graph = temp_graph("runtime-boundary-connected");
        crate::build(
            &code,
            &graph,
            &crate::BuildOptions {
                full: true,
                scip_index: Some(code.join("scip.json")),
                dynamic_dispatch: false,
                ..crate::BuildOptions::default()
            },
        )
        .unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "atlas_runtime_boundaries::dispatch".to_string(),
                to: "atlas_runtime_boundaries::callback_handler".to_string(),
            },
            &crate::FlowOptions {
                frozen: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.state, crate::FlowState::Found);
        assert!(result.runtime_boundaries.is_empty());
        assert!(!result.runtime_boundary_truncated);
        assert!(
            result
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "atlas-flow-runtime-boundary")
        );

        fs::remove_dir_all(&graph).ok();
    }

    #[test]
    fn test_atlas_flow_runtime_hints_survive_fresh_scip_helper_edges() {
        let code = fixture_root();
        let graph = temp_graph("runtime-boundary-scip-helper");
        crate::build(
            &code,
            &graph,
            &crate::BuildOptions {
                full: true,
                scip_index: Some(code.join("scip-helper.json")),
                dynamic_dispatch: false,
                ..crate::BuildOptions::default()
            },
        )
        .unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "atlas_runtime_boundaries::dispatch".to_string(),
                to: "atlas_runtime_boundaries::callback_handler".to_string(),
            },
            &crate::FlowOptions {
                frozen: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.state, crate::FlowState::NoPath);
        assert_eq!(result.status.scip.state, crate::LayerState::Fresh);
        assert!(result.runtime_boundaries.iter().any(|hint| {
            hint.source.symbol == "atlas_runtime_boundaries::dispatch"
                && hint.mechanism == RuntimeBoundaryMechanism::CallbackRegistry
                && hint.candidate_texts == ["callback_handler"]
        }));

        fs::remove_dir_all(&graph).ok();
    }

    #[test]
    fn test_atlas_flow_runtime_hints_require_fresh_source() {
        let sandbox = temp_graph("runtime-boundary-stale-code");
        copy_fixture(&fixture_root(), &sandbox);
        let graph = temp_graph("runtime-boundary-stale-graph");
        crate::build(&sandbox, &graph, &crate::BuildOptions::default()).unwrap();
        let source = sandbox.join("src/lib.rs");
        let mut changed = fs::read_to_string(&source).unwrap();
        changed.push_str("\npub fn changed_after_build() {}\n");
        fs::write(&source, changed).unwrap();

        let result = crate::flow(
            &sandbox,
            &graph,
            crate::FlowQuery::Between {
                from: "atlas_runtime_boundaries::dispatch".to_string(),
                to: "atlas_runtime_boundaries::callback_handler".to_string(),
            },
            &crate::FlowOptions {
                frozen: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.status.syn.state, crate::LayerState::Stale);
        assert!(result.runtime_boundaries.is_empty());
        assert!(!result.runtime_boundary_truncated);

        fs::remove_dir_all(&graph).ok();
        fs::remove_dir_all(&sandbox).ok();
    }

    #[test]
    fn test_runtime_boundary_stale_source_does_not_expand_to_fresh_descendants() {
        let code = temp_graph("runtime-boundary-stale-frontier");
        fs::create_dir_all(code.join("src")).unwrap();
        let recorded_start = "pub fn start() { helper::boundary(); }\npub fn target() {}\n";
        let current_start = "pub fn start() {}\npub fn target() {}\n";
        let helper_source = r#"
pub fn boundary(registry: &Registry) {
    registry.register("save", crate::target);
}
"#;
        fs::write(code.join("src/lib.rs"), current_start).unwrap();
        fs::write(code.join("src/helper.rs"), helper_source).unwrap();

        let mut start = parsed_function_node(current_start, "start", "crate::start");
        start.id = "start".into();
        let mut target = parsed_function_node(current_start, "target", "crate::target");
        target.id = "target".into();
        let mut helper = parsed_function_node(helper_source, "boundary", "crate::helper::boundary");
        helper.id = "helper".into();
        helper.file = "src/helper.rs".into();
        let index = QueryIndex::from_test_parts(
            "stale-frontier",
            vec![start, target, helper],
            vec![test_edge("start", "helper")],
        );
        let meta = Meta {
            schema_version: crate::SCHEMA_VERSION,
            package: "crate".into(),
            packages: vec!["crate".into()],
            roots: vec!["src/lib.rs".into()],
            capability: crate::Capability::default(),
            files: BTreeMap::from([
                (
                    "src/lib.rs".into(),
                    blake3::hash(recorded_start.as_bytes()).to_hex().to_string(),
                ),
                (
                    "src/helper.rs".into(),
                    blake3::hash(helper_source.as_bytes()).to_hex().to_string(),
                ),
            ]),
            graph_fingerprint: "stale-frontier".into(),
        };

        let result = project_runtime_boundaries(
            &code,
            &meta,
            &index,
            &test_status(crate::LayerState::Unavailable),
            &FlowQuery::Between {
                from: "crate::start".into(),
                to: "crate::target".into(),
            },
            TraversalLimits::flow_default(),
        );

        assert!(result.hints.is_empty());
        assert!(!result.truncated);

        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_stale_semantic_edges_do_not_expand_scan_frontier() {
        let code = temp_graph("runtime-boundary-stale-scip");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"stale-scip-runtime\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            "pub mod helper;\npub fn dispatch() {\n    // recorded SCIP used to claim a helper call here\n}\npub fn callback() {}\n",
        )
        .unwrap();
        fs::write(
            code.join("src/helper.rs"),
            "pub struct Registry;\nimpl Registry { pub fn register(&self, _: &str, _: fn()) {} }\npub fn boundary(registry: &Registry) {\n    registry.register(\"save\", crate::callback);\n}\n",
        )
        .unwrap();
        let scip = code.join("scip.json");
        fs::write(
            &scip,
            r#"{
  "metadata": {"tool_info": {"name": "rust-analyzer", "version": "test"}},
  "documents": [
    {"relative_path": "src/helper.rs", "occurrences": [
      {"symbol": "rust-analyzer cargo stale-scip-runtime 0.1.0 boundary().", "symbol_roles": 1, "range": [2, 7, 15]}
    ]},
    {"relative_path": "src/lib.rs", "occurrences": [
      {"symbol": "rust-analyzer cargo stale-scip-runtime 0.1.0 boundary().", "symbol_roles": 0, "range": [2, 4, 20]}
    ]}
  ]
}"#,
        )
        .unwrap();
        let graph = temp_graph("runtime-boundary-stale-scip-graph");
        crate::build(
            &code,
            &graph,
            &crate::BuildOptions {
                full: true,
                scip_index: Some(scip.clone()),
                dynamic_dispatch: false,
                ..crate::BuildOptions::default()
            },
        )
        .unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(&scip)
            .unwrap()
            .write_all(b"\n")
            .unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "stale_scip_runtime::dispatch".into(),
                to: "stale_scip_runtime::callback".into(),
            },
            &crate::FlowOptions {
                frozen: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.status.syn.state, crate::LayerState::Fresh);
        assert_eq!(result.status.scip.state, crate::LayerState::Stale);
        assert!(result.runtime_boundaries.is_empty());

        fs::remove_dir_all(&graph).ok();
        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_trait_default_candidates_use_declaring_module() {
        let code = temp_graph("runtime-boundary-trait-module");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"trait-default\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            r#"
pub struct Registry;
impl Registry {
    pub fn register(&self, _: &str, _: fn()) {}
}
pub mod nested {
    pub fn handler() {}
    pub trait Runner {
        fn dispatch(registry: &crate::Registry) {
            registry.register("save", self::handler);
        }
    }
}
"#,
        )
        .unwrap();
        let graph = temp_graph("runtime-boundary-trait-module-graph");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "trait_default::nested::Runner::dispatch".into(),
                to: "trait_default::nested::handler".into(),
            },
            &crate::FlowOptions::default(),
        )
        .unwrap();

        assert_eq!(result.runtime_boundaries.len(), 1);
        assert_eq!(
            result.runtime_boundaries[0].candidate_texts,
            ["self::handler"]
        );
        assert_eq!(
            result.runtime_boundaries[0]
                .candidates
                .iter()
                .map(|node| node.symbol.as_str())
                .collect::<Vec<_>>(),
            ["trait_default::nested::handler"]
        );

        fs::remove_dir_all(&graph).ok();
        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_trait_default_resolves_qualified_self_candidate() {
        let code = temp_graph("runtime-boundary-trait-qualified-self");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"trait-qualified-self\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            r#"
pub struct Registry;
impl Registry {
    pub fn register(&self, _: &str, _: fn()) {}
}
pub trait Runner {
    fn handler() {}
    fn dispatch(registry: &Registry) {
        registry.register("save", <Self as Runner>::handler);
    }
}
"#,
        )
        .unwrap();
        let graph = temp_graph("runtime-boundary-trait-qualified-self-graph");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();

        let result = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "trait_qualified_self::Runner::dispatch".into(),
                to: "trait_qualified_self::Runner::handler".into(),
            },
            &crate::FlowOptions::default(),
        )
        .unwrap();

        assert_eq!(result.runtime_boundaries.len(), 1);
        assert_eq!(
            result.runtime_boundaries[0].candidate_texts,
            ["< Self as Runner >::handler"]
        );
        assert_eq!(
            result.runtime_boundaries[0]
                .candidates
                .iter()
                .map(|node| node.symbol.as_str())
                .collect::<Vec<_>>(),
            ["trait_qualified_self::Runner::handler"]
        );

        fs::remove_dir_all(&graph).ok();
        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_json_marks_query_hint_not_graph_fact() {
        let hint = RuntimeBoundaryHint {
            source: test_node("source", "crate::source"),
            site: EdgeSite {
                file: "src/lib.rs".to_string(),
                line_start: 4,
                column_start: 5,
                line_end: 4,
                column_end: 28,
            },
            mechanism: RuntimeBoundaryMechanism::CallbackRegistry,
            form: "registry.register(\"save\", handler)".to_string(),
            key: Some("save".to_string()),
            candidate_texts: vec!["handler".to_string()],
            candidates: vec![test_node("handler", "crate::handler")],
            candidates_truncated: false,
            authority: RuntimeBoundaryAuthority::QueryHint,
            confidence: EdgeConfidence::Heuristic,
        };

        let value = serde_json::to_value(hint).unwrap();

        assert_eq!(value["authority"], "query-hint");
        assert_eq!(value["confidence"], "heuristic");
        assert!(value.get("provenance").is_none());
        assert!(value.get("resolution").is_none());
        assert!(value.get("edge").is_none());
    }

    #[test]
    fn test_runtime_boundary_query_does_not_change_impact_or_bindable_edges() {
        let code = fixture_root();
        let graph = temp_graph("runtime-boundary-impact");
        crate::build(&code, &graph, &crate::BuildOptions::default()).unwrap();
        let flow = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "atlas_runtime_boundaries::dispatch".to_string(),
                to: "atlas_runtime_boundaries::callback_handler".to_string(),
            },
            &crate::FlowOptions::default(),
        )
        .unwrap();
        assert!(!flow.runtime_boundaries.is_empty());

        let adjacency = crate::query(
            &code,
            &graph,
            "atlas_runtime_boundaries::callback_handler",
            &crate::QueryOptions::default(),
        )
        .unwrap();
        assert!(
            adjacency
                .edges_in
                .iter()
                .all(|edge| !edge.from.contains("dispatch"))
        );
        let impact = crate::impact(
            &code,
            &graph,
            "atlas_runtime_boundaries::callback_handler",
            &crate::ImpactOptions::default(),
        )
        .unwrap();
        assert!(
            impact
                .affected
                .iter()
                .all(|entry| !entry.node.symbol.ends_with("::dispatch"))
        );

        fs::remove_dir_all(&graph).ok();
    }

    #[test]
    fn test_runtime_boundary_limits_fail_closed_and_order_deterministically() {
        let code = temp_graph("runtime-boundary-limits");
        fs::create_dir_all(code.join("src")).unwrap();
        let mut source = String::from("fn start() {}\nfn target() {}\nfn handler() {}\n");
        for index in 0..5 {
            source.push_str(&format!(
                "fn terminal_{index:02}() {{ registry.register(\"run\", handler); }}\n"
            ));
        }
        fs::write(code.join("src/lib.rs"), &source).unwrap();
        let terminal_nodes = (0..5)
            .map(|index| {
                let name = format!("terminal_{index:02}");
                Node {
                    id: format!("terminal-{index:02}"),
                    ..parsed_function_node(&source, &name, &format!("crate::{name}"))
                }
            })
            .collect::<Vec<_>>();

        let start = test_node("start", "crate::start");
        let target = test_node("target", "crate::target");
        let mut nodes = vec![start.clone(), target];
        nodes.extend(terminal_nodes.iter().cloned());
        for index in 0..17 {
            nodes.push(test_node(
                &format!("handler-{index:02}"),
                &format!("crate::Impl{index:02}::handler"),
            ));
        }
        let edges = terminal_nodes
            .iter()
            .map(|terminal| test_edge(&start.id, &terminal.id))
            .collect();
        let index = QueryIndex::from_test_parts("runtime-limit", nodes, edges);
        let meta = Meta {
            schema_version: crate::SCHEMA_VERSION,
            package: "runtime-limits".to_string(),
            packages: vec!["runtime-limits".to_string()],
            roots: vec!["src/lib.rs".to_string()],
            capability: crate::Capability::default(),
            files: BTreeMap::from([(
                "src/lib.rs".to_string(),
                blake3::hash(source.as_bytes()).to_hex().to_string(),
            )]),
            graph_fingerprint: "runtime-limit".to_string(),
        };
        let query = FlowQuery::Between {
            from: "crate::start".to_string(),
            to: "crate::target".to_string(),
        };

        let left = project_runtime_boundaries(
            &code,
            &meta,
            &index,
            &test_status(crate::LayerState::Unavailable),
            &query,
            TraversalLimits::flow_default(),
        );
        let right = project_runtime_boundaries(
            &code,
            &meta,
            &index,
            &test_status(crate::LayerState::Unavailable),
            &query,
            TraversalLimits::flow_default(),
        );

        assert_eq!(left, right);
        assert!(left.truncated);
        assert_eq!(left.hints.len(), MAX_HINTS);
        assert!(left.hints.iter().all(|hint| {
            hint.candidate_texts == ["handler"]
                && hint.candidates.is_empty()
                && hint.candidates_truncated
        }));
        assert_eq!(
            left.hints
                .iter()
                .map(|hint| hint.source.id.as_str())
                .collect::<Vec<_>>(),
            ["terminal-00", "terminal-01", "terminal-02", "terminal-03"]
        );

        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_scan_frontier_enforces_node_and_byte_limits() {
        let code = temp_graph("runtime-boundary-frontier-limits");
        fs::create_dir_all(code.join("src")).unwrap();
        let source = "pub fn start() {}\npub fn target() {}\n";
        fs::write(code.join("src/lib.rs"), source).unwrap();
        let mut nodes = vec![
            Node {
                id: "start".into(),
                ..parsed_function_node(source, "start", "crate::start")
            },
            Node {
                id: "target".into(),
                ..parsed_function_node(source, "target", "crate::target")
            },
        ];
        for index in 0..8 {
            nodes.push(Node {
                id: format!("reachable-{index}"),
                ..test_node(
                    &format!("reachable_{index}"),
                    &format!("crate::reachable_{index}"),
                )
            });
        }
        let edges = nodes
            .iter()
            .skip(1)
            .map(|node| test_edge("start", &node.id))
            .collect();
        let index = QueryIndex::from_test_parts("frontier-node-limit", nodes, edges);
        let meta = Meta {
            schema_version: crate::SCHEMA_VERSION,
            package: "crate".into(),
            packages: vec!["crate".into()],
            roots: vec!["src/lib.rs".into()],
            capability: crate::Capability::default(),
            files: BTreeMap::from([(
                "src/lib.rs".into(),
                blake3::hash(source.as_bytes()).to_hex().to_string(),
            )]),
            graph_fingerprint: "frontier-node-limit".into(),
        };
        let (frontier, source_cache, truncated) = scan_nodes(
            &fs::canonicalize(&code).unwrap(),
            &meta,
            &index,
            &test_status(crate::LayerState::Unavailable),
            &FlowQuery::Between {
                from: "crate::start".into(),
                to: "crate::target".into(),
            },
            TraversalLimits::flow_default(),
        );

        assert_eq!(frontier.len(), MAX_SCAN_NODES);
        assert_eq!(source_cache.len(), 1);
        assert!(truncated);

        let oversized_code = temp_graph("runtime-boundary-frontier-byte-limit");
        fs::create_dir_all(oversized_code.join("src")).unwrap();
        let mut oversized = "pub fn start() {}\npub fn target() {}\n".to_string();
        oversized.push_str(&" ".repeat(MAX_SCAN_BYTES));
        fs::write(oversized_code.join("src/lib.rs"), &oversized).unwrap();
        let oversized_index = QueryIndex::from_test_parts(
            "frontier-byte-limit",
            vec![
                Node {
                    id: "start".into(),
                    ..parsed_function_node(&oversized, "start", "crate::start")
                },
                Node {
                    id: "target".into(),
                    ..parsed_function_node(&oversized, "target", "crate::target")
                },
            ],
            Vec::new(),
        );
        let oversized_meta = Meta {
            files: BTreeMap::from([(
                "src/lib.rs".into(),
                blake3::hash(oversized.as_bytes()).to_hex().to_string(),
            )]),
            graph_fingerprint: "frontier-byte-limit".into(),
            ..meta
        };
        let (frontier, source_cache, truncated) = scan_nodes(
            &fs::canonicalize(&oversized_code).unwrap(),
            &oversized_meta,
            &oversized_index,
            &test_status(crate::LayerState::Unavailable),
            &FlowQuery::Between {
                from: "crate::start".into(),
                to: "crate::target".into(),
            },
            TraversalLimits::flow_default(),
        );

        assert!(frontier.is_empty());
        assert!(source_cache.is_empty());
        assert!(truncated);

        fs::remove_dir_all(&oversized_code).ok();
        fs::remove_dir_all(&code).ok();
    }

    #[test]
    fn test_runtime_boundary_scan_frontier_orders_each_depth_before_limits() {
        let code = temp_graph("runtime-boundary-frontier-canonical-layer");
        fs::create_dir_all(code.join("src")).unwrap();
        let source = "pub fn start() {}\npub fn target() {}\n";
        fs::write(code.join("src/lib.rs"), source).unwrap();
        let mut nodes = vec![
            Node {
                id: "start".into(),
                ..parsed_function_node(source, "start", "crate::start")
            },
            Node {
                id: "target".into(),
                ..parsed_function_node(source, "target", "crate::target")
            },
            test_node("parent-a", "crate::parent_a"),
            test_node("parent-b", "crate::parent_b"),
        ];
        let mut edges = vec![
            test_edge("start", "parent-a"),
            test_edge("start", "parent-b"),
        ];
        for index in 0..6 {
            let a = format!("a-{index}");
            let z = format!("z-{index}");
            nodes.push(test_node(&a, &format!("crate::a_{index}")));
            nodes.push(test_node(&z, &format!("crate::z_{index}")));
            edges.push(test_edge("parent-b", &a));
            edges.push(test_edge("parent-a", &z));
        }
        let index = QueryIndex::from_test_parts("canonical-frontier", nodes, edges);
        let meta = Meta {
            schema_version: crate::SCHEMA_VERSION,
            package: "crate".into(),
            packages: vec!["crate".into()],
            roots: vec!["start".into()],
            capability: crate::Capability::default(),
            files: BTreeMap::from([(
                "src/lib.rs".into(),
                blake3::hash(source.as_bytes()).to_hex().to_string(),
            )]),
            graph_fingerprint: "canonical-frontier".into(),
        };

        let (frontier, _, truncated) = scan_nodes(
            &fs::canonicalize(&code).unwrap(),
            &meta,
            &index,
            &test_status(crate::LayerState::Unavailable),
            &FlowQuery::Between {
                from: "crate::start".into(),
                to: "crate::target".into(),
            },
            TraversalLimits::flow_default(),
        );

        assert!(truncated);
        assert_eq!(
            frontier
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            [
                "start", "parent-a", "parent-b", "a-0", "a-1", "a-2", "a-3", "a-4"
            ]
        );
        fs::remove_dir_all(&code).ok();
    }

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/atlas/runtime-boundaries")
    }

    fn temp_graph(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rust-atlas-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn graph_bytes(graph: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut paths = fs::read_dir(graph)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        let shards = graph.join("shards");
        if shards.is_dir() {
            paths.extend(
                fs::read_dir(shards)
                    .unwrap()
                    .filter_map(Result::ok)
                    .map(|entry| entry.path()),
            );
        }
        paths.sort();
        paths
            .into_iter()
            .filter(|path| path.is_file())
            .map(|path| {
                let relative = path
                    .strip_prefix(graph)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                (relative, fs::read(path).unwrap())
            })
            .collect()
    }

    fn copy_fixture(source: &Path, target: &Path) {
        fs::create_dir_all(target.join("src")).unwrap();
        fs::copy(source.join("Cargo.toml"), target.join("Cargo.toml")).unwrap();
        fs::copy(source.join("src/lib.rs"), target.join("src/lib.rs")).unwrap();
    }

    fn test_node(id: &str, symbol: &str) -> Node {
        Node {
            id: id.to_string(),
            symbol: symbol.to_string(),
            kind: NodeKind::Fn,
            file: "src/lib.rs".to_string(),
            line_start: 1,
            line_end: 1,
            visibility: "pub".to_string(),
            signature: format!("pub fn {id}()"),
            doc: None,
            cfg: None,
        }
    }

    fn parsed_function_node(source: &str, name: &str, symbol: &str) -> Node {
        let parsed = syn::parse_file(source).unwrap();
        let function = parsed
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == name => Some(function),
                _ => None,
            })
            .unwrap();
        let vis = &function.vis;
        let sig = &function.sig;
        Node {
            id: name.to_string(),
            symbol: symbol.to_string(),
            kind: NodeKind::Fn,
            file: "src/lib.rs".to_string(),
            line_start: function.span().start().line,
            line_end: function.span().end().line,
            visibility: "private".to_string(),
            signature: normalized_tokens(&quote::quote!(#vis #sig)),
            doc: None,
            cfg: None,
        }
    }

    fn test_edge(from: &str, to: &str) -> crate::Edge {
        crate::Edge {
            from: from.to_string(),
            to: to.to_string(),
            target_text: Some(to.to_string()),
            resolution: crate::EdgeResolution::Resolved,
            kind: crate::EdgeKind::Calls,
            provenance: crate::Provenance::Syn,
            site: None,
            extractor: None,
            dispatch: Some(crate::DispatchKind::Static),
            confidence: Some(EdgeConfidence::Exact),
            candidates: Vec::new(),
            evidence: None,
            generic: false,
        }
    }

    fn test_status(scip: crate::LayerState) -> crate::AtlasStatus {
        let identity = crate::GraphIdentity {
            repository_root: "/repo".into(),
            git_common_dir: None,
            worktree_root: "/repo".into(),
            graph_root: "/repo/.agent-spec/graph".into(),
            toolchain: "test".into(),
        };
        let layer = |state| crate::LayerStatus {
            state,
            extractor: None,
            recorded_fingerprint: None,
            current_fingerprint: None,
            recorded_source_fingerprint: None,
            current_source_fingerprint: None,
            stale_files: Vec::new(),
            diagnostics: Vec::new(),
        };
        crate::AtlasStatus {
            generation: Some("g-runtime-test".into()),
            graph_fingerprint: "test".into(),
            recorded_identity: identity.clone(),
            current_identity: identity,
            worktree_mismatch: None,
            syn: layer(crate::LayerState::Fresh),
            scip: layer(scip),
            mir: layer(crate::LayerState::Unavailable),
        }
    }
}
