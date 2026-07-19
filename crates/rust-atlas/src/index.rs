use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{AtlasError, Capability, Edge, Meta, Node, SCHEMA_VERSION, Shard};

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
}

#[derive(Debug, PartialEq, Eq)]
struct Locators {
    id: BTreeMap<String, Vec<usize>>,
    symbol: BTreeMap<String, Vec<usize>>,
    file: BTreeMap<String, Vec<usize>>,
    incoming: BTreeMap<String, Vec<usize>>,
    outgoing: BTreeMap<String, Vec<usize>>,
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

        let mut incoming = BTreeMap::new();
        let mut outgoing = BTreeMap::new();
        for (position, edge) in edges.iter().enumerate() {
            insert_locator(&mut incoming, &edge.to, position);
            insert_locator(&mut outgoing, &edge.from, position);
        }

        Self {
            id,
            symbol,
            file,
            incoming,
            outgoing,
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

        let edges: Vec<Edge> = shards
            .iter()
            .flat_map(|shard| shard.edges.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

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

fn insert_locator(locator: &mut BTreeMap<String, Vec<usize>>, key: &str, position: usize) {
    locator.entry(key.to_string()).or_default().push(position);
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
    let index = QueryIndex::from_graph(meta, shards);
    write_json_atomic(&graph_dir.join(QUERY_INDEX_FILE), &index)?;
    Ok(index)
}

pub fn load_query_index(graph_dir: &Path, meta: &Meta) -> Result<QueryIndex, AtlasError> {
    let path = graph_dir.join(QUERY_INDEX_FILE);
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
        AtlasError, BuildOptions, Capability, EdgeSite, Meta, QueryIndex, SCHEMA_VERSION, build,
        canonical_graph_fingerprint, load_graph, load_query_index, read_meta, write_json_atomic,
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
        let path = graph.join("query-index.json");
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

    #[test]
    fn test_atlas_build_writes_current_query_index() {
        let (code, graph) = build_fixture("atlas-query-index-build");
        let meta = read_meta(&graph).unwrap();
        let index: QueryIndex =
            serde_json::from_str(&fs::read_to_string(graph.join("query-index.json")).unwrap())
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
        let before = fs::read(graph.join("query-index.json")).unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let after = fs::read(graph.join("query-index.json")).unwrap();
        assert_eq!(before, after);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_load_query_index_reports_missing_index_exactly() {
        let (code, graph) = build_fixture("atlas-query-index-missing");
        let meta = read_meta(&graph).unwrap();
        fs::remove_file(graph.join("query-index.json")).unwrap();
        let error = load_query_index(&graph, &meta).unwrap_err();
        assert!(matches!(error, AtlasError::QueryIndexMissing { .. }));
        assert_eq!(
            error.to_string(),
            format!(
                "atlas-query-index-missing: no query index at {}; rebuild with `atlas build`",
                graph.join("query-index.json").display()
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
            graph.join("query-index.json"),
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
            graph.join("query-index.json"),
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
        let meta_path = graph.join("meta.json");
        let mut meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        meta["schema_version"] = serde_json::json!(SCHEMA_VERSION - 1);
        fs::write(meta_path, serde_json::to_vec_pretty(&meta).unwrap()).unwrap();
        fs::remove_file(graph.join("query-index.json")).unwrap();

        let error = load_graph(&graph).unwrap_err();
        assert!(matches!(error, AtlasError::SchemaMismatch { .. }));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atomic_build_writes_parseable_json_without_temporary_files() {
        let (code, graph) = build_fixture("atlas-query-index-atomic-success");
        let _: Meta = serde_json::from_slice(&fs::read(graph.join("meta.json")).unwrap()).unwrap();
        let _: QueryIndex =
            serde_json::from_slice(&fs::read(graph.join("query-index.json")).unwrap()).unwrap();
        let temporary_files: Vec<_> = fs::read_dir(&graph)
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
