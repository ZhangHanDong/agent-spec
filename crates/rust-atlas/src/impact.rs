use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use crate::traversal::{
    EndpointResolution, canonical_path_signature, edge_targets, graph_path, resolve_endpoint,
};
use crate::{
    AtlasError, AtlasStatus, EdgeKind, GraphPath, Node, PathDirection, PathHop, QueryIndex,
    QueryOptions, indexed_query_state,
};

#[derive(Debug, Clone)]
pub struct ImpactOptions {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub frozen: bool,
}

impl Default for ImpactOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_nodes: 200,
            frozen: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone)]
struct Candidate {
    node: Node,
    path: GraphPath,
}

#[derive(Debug)]
pub(crate) struct ImpactTraversal {
    pub affected: Vec<ImpactEntry>,
    pub truncated: bool,
    pub diagnostics: Vec<ImpactDiagnostic>,
    #[cfg(test)]
    peak_frontier: usize,
}

pub fn impact(
    code_root: &Path,
    graph_dir: &Path,
    symbol: &str,
    options: &ImpactOptions,
) -> Result<ImpactResult, AtlasError> {
    let (_, index, status) = indexed_query_state(
        code_root,
        graph_dir,
        &QueryOptions {
            frozen: options.frozen,
        },
    )?;
    let seed = match resolve_endpoint(&index, symbol) {
        EndpointResolution::Found(node) => node,
        EndpointResolution::Unknown => {
            return Err(AtlasError::UnknownSymbol {
                symbol: symbol.into(),
            });
        }
        EndpointResolution::Ambiguous(candidates) => {
            return Err(AtlasError::AmbiguousSymbol {
                symbol: symbol.into(),
                declarations: candidates.len(),
            });
        }
    };
    impact_index(&index, &seed, options, &status)
}

pub(crate) fn impact_index(
    index: &QueryIndex,
    seed: &Node,
    options: &ImpactOptions,
    status: &AtlasStatus,
) -> Result<ImpactResult, AtlasError> {
    let traversal = impact_many_index(index, std::slice::from_ref(seed), options)?;
    Ok(ImpactResult {
        schema: "agent-spec/rust-atlas/impact-v1".into(),
        seed: seed.clone(),
        affected: traversal.affected,
        truncated: traversal.truncated,
        diagnostics: traversal.diagnostics,
        status: status.clone(),
        stale: status.syn.stale_files.clone(),
    })
}

pub(crate) fn impact_many_index(
    index: &QueryIndex,
    seeds: &[Node],
    options: &ImpactOptions,
) -> Result<ImpactTraversal, AtlasError> {
    validate_options(options)?;
    let mut canonical_seeds = BTreeMap::new();
    let mut truncated = false;
    for seed in seeds {
        canonical_seeds.insert(seed.id.clone(), seed.clone());
        if canonical_seeds.len() > options.max_nodes {
            canonical_seeds.pop_last();
            truncated = true;
        }
    }
    let seed_ids = canonical_seeds.keys().cloned().collect::<BTreeSet<_>>();
    let candidate_limit = seed_ids.len().saturating_add(options.max_nodes);
    let mut layer = canonical_seeds
        .into_values()
        .map(|seed| {
            (
                seed.id.clone(),
                Candidate {
                    path: graph_path(vec![seed.clone()], Vec::new()),
                    node: seed,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut settled = BTreeSet::new();
    let mut affected = Vec::new();
    #[cfg(test)]
    let mut peak_frontier = layer.len();

    'distances: for distance in 0..=options.max_depth {
        truncated |= containment_closure(index, &mut layer, &settled, candidate_limit);
        #[cfg(test)]
        {
            peak_frontier = peak_frontier.max(layer.len());
        }
        let mut candidates = layer.into_values().collect::<Vec<_>>();
        candidates.sort_by_cached_key(|candidate| {
            (
                candidate.node.id.clone(),
                canonical_path_signature(&candidate.path),
            )
        });
        for candidate in &candidates {
            if settled.contains(&candidate.node.id) {
                continue;
            }
            if !seed_ids.contains(&candidate.node.id) && affected.len() == options.max_nodes {
                truncated = true;
                break 'distances;
            }
            settled.insert(candidate.node.id.clone());
            if !seed_ids.contains(&candidate.node.id) {
                affected.push(ImpactEntry {
                    node: candidate.node.clone(),
                    distance,
                    path: candidate.path.clone(),
                });
            }
        }

        let mut next = BTreeMap::new();
        for candidate in &candidates {
            let (neighbors, neighbors_truncated) =
                dependency_neighbors(index, &candidate.node, candidate_limit);
            truncated |= neighbors_truncated;
            for (node, hop) in neighbors {
                if settled.contains(&node.id)
                    || candidate
                        .path
                        .nodes
                        .iter()
                        .any(|visited| visited.id == node.id)
                {
                    continue;
                }
                let outcome = insert_best_bounded(
                    &mut next,
                    Candidate {
                        path: append_neighbor(&candidate.path, node.clone(), hop),
                        node,
                    },
                    candidate_limit,
                );
                truncated |= outcome.truncated;
                #[cfg(test)]
                {
                    peak_frontier = peak_frontier.max(next.len());
                }
            }
        }
        if distance == options.max_depth {
            if !next.is_empty() {
                truncated = true;
            }
            break;
        }
        if next.is_empty() {
            break;
        }
        layer = next;
    }

    affected.sort_by(|left, right| {
        left.distance
            .cmp(&right.distance)
            .then_with(|| left.node.id.cmp(&right.node.id))
    });
    let diagnostics = truncated
        .then(|| ImpactDiagnostic {
            code: "atlas-impact-truncated".into(),
            message: format!(
                "impact traversal reached depth {} or node limit {}",
                options.max_depth, options.max_nodes
            ),
        })
        .into_iter()
        .collect();
    Ok(ImpactTraversal {
        affected,
        truncated,
        diagnostics,
        #[cfg(test)]
        peak_frontier,
    })
}

fn containment_closure(
    index: &QueryIndex,
    layer: &mut BTreeMap<String, Candidate>,
    settled: &BTreeSet<String>,
    limit: usize,
) -> bool {
    let mut frontier = layer.values().cloned().collect::<Vec<_>>();
    let mut truncated = false;
    while !frontier.is_empty() {
        frontier.sort_by_cached_key(|candidate| {
            (
                canonical_path_signature(&candidate.path),
                candidate.node.id.clone(),
            )
        });
        let candidate = frontier.remove(0);
        if layer.get(&candidate.node.id).is_none_or(|current| {
            canonical_path_signature(&current.path) != canonical_path_signature(&candidate.path)
        }) {
            continue;
        }
        let (neighbors, neighbors_truncated) = containment_neighbors(index, &candidate.node, limit);
        truncated |= neighbors_truncated;
        for (node, hop) in neighbors {
            if settled.contains(&node.id)
                || candidate
                    .path
                    .nodes
                    .iter()
                    .any(|visited| visited.id == node.id)
            {
                continue;
            }
            let next = Candidate {
                path: append_neighbor(&candidate.path, node.clone(), hop),
                node,
            };
            let outcome = insert_best_bounded(layer, next.clone(), limit);
            truncated |= outcome.truncated;
            if outcome.inserted {
                frontier.push(next);
            }
        }
    }
    truncated
}

pub(crate) fn validate_options(options: &ImpactOptions) -> Result<(), AtlasError> {
    if !(1..=8).contains(&options.max_depth) {
        return Err(AtlasError::TraversalLimit {
            detail: format!(
                "impact max_depth {} is outside the supported range 1..=8",
                options.max_depth
            ),
        });
    }
    if options.max_nodes == 0 {
        return Err(AtlasError::TraversalLimit {
            detail: "impact max_nodes must be greater than zero".into(),
        });
    }
    Ok(())
}

fn insert_best(layer: &mut BTreeMap<String, Candidate>, candidate: Candidate) -> bool {
    let signature = canonical_path_signature(&candidate.path);
    if layer
        .get(&candidate.node.id)
        .is_some_and(|current| canonical_path_signature(&current.path) <= signature)
    {
        return false;
    }
    layer.insert(candidate.node.id.clone(), candidate);
    true
}

#[derive(Debug, Clone, Copy)]
struct BoundedInsert {
    inserted: bool,
    truncated: bool,
}

fn insert_best_bounded(
    layer: &mut BTreeMap<String, Candidate>,
    candidate: Candidate,
    limit: usize,
) -> BoundedInsert {
    let candidate_id = candidate.node.id.clone();
    let inserted = insert_best(layer, candidate);
    if !inserted || layer.len() <= limit {
        return BoundedInsert {
            inserted,
            truncated: false,
        };
    }
    let removed = layer.pop_last().map(|(id, _)| id);
    BoundedInsert {
        inserted: removed.as_deref() != Some(candidate_id.as_str()),
        truncated: true,
    }
}

fn append_neighbor(path: &GraphPath, node: Node, hop: PathHop) -> GraphPath {
    let mut nodes = path.nodes.clone();
    nodes.push(node);
    let mut hops = path.hops.clone();
    hops.push(hop);
    graph_path(nodes, hops)
}

fn containment_neighbors(
    index: &QueryIndex,
    current: &Node,
    limit: usize,
) -> (Vec<(Node, PathHop)>, bool) {
    let mut neighbors = BTreeMap::new();
    let mut truncated = false;
    for edge in index
        .outgoing_edges([current.id.as_str()])
        .into_iter()
        .filter(|edge| edge.kind == EdgeKind::Contains)
    {
        for neighbor in edge_targets(index, edge) {
            truncated |= insert_bounded_neighbor(&mut neighbors, neighbor, limit);
        }
    }
    (neighbors.into_values().collect(), truncated)
}

fn dependency_neighbors(
    index: &QueryIndex,
    current: &Node,
    limit: usize,
) -> (Vec<(Node, PathHop)>, bool) {
    let mut neighbors = BTreeMap::new();
    let mut truncated = false;
    for edge in &index.edges {
        if !matches!(
            edge.kind,
            EdgeKind::Calls
                | EdgeKind::References
                | EdgeKind::UsesType
                | EdgeKind::ImplsTrait
                | EdgeKind::ImplFor
        ) {
            continue;
        }
        let Some((_, mut hop)) = edge_targets(index, edge)
            .into_iter()
            .find(|(target, _)| target.id == current.id)
        else {
            continue;
        };
        let EndpointResolution::Found(dependent) = resolve_endpoint(index, &edge.from) else {
            continue;
        };
        hop.direction = PathDirection::Reverse;
        truncated |= insert_bounded_neighbor(&mut neighbors, (dependent, hop), limit);
    }
    (neighbors.into_values().collect(), truncated)
}

type NeighborKey = (String, crate::Edge, String, bool, u8);

fn insert_bounded_neighbor(
    neighbors: &mut BTreeMap<NeighborKey, (Node, PathHop)>,
    neighbor: (Node, PathHop),
    limit: usize,
) -> bool {
    let direction = match neighbor.1.direction {
        PathDirection::Forward => 0,
        PathDirection::Reverse => 1,
    };
    let key = (
        neighbor.0.id.clone(),
        neighbor.1.edge.clone(),
        neighbor.1.chosen_target.clone(),
        neighbor.1.candidate,
        direction,
    );
    neighbors.entry(key).or_insert(neighbor);
    if neighbors.len() <= limit {
        return false;
    }
    neighbors.pop_last();
    true
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        AtlasStatus, Edge, EdgeConfidence, EdgeKind, EdgeResolution, GraphIdentity, LayerState,
        LayerStatus, Node, NodeKind, Provenance, QueryIndex, SCHEMA_VERSION,
    };

    fn node(id: &str, kind: NodeKind) -> Node {
        Node {
            id: id.into(),
            symbol: id.into(),
            kind,
            file: format!("{id}.rs"),
            line_start: 1,
            line_end: 1,
            visibility: "pub".into(),
            signature: id.into(),
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
            graph_fingerprint: "impact-test".into(),
            nodes,
            edges,
            id,
            symbol,
            file,
            incoming,
            outgoing,
        }
    }

    fn status() -> AtlasStatus {
        let identity = GraphIdentity {
            repository_root: "/repo".into(),
            git_common_dir: None,
            worktree_root: "/repo".into(),
            graph_root: "/repo/graph".into(),
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
            graph_fingerprint: "impact-test".into(),
            recorded_identity: identity.clone(),
            current_identity: identity,
            worktree_mismatch: None,
            syn: layer(LayerState::Fresh),
            scip: layer(LayerState::Fresh),
            mir: layer(LayerState::Unavailable),
        }
    }

    #[test]
    fn test_atlas_impact_returns_distance_and_explanation_paths() {
        let graph = index(
            vec![
                node("seed", NodeKind::Fn),
                node("left", NodeKind::Fn),
                node("right", NodeKind::Fn),
                node("shared", NodeKind::Fn),
            ],
            vec![
                edge("left", "seed", EdgeKind::Calls),
                edge("right", "seed", EdgeKind::References),
                edge("shared", "left", EdgeKind::UsesType),
                edge("shared", "right", EdgeKind::Calls),
            ],
        );
        let result = impact_index(
            &graph,
            &node("seed", NodeKind::Fn),
            &ImpactOptions::default(),
            &status(),
        )
        .unwrap();
        let shared = result
            .affected
            .iter()
            .find(|entry| entry.node.id == "shared")
            .unwrap();
        assert_eq!(shared.distance, 2);
        assert_eq!(shared.path.nodes.first().unwrap().id, result.seed.id);
        assert_eq!(shared.path.nodes.last().unwrap().id, "shared");
        assert_eq!(
            result
                .affected
                .iter()
                .filter(|entry| entry.node.id == "shared")
                .count(),
            1
        );
        assert!(
            shared
                .path
                .hops
                .iter()
                .all(|hop| hop.edge.evidence.is_some())
        );
        for (nodes, hop) in shared.path.nodes.windows(2).zip(&shared.path.hops) {
            assert_eq!(hop.direction, PathDirection::Reverse);
            assert_eq!(hop.edge.from, nodes[1].id);
            assert_eq!(hop.chosen_target, nodes[0].id);
        }
    }

    #[test]
    fn test_atlas_impact_container_expansion_avoids_sibling_explosion() {
        let graph = index(
            vec![
                node("container", NodeKind::Module),
                node("member", NodeKind::Fn),
                node("sibling", NodeKind::Fn),
            ],
            vec![
                edge("container", "member", EdgeKind::Contains),
                edge("container", "sibling", EdgeKind::Contains),
            ],
        );
        let leaf = impact_index(
            &graph,
            &node("member", NodeKind::Fn),
            &ImpactOptions::default(),
            &status(),
        )
        .unwrap();
        assert!(!leaf.affected.iter().any(|entry| entry.node.id == "sibling"));

        let container = impact_index(
            &graph,
            &node("container", NodeKind::Module),
            &ImpactOptions::default(),
            &status(),
        )
        .unwrap();
        assert_eq!(
            container
                .affected
                .iter()
                .find(|entry| entry.node.id == "member")
                .unwrap()
                .distance,
            0
        );
        let member = container
            .affected
            .iter()
            .find(|entry| entry.node.id == "member")
            .unwrap();
        assert_eq!(member.path.hops[0].direction, PathDirection::Forward);
    }

    #[test]
    fn containment_closure_chooses_canonical_equal_distance_path() {
        let nodes = vec![
            node("container", NodeKind::Module),
            node("left", NodeKind::Module),
            node("right", NodeKind::Module),
            node("shared", NodeKind::Fn),
        ];
        let edges = vec![
            edge("container", "right", EdgeKind::Contains),
            edge("right", "shared", EdgeKind::Contains),
            edge("container", "left", EdgeKind::Contains),
            edge("left", "shared", EdgeKind::Contains),
        ];
        let forward = impact_index(
            &index(nodes.clone(), edges.clone()),
            &node("container", NodeKind::Module),
            &ImpactOptions::default(),
            &status(),
        )
        .unwrap();
        let reverse = impact_index(
            &index(nodes, edges.into_iter().rev().collect()),
            &node("container", NodeKind::Module),
            &ImpactOptions::default(),
            &status(),
        )
        .unwrap();

        assert_eq!(
            serde_json::to_vec(&forward).unwrap(),
            serde_json::to_vec(&reverse).unwrap()
        );
        let shared = forward
            .affected
            .iter()
            .find(|entry| entry.node.id == "shared")
            .unwrap();
        assert_eq!(shared.distance, 0);
        assert_eq!(shared.path.nodes[1].id, "left");
    }

    #[test]
    fn impact_limits_are_deterministic_and_never_overshoot() {
        let graph = index(
            vec![
                node("seed", NodeKind::Fn),
                node("z", NodeKind::Fn),
                node("a", NodeKind::Fn),
                node("b", NodeKind::Fn),
            ],
            vec![
                edge("z", "seed", EdgeKind::Calls),
                edge("a", "seed", EdgeKind::Calls),
                edge("b", "seed", EdgeKind::Calls),
            ],
        );
        let options = ImpactOptions {
            max_nodes: 2,
            ..ImpactOptions::default()
        };
        let left = impact_index(&graph, &node("seed", NodeKind::Fn), &options, &status()).unwrap();
        let right = impact_index(&graph, &node("seed", NodeKind::Fn), &options, &status()).unwrap();
        assert_eq!(
            left.affected
                .iter()
                .map(|entry| (entry.distance, entry.node.id.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "a"), (1, "b")]
        );
        assert!(left.truncated);
        assert_eq!(
            serde_json::to_vec(&left).unwrap(),
            serde_json::to_vec(&right).unwrap()
        );

        for options in [
            ImpactOptions {
                max_depth: 0,
                ..ImpactOptions::default()
            },
            ImpactOptions {
                max_depth: 9,
                ..ImpactOptions::default()
            },
            ImpactOptions {
                max_nodes: 0,
                ..ImpactOptions::default()
            },
        ] {
            assert!(matches!(
                impact_index(&graph, &node("seed", NodeKind::Fn), &options, &status()),
                Err(AtlasError::TraversalLimit { .. })
            ));
        }
    }

    #[test]
    fn impact_bounds_containment_and_dependency_frontiers() {
        let mut containment_nodes = vec![node("container", NodeKind::Module)];
        let mut containment_edges = Vec::new();
        for position in 0..100 {
            let id = format!("member-{position:03}");
            containment_nodes.push(node(&id, NodeKind::Fn));
            containment_edges.push(edge("container", &id, EdgeKind::Contains));
        }
        let options = ImpactOptions {
            max_nodes: 2,
            ..ImpactOptions::default()
        };
        let containment = impact_many_index(
            &index(containment_nodes, containment_edges),
            &[node("container", NodeKind::Module)],
            &options,
        )
        .unwrap();
        assert!(containment.peak_frontier <= 3);
        assert_eq!(containment.affected.len(), 2);
        assert!(containment.truncated);

        let mut dependency_nodes = vec![node("seed", NodeKind::Fn)];
        let mut dependency_edges = Vec::new();
        for position in 0..100 {
            let id = format!("caller-{position:03}");
            dependency_nodes.push(node(&id, NodeKind::Fn));
            dependency_edges.push(edge(&id, "seed", EdgeKind::Calls));
        }
        let dependency = impact_many_index(
            &index(dependency_nodes, dependency_edges),
            &[node("seed", NodeKind::Fn)],
            &options,
        )
        .unwrap();
        assert!(dependency.peak_frontier <= 3);
        assert_eq!(dependency.affected.len(), 2);
        assert!(dependency.truncated);
    }
}
