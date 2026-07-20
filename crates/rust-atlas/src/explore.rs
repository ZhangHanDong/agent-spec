use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::affected::normalize_affected_path;
use crate::impact::impact_many_index;
use crate::traversal::{
    EndpointResolution, canonical_path_signature, edge_targets, enumerate_paths, resolve_endpoint,
    sort_paths,
};
use crate::{
    AtlasError, AtlasStatus, Edge, GraphPath, ImpactEntry, ImpactOptions, Meta, Node, QueryIndex,
    QueryOptions, TraversalLimits, indexed_query_state,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExploreProfile {
    Compact,
    Deep,
}

impl ExploreProfile {
    pub fn budget(self) -> ExploreBudget {
        match self {
            Self::Compact => ExploreBudget::compact(),
            Self::Deep => ExploreBudget::deep(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreOptions {
    pub profile: ExploreProfile,
    pub frozen: bool,
}

impl Default for ExploreOptions {
    fn default() -> Self {
        Self {
            profile: ExploreProfile::Compact,
            frozen: false,
        }
    }
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

impl ExploreBudget {
    pub fn compact() -> Self {
        Self {
            max_seeds: 8,
            max_nodes: 32,
            max_edges: 48,
            max_paths: 8,
            max_excerpts: 4,
            max_excerpt_lines: 20,
            max_serialized_bytes: 16_000,
        }
    }

    pub fn deep() -> Self {
        Self {
            max_seeds: 16,
            max_nodes: 96,
            max_edges: 160,
            max_paths: 20,
            max_excerpts: 12,
            max_excerpt_lines: 40,
            max_serialized_bytes: 24_000,
        }
    }
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

struct ComposedPaths {
    primary: Vec<GraphPath>,
    alternatives: Vec<GraphPath>,
    truncation_reasons: Vec<String>,
}

pub fn explore(
    code_root: &Path,
    graph_dir: &Path,
    query: &str,
    options: &ExploreOptions,
) -> Result<ExploreResult, AtlasError> {
    let (meta, index, status) = indexed_query_state(
        code_root,
        graph_dir,
        &QueryOptions {
            frozen: options.frozen,
        },
    )?;
    explore_index(code_root, &meta, &index, &status, query, options)
}

pub(crate) fn explore_index(
    code_root: &Path,
    meta: &Meta,
    index: &QueryIndex,
    status: &AtlasStatus,
    query: &str,
    options: &ExploreOptions,
) -> Result<ExploreResult, AtlasError> {
    let limits = options.profile.budget();
    let (terms, terms_truncated) = tokenize_query(query);
    let (seeds, seeds_truncated) = ranked_seeds(index, &terms, limits.max_seeds);
    let mut diagnostics = Vec::new();
    let mut truncation_reasons = Vec::new();
    if terms_truncated {
        push_reason(&mut truncation_reasons, "query-terms-count");
    }
    if seeds.is_empty() {
        diagnostics.push(ExploreDiagnostic {
            code: "atlas-explore-no-match".into(),
            message: "no repository path or identifier matched the query".into(),
            file: None,
        });
    }
    if seeds_truncated {
        push_reason(&mut truncation_reasons, "seeds-count");
    }

    let mut nodes = seeds
        .iter()
        .map(|node| (node.id.clone(), node.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut edges = BTreeSet::new();
    collect_seed_relationships(index, &seeds, &mut nodes, &mut edges);
    let composed_paths = compose_paths(index, &seeds, limits.max_paths)?;
    let primary_paths = composed_paths.primary;
    let alternative_paths = composed_paths.alternatives;
    for path in primary_paths.iter().chain(&alternative_paths) {
        collect_path(path, &mut nodes, &mut edges);
    }
    for reason in composed_paths.truncation_reasons {
        push_reason(&mut truncation_reasons, &reason);
    }

    let impact = impact_many_index(
        index,
        &seeds,
        &ImpactOptions {
            max_depth: 3,
            max_nodes: limits.max_nodes,
            frozen: options.frozen,
        },
    )?;
    for diagnostic in &impact.diagnostics {
        diagnostics.push(ExploreDiagnostic {
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            file: None,
        });
    }
    for entry in &impact.affected {
        nodes.insert(entry.node.id.clone(), entry.node.clone());
        collect_path(&entry.path, &mut nodes, &mut edges);
    }
    if impact.truncated {
        push_reason(&mut truncation_reasons, "impact-count");
    }

    let seed_ids = seeds
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let seed_rank = seeds
        .iter()
        .enumerate()
        .map(|(rank, node)| (node.id.as_str(), rank))
        .collect::<BTreeMap<_, _>>();
    let spine_ids = primary_paths
        .iter()
        .flat_map(|path| path.nodes.iter().map(|node| node.id.as_str()))
        .collect::<BTreeSet<_>>();
    let mut nodes = nodes
        .into_values()
        .map(|node| ExploreNode {
            seed: seed_ids.contains(node.id.as_str()),
            spine: spine_ids.contains(node.id.as_str()),
            node,
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        seed_rank
            .get(left.node.id.as_str())
            .unwrap_or(&usize::MAX)
            .cmp(seed_rank.get(right.node.id.as_str()).unwrap_or(&usize::MAX))
            .then_with(|| right.spine.cmp(&left.spine))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    let (excerpts, excerpt_diagnostics, excerpts_truncated) =
        source_excerpts(code_root, meta, &nodes, limits);
    diagnostics.extend(excerpt_diagnostics);
    if excerpts_truncated {
        push_reason(&mut truncation_reasons, "excerpts-count");
    }
    diagnostics.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    diagnostics.dedup();
    let mut result = ExploreResult {
        schema: "agent-spec/rust-atlas/explore-v1".into(),
        query: query.into(),
        profile: options.profile,
        limits,
        usage: BudgetUsage::default(),
        seeds,
        nodes,
        edges: edges.into_iter().collect(),
        primary_paths,
        alternative_paths,
        impact: impact.affected,
        excerpts,
        truncated: !truncation_reasons.is_empty(),
        truncation_reasons,
        diagnostics,
        status: status.clone(),
        stale: status.syn.stale_files.clone(),
    };
    refresh_usage(&mut result);
    Ok(result)
}

fn tokenize_query(query: &str) -> (Vec<String>, bool) {
    let mut terms = Vec::new();
    let mut seen = BTreeSet::new();
    let mut truncated = false;
    for raw in query.split(|character: char| {
        !(character.is_ascii_alphanumeric() || matches!(character, '_' | ':' | '.' | '/' | '-'))
    }) {
        let term = raw.trim_matches([':', '.', '/', '-']);
        if term.is_empty() || !seen.insert(term.to_string()) {
            continue;
        }
        if terms.len() == 32 {
            truncated = true;
        } else {
            terms.push(term.to_string());
        }
    }
    (terms, truncated)
}

fn ranked_seeds(index: &QueryIndex, terms: &[String], max_seeds: usize) -> (Vec<Node>, bool) {
    let mut ranked = Vec::new();
    let mut seen = BTreeSet::new();
    let mut truncated = false;
    for term in terms {
        let path = term.trim_start_matches("./").replace('\\', "/");
        for position in index.file.get(&path).into_iter().flatten() {
            if let Some(node) = index.nodes.get(*position)
                && seen.insert(node.id.clone())
            {
                if ranked.len() == max_seeds {
                    truncated = true;
                } else {
                    ranked.push(node.clone());
                }
            }
        }
        for hit in index.search_nodes(term) {
            if seen.insert(hit.node.id.clone()) {
                if ranked.len() == max_seeds {
                    truncated = true;
                } else {
                    ranked.push(hit.node);
                }
            }
        }
    }
    (ranked, truncated)
}

fn collect_seed_relationships(
    index: &QueryIndex,
    seeds: &[Node],
    nodes: &mut BTreeMap<String, Node>,
    edges: &mut BTreeSet<Edge>,
) {
    let seed_ids = seeds
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    for edge in &index.edges {
        let sources = endpoint_nodes(resolve_endpoint(index, &edge.from));
        let targets = edge_targets(index, edge)
            .into_iter()
            .map(|(node, _)| node)
            .collect::<Vec<_>>();
        if !sources
            .iter()
            .chain(&targets)
            .any(|node| seed_ids.contains(node.id.as_str()))
        {
            continue;
        }
        edges.insert(edge.clone());
        for node in sources.into_iter().chain(targets) {
            nodes.insert(node.id.clone(), node);
        }
    }
}

fn endpoint_nodes(resolution: EndpointResolution) -> Vec<Node> {
    match resolution {
        EndpointResolution::Found(node) => vec![node],
        EndpointResolution::Ambiguous(nodes) => nodes,
        EndpointResolution::Unknown => Vec::new(),
    }
}

fn compose_paths(
    index: &QueryIndex,
    seeds: &[Node],
    max_paths: usize,
) -> Result<ComposedPaths, AtlasError> {
    let mut primary = Vec::new();
    let mut alternatives = Vec::new();
    let mut primary_truncated = false;
    let mut search_truncated = false;
    for left in 0..seeds.len() {
        for right in (left + 1)..seeds.len() {
            let mut enumeration = enumerate_paths(
                index,
                &seeds[left].id,
                Some(&seeds[right].id),
                TraversalLimits {
                    max_depth: 8,
                    max_expansions: 2_000,
                    max_paths,
                },
            )?;
            search_truncated |= enumeration.truncated;
            if enumeration.paths.is_empty() {
                enumeration = enumerate_paths(
                    index,
                    &seeds[right].id,
                    Some(&seeds[left].id),
                    TraversalLimits {
                        max_depth: 8,
                        max_expansions: 2_000,
                        max_paths,
                    },
                )?;
                search_truncated |= enumeration.truncated;
            }
            let Some(first) = enumeration.paths.first().cloned() else {
                continue;
            };
            if primary.len() == max_paths {
                primary_truncated = true;
                continue;
            }
            primary.push(first.clone());
            alternatives.extend(enumeration.paths.into_iter().filter(|path| path != &first));
        }
    }
    sort_paths(&mut alternatives);
    alternatives
        .dedup_by(|left, right| canonical_path_signature(left) == canonical_path_signature(right));
    alternatives.retain(|path| !primary.contains(path));
    let remaining = max_paths.saturating_sub(primary.len());
    let alternatives_truncated = alternatives.len() > remaining;
    if alternatives.len() > remaining {
        alternatives.truncate(remaining);
    }
    let mut reasons = Vec::new();
    if primary_truncated {
        reasons.push("primary-paths-count".into());
    }
    if alternatives_truncated {
        reasons.push("alternative-paths-count".into());
    }
    if search_truncated {
        reasons.push("path-search-limit".into());
    }
    Ok(ComposedPaths {
        primary,
        alternatives,
        truncation_reasons: reasons,
    })
}

fn collect_path(path: &GraphPath, nodes: &mut BTreeMap<String, Node>, edges: &mut BTreeSet<Edge>) {
    for node in &path.nodes {
        nodes.insert(node.id.clone(), node.clone());
    }
    edges.extend(path.hops.iter().map(|hop| hop.edge.clone()));
}

fn source_excerpts(
    code_root: &Path,
    meta: &Meta,
    nodes: &[ExploreNode],
    limits: ExploreBudget,
) -> (Vec<SourceExcerpt>, Vec<ExploreDiagnostic>, bool) {
    let mut excerpts = Vec::new();
    let mut diagnostics = Vec::new();
    let mut files = BTreeSet::new();
    let mut truncated = false;
    for selected in nodes {
        let file = selected.node.file.clone();
        let normalized = match normalize_affected_path(code_root, Path::new(&file)) {
            Ok(normalized) => normalized,
            Err(error) => {
                diagnostics.push(excerpt_diagnostic(
                    "atlas-excerpt-unsafe-source",
                    &file,
                    format!("source path is unsafe: {error}"),
                ));
                continue;
            }
        };
        if !files.insert(normalized.clone()) {
            continue;
        }
        match source_excerpt(
            code_root,
            meta,
            &selected.node,
            &normalized,
            limits.max_excerpt_lines,
        ) {
            Ok(_) if excerpts.len() == limits.max_excerpts => truncated = true,
            Ok(excerpt) => excerpts.push(excerpt),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    (excerpts, diagnostics, truncated)
}

fn source_excerpt(
    code_root: &Path,
    meta: &Meta,
    node: &Node,
    normalized: &str,
    max_lines: usize,
) -> Result<SourceExcerpt, ExploreDiagnostic> {
    let file = node.file.clone();
    let expected_hash = meta.files.get(normalized).ok_or_else(|| {
        excerpt_diagnostic(
            "atlas-excerpt-missing-source",
            &file,
            "source file has no recorded graph hash",
        )
    })?;
    let path = code_root.join(normalized);
    let bytes = std::fs::read(&path).map_err(|error| {
        excerpt_diagnostic(
            "atlas-excerpt-missing-source",
            &file,
            format!("cannot read source file: {error}"),
        )
    })?;
    let current_hash = blake3::hash(&bytes).to_hex().to_string();
    if &current_hash != expected_hash {
        return Err(excerpt_diagnostic(
            "atlas-excerpt-stale-source",
            &file,
            "source bytes no longer match the selected graph shard",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        excerpt_diagnostic(
            "atlas-excerpt-invalid-utf8",
            &file,
            format!("source file is not UTF-8: {error}"),
        )
    })?;
    let lines = text.lines().collect::<Vec<_>>();
    let (line_start, line_end) =
        excerpt_window(lines.len(), node.line_start, node.line_end, max_lines);
    let excerpt_text = if line_start == 0 {
        String::new()
    } else {
        lines[(line_start - 1)..line_end].join("\n")
    };
    Ok(SourceExcerpt {
        file: normalized.into(),
        line_start,
        line_end,
        text: excerpt_text,
        source_hash: expected_hash.clone(),
    })
}

fn excerpt_window(
    total_lines: usize,
    node_start: usize,
    node_end: usize,
    max_lines: usize,
) -> (usize, usize) {
    if total_lines == 0 || max_lines == 0 {
        return (0, 0);
    }
    let target_start = node_start.clamp(1, total_lines);
    let target_end = node_end.clamp(target_start, total_lines);
    let required_end = target_end.min(target_start.saturating_add(max_lines - 1));
    let required_lines = required_end - target_start + 1;
    let context = max_lines.saturating_sub(required_lines);
    let before = (context / 2).min(target_start - 1);
    let mut line_start = target_start - before;
    let line_end = (line_start + max_lines - 1).min(total_lines);
    let selected_lines = line_end - line_start + 1;
    line_start = line_start.saturating_sub(max_lines - selected_lines).max(1);
    (line_start, line_end)
}

fn excerpt_diagnostic(code: &str, file: &str, message: impl Into<String>) -> ExploreDiagnostic {
    ExploreDiagnostic {
        code: code.into(),
        message: message.into(),
        file: Some(file.into()),
    }
}

fn push_reason(reasons: &mut Vec<String>, reason: &str) {
    if !reasons.iter().any(|existing| existing == reason) {
        reasons.push(reason.into());
    }
}

fn refresh_usage(result: &mut ExploreResult) {
    result.usage = BudgetUsage {
        seeds: result.seeds.len(),
        nodes: result.nodes.len(),
        edges: result.edges.len(),
        paths: result.primary_paths.len() + result.alternative_paths.len(),
        excerpts: result.excerpts.len(),
        serialized_bytes: 0,
    };
    for _ in 0..8 {
        let bytes = serde_json::to_vec(result).map_or(0, |bytes| bytes.len());
        if result.usage.serialized_bytes == bytes {
            break;
        }
        result.usage.serialized_bytes = bytes;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::{
        BuildOptions, Capability, EdgeConfidence, EdgeKind, EdgeResolution, GraphIdentity,
        LayerState, LayerStatus, NodeKind, Provenance, SCHEMA_VERSION, build,
    };

    struct Fixture {
        root: PathBuf,
        code: PathBuf,
        graph: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture(name: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "rust-atlas-explore-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        let code = root.join("code");
        let graph = root.join("graph");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"explore-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            "pub fn entry() -> usize { helper() }\nfn helper() -> usize { 42 }\n",
        )
        .unwrap();
        build(
            &code,
            &graph,
            &BuildOptions {
                full: true,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        Fixture { root, code, graph }
    }

    fn node(id: &str, symbol: &str, file: &str) -> Node {
        Node {
            id: id.into(),
            symbol: symbol.into(),
            kind: NodeKind::Fn,
            file: file.into(),
            line_start: 1,
            line_end: 1,
            visibility: "pub".into(),
            signature: format!("fn {symbol}()"),
            doc: None,
        }
    }

    fn edge(from: &str, to: &str, kind: EdgeKind) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            target_text: Some(to.into()),
            resolution: EdgeResolution::Resolved,
            kind,
            provenance: Provenance::Scip,
            site: None,
            extractor: None,
            dispatch: None,
            confidence: Some(EdgeConfidence::Exact),
            candidates: Vec::new(),
            evidence: Some(format!("{from}->{to}")),
        }
    }

    fn index(nodes: Vec<Node>, edges: Vec<Edge>) -> QueryIndex {
        let mut id = BTreeMap::<String, Vec<usize>>::new();
        let mut symbol = BTreeMap::<String, Vec<usize>>::new();
        let mut file = BTreeMap::<String, Vec<usize>>::new();
        let mut incoming = BTreeMap::<String, Vec<usize>>::new();
        let mut outgoing = BTreeMap::<String, Vec<usize>>::new();
        for (position, node) in nodes.iter().enumerate() {
            id.entry(node.id.clone()).or_default().push(position);
            symbol
                .entry(node.symbol.clone())
                .or_default()
                .push(position);
            file.entry(node.file.clone()).or_default().push(position);
        }
        for (position, edge) in edges.iter().enumerate() {
            incoming.entry(edge.to.clone()).or_default().push(position);
            outgoing
                .entry(edge.from.clone())
                .or_default()
                .push(position);
        }
        QueryIndex {
            schema_version: SCHEMA_VERSION,
            graph_fingerprint: "explore-test".into(),
            nodes,
            edges,
            id,
            symbol,
            file,
            incoming,
            outgoing,
        }
    }

    fn status(code: &Path) -> AtlasStatus {
        let identity = GraphIdentity {
            repository_root: code.display().to_string(),
            git_common_dir: None,
            worktree_root: code.display().to_string(),
            graph_root: code.join("graph").display().to_string(),
            toolchain: "test".into(),
        };
        let layer = |state| LayerStatus {
            state,
            recorded_fingerprint: None,
            current_fingerprint: None,
            stale_files: Vec::new(),
            diagnostics: Vec::new(),
        };
        AtlasStatus {
            graph_fingerprint: "explore-test".into(),
            recorded_identity: identity.clone(),
            current_identity: identity,
            worktree_mismatch: None,
            syn: layer(LayerState::Fresh),
            scip: layer(LayerState::Fresh),
            mir: layer(LayerState::Unavailable),
        }
    }

    fn write_sources(code: &Path, files: &[&str]) -> Meta {
        let mut hashes = BTreeMap::new();
        for file in files {
            let path = code.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let source = format!("pub fn {}() {{}}\n", file.replace(['/', '.'], "_"));
            fs::write(&path, source.as_bytes()).unwrap();
            hashes.insert(
                (*file).to_string(),
                blake3::hash(source.as_bytes()).to_hex().to_string(),
            );
        }
        Meta {
            schema_version: SCHEMA_VERSION,
            package: "explore-test".into(),
            packages: vec!["explore-test".into()],
            roots: vec!["src/lib.rs".into()],
            capability: Capability::default(),
            files: hashes,
            graph_fingerprint: "explore-test".into(),
        }
    }

    #[test]
    fn test_atlas_explore_omits_excerpt_when_selected_source_hash_is_stale() {
        let fixture = fixture("stale-excerpt");
        fs::write(fixture.code.join("src/lib.rs"), "pub fn changed() {}\n").unwrap();
        let result = explore(
            &fixture.code,
            &fixture.graph,
            "entry",
            &ExploreOptions {
                profile: ExploreProfile::Compact,
                frozen: true,
            },
        )
        .unwrap();
        assert!(
            result
                .nodes
                .iter()
                .any(|node| node.node.file == "src/lib.rs")
        );
        assert!(
            !result
                .excerpts
                .iter()
                .any(|excerpt| excerpt.file == "src/lib.rs")
        );
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "atlas-excerpt-stale-source"
                && diagnostic.file.as_deref() == Some("src/lib.rs")
        }));
    }

    #[test]
    fn test_atlas_explore_composes_ranked_context_and_relationships() {
        let fixture = fixture("composition");
        let files = [
            "src/entry.rs",
            "src/callee.rs",
            "src/alt.rs",
            "src/api.rs",
            "src/caller.rs",
            "src/impl.rs",
        ];
        let meta = write_sources(&fixture.code, &files);
        let graph = index(
            vec![
                node("entry", "entry", files[0]),
                node("callee", "callee", files[1]),
                node("alt", "alt", files[2]),
                node("api", "api", files[3]),
                node("caller", "caller", files[4]),
                node("impl", "impl", files[5]),
            ],
            vec![
                edge("caller", "entry", EdgeKind::Calls),
                edge("entry", "callee", EdgeKind::Calls),
                edge("callee", "api", EdgeKind::UsesType),
                edge("entry", "alt", EdgeKind::References),
                edge("alt", "api", EdgeKind::Calls),
                edge("impl", "api", EdgeKind::ImplsTrait),
            ],
        );
        let options = ExploreOptions::default();
        let left = explore_index(
            &fixture.code,
            &meta,
            &graph,
            &status(&fixture.code),
            "src/entry.rs api",
            &options,
        )
        .unwrap();
        let right = explore_index(
            &fixture.code,
            &meta,
            &graph,
            &status(&fixture.code),
            "src/entry.rs api",
            &options,
        )
        .unwrap();

        assert_eq!(
            left.seeds
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["entry", "api"]
        );
        assert!(
            left.excerpts
                .iter()
                .all(|excerpt| { meta.files.get(&excerpt.file) == Some(&excerpt.source_hash) })
        );
        assert!(left.edges.iter().any(|edge| edge.kind == EdgeKind::Calls));
        assert!(
            left.edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::ImplsTrait)
        );
        assert_eq!(left.primary_paths.len(), 1);
        assert!(!left.alternative_paths.is_empty());
        assert!(left.impact.iter().any(|entry| entry.node.id == "caller"));
        assert!(left.edges.iter().all(|edge| edge.evidence.is_some()));
        assert_eq!(
            serde_json::to_vec(&left).unwrap(),
            serde_json::to_vec(&right).unwrap()
        );
    }

    #[test]
    fn test_atlas_explore_ranks_query_terms_and_reports_no_match() {
        let fixture = fixture("ranking");
        let meta = write_sources(&fixture.code, &["src/a.rs", "src/b.rs", "src/path.rs"]);
        let graph = index(
            vec![
                node("b", "crate::b::entry", "src/b.rs"),
                node("a", "crate::a::entry", "src/a.rs"),
                node("path", "path_seed", "src/path.rs"),
            ],
            Vec::new(),
        );
        let ranked = explore_index(
            &fixture.code,
            &meta,
            &graph,
            &status(&fixture.code),
            "(src/path.rs), entry entry;",
            &ExploreOptions::default(),
        )
        .unwrap();
        assert_eq!(
            ranked
                .seeds
                .iter()
                .map(|node| node.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["path_seed", "crate::a::entry", "crate::b::entry"]
        );

        let capped_query = (0..32)
            .map(|index| format!("missing{index}"))
            .chain(std::iter::once("entry".into()))
            .collect::<Vec<_>>()
            .join(" ");
        let capped = explore_index(
            &fixture.code,
            &meta,
            &graph,
            &status(&fixture.code),
            &capped_query,
            &ExploreOptions::default(),
        )
        .unwrap();
        assert!(capped.seeds.is_empty());
        assert_eq!(capped.truncation_reasons[0], "query-terms-count");

        let no_match = explore_index(
            &fixture.code,
            &meta,
            &graph,
            &status(&fixture.code),
            "does-not-exist",
            &ExploreOptions::default(),
        )
        .unwrap();
        assert!(no_match.seeds.is_empty());
        assert_eq!(no_match.diagnostics[0].code, "atlas-explore-no-match");
    }

    #[cfg(unix)]
    #[test]
    fn test_atlas_explore_rejects_missing_and_escaping_excerpt_sources() {
        let fixture = fixture("unsafe-sources");
        let outside = fixture.root.join("outside.rs");
        fs::write(&outside, "pub fn outside() {}\n").unwrap();
        std::os::unix::fs::symlink(&outside, fixture.code.join("escape.rs")).unwrap();
        let mut meta = write_sources(&fixture.code, &[]);
        for file in ["src/missing.rs", "../outside.rs", "escape.rs"] {
            meta.files.insert(file.into(), "missing-hash".into());
        }
        let graph = index(
            vec![
                node("missing", "missing", "src/missing.rs"),
                node("outside", "outside", "../outside.rs"),
                node("escape", "escape", "escape.rs"),
            ],
            Vec::new(),
        );
        let result = explore_index(
            &fixture.code,
            &meta,
            &graph,
            &status(&fixture.code),
            "missing outside escape",
            &ExploreOptions::default(),
        )
        .unwrap();
        assert!(result.excerpts.is_empty());
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.file.as_deref(), diagnostic.code.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (Some("../outside.rs"), "atlas-excerpt-unsafe-source"),
                (Some("escape.rs"), "atlas-excerpt-unsafe-source"),
                (Some("src/missing.rs"), "atlas-excerpt-missing-source"),
            ]
        );
    }

    #[test]
    fn source_validation_continues_beyond_excerpt_cap() {
        let fixture = fixture("excerpt-cap-validation");
        let valid_files = [
            "src/valid0.rs",
            "src/valid1.rs",
            "src/valid2.rs",
            "src/valid3.rs",
        ];
        let mut meta = write_sources(&fixture.code, &valid_files);
        fs::write(fixture.code.join("src/stale.rs"), "pub fn new() {}\n").unwrap();
        meta.files.insert("src/stale.rs".into(), "old-hash".into());
        let nodes = valid_files
            .iter()
            .enumerate()
            .map(|(index, file)| ExploreNode {
                node: node(&format!("valid{index}"), &format!("valid{index}"), file),
                seed: true,
                spine: false,
            })
            .chain(std::iter::once(ExploreNode {
                node: node("stale", "stale", "src/stale.rs"),
                seed: true,
                spine: false,
            }))
            .collect::<Vec<_>>();
        let (excerpts, diagnostics, truncated) =
            source_excerpts(&fixture.code, &meta, &nodes, ExploreBudget::compact());
        assert_eq!(excerpts.len(), 4);
        assert!(!truncated, "an ineligible excerpt is not truncation");
        assert_eq!(diagnostics[0].code, "atlas-excerpt-stale-source");
    }

    #[test]
    fn excerpt_validation_rejects_invalid_utf8_and_bounds_line_windows() {
        let fixture = fixture("excerpt-boundaries");
        let invalid = [0xff, 0xfe, b'\n'];
        fs::write(fixture.code.join("src/invalid.rs"), invalid).unwrap();
        let mut meta = write_sources(&fixture.code, &[]);
        meta.files.insert(
            "src/invalid.rs".into(),
            blake3::hash(&invalid).to_hex().to_string(),
        );
        let error = source_excerpt(
            &fixture.code,
            &meta,
            &node("invalid", "invalid", "src/invalid.rs"),
            "src/invalid.rs",
            20,
        )
        .unwrap_err();
        assert_eq!(error.code, "atlas-excerpt-invalid-utf8");

        assert_eq!(excerpt_window(100, 0, 0, 20), (1, 20));
        assert_eq!(excerpt_window(100, 50, 55, 20), (43, 62));
        assert_eq!(excerpt_window(100, 50, 90, 20), (50, 69));
        assert_eq!(excerpt_window(100, 100, 100, 20), (81, 100));
        assert_eq!(excerpt_window(0, 1, 1, 20), (0, 0));
    }
}
