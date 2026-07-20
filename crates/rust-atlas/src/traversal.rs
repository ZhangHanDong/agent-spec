use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{AtlasError, Edge, EdgeConfidence, EdgeResolution, Node, QueryIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathConfidence {
    Exact,
    BoundedCandidates,
    Heuristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathHop {
    pub edge: Edge,
    pub chosen_target: String,
    pub candidate: bool,
    pub direction: PathDirection,
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

impl TraversalLimits {
    pub fn flow_default() -> Self {
        Self {
            max_depth: 8,
            max_expansions: 2_000,
            max_paths: 8,
        }
    }
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

#[derive(Debug, Clone)]
struct PartialPath {
    nodes: Vec<Node>,
    hops: Vec<PathHop>,
}

pub(crate) fn resolve_endpoint(index: &QueryIndex, value: &str) -> EndpointResolution {
    let exact = canonical_nodes(index.matching_nodes(value));
    if !exact.is_empty() {
        return endpoint_resolution(exact);
    }
    endpoint_resolution(canonical_nodes(index.nodes_with_symbol_suffix(value)))
}

pub(crate) fn enumerate_paths(
    index: &QueryIndex,
    start: &str,
    end: Option<&str>,
    limits: TraversalLimits,
) -> Result<PathEnumeration, AtlasError> {
    validate_limits(limits)?;
    let EndpointResolution::Found(start) = resolve_endpoint(index, start) else {
        return Ok(empty_enumeration());
    };
    let end = match end {
        Some(value) => match resolve_endpoint(index, value) {
            EndpointResolution::Found(node) => Some(node.id),
            EndpointResolution::Unknown | EndpointResolution::Ambiguous(_) => {
                return Ok(empty_enumeration());
            }
        },
        None => None,
    };

    let mut queue = VecDeque::from([PartialPath {
        nodes: vec![start],
        hops: Vec::new(),
    }]);
    let mut paths = Vec::new();
    let mut expansions = 0;
    let mut truncated = false;

    while let Some(path) = queue.pop_front() {
        if expansions == limits.max_expansions {
            truncated = true;
            break;
        }
        expansions += 1;

        let Some(current) = path.nodes.last() else {
            return Err(AtlasError::Invariant(
                "traversal queue contained a path without a start node".into(),
            ));
        };
        let complete = match end.as_deref() {
            Some(end) => current.id == end,
            None => !path.hops.is_empty(),
        };
        if complete {
            paths.push(complete_path(&path));
            if end.is_some() {
                continue;
            }
        }

        let neighbors = path_neighbors(index, &path);
        if path.hops.len() == limits.max_depth {
            truncated |= !neighbors.is_empty();
            continue;
        }
        for (node, hop) in neighbors {
            let mut next = path.clone();
            next.nodes.push(node);
            next.hops.push(hop);
            queue.push_back(next);
        }
    }

    let highest_confidence = highest_confidence_path(&paths);
    truncated |= bound_paths(&mut paths, limits.max_paths);
    Ok(PathEnumeration {
        paths,
        highest_confidence,
        expansions,
        truncated,
    })
}

pub(crate) fn confidence_cost(edge: &Edge, candidate: bool) -> usize {
    match edge.confidence {
        Some(EdgeConfidence::Heuristic) => 100,
        Some(EdgeConfidence::BoundedCandidates) => 10,
        None if edge.resolution == EdgeResolution::Unresolved => 100,
        _ if candidate => 10,
        Some(EdgeConfidence::Exact) => 0,
        None if edge.resolution == EdgeResolution::Resolved => 0,
        None => 100,
    }
}

pub(crate) fn path_total_cost(path: &GraphPath) -> usize {
    path.hops
        .iter()
        .map(|hop| confidence_cost(&hop.edge, hop.candidate))
        .sum()
}

pub(crate) fn sort_paths(paths: &mut [GraphPath]) {
    paths.sort_by_cached_key(|path| {
        (
            path.hops.len(),
            path_total_cost(path),
            canonical_path_signature(path),
        )
    });
}

pub(crate) fn highest_confidence_path(paths: &[GraphPath]) -> Option<GraphPath> {
    paths
        .iter()
        .min_by(|left, right| {
            path_total_cost(left)
                .cmp(&path_total_cost(right))
                .then_with(|| left.hops.len().cmp(&right.hops.len()))
                .then_with(|| canonical_path_signature(left).cmp(&canonical_path_signature(right)))
        })
        .cloned()
}

pub(crate) fn bound_paths(paths: &mut Vec<GraphPath>, max_paths: usize) -> bool {
    sort_paths(paths);
    if paths.len() <= max_paths {
        return false;
    }
    let highest_confidence = highest_confidence_path(paths);
    paths.truncate(max_paths);
    if max_paths >= 2
        && let Some(highest_confidence) = highest_confidence
        && !paths.contains(&highest_confidence)
    {
        paths.pop();
        paths.push(highest_confidence);
        sort_paths(paths);
    }
    true
}

pub(crate) fn validate_limits(limits: TraversalLimits) -> Result<(), AtlasError> {
    if !(1..=32).contains(&limits.max_depth) {
        return Err(AtlasError::TraversalLimit {
            detail: format!(
                "max_depth {} is outside the supported range 1..=32",
                limits.max_depth
            ),
        });
    }
    if limits.max_expansions == 0 {
        return Err(AtlasError::TraversalLimit {
            detail: "max_expansions must be greater than zero".into(),
        });
    }
    if limits.max_paths == 0 {
        return Err(AtlasError::TraversalLimit {
            detail: "max_paths must be greater than zero".into(),
        });
    }
    Ok(())
}

fn empty_enumeration() -> PathEnumeration {
    PathEnumeration {
        paths: Vec::new(),
        highest_confidence: None,
        expansions: 0,
        truncated: false,
    }
}

fn endpoint_resolution(nodes: Vec<Node>) -> EndpointResolution {
    match nodes.as_slice() {
        [] => EndpointResolution::Unknown,
        [node] => EndpointResolution::Found(node.clone()),
        _ => EndpointResolution::Ambiguous(nodes),
    }
}

fn canonical_nodes(nodes: Vec<&Node>) -> Vec<Node> {
    let mut nodes: Vec<Node> = nodes.into_iter().cloned().collect();
    nodes.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.symbol.cmp(&right.symbol))
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.line_start.cmp(&right.line_start))
            .then_with(|| left.line_end.cmp(&right.line_end))
    });
    nodes.dedup_by(|left, right| left.id == right.id);
    nodes
}

fn path_neighbors(index: &QueryIndex, path: &PartialPath) -> Vec<(Node, PathHop)> {
    let Some(current) = path.nodes.last() else {
        return Vec::new();
    };
    edge_targets_for(index, index.outgoing_edges_for(&current.id))
        .into_iter()
        .filter(|(node, _)| !path.nodes.iter().any(|visited| visited.id == node.id))
        .collect()
}

pub(crate) fn edge_targets(index: &QueryIndex, edge: &Edge) -> Vec<(Node, PathHop)> {
    edge_targets_for(index, [edge])
}

fn edge_targets_for<'a>(
    index: &QueryIndex,
    edges: impl IntoIterator<Item = &'a Edge>,
) -> Vec<(Node, PathHop)> {
    let mut neighbors = Vec::new();
    for edge in edges {
        for_each_edge_target(index, edge, |node, hop| {
            neighbors.push((node, hop));
            true
        });
    }
    neighbors.sort_by(|(left_node, left_hop), (right_node, right_hop)| {
        left_node
            .id
            .cmp(&right_node.id)
            .then_with(|| left_hop.edge.cmp(&right_hop.edge))
            .then_with(|| left_hop.chosen_target.cmp(&right_hop.chosen_target))
            .then_with(|| left_hop.candidate.cmp(&right_hop.candidate))
    });
    neighbors.dedup_by(|(left_node, left_hop), (right_node, right_hop)| {
        left_node.id == right_node.id && left_hop == right_hop
    });
    neighbors
}

pub(crate) fn for_each_edge_target(
    index: &QueryIndex,
    edge: &Edge,
    mut visitor: impl FnMut(Node, PathHop) -> bool,
) -> bool {
    let candidate = edge.resolution != EdgeResolution::Resolved;
    let targets: Box<dyn Iterator<Item = &str> + '_> = match edge.resolution {
        EdgeResolution::External => return true,
        EdgeResolution::Unresolved if !edge.candidates.is_empty() => {
            Box::new(edge.candidates.iter().map(String::as_str))
        }
        EdgeResolution::Resolved | EdgeResolution::Unresolved => {
            Box::new(std::iter::once(edge.to.as_str()))
        }
    };
    for target in targets {
        for node in index.target_nodes(target) {
            let node = node.clone();
            let hop = PathHop {
                edge: edge.clone(),
                chosen_target: node.id.clone(),
                candidate,
                direction: PathDirection::Forward,
            };
            if !visitor(node, hop) {
                return false;
            }
        }
    }
    true
}

fn complete_path(path: &PartialPath) -> GraphPath {
    graph_path(path.nodes.clone(), path.hops.clone())
}

pub(crate) fn graph_path(nodes: Vec<Node>, hops: Vec<PathHop>) -> GraphPath {
    let confidence = hops
        .iter()
        .map(|hop| confidence_cost(&hop.edge, hop.candidate))
        .max()
        .map_or(PathConfidence::Exact, |cost| match cost {
            0 => PathConfidence::Exact,
            10 => PathConfidence::BoundedCandidates,
            _ => PathConfidence::Heuristic,
        });
    GraphPath {
        nodes,
        hops,
        confidence,
    }
}

pub(crate) fn canonical_path_signature(path: &GraphPath) -> Vec<u8> {
    serde_json::to_vec(&(path.nodes.as_slice(), path.hops.as_slice())).unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::{
        AtlasError, Edge, EdgeConfidence, EdgeKind, EdgeResolution, Node, NodeKind, Provenance,
        QueryIndex,
    };

    fn node(id: &str) -> Node {
        named_node(id, id)
    }

    fn named_node(id: &str, symbol: &str) -> Node {
        Node {
            id: id.into(),
            symbol: symbol.into(),
            kind: NodeKind::Fn,
            file: format!("{id}.rs"),
            line_start: 1,
            line_end: 1,
            visibility: "pub".into(),
            signature: format!("fn {id}()"),
            doc: None,
        }
    }

    fn edge(from: &str, to: &str, confidence: EdgeConfidence) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            target_text: None,
            resolution: EdgeResolution::Resolved,
            kind: EdgeKind::Calls,
            provenance: Provenance::Scip,
            site: None,
            extractor: None,
            dispatch: None,
            confidence: Some(confidence),
            candidates: Vec::new(),
            evidence: None,
        }
    }

    fn index(nodes: &[Node], edges: &[Edge]) -> QueryIndex {
        QueryIndex::from_test_parts("test-graph", nodes.to_vec(), edges.to_vec())
    }

    #[test]
    fn test_atlas_query_surfaces_share_traversal_contract() {
        let root = std::env::temp_dir().join(format!(
            "rust-atlas-shared-traversal-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        let code = root.join("code");
        let graph = root.join("graph");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"shared-traversal\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            "pub mod helper;\npub fn entry() -> usize { helper::helper() }\n",
        )
        .unwrap();
        fs::write(
            code.join("src/helper.rs"),
            "pub fn helper() -> usize { 42 }\n",
        )
        .unwrap();
        crate::build(
            &code,
            &graph,
            &crate::BuildOptions {
                full: true,
                ..crate::BuildOptions::default()
            },
        )
        .unwrap();

        let flow = crate::flow(
            &code,
            &graph,
            crate::FlowQuery::Between {
                from: "shared_traversal".into(),
                to: "shared_traversal::helper".into(),
            },
            &crate::FlowOptions::default(),
        )
        .unwrap();
        let impact = crate::impact(
            &code,
            &graph,
            "shared_traversal",
            &crate::ImpactOptions::default(),
        )
        .unwrap();
        let affected = crate::affected_paths(
            &code,
            &graph,
            &[PathBuf::from("src/lib.rs")],
            &crate::AffectedOptions::default(),
        )
        .unwrap();
        let explore = crate::explore(
            &code,
            &graph,
            "entry helper",
            &crate::ExploreOptions::default(),
        )
        .unwrap();

        let values = [
            serde_json::to_value(flow).unwrap(),
            serde_json::to_value(impact).unwrap(),
            serde_json::to_value(affected).unwrap(),
            serde_json::to_value(explore).unwrap(),
        ];
        let paths = [
            &values[0]["shortest"],
            &values[1]["affected"][0]["path"],
            &values[2]["affected"][0]["path"],
            &values[3]["primary_paths"][0],
        ];
        let expected_path_keys = paths[0].as_object().unwrap().keys().collect::<Vec<_>>();
        let expected_hop_keys = paths[0]["hops"][0]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>();
        let expected_edge_keys = paths[0]["hops"][0]["edge"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>();
        for path in paths {
            assert_eq!(
                path.as_object().unwrap().keys().collect::<Vec<_>>(),
                expected_path_keys
            );
            assert_eq!(
                path["hops"][0]
                    .as_object()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>(),
                expected_hop_keys
            );
            assert_eq!(
                path["hops"][0]["edge"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>(),
                expected_edge_keys
            );
        }
        for value in &values {
            assert_eq!(value["stale"], value["status"]["syn"]["stale_files"]);
        }
        assert_eq!(values[0]["state"], "found");
        assert!(
            values[0]["shortest"]["hops"][0]["edge"]
                .as_object()
                .unwrap()
                .contains_key("evidence")
        );
        assert_eq!(TraversalLimits::flow_default().max_expansions, 2_000);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn candidate_neighbors_keep_original_edge_and_sorted_targets() {
        let mut candidate = edge("a", "unresolved", EdgeConfidence::BoundedCandidates);
        candidate.resolution = EdgeResolution::Unresolved;
        candidate.candidates = vec!["c".into(), "b".into()];
        let graph = index(&[node("a"), node("b"), node("c")], &[candidate]);
        let paths =
            enumerate_paths(&graph, "a", Some("c"), TraversalLimits::flow_default()).unwrap();
        assert_eq!(paths.paths[0].hops[0].chosen_target, "c");
        assert!(paths.paths[0].hops[0].candidate);
        assert_eq!(
            paths.paths[0].hops[0].edge.resolution,
            EdgeResolution::Unresolved
        );
    }

    #[test]
    fn endpoint_resolution_prefers_exact_and_sorts_suffix_candidates() {
        let exact_graph = index(
            &[
                named_node("suffix-b", "crate::b::run"),
                named_node("exact", "run"),
                named_node("suffix-a", "crate::a::run"),
            ],
            &[],
        );
        assert_eq!(
            resolve_endpoint(&exact_graph, "run"),
            EndpointResolution::Found(named_node("exact", "run"))
        );

        let suffix_graph = index(
            &[
                named_node("crate::b::run", "crate::b::run"),
                named_node("crate::a::run", "crate::a::run"),
            ],
            &[],
        );
        let EndpointResolution::Ambiguous(candidates) = resolve_endpoint(&suffix_graph, "run")
        else {
            panic!("suffix resolution should be ambiguous");
        };
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.id.as_str())
                .collect::<Vec<_>>(),
            vec!["crate::a::run", "crate::b::run"]
        );
    }

    #[test]
    fn path_enumeration_terminates_cycles() {
        let graph = index(
            &[node("a"), node("b"), node("c")],
            &[
                edge("b", "a", EdgeConfidence::Exact),
                edge("a", "b", EdgeConfidence::Exact),
                edge("b", "c", EdgeConfidence::Exact),
            ],
        );
        let enumeration =
            enumerate_paths(&graph, "a", Some("c"), TraversalLimits::flow_default()).unwrap();
        assert_eq!(enumeration.paths.len(), 1);
        assert_eq!(
            enumeration.paths[0]
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
        assert!(!enumeration.truncated);
    }

    #[test]
    fn complete_paths_use_canonical_tie_ordering() {
        let graph = index(
            &[node("d"), node("c"), node("b"), node("a")],
            &[
                edge("c", "d", EdgeConfidence::Exact),
                edge("a", "c", EdgeConfidence::Exact),
                edge("b", "d", EdgeConfidence::Exact),
                edge("a", "b", EdgeConfidence::Exact),
            ],
        );
        let enumeration =
            enumerate_paths(&graph, "a", Some("d"), TraversalLimits::flow_default()).unwrap();
        let node_ids = enumeration
            .paths
            .iter()
            .map(|path| {
                path.nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(node_ids, vec![vec!["a", "b", "d"], vec!["a", "c", "d"]]);
    }

    #[test]
    fn max_paths_is_applied_after_confidence_ranking() {
        let graph = index(
            &[node("a"), node("b"), node("c"), node("z")],
            &[
                edge("a", "b", EdgeConfidence::BoundedCandidates),
                edge("b", "z", EdgeConfidence::Exact),
                edge("a", "c", EdgeConfidence::Exact),
                edge("c", "z", EdgeConfidence::Exact),
            ],
        );
        let enumeration = enumerate_paths(
            &graph,
            "a",
            Some("z"),
            TraversalLimits {
                max_paths: 1,
                ..TraversalLimits::flow_default()
            },
        )
        .unwrap();

        assert_eq!(enumeration.paths.len(), 1);
        assert_eq!(enumeration.paths[0].confidence, PathConfidence::Exact);
        assert_eq!(
            enumeration.paths[0]
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "c", "z"]
        );
        assert!(enumeration.truncated);
    }

    #[test]
    fn max_paths_preserves_shortest_and_highest_confidence_extremes() {
        let mut nodes = vec![node("a"), node("x"), node("y"), node("z")];
        let mut edges = vec![
            edge("a", "x", EdgeConfidence::Exact),
            edge("x", "y", EdgeConfidence::Exact),
            edge("y", "z", EdgeConfidence::Exact),
        ];
        for index in 0..8 {
            let middle = format!("b{index}");
            nodes.push(node(&middle));
            edges.push(edge("a", &middle, EdgeConfidence::BoundedCandidates));
            edges.push(edge(&middle, "z", EdgeConfidence::Exact));
        }
        let graph = index(&nodes, &edges);
        let enumeration =
            enumerate_paths(&graph, "a", Some("z"), TraversalLimits::flow_default()).unwrap();

        assert_eq!(enumeration.paths.len(), 8);
        assert!(enumeration.truncated);
        assert!(
            enumeration
                .paths
                .iter()
                .any(|path| { path.confidence == PathConfidence::Exact && path.hops.len() == 3 })
        );
        assert!(enumeration.paths.iter().any(|path| path.hops.len() == 2));
    }

    #[test]
    fn confidence_costs_are_exact_bounded_and_heuristic() {
        let exact = edge("a", "b", EdgeConfidence::Exact);
        let mut implicit = exact.clone();
        implicit.confidence = None;
        let bounded = edge("a", "b", EdgeConfidence::BoundedCandidates);
        let heuristic = edge("a", "b", EdgeConfidence::Heuristic);
        let mut unresolved = implicit.clone();
        unresolved.resolution = EdgeResolution::Unresolved;

        assert_eq!(confidence_cost(&exact, false), 0);
        assert_eq!(confidence_cost(&implicit, false), 0);
        assert_eq!(confidence_cost(&exact, true), 10);
        assert_eq!(confidence_cost(&bounded, false), 10);
        assert_eq!(confidence_cost(&heuristic, false), 100);
        assert_eq!(confidence_cost(&unresolved, false), 100);
    }

    #[test]
    fn unresolved_heuristics_are_marked_and_external_edges_do_not_reenter() {
        let mut heuristic = edge("a", "run", EdgeConfidence::Exact);
        heuristic.resolution = EdgeResolution::Unresolved;
        heuristic.confidence = None;
        let heuristic_graph = index(
            &[node("a"), named_node("crate::run", "crate::run")],
            &[heuristic],
        );
        let enumeration = enumerate_paths(
            &heuristic_graph,
            "a",
            Some("crate::run"),
            TraversalLimits::flow_default(),
        )
        .unwrap();
        assert_eq!(enumeration.paths[0].confidence, PathConfidence::Heuristic);
        assert!(enumeration.paths[0].hops[0].candidate);

        let mut external = edge("a", "run", EdgeConfidence::Exact);
        external.resolution = EdgeResolution::External;
        let external_graph = index(
            &[node("a"), named_node("crate::run", "crate::run")],
            &[external],
        );
        assert!(
            enumerate_paths(
                &external_graph,
                "a",
                Some("crate::run"),
                TraversalLimits::flow_default(),
            )
            .unwrap()
            .paths
            .is_empty()
        );
    }

    #[test]
    fn path_confidence_is_the_worst_hop_class() {
        let graph = index(
            &[node("a"), node("b"), node("c")],
            &[
                edge("a", "b", EdgeConfidence::BoundedCandidates),
                edge("b", "c", EdgeConfidence::Heuristic),
            ],
        );
        let enumeration =
            enumerate_paths(&graph, "a", Some("c"), TraversalLimits::flow_default()).unwrap();
        assert_eq!(enumeration.paths[0].confidence, PathConfidence::Heuristic);
    }

    #[test]
    fn traversal_limits_reject_invalid_values_and_stop_without_overshoot() {
        let graph = index(
            &[node("a"), node("b"), node("c")],
            &[
                edge("a", "c", EdgeConfidence::Exact),
                edge("a", "b", EdgeConfidence::Exact),
            ],
        );
        for limits in [
            TraversalLimits {
                max_depth: 0,
                max_expansions: 1,
                max_paths: 1,
            },
            TraversalLimits {
                max_depth: 33,
                max_expansions: 1,
                max_paths: 1,
            },
            TraversalLimits {
                max_depth: 1,
                max_expansions: 0,
                max_paths: 1,
            },
            TraversalLimits {
                max_depth: 1,
                max_expansions: 1,
                max_paths: 0,
            },
        ] {
            assert!(matches!(
                enumerate_paths(&graph, "a", None, limits),
                Err(AtlasError::TraversalLimit { .. })
            ));
        }

        let enumeration = enumerate_paths(
            &graph,
            "a",
            None,
            TraversalLimits {
                max_depth: 1,
                max_expansions: 2,
                max_paths: 1,
            },
        )
        .unwrap();
        assert_eq!(enumeration.paths.len(), 1);
        assert_eq!(enumeration.expansions, 2);
        assert!(enumeration.truncated);

        let depth_limited = index(
            &[node("a"), node("b"), node("c"), node("d")],
            &[
                edge("a", "b", EdgeConfidence::Exact),
                edge("b", "d", EdgeConfidence::Exact),
                edge("a", "c", EdgeConfidence::Exact),
            ],
        );
        let enumeration = enumerate_paths(
            &depth_limited,
            "a",
            Some("c"),
            TraversalLimits {
                max_depth: 1,
                max_expansions: 4,
                max_paths: 1,
            },
        )
        .unwrap();
        assert_eq!(enumeration.paths.len(), 1);
        assert!(enumeration.truncated);
    }
}
