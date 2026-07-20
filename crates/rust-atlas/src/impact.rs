use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use crate::traversal::{
    EndpointResolution, canonical_hop_signature, canonical_path_signature, graph_path,
    resolve_endpoint,
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
    #[cfg(test)]
    peak_containment_queue: usize,
    #[cfg(test)]
    peak_neighbor_buffer: usize,
    #[cfg(test)]
    examined_edges: usize,
    #[cfg(test)]
    examined_targets: usize,
}

#[derive(Debug, Default)]
struct TraversalMetrics {
    #[cfg(test)]
    peak_containment_queue: usize,
    #[cfg(test)]
    peak_neighbor_buffer: usize,
    #[cfg(test)]
    examined_edges: usize,
    #[cfg(test)]
    examined_targets: usize,
}

#[derive(Debug, Default)]
struct NeighborScan {
    neighbors: Vec<(Node, PathHop)>,
    truncated: bool,
    #[cfg(test)]
    examined_edges: usize,
    #[cfg(test)]
    examined_targets: usize,
}

impl TraversalMetrics {
    fn record_scan(&mut self, scan: &NeighborScan) {
        #[cfg(test)]
        {
            self.peak_neighbor_buffer = self.peak_neighbor_buffer.max(scan.neighbors.len());
            self.examined_edges += scan.examined_edges;
            self.examined_targets += scan.examined_targets;
        }
        #[cfg(not(test))]
        let _ = scan;
    }

    fn record_containment_queue(&mut self, size: usize) {
        #[cfg(test)]
        {
            self.peak_containment_queue = self.peak_containment_queue.max(size);
        }
        #[cfg(not(test))]
        let _ = size;
    }
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
    let mut result = impact_index(&index, &seed, options, &status)?;
    let mut represented = BTreeSet::from([result.seed.file.clone()]);
    for entry in &result.affected {
        represented.insert(entry.node.file.clone());
        represented.extend(entry.path.nodes.iter().map(|node| node.file.clone()));
        represented.extend(
            entry
                .path
                .hops
                .iter()
                .filter_map(|hop| hop.edge.site.as_ref().map(|site| site.file.clone())),
        );
    }
    crate::status::scope_live_status(&mut result.status, represented);
    Ok(result)
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
    let mut metrics = TraversalMetrics::default();

    'distances: for distance in 0..=options.max_depth {
        truncated |=
            containment_closure(index, &mut layer, &settled, candidate_limit, &mut metrics);
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
            let neighbors = dependency_neighbors(
                index,
                &candidate.node,
                candidate_limit,
                &settled,
                &candidate.path,
            );
            truncated |= neighbors.truncated;
            metrics.record_scan(&neighbors);
            for (node, hop) in neighbors.neighbors {
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
        #[cfg(test)]
        peak_containment_queue: metrics.peak_containment_queue,
        #[cfg(test)]
        peak_neighbor_buffer: metrics.peak_neighbor_buffer,
        #[cfg(test)]
        examined_edges: metrics.examined_edges,
        #[cfg(test)]
        examined_targets: metrics.examined_targets,
    })
}

fn containment_closure(
    index: &QueryIndex,
    layer: &mut BTreeMap<String, Candidate>,
    settled: &BTreeSet<String>,
    limit: usize,
    metrics: &mut TraversalMetrics,
) -> bool {
    let mut frontier = layer.clone();
    let mut truncated = false;
    while !frontier.is_empty() {
        metrics.record_containment_queue(frontier.len());
        let next_id = frontier
            .iter()
            .min_by(|(_, left), (_, right)| {
                canonical_path_signature(&left.path)
                    .cmp(&canonical_path_signature(&right.path))
                    .then_with(|| left.node.id.cmp(&right.node.id))
            })
            .map(|(id, _)| id.clone());
        let Some(next_id) = next_id else {
            break;
        };
        let Some(candidate) = frontier.remove(&next_id) else {
            continue;
        };
        if layer.get(&candidate.node.id).is_none_or(|current| {
            canonical_path_signature(&current.path) != canonical_path_signature(&candidate.path)
        }) {
            continue;
        }
        let neighbors =
            containment_neighbors(index, &candidate.node, limit, settled, &candidate.path);
        truncated |= neighbors.truncated;
        metrics.record_scan(&neighbors);
        for (node, hop) in neighbors.neighbors {
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
            if let Some(removed) = outcome.removed.as_ref() {
                frontier.remove(removed);
            }
            if outcome.inserted {
                frontier.insert(next.node.id.clone(), next);
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

#[derive(Debug, Clone)]
struct BoundedInsert {
    inserted: bool,
    truncated: bool,
    removed: Option<String>,
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
            removed: None,
        };
    }
    let removed = layer.pop_last().map(|(id, _)| id);
    BoundedInsert {
        inserted: removed.as_deref() != Some(candidate_id.as_str()),
        truncated: true,
        removed,
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
    settled: &BTreeSet<String>,
    path: &GraphPath,
) -> NeighborScan {
    let mut neighbors = BTreeMap::new();
    let mut truncated = false;
    #[cfg(test)]
    let mut examined_edges = 0;
    #[cfg(test)]
    let mut examined_targets = 0;
    for (node, edge) in index.outgoing_neighbors_for(&current.id) {
        if edge.kind != EdgeKind::Contains {
            continue;
        }
        #[cfg(test)]
        {
            examined_edges += 1;
        }
        #[cfg(test)]
        {
            examined_targets += 1;
        }
        if excluded_neighbor(node, settled, path) {
            continue;
        }
        let hop = PathHop {
            edge: edge.clone(),
            chosen_target: node.id.clone(),
            candidate: edge.resolution != crate::EdgeResolution::Resolved,
            direction: PathDirection::Forward,
        };
        if !insert_neighbor_until_full(&mut neighbors, (node.clone(), hop), limit) {
            truncated = true;
            break;
        }
    }
    NeighborScan {
        neighbors: neighbors.into_values().collect(),
        truncated,
        #[cfg(test)]
        examined_edges,
        #[cfg(test)]
        examined_targets,
    }
}

fn dependency_neighbors(
    index: &QueryIndex,
    current: &Node,
    limit: usize,
    settled: &BTreeSet<String>,
    path: &GraphPath,
) -> NeighborScan {
    let mut neighbors = BTreeMap::new();
    let mut truncated = false;
    #[cfg(test)]
    let mut examined_edges = 0;
    #[cfg(test)]
    let mut examined_targets = 0;
    for (dependent, edge) in index.incoming_neighbors_for(&current.id) {
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
        #[cfg(test)]
        {
            examined_edges += 1;
            examined_targets += 1;
        }
        if excluded_neighbor(dependent, settled, path) {
            continue;
        }
        let hop = PathHop {
            edge: edge.clone(),
            chosen_target: current.id.clone(),
            candidate: edge.resolution != crate::EdgeResolution::Resolved,
            direction: PathDirection::Reverse,
        };
        if !insert_neighbor_until_full(&mut neighbors, (dependent.clone(), hop), limit) {
            truncated = true;
            break;
        }
    }
    NeighborScan {
        neighbors: neighbors.into_values().collect(),
        truncated,
        #[cfg(test)]
        examined_edges,
        #[cfg(test)]
        examined_targets,
    }
}

fn insert_neighbor_until_full(
    neighbors: &mut BTreeMap<String, (Node, PathHop)>,
    neighbor: (Node, PathHop),
    limit: usize,
) -> bool {
    let key = neighbor.0.id.clone();
    if let Some(existing) = neighbors.get(&key) {
        if canonical_hop_cmp(&neighbor.1, &existing.1).is_lt() {
            neighbors.insert(key, neighbor);
        }
        return true;
    }
    if neighbors.len() == limit {
        return false;
    }
    neighbors.insert(key, neighbor);
    true
}

fn excluded_neighbor(node: &Node, settled: &BTreeSet<String>, path: &GraphPath) -> bool {
    settled.contains(&node.id) || path.nodes.iter().any(|visited| visited.id == node.id)
}

fn canonical_hop_cmp(left: &PathHop, right: &PathHop) -> std::cmp::Ordering {
    canonical_hop_signature(left).cmp(&canonical_hop_signature(right))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        AtlasStatus, Edge, EdgeConfidence, EdgeKind, EdgeResolution, EdgeSite, GraphIdentity,
        LayerState, LayerStatus, Node, NodeKind, Provenance, QueryIndex,
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
            cfg: None,
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
            generic: false,
        }
    }

    fn index(nodes: Vec<Node>, edges: Vec<Edge>) -> QueryIndex {
        QueryIndex::from_test_parts("impact-test", nodes, edges)
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
            extractor: None,
            recorded_fingerprint: None,
            current_fingerprint: None,
            recorded_source_fingerprint: None,
            current_source_fingerprint: None,
            stale_files: Vec::new(),
            diagnostics: Vec::new(),
        };
        AtlasStatus {
            live: crate::live::LiveRuntimeStatus::new(crate::live::LiveRuntimeState::Unavailable),
            generation: Some("g-impact-test".into()),
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
        assert!(containment.peak_containment_queue <= 3);
        assert!(containment.peak_neighbor_buffer <= 3);
        assert!(containment.examined_edges <= 4);
        assert!(containment.examined_targets <= 4);
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
        assert!(dependency.peak_containment_queue <= 3);
        assert!(dependency.peak_neighbor_buffer <= 3);
        assert!(dependency.examined_edges <= 4);
        assert!(dependency.examined_targets <= 4);
        assert_eq!(dependency.affected.len(), 2);
        assert!(dependency.truncated);
    }

    #[test]
    fn impact_neighbor_budget_counts_unique_nodes_not_parallel_call_sites() {
        let mut edges = Vec::new();
        for column in 0..20 {
            let mut parallel = edge("caller-a", "seed", EdgeKind::Calls);
            parallel.site = Some(EdgeSite {
                file: "src/caller_a.rs".into(),
                line_start: 1,
                column_start: column,
                line_end: 1,
                column_end: column + 1,
            });
            edges.push(parallel);
        }
        edges.push(edge("caller-z", "seed", EdgeKind::Calls));
        let traversal = impact_many_index(
            &index(
                vec![
                    node("seed", NodeKind::Fn),
                    node("caller-a", NodeKind::Fn),
                    node("caller-z", NodeKind::Fn),
                ],
                edges,
            ),
            &[node("seed", NodeKind::Fn)],
            &ImpactOptions {
                max_nodes: 2,
                ..ImpactOptions::default()
            },
        )
        .unwrap();

        assert_eq!(
            traversal
                .affected
                .iter()
                .map(|entry| entry.node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["caller-a", "caller-z"]
        );
    }

    #[test]
    fn impact_neighbor_budget_excludes_settled_cycle_edges_before_counting() {
        let mut edges = vec![edge("a", "seed", EdgeKind::Calls)];
        for column in 0..20 {
            let mut cycle = edge("seed", "a", EdgeKind::Calls);
            cycle.site = Some(EdgeSite {
                file: "src/seed.rs".into(),
                line_start: 2,
                column_start: column,
                line_end: 2,
                column_end: column + 1,
            });
            edges.push(cycle);
        }
        edges.push(edge("z", "a", EdgeKind::Calls));
        let traversal = impact_many_index(
            &index(
                vec![
                    node("seed", NodeKind::Fn),
                    node("a", NodeKind::Fn),
                    node("z", NodeKind::Fn),
                ],
                edges,
            ),
            &[node("seed", NodeKind::Fn)],
            &ImpactOptions {
                max_nodes: 2,
                ..ImpactOptions::default()
            },
        )
        .unwrap();

        assert_eq!(
            traversal
                .affected
                .iter()
                .map(|entry| entry.node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "z"]
        );
    }

    #[test]
    fn impact_parallel_edges_use_serialized_path_tie_break() {
        let mut reference = edge("kind-caller", "seed", EdgeKind::References);
        reference.provenance = Provenance::Syn;
        let mut call = edge("kind-caller", "seed", EdgeKind::Calls);
        call.provenance = Provenance::Mir;

        let mut syn = edge("provenance-caller", "seed", EdgeKind::Calls);
        syn.provenance = Provenance::Syn;
        let mut mir = edge("provenance-caller", "seed", EdgeKind::Calls);
        mir.provenance = Provenance::Mir;

        let mut column_two = edge("site-caller", "seed", EdgeKind::Calls);
        column_two.site = Some(EdgeSite {
            file: "src/site.rs".into(),
            line_start: 1,
            column_start: 2,
            line_end: 1,
            column_end: 3,
        });
        let mut column_ten = column_two.clone();
        column_ten.site.as_mut().unwrap().column_start = 10;
        column_ten.site.as_mut().unwrap().column_end = 11;

        let traversal = impact_many_index(
            &index(
                vec![
                    node("seed", NodeKind::Fn),
                    node("kind-caller", NodeKind::Fn),
                    node("provenance-caller", NodeKind::Fn),
                    node("site-caller", NodeKind::Fn),
                ],
                vec![reference, call, syn, mir, column_two, column_ten],
            ),
            &[node("seed", NodeKind::Fn)],
            &ImpactOptions {
                max_nodes: 3,
                ..ImpactOptions::default()
            },
        )
        .unwrap();

        let selected = |id: &str| {
            &traversal
                .affected
                .iter()
                .find(|entry| entry.node.id == id)
                .unwrap()
                .path
                .hops[0]
                .edge
        };
        assert_eq!(selected("kind-caller").kind, EdgeKind::Calls);
        assert_eq!(selected("provenance-caller").provenance, Provenance::Mir);
        assert_eq!(
            selected("site-caller").site.as_ref().unwrap().column_start,
            10
        );
    }
}
