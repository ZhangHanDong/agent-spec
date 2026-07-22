use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::traversal::{canonical_edge_signature, confidence_cost};
use crate::{
    AtlasError, Capability, Edge, EdgeResolution, MatchKind, Meta, Node, SCHEMA_VERSION, SearchHit,
    Shard,
};

const QUERY_INDEX_FILE: &str = "query-index.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryIndex {
    pub schema_version: u32,
    pub graph_fingerprint: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub id: BTreeMap<String, Vec<usize>>,
    pub symbol: BTreeMap<String, Vec<usize>>,
    pub file: BTreeMap<String, Vec<usize>>,
    pub incoming: BTreeMap<String, Vec<usize>>,
    pub outgoing: BTreeMap<String, Vec<usize>>,
    pub target: BTreeMap<String, Vec<usize>>,
    incoming_nodes: BTreeMap<String, Vec<AdjacentLocator>>,
    outgoing_nodes: BTreeMap<String, Vec<AdjacentLocator>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AdjacentLocator {
    edge_position: usize,
    node_position: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct Locators {
    id: BTreeMap<String, Vec<usize>>,
    symbol: BTreeMap<String, Vec<usize>>,
    file: BTreeMap<String, Vec<usize>>,
    incoming: BTreeMap<String, Vec<usize>>,
    outgoing: BTreeMap<String, Vec<usize>>,
    target: BTreeMap<String, Vec<usize>>,
    incoming_nodes: BTreeMap<String, Vec<AdjacentLocator>>,
    outgoing_nodes: BTreeMap<String, Vec<AdjacentLocator>>,
}

impl Locators {
    fn from_tables(nodes: &[Node], edges: &[Edge]) -> Self {
        let mut id = BTreeMap::new();
        let mut symbol = BTreeMap::new();
        let mut file = BTreeMap::new();
        for (position, node) in nodes.iter().enumerate() {
            insert_locator(&mut id, &node.id, position);
            insert_locator(&mut symbol, &node.symbol, position);
            insert_locator(&mut file, &node.file, position);
        }

        let mut target = BTreeMap::new();
        for edge in edges {
            for value in edge_target_values(edge) {
                target
                    .entry(value.to_string())
                    .or_insert_with(|| resolve_target_positions(value, &id, &symbol));
            }
        }

        let mut incoming = BTreeMap::new();
        let mut outgoing = BTreeMap::new();
        let mut incoming_nodes = BTreeMap::new();
        let mut outgoing_nodes = BTreeMap::new();
        for (position, edge) in edges.iter().enumerate() {
            insert_locator(&mut outgoing, &edge.from, position);
            let source_position = id
                .get(&edge.from)
                .and_then(|positions| positions.first())
                .copied();
            for value in edge_target_values(edge) {
                for node_position in target.get(value).into_iter().flatten() {
                    if let Some(node) = nodes.get(*node_position) {
                        insert_locator(&mut incoming, &node.id, position);
                        if let Some(source_position) = source_position {
                            incoming_nodes
                                .entry(node.id.clone())
                                .or_insert_with(Vec::new)
                                .push(AdjacentLocator {
                                    edge_position: position,
                                    node_position: source_position,
                                });
                            outgoing_nodes
                                .entry(edge.from.clone())
                                .or_insert_with(Vec::new)
                                .push(AdjacentLocator {
                                    edge_position: position,
                                    node_position: *node_position,
                                });
                        }
                    }
                }
            }
        }
        canonicalize_adjacent_locators(&mut incoming_nodes, nodes, edges);
        canonicalize_adjacent_locators(&mut outgoing_nodes, nodes, edges);

        Self {
            id,
            symbol,
            file,
            incoming,
            outgoing,
            target,
            incoming_nodes,
            outgoing_nodes,
        }
    }
}

impl QueryIndex {
    fn from_graph(meta: &Meta, shards: &[Shard]) -> Self {
        let mut nodes: Vec<Node> = shards
            .iter()
            .flat_map(|shard| shard.nodes.iter().cloned())
            .collect();
        nodes.sort_by(|left, right| left.id.cmp(&right.id));

        let edges = crate::preferred_query_edges(
            shards
                .iter()
                .flat_map(|shard| shard.edges.iter().cloned().map(canonical_edge))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        );

        let locators = Locators::from_tables(&nodes, &edges);

        Self {
            schema_version: meta.schema_version,
            graph_fingerprint: meta.graph_fingerprint.clone(),
            nodes,
            edges,
            id: locators.id,
            symbol: locators.symbol,
            file: locators.file,
            incoming: locators.incoming,
            outgoing: locators.outgoing,
            target: locators.target,
            incoming_nodes: locators.incoming_nodes,
            outgoing_nodes: locators.outgoing_nodes,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_parts(
        graph_fingerprint: &str,
        nodes: Vec<Node>,
        edges: Vec<Edge>,
    ) -> Self {
        let locators = Locators::from_tables(&nodes, &edges);
        Self {
            schema_version: SCHEMA_VERSION,
            graph_fingerprint: graph_fingerprint.into(),
            nodes,
            edges,
            id: locators.id,
            symbol: locators.symbol,
            file: locators.file,
            incoming: locators.incoming,
            outgoing: locators.outgoing,
            target: locators.target,
            incoming_nodes: locators.incoming_nodes,
            outgoing_nodes: locators.outgoing_nodes,
        }
    }

    fn validate_locators(&self) -> Result<(), AtlasError> {
        let expected = Locators::from_tables(&self.nodes, &self.edges);
        for (name, persisted_matches) in [
            ("id", self.id == expected.id),
            ("symbol", self.symbol == expected.symbol),
            ("file", self.file == expected.file),
            ("incoming", self.incoming == expected.incoming),
            ("outgoing", self.outgoing == expected.outgoing),
            ("target", self.target == expected.target),
            (
                "incoming_nodes",
                self.incoming_nodes == expected.incoming_nodes,
            ),
            (
                "outgoing_nodes",
                self.outgoing_nodes == expected.outgoing_nodes,
            ),
        ] {
            if !persisted_matches {
                return Err(AtlasError::QueryIndexCorrupt {
                    detail: format!(
                        "{name} locator does not exactly cover its canonical value table"
                    ),
                });
            }
        }
        Ok(())
    }

    pub fn matching_nodes(&self, value: &str) -> Vec<&Node> {
        self.node_positions(value)
            .into_iter()
            .filter_map(|position| self.nodes.get(position))
            .collect()
    }

    pub fn nodes_with_symbol_suffix(&self, value: &str) -> Vec<&Node> {
        let suffix = format!("::{value}");
        let positions: BTreeSet<usize> = self
            .symbol
            .iter()
            .filter(|(symbol, _)| symbol.as_str() == value || symbol.ends_with(&suffix))
            .flat_map(|(_, positions)| positions.iter().copied())
            .collect();
        positions
            .into_iter()
            .filter_map(|position| self.nodes.get(position))
            .collect()
    }

    pub fn incoming_edges<'a>(
        &'a self,
        node_ids: impl IntoIterator<Item = &'a str>,
    ) -> Vec<&'a Edge> {
        self.edges_at(&self.incoming, node_ids)
    }

    pub fn outgoing_edges<'a>(
        &'a self,
        node_ids: impl IntoIterator<Item = &'a str>,
    ) -> Vec<&'a Edge> {
        self.edges_at(&self.outgoing, node_ids)
    }

    pub fn incoming_edges_for<'a>(&'a self, node_id: &str) -> impl Iterator<Item = &'a Edge> + 'a {
        self.incoming
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|position| self.edges.get(*position))
    }

    pub fn outgoing_edges_for<'a>(&'a self, node_id: &str) -> impl Iterator<Item = &'a Edge> + 'a {
        self.outgoing
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|position| self.edges.get(*position))
    }

    pub fn target_nodes<'a>(&'a self, target: &str) -> impl Iterator<Item = &'a Node> + 'a {
        self.target
            .get(target)
            .into_iter()
            .flatten()
            .filter_map(|position| self.nodes.get(*position))
    }

    pub fn node_by_id(&self, node_id: &str) -> Option<&Node> {
        self.id
            .get(node_id)
            .and_then(|positions| positions.first())
            .and_then(|position| self.nodes.get(*position))
    }

    pub fn incoming_neighbors_for<'a>(
        &'a self,
        node_id: &str,
    ) -> impl Iterator<Item = (&'a Node, &'a Edge)> + 'a {
        self.incoming_nodes
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|locator| {
                Some((
                    self.nodes.get(locator.node_position)?,
                    self.edges.get(locator.edge_position)?,
                ))
            })
    }

    pub fn outgoing_neighbors_for<'a>(
        &'a self,
        node_id: &str,
    ) -> impl Iterator<Item = (&'a Node, &'a Edge)> + 'a {
        self.outgoing_nodes
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|locator| {
                Some((
                    self.nodes.get(locator.node_position)?,
                    self.edges.get(locator.edge_position)?,
                ))
            })
    }

    pub fn search_nodes(&self, query: &str) -> Vec<SearchHit> {
        let mut hits: Vec<SearchHit> = self
            .nodes
            .iter()
            .filter_map(|node| {
                classify_match(node, query).map(|match_kind| SearchHit {
                    score: match_kind.score(),
                    match_kind,
                    node: node.clone(),
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            left.match_kind
                .rank()
                .cmp(&right.match_kind.rank())
                .then_with(|| left.node.symbol.cmp(&right.node.symbol))
                .then_with(|| left.node.file.cmp(&right.node.file))
                .then_with(|| left.node.line_start.cmp(&right.node.line_start))
                .then_with(|| left.node.id.cmp(&right.node.id))
        });
        hits
    }

    fn node_positions(&self, value: &str) -> BTreeSet<usize> {
        self.id
            .get(value)
            .into_iter()
            .chain(self.symbol.get(value))
            .flatten()
            .copied()
            .collect()
    }

    fn edges_at<'a>(
        &'a self,
        locator: &'a BTreeMap<String, Vec<usize>>,
        node_ids: impl IntoIterator<Item = &'a str>,
    ) -> Vec<&'a Edge> {
        let positions: BTreeSet<usize> = node_ids
            .into_iter()
            .filter_map(|node_id| locator.get(node_id))
            .flatten()
            .copied()
            .collect();
        positions
            .into_iter()
            .filter_map(|position| self.edges.get(position))
            .collect()
    }
}

fn canonical_edge(mut edge: Edge) -> Edge {
    edge.candidates.sort();
    edge.candidates.dedup();
    edge
}

fn canonicalize_adjacent_locators(
    locators: &mut BTreeMap<String, Vec<AdjacentLocator>>,
    nodes: &[Node],
    edges: &[Edge],
) {
    for values in locators.values_mut() {
        values.sort_by(|left, right| {
            let left_edge = &edges[left.edge_position];
            let right_edge = &edges[right.edge_position];
            nodes[left.node_position]
                .id
                .cmp(&nodes[right.node_position].id)
                .then_with(|| {
                    confidence_cost(left_edge, left_edge.resolution != EdgeResolution::Resolved)
                        .cmp(&confidence_cost(
                            right_edge,
                            right_edge.resolution != EdgeResolution::Resolved,
                        ))
                })
                .then_with(|| right_edge.provenance.cmp(&left_edge.provenance))
                .then_with(|| {
                    canonical_edge_signature(left_edge).cmp(&canonical_edge_signature(right_edge))
                })
        });
        let mut seen = BTreeSet::new();
        values.retain(|locator| {
            seen.insert((
                nodes[locator.node_position].id.clone(),
                edges[locator.edge_position].kind,
            ))
        });
    }
}

fn classify_match(node: &Node, query: &str) -> Option<MatchKind> {
    if node.id == query {
        return Some(MatchKind::ExactId);
    }
    if node.symbol == query {
        return Some(MatchKind::ExactSymbol);
    }
    if node.id.eq_ignore_ascii_case(query) || node.symbol.eq_ignore_ascii_case(query) {
        return Some(MatchKind::CaseInsensitiveExact);
    }
    if node.symbol.ends_with(&format!("::{query}")) {
        return Some(MatchKind::QualifiedSuffix);
    }
    if segmented_identifier_match(&node.symbol, query) {
        return Some(MatchKind::SegmentedIdentifier);
    }
    let normalized_query = normalize_identifier(query);
    if !normalized_query.is_empty()
        && normalize_identifier(&node.symbol).contains(&normalized_query)
    {
        return Some(MatchKind::NormalizedSubstring);
    }
    None
}

fn segmented_identifier_match(symbol: &str, query: &str) -> bool {
    let Some(identifier) = symbol.rsplit("::").next() else {
        return false;
    };
    let symbol_segments = identifier_segments(identifier);
    let query_segments = identifier_segments(query);
    !query_segments.is_empty() && symbol_segments == query_segments
}

fn identifier_segments(value: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut previous = None;
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if !character.is_ascii_alphanumeric() {
            if !current.is_empty() {
                segments.push(std::mem::take(&mut current));
            }
            previous = None;
            continue;
        }
        let next = characters.peek().copied();
        let starts_new_segment = previous.is_some_and(|previous: char| {
            (previous.is_ascii_lowercase() && character.is_ascii_uppercase())
                || (previous.is_ascii_alphabetic() && character.is_ascii_digit())
                || (previous.is_ascii_digit() && character.is_ascii_alphabetic())
                || (previous.is_ascii_uppercase()
                    && character.is_ascii_uppercase()
                    && next.is_some_and(|next| next.is_ascii_lowercase()))
        });
        if starts_new_segment && !current.is_empty() {
            segments.push(std::mem::take(&mut current));
        }
        current.push(character.to_ascii_lowercase());
        previous = Some(character);
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn insert_locator(locator: &mut BTreeMap<String, Vec<usize>>, key: &str, position: usize) {
    let positions = locator.entry(key.to_string()).or_default();
    if positions.last().copied() != Some(position) {
        positions.push(position);
    }
}

fn edge_target_values(edge: &Edge) -> Box<dyn Iterator<Item = &str> + '_> {
    match edge.resolution {
        crate::EdgeResolution::External => Box::new(std::iter::empty()),
        crate::EdgeResolution::Unresolved if !edge.candidates.is_empty() => {
            Box::new(edge.candidates.iter().map(String::as_str))
        }
        crate::EdgeResolution::Resolved | crate::EdgeResolution::Unresolved => {
            Box::new(std::iter::once(edge.to.as_str()))
        }
    }
}

fn resolve_target_positions(
    value: &str,
    id: &BTreeMap<String, Vec<usize>>,
    symbol: &BTreeMap<String, Vec<usize>>,
) -> Vec<usize> {
    let exact = id
        .get(value)
        .into_iter()
        .chain(symbol.get(value))
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    if !exact.is_empty() {
        return exact.into_iter().collect();
    }
    let suffix = format!("::{value}");
    symbol
        .iter()
        .filter(|(candidate, _)| candidate.ends_with(&suffix))
        .flat_map(|(_, positions)| positions.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Serialize)]
struct CanonicalGraph<'a> {
    schema_version: u32,
    package: &'a str,
    packages: &'a [String],
    roots: &'a [String],
    capability: &'a Capability,
    files: &'a BTreeMap<String, String>,
    shards: &'a [Shard],
}

#[derive(Deserialize)]
struct QueryIndexHeader {
    schema_version: u32,
}

pub fn canonical_graph_fingerprint(meta: &Meta, shards: &[Shard]) -> Result<String, AtlasError> {
    let mut canonical_shards = shards.to_vec();
    for shard in &mut canonical_shards {
        shard.nodes.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then_with(|| left.symbol.cmp(&right.symbol))
                .then_with(|| left.file.cmp(&right.file))
                .then_with(|| left.line_start.cmp(&right.line_start))
                .then_with(|| left.line_end.cmp(&right.line_end))
        });
        shard.edges.sort();
    }
    canonical_shards.sort_by(|left, right| left.file.cmp(&right.file));

    let canonical = CanonicalGraph {
        schema_version: meta.schema_version,
        package: &meta.package,
        packages: &meta.packages,
        roots: &meta.roots,
        capability: &meta.capability,
        files: &meta.files,
        shards: &canonical_shards,
    };
    let bytes =
        serde_json::to_vec(&canonical).map_err(|error| AtlasError::Io(error.to_string()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

pub fn rebuild_query_index(
    graph_dir: &Path,
    meta: &Meta,
    shards: &[Shard],
) -> Result<QueryIndex, AtlasError> {
    let snapshot = crate::generation::resolve_snapshot(graph_dir)?;
    rebuild_query_index_at(&snapshot.data_dir, meta, shards)
}

pub(crate) fn rebuild_query_index_at(
    data_dir: &Path,
    meta: &Meta,
    shards: &[Shard],
) -> Result<QueryIndex, AtlasError> {
    let index = QueryIndex::from_graph(meta, shards);
    write_json_atomic(&data_dir.join(QUERY_INDEX_FILE), &index)?;
    Ok(index)
}

pub fn load_query_index(graph_dir: &Path, meta: &Meta) -> Result<QueryIndex, AtlasError> {
    let snapshot = crate::generation::resolve_snapshot(graph_dir)?;
    load_query_index_at(&snapshot.data_dir, meta)
}

pub(crate) fn load_query_index_at(data_dir: &Path, meta: &Meta) -> Result<QueryIndex, AtlasError> {
    let path = data_dir.join(QUERY_INDEX_FILE);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AtlasError::QueryIndexMissing {
                index_path: path.display().to_string(),
            }
        } else {
            AtlasError::Io(error.to_string())
        }
    })?;
    let header: QueryIndexHeader =
        serde_json::from_str(&text).map_err(|error| AtlasError::QueryIndexCorrupt {
            detail: format!("invalid query index header: {error}"),
        })?;
    if header.schema_version != SCHEMA_VERSION {
        return Err(AtlasError::QueryIndexSchema {
            found: header.schema_version,
            expected: SCHEMA_VERSION,
        });
    }
    let index: QueryIndex =
        serde_json::from_str(&text).map_err(|error| AtlasError::QueryIndexCorrupt {
            detail: format!("invalid query index body: {error}"),
        })?;
    if index.graph_fingerprint != meta.graph_fingerprint {
        return Err(AtlasError::QueryIndexStale {
            found: index.graph_fingerprint,
            expected: meta.graph_fingerprint.clone(),
        });
    }
    index.validate_locators()?;
    Ok(index)
}

pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), AtlasError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| AtlasError::Io(error.to_string()))?;
    bytes.push(b'\n');

    let parent = path
        .parent()
        .ok_or_else(|| AtlasError::Io(format!("{} has no parent directory", path.display())))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AtlasError::Io(format!("{} has no UTF-8 file name", path.display())))?;

    let mut temporary = None;
    for attempt in 0..1024_u32 {
        let candidate = parent.join(format!(".{name}.tmp-{}-{attempt}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(AtlasError::Io(error.to_string())),
        }
    }
    let (temporary_path, mut file) = temporary.ok_or_else(|| {
        AtlasError::Io(format!(
            "could not allocate a temporary file beside {}",
            path.display()
        ))
    })?;

    let write_result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(AtlasError::Io(error.to_string()));
    }
    if let Err(error) = std::fs::rename(&temporary_path, path) {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(AtlasError::Io(error.to_string()));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::{
        AtlasError, BuildOptions, Capability, Edge, EdgeKind, EdgeResolution, EdgeSite, MatchKind,
        Meta, Node, NodeKind, Provenance, QueryIndex, SCHEMA_VERSION, SearchOptions, Shard,
        active_data_dir, build, canonical_graph_fingerprint, load_graph, load_query_index,
        read_meta, search, write_json_atomic,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_fixture(name: &str) -> (PathBuf, PathBuf) {
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
        fs::write(
            code.join("src/lib.rs"),
            "pub mod target;\npub mod unrelated;\n",
        )
        .unwrap();
        fs::write(
            code.join("src/target.rs"),
            "pub trait Service { fn run(&self); }\npub struct Target;\nimpl Service for Target { fn run(&self) {} }\n",
        )
        .unwrap();
        fs::write(code.join("src/unrelated.rs"), "pub fn unrelated() {}\n").unwrap();
        let graph = base.join("graph");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        (code, graph)
    }

    fn rewrite_index(graph: &Path, update: impl FnOnce(&mut serde_json::Value)) {
        let path = active_data_dir(graph).join("query-index.json");
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        update(&mut value);
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }

    fn sample_meta() -> Meta {
        Meta {
            schema_version: SCHEMA_VERSION,
            package: "fixture".to_string(),
            packages: vec!["fixture".to_string()],
            roots: vec!["fixture#crate".to_string()],
            capability: Capability::default(),
            files: BTreeMap::from([("src/lib.rs".to_string(), "hash".to_string())]),
            graph_fingerprint: "ignored".to_string(),
        }
    }

    fn search_node(id: &str, symbol: &str, file: &str, line_start: usize) -> Node {
        Node {
            id: id.to_string(),
            symbol: symbol.to_string(),
            kind: NodeKind::Fn,
            file: file.to_string(),
            line_start,
            line_end: line_start,
            visibility: "pub".to_string(),
            signature: format!("fn {symbol}()"),
            doc: None,
            cfg: None,
        }
    }

    fn test_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            target_text: Some(to.into()),
            resolution: EdgeResolution::Resolved,
            kind: EdgeKind::Calls,
            provenance: Provenance::Scip,
            site: None,
            extractor: None,
            dispatch: None,
            confidence: None,
            candidates: Vec::new(),
            evidence: None,
            generic: false,
        }
    }

    fn write_search_index(graph: &Path, nodes: Vec<Node>) {
        let meta = read_meta(graph).unwrap();
        let index = QueryIndex::from_graph(
            &meta,
            &[Shard {
                file: "src/search.rs".to_string(),
                hash: "search-fixture".to_string(),
                unparsed: None,
                nodes,
                edges: Vec::new(),
            }],
        );
        write_json_atomic(
            &active_data_dir(graph).join(super::QUERY_INDEX_FILE),
            &index,
        )
        .unwrap();
    }

    #[test]
    fn query_index_exposes_lazy_resolved_edge_locators() {
        let meta = sample_meta();
        let nodes = vec![
            search_node("caller-a", "pkg::caller_a", "src/lib.rs", 1),
            search_node("caller-b", "pkg::caller_b", "src/lib.rs", 2),
            search_node("target", "pkg::target", "src/lib.rs", 3),
        ];
        let edges = vec![
            test_edge("caller-a", "target"),
            test_edge("caller-b", "target"),
        ];
        let index = QueryIndex::from_graph(
            &meta,
            &[Shard {
                file: "src/lib.rs".into(),
                hash: "hash".into(),
                unparsed: None,
                nodes,
                edges,
            }],
        );

        assert_eq!(index.outgoing_edges_for("caller-a").count(), 1);
        assert_eq!(index.incoming_edges_for("target").take(1).count(), 1);
        assert_eq!(index.target_nodes("target").count(), 1);
    }

    #[test]
    fn test_query_index_projects_highest_provenance_relations() {
        let meta = sample_meta();
        let nodes = vec![
            search_node("caller", "pkg::caller", "src/lib.rs", 1),
            search_node("target", "pkg::target", "src/lib.rs", 2),
        ];
        let mut syn = test_edge("caller", "target");
        syn.kind = EdgeKind::References;
        syn.provenance = Provenance::Syn;
        let mut scip = test_edge("caller", "target");
        scip.provenance = Provenance::Scip;
        let mut mir = scip.clone();
        mir.provenance = Provenance::Mir;
        let index = QueryIndex::from_graph(
            &meta,
            &[Shard {
                file: "src/lib.rs".into(),
                hash: "hash".into(),
                unparsed: None,
                nodes,
                edges: vec![syn, scip, mir.clone()],
            }],
        );

        assert_eq!(index.edges, vec![mir]);
        assert_eq!(index.outgoing_edges_for("caller").count(), 1);
        assert_eq!(index.incoming_edges_for("target").count(), 1);
    }

    #[test]
    fn test_query_index_adjacent_projection_prefers_exact_edge_over_candidate_target() {
        let meta = sample_meta();
        let nodes = vec![
            search_node("caller", "pkg::caller", "src/lib.rs", 1),
            search_node("a-trait-method", "pkg::Service::run", "src/lib.rs", 2),
            search_node(
                "z-impl-method",
                "pkg::impl Service for Target::run",
                "src/lib.rs",
                3,
            ),
        ];
        let mut exact = test_edge("caller", "z-impl-method");
        exact.provenance = Provenance::Mir;
        exact.confidence = Some(crate::EdgeConfidence::Exact);
        let mut candidate = test_edge("caller", "a-trait-method");
        candidate.resolution = EdgeResolution::Unresolved;
        candidate.confidence = Some(crate::EdgeConfidence::BoundedCandidates);
        candidate.candidates = vec!["z-impl-method".into()];
        let index = QueryIndex::from_graph(
            &meta,
            &[Shard {
                file: "src/lib.rs".into(),
                hash: "hash".into(),
                unparsed: None,
                nodes,
                edges: vec![candidate, exact.clone()],
            }],
        );

        assert_eq!(index.edges.len(), 2);
        let incoming = index
            .incoming_neighbors_for("z-impl-method")
            .collect::<Vec<_>>();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].1, &exact);
        let outgoing = index.outgoing_neighbors_for("caller").collect::<Vec<_>>();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].1, &exact);
    }

    type SearchIndexErrorMatcher = fn(&AtlasError) -> bool;
    type SearchIndexErrorCase = (
        &'static str,
        Option<serde_json::Value>,
        SearchIndexErrorMatcher,
    );

    #[test]
    fn test_atlas_search_orders_exact_suffix_segment_and_fuzzy_matches() {
        let (code, graph) = build_fixture("atlas-search-ranks");
        write_search_index(
            &graph,
            vec![
                search_node("mem_store", "pkg::id", "src/z.rs", 9),
                search_node("symbol", "mem_store", "src/y.rs", 8),
                search_node("case", "MEM_STORE", "src/x.rs", 7),
                search_node("suffix", "pkg::mem_store", "src/w.rs", 6),
                search_node("segment", "pkg::MemStore", "src/v.rs", 5),
                search_node("fuzzy", "pkg::SomeMemStoreThing", "src/u.rs", 4),
            ],
        );

        let options = SearchOptions {
            limit: 20,
            frozen: true,
        };
        let first = search(&code, &graph, "mem_store", &options).unwrap();
        let kinds: Vec<_> = first.matches.iter().map(|hit| hit.match_kind).collect();
        let scores: Vec<_> = first.matches.iter().map(|hit| hit.score).collect();
        assert_eq!(
            kinds,
            vec![
                MatchKind::ExactId,
                MatchKind::ExactSymbol,
                MatchKind::CaseInsensitiveExact,
                MatchKind::QualifiedSuffix,
                MatchKind::SegmentedIdentifier,
                MatchKind::NormalizedSubstring,
            ]
        );
        assert_eq!(scores, vec![600, 500, 400, 300, 200, 100]);
        assert_eq!(first.limit, 20);
        assert_eq!(
            first.graph_fingerprint,
            read_meta(&graph).unwrap().graph_fingerprint
        );
        assert!(first.stale.is_empty());
        assert_eq!(first.matches[0].node.id, "mem_store");
        assert_eq!(first.matches[0].node.signature, "fn pkg::id()");
        let second = search(&code, &graph, "mem_store", &options).unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_search_sorts_ties_and_truncates_to_limit() {
        let (code, graph) = build_fixture("atlas-search-ties");
        write_search_index(
            &graph,
            vec![
                search_node("z", "pkg::MemStore", "src/z.rs", 3),
                search_node("b", "pkg::MemStore", "src/a.rs", 2),
                search_node("a", "pkg::MemStore", "src/a.rs", 2),
                search_node("d", "pkg::MemStore", "src/a.rs", 1),
                search_node("symbol-a", "a::MemStore", "src/z.rs", 9),
                search_node("c", "pkg::AMemStore", "src/x.rs", 1),
            ],
        );

        let result = search(
            &code,
            &graph,
            "mem_store",
            &SearchOptions {
                limit: 5,
                frozen: true,
            },
        )
        .unwrap();
        let ids: Vec<_> = result
            .matches
            .iter()
            .map(|hit| hit.node.id.as_str())
            .collect();
        assert_eq!(result.limit, 5);
        assert_eq!(ids, vec!["symbol-a", "d", "a", "b", "z"]);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_search_segments_acronyms_and_digit_boundaries() {
        let (code, graph) = build_fixture("atlas-search-segment-boundaries");
        write_search_index(
            &graph,
            vec![
                search_node("http", "pkg::HTTPServer", "src/http.rs", 1),
                search_node("digits", "pkg::Foo2Bar", "src/digits.rs", 2),
            ],
        );
        let options = SearchOptions {
            limit: 20,
            frozen: true,
        };

        let http = search(&code, &graph, "http_server", &options).unwrap();
        assert_eq!(http.matches.len(), 1);
        assert_eq!(http.matches[0].match_kind, MatchKind::SegmentedIdentifier);

        let digits = search(&code, &graph, "foo_2_bar", &options).unwrap();
        assert_eq!(digits.matches.len(), 1);
        assert_eq!(digits.matches[0].match_kind, MatchKind::SegmentedIdentifier);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_search_rejects_limit_outside_range() {
        assert_eq!(SearchOptions::default().limit, 20);
        assert!(!SearchOptions::default().frozen);
        for limit in [0, 201] {
            let error = crate::validate_search_limit(limit).unwrap_err();
            assert!(error.to_string().contains("atlas-search-limit"));
        }
    }

    #[test]
    fn test_atlas_search_propagates_index_errors_without_shard_fallback() {
        let cases: [SearchIndexErrorCase; 4] = [
            ("missing", None, |error| {
                matches!(error, AtlasError::QueryIndexMissing { .. })
            }),
            (
                "schema",
                Some(serde_json::json!(SCHEMA_VERSION - 1)),
                |error| matches!(error, AtlasError::QueryIndexSchema { .. }),
            ),
            ("stale", Some(serde_json::json!("wrong")), |error| {
                matches!(error, AtlasError::QueryIndexStale { .. })
            }),
            (
                "corrupt",
                Some(serde_json::json!("not a node table")),
                |error| matches!(error, AtlasError::QueryIndexCorrupt { .. }),
            ),
        ];

        for (case, replacement, matches_error) in cases {
            let (code, graph) = build_fixture(&format!("atlas-search-index-{case}"));
            let data_dir = active_data_dir(&graph);
            fs::write(
                data_dir.join("shards").join("src%2Flib.rs.json"),
                "not valid JSON",
            )
            .unwrap();
            let index_path = data_dir.join(super::QUERY_INDEX_FILE);
            match replacement {
                None => fs::remove_file(&index_path).unwrap(),
                Some(value) => {
                    let mut index: serde_json::Value =
                        serde_json::from_str(&fs::read_to_string(&index_path).unwrap()).unwrap();
                    if case == "schema" {
                        index["schema_version"] = value;
                    } else if case == "stale" {
                        index["graph_fingerprint"] = value;
                    } else {
                        index["nodes"] = value;
                    }
                    fs::write(&index_path, serde_json::to_vec_pretty(&index).unwrap()).unwrap();
                }
            }
            let error = search(
                &code,
                &graph,
                "anything",
                &SearchOptions {
                    limit: 20,
                    frozen: true,
                },
            )
            .unwrap_err();
            assert!(matches_error(&error), "{case}: {error}");
            fs::remove_dir_all(code.parent().unwrap()).ok();
        }
    }

    #[test]
    fn test_atlas_build_writes_current_query_index() {
        let (code, graph) = build_fixture("atlas-query-index-build");
        let meta = read_meta(&graph).unwrap();
        let index: QueryIndex = serde_json::from_str(
            &fs::read_to_string(active_data_dir(&graph).join("query-index.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(index.schema_version, SCHEMA_VERSION);
        assert_eq!(index.graph_fingerprint, meta.graph_fingerprint);
        assert!(!index.nodes.is_empty());
        assert!(!index.edges.is_empty());
        let target_positions = &index.symbol["atlas_query_index_build::target::Target"];
        let target = &index.nodes[target_positions[0]];
        assert_eq!(index.id[&target.id], target_positions.as_slice());
        assert!(index.file.contains_key("src/target.rs"));
        assert!(!index.incoming.is_empty());
        assert!(!index.outgoing.is_empty());

        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_graph_fingerprint_excludes_itself_and_is_canonical() {
        let mut meta = sample_meta();
        let first = canonical_graph_fingerprint(&meta, &[]).unwrap();
        meta.graph_fingerprint = "a previous fingerprint".to_string();
        let second = canonical_graph_fingerprint(&meta, &[]).unwrap();
        assert_eq!(first, second);

        meta.files = BTreeMap::from([
            ("src/z.rs".to_string(), "z".to_string()),
            ("src/a.rs".to_string(), "a".to_string()),
        ]);
        let ordered = canonical_graph_fingerprint(&meta, &[]).unwrap();
        meta.files = BTreeMap::from([
            ("src/a.rs".to_string(), "a".to_string()),
            ("src/z.rs".to_string(), "z".to_string()),
        ]);
        assert_eq!(ordered, canonical_graph_fingerprint(&meta, &[]).unwrap());
    }

    #[test]
    fn test_graph_fingerprint_changes_with_edge_site_and_evidence() {
        let (code, graph) = build_fixture("atlas-query-index-fingerprint-facts");
        let (meta, shards) = load_graph(&graph).unwrap();
        let original = canonical_graph_fingerprint(&meta, &shards).unwrap();

        let mut evidence_changed = shards.clone();
        evidence_changed
            .iter_mut()
            .find_map(|shard| shard.edges.first_mut())
            .unwrap()
            .evidence = Some("different evidence".to_string());
        assert_ne!(
            original,
            canonical_graph_fingerprint(&meta, &evidence_changed).unwrap()
        );

        let mut site_changed = shards.clone();
        site_changed
            .iter_mut()
            .find_map(|shard| shard.edges.first_mut())
            .unwrap()
            .site = Some(EdgeSite {
            file: "src/lib.rs".to_string(),
            line_start: 1,
            column_start: 1,
            line_end: 1,
            column_end: 2,
        });
        assert_ne!(
            original,
            canonical_graph_fingerprint(&meta, &site_changed).unwrap()
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_query_index_serialization_is_deterministic() {
        let (code, graph) = build_fixture("atlas-query-index-deterministic");
        let before = fs::read(active_data_dir(&graph).join("query-index.json")).unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let after = fs::read(active_data_dir(&graph).join("query-index.json")).unwrap();
        assert_eq!(before, after);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_load_query_index_reports_missing_index_exactly() {
        let (code, graph) = build_fixture("atlas-query-index-missing");
        let meta = read_meta(&graph).unwrap();
        let index_path = active_data_dir(&graph).join("query-index.json");
        fs::remove_file(&index_path).unwrap();
        let error = load_query_index(&graph, &meta).unwrap_err();
        assert!(matches!(error, AtlasError::QueryIndexMissing { .. }));
        assert_eq!(
            error.to_string(),
            format!(
                "atlas-query-index-missing: no query index at {}; rebuild with `atlas build`",
                index_path.display()
            )
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_load_query_index_reports_schema_mismatch_exactly() {
        let (code, graph) = build_fixture("atlas-query-index-schema");
        let meta = read_meta(&graph).unwrap();
        rewrite_index(&graph, |value| {
            value["schema_version"] = serde_json::json!(SCHEMA_VERSION - 1);
        });
        let error = load_query_index(&graph, &meta).unwrap_err();
        assert!(matches!(error, AtlasError::QueryIndexSchema { .. }));
        assert_eq!(
            error.to_string(),
            format!(
                "atlas-query-index-schema: query index schema v{} != binary v{}; rebuild with `atlas build`",
                SCHEMA_VERSION - 1,
                SCHEMA_VERSION
            )
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_load_query_index_reports_fingerprint_mismatch_exactly() {
        let (code, graph) = build_fixture("atlas-query-index-stale");
        let meta = read_meta(&graph).unwrap();
        rewrite_index(&graph, |value| {
            value["graph_fingerprint"] = serde_json::json!("wrong");
        });
        let error = load_query_index(&graph, &meta).unwrap_err();
        assert!(matches!(error, AtlasError::QueryIndexStale { .. }));
        assert_eq!(
            error.to_string(),
            format!(
                "atlas-query-index-stale: query index fingerprint wrong != graph fingerprint {}; rebuild with `atlas build`",
                meta.graph_fingerprint
            )
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    fn assert_locator_corrupt(
        name: &str,
        update: impl FnOnce(&mut serde_json::Value),
    ) -> AtlasError {
        let (code, graph) = build_fixture(name);
        let meta = read_meta(&graph).unwrap();
        rewrite_index(&graph, update);
        let error = load_query_index(&graph, &meta).unwrap_err();
        fs::remove_dir_all(code.parent().unwrap()).ok();
        error
    }

    #[test]
    fn test_load_query_index_rejects_out_of_bounds_locator() {
        let error = assert_locator_corrupt("atlas-query-index-locator-bounds", |value| {
            let positions = value["outgoing"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap()
                .as_array_mut()
                .unwrap();
            positions[0] = serde_json::json!(usize::MAX);
        });
        assert!(matches!(error, AtlasError::QueryIndexCorrupt { .. }));
        assert!(error.to_string().contains("atlas-query-index-corrupt"));
        assert!(error.to_string().contains("atlas build"));
    }

    #[test]
    fn test_load_query_index_rejects_wrong_locator_key() {
        let error = assert_locator_corrupt("atlas-query-index-locator-key", |value| {
            let locator = value["id"].as_object_mut().unwrap();
            let key = locator.keys().next().unwrap().clone();
            let positions = locator.remove(&key).unwrap();
            locator.insert(format!("{key}-wrong"), positions);
        });
        assert!(matches!(error, AtlasError::QueryIndexCorrupt { .. }));
    }

    #[test]
    fn test_load_query_index_rejects_missing_locator_entry() {
        let error = assert_locator_corrupt("atlas-query-index-locator-missing", |value| {
            let locator = value["file"].as_object_mut().unwrap();
            let key = locator.keys().next().unwrap().clone();
            locator.remove(&key);
        });
        assert!(matches!(error, AtlasError::QueryIndexCorrupt { .. }));
    }

    #[test]
    fn test_load_query_index_rejects_duplicate_locator_position() {
        let error = assert_locator_corrupt("atlas-query-index-locator-duplicate", |value| {
            let positions = value["symbol"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap()
                .as_array_mut()
                .unwrap();
            positions.push(positions[0].clone());
        });
        assert!(matches!(error, AtlasError::QueryIndexCorrupt { .. }));
    }

    #[test]
    fn test_old_schema_precedes_incompatible_query_index_body() {
        let (code, graph) = build_fixture("atlas-query-index-old-schema-body");
        let meta = read_meta(&graph).unwrap();
        fs::write(
            active_data_dir(&graph).join("query-index.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": SCHEMA_VERSION - 1,
                "nodes": "incompatible",
                "edges": false
            }))
            .unwrap(),
        )
        .unwrap();

        let error = load_query_index(&graph, &meta).unwrap_err();
        assert!(matches!(error, AtlasError::QueryIndexSchema { .. }));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_current_schema_malformed_query_index_is_corrupt() {
        let (code, graph) = build_fixture("atlas-query-index-current-schema-body");
        let meta = read_meta(&graph).unwrap();
        fs::write(
            active_data_dir(&graph).join("query-index.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": SCHEMA_VERSION,
                "nodes": "incompatible"
            }))
            .unwrap(),
        )
        .unwrap();

        let error = load_query_index(&graph, &meta).unwrap_err();
        assert!(matches!(error, AtlasError::QueryIndexCorrupt { .. }));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_graph_schema_mismatch_precedes_query_index_errors() {
        let (code, graph) = build_fixture("atlas-query-index-schema-priority");
        let data_dir = active_data_dir(&graph);
        let meta_path = data_dir.join("meta.json");
        let mut meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        meta["schema_version"] = serde_json::json!(SCHEMA_VERSION - 1);
        fs::write(meta_path, serde_json::to_vec_pretty(&meta).unwrap()).unwrap();
        fs::remove_file(data_dir.join("query-index.json")).unwrap();

        let error = load_graph(&graph).unwrap_err();
        assert!(matches!(error, AtlasError::SchemaMismatch { .. }));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atomic_build_writes_parseable_json_without_temporary_files() {
        let (code, graph) = build_fixture("atlas-query-index-atomic-success");
        let data_dir = active_data_dir(&graph);
        let _: Meta =
            serde_json::from_slice(&fs::read(data_dir.join("meta.json")).unwrap()).unwrap();
        let _: QueryIndex =
            serde_json::from_slice(&fs::read(data_dir.join("query-index.json")).unwrap()).unwrap();
        let temporary_files: Vec<_> = fs::read_dir(&data_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp-"))
            .collect();
        assert!(temporary_files.is_empty(), "{temporary_files:?}");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atomic_write_failure_keeps_final_path_unmodified() {
        let dir = temp_dir("atlas-query-index-atomic-failure");
        let final_path = dir.join("query-index.json");
        fs::create_dir(&final_path).unwrap();

        let error = write_json_atomic(&final_path, &sample_meta()).unwrap_err();
        assert!(matches!(error, AtlasError::Io(_)));
        assert!(final_path.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).ok();
    }
}
