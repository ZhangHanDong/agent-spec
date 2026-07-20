use std::path::Path;

use serde::Serialize;

use crate::traversal::{
    EndpointResolution, bound_paths, edge_targets, enumerate_paths, graph_path,
    highest_confidence_path, resolve_endpoint, validate_limits,
};
use crate::{
    AtlasError, AtlasStatus, FlowState, GraphPath, LayerState, Node, PathHop, QueryIndex,
    QueryOptions, RuntimeBoundaryHint, TraversalLimits, indexed_query_state,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowQuery {
    Between { from: String, to: String },
    Through { symbol: String },
}

#[derive(Debug, Clone)]
pub struct FlowOptions {
    pub limits: TraversalLimits,
    pub frozen: bool,
}

impl Default for FlowOptions {
    fn default() -> Self {
        Self {
            limits: TraversalLimits::flow_default(),
            frozen: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowEndpoint {
    pub query: String,
    pub selected: Option<Node>,
    pub candidates: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowResult {
    pub schema: String,
    pub state: FlowState,
    pub endpoints: Vec<FlowEndpoint>,
    pub shortest: Option<GraphPath>,
    pub highest_confidence: Option<GraphPath>,
    pub alternatives: Vec<GraphPath>,
    pub expansions: usize,
    pub truncated: bool,
    pub diagnostics: Vec<FlowDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_boundaries: Vec<RuntimeBoundaryHint>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub runtime_boundary_truncated: bool,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}

pub fn flow(
    code_root: &Path,
    graph_dir: &Path,
    query: FlowQuery,
    options: &FlowOptions,
) -> Result<FlowResult, AtlasError> {
    let (meta, index, status) = indexed_query_state(
        code_root,
        graph_dir,
        &QueryOptions {
            frozen: options.frozen,
        },
    )?;
    let mut result = flow_index(&index, query.clone(), options.clone(), &status)?;
    if matches!(
        result.state,
        FlowState::NoPath | FlowState::CapabilityUnavailable
    ) {
        let projection = crate::runtime_boundary::project_runtime_boundaries(
            code_root,
            &meta,
            &index,
            &status,
            &query,
            options.limits,
        );
        result.runtime_boundaries = projection.hints;
        result.runtime_boundary_truncated = projection.truncated;
        if !result.runtime_boundaries.is_empty() {
            result.diagnostics.push(FlowDiagnostic {
                code: "atlas-flow-runtime-boundary".into(),
                message: "the static path ends at runtime dispatch; candidates are query hints, not graph edges".into(),
            });
        }
        if result.runtime_boundary_truncated {
            result.diagnostics.push(FlowDiagnostic {
                code: "atlas-flow-runtime-boundary-truncated".into(),
                message: "runtime-boundary scanning exhausted a configured query limit".into(),
            });
        }
        result.diagnostics.sort_by(|left, right| {
            left.code
                .cmp(&right.code)
                .then_with(|| left.message.cmp(&right.message))
        });
    }
    Ok(result)
}

pub(crate) fn flow_index(
    index: &QueryIndex,
    query: FlowQuery,
    options: FlowOptions,
    status: &AtlasStatus,
) -> Result<FlowResult, AtlasError> {
    validate_limits(options.limits)?;
    match query {
        FlowQuery::Between { from, to } => between_flow(index, from, to, options, status),
        FlowQuery::Through { symbol } => through_flow(index, symbol, options, status),
    }
}

fn between_flow(
    index: &QueryIndex,
    from: String,
    to: String,
    options: FlowOptions,
    status: &AtlasStatus,
) -> Result<FlowResult, AtlasError> {
    let from_resolution = resolve_endpoint(index, &from);
    let to_resolution = resolve_endpoint(index, &to);
    let endpoints = vec![
        flow_endpoint(&from, &from_resolution),
        flow_endpoint(&to, &to_resolution),
    ];
    if let Some(state) = endpoint_failure_state([&from_resolution, &to_resolution]) {
        return Ok(empty_result(state, endpoints, status));
    }
    let (EndpointResolution::Found(from_node), EndpointResolution::Found(to_node)) =
        (from_resolution, to_resolution)
    else {
        return Err(AtlasError::Invariant(
            "flow endpoint state changed after validation".into(),
        ));
    };
    let enumeration = enumerate_paths(index, &from_node.id, Some(&to_node.id), options.limits)?;
    Ok(result_from_paths(
        enumeration.paths,
        enumeration.highest_confidence,
        enumeration.expansions,
        enumeration.truncated,
        endpoints,
        status,
    ))
}

fn through_flow(
    index: &QueryIndex,
    symbol: String,
    options: FlowOptions,
    status: &AtlasStatus,
) -> Result<FlowResult, AtlasError> {
    let resolution = resolve_endpoint(index, &symbol);
    let endpoints = vec![flow_endpoint(&symbol, &resolution)];
    if let Some(state) = endpoint_failure_state([&resolution]) {
        return Ok(empty_result(state, endpoints, status));
    }
    let EndpointResolution::Found(through) = resolution else {
        return Err(AtlasError::Invariant(
            "through endpoint state changed after validation".into(),
        ));
    };

    let incoming = incoming_hops(index, &through);
    let outgoing = index
        .outgoing_edges([through.id.as_str()])
        .into_iter()
        .flat_map(|edge| edge_targets(index, edge))
        .collect::<Vec<_>>();
    let has_valid_pairs = incoming.iter().any(|(caller, _)| {
        outgoing
            .iter()
            .any(|(target, _)| valid_through_pair(caller, &through, target))
    });
    let mut paths = Vec::new();
    let mut expansions = 0;
    let mut truncated = false;

    if options.limits.max_depth < 2 {
        truncated = has_valid_pairs;
    } else {
        'incoming: for (caller, incoming_hop) in incoming {
            for (target, outgoing_hop) in &outgoing {
                if expansions == options.limits.max_expansions {
                    truncated = true;
                    break 'incoming;
                }
                expansions += 1;
                if !valid_through_pair(&caller, &through, target) {
                    continue;
                }
                paths.push(graph_path(
                    vec![caller.clone(), through.clone(), target.clone()],
                    vec![incoming_hop.clone(), outgoing_hop.clone()],
                ));
            }
        }
    }
    let highest_confidence = highest_confidence_path(&paths);
    truncated |= bound_paths(&mut paths, options.limits.max_paths);
    Ok(result_from_paths(
        paths,
        highest_confidence,
        expansions,
        truncated,
        endpoints,
        status,
    ))
}

fn valid_through_pair(caller: &Node, through: &Node, target: &Node) -> bool {
    caller.id != through.id && target.id != through.id && target.id != caller.id
}

fn incoming_hops(index: &QueryIndex, through: &Node) -> Vec<(Node, PathHop)> {
    let mut incoming = Vec::new();
    for edge in &index.edges {
        for (target, hop) in edge_targets(index, edge) {
            if target.id != through.id {
                continue;
            }
            if let EndpointResolution::Found(caller) = resolve_endpoint(index, &edge.from) {
                incoming.push((caller, hop));
            }
        }
    }
    incoming.sort_by(|(left_node, left_hop), (right_node, right_hop)| {
        left_node
            .id
            .cmp(&right_node.id)
            .then_with(|| left_hop.edge.cmp(&right_hop.edge))
    });
    incoming.dedup();
    incoming
}

fn flow_endpoint(query: &str, resolution: &EndpointResolution) -> FlowEndpoint {
    match resolution {
        EndpointResolution::Found(node) => FlowEndpoint {
            query: query.into(),
            selected: Some(node.clone()),
            candidates: Vec::new(),
        },
        EndpointResolution::Ambiguous(candidates) => FlowEndpoint {
            query: query.into(),
            selected: None,
            candidates: candidates.clone(),
        },
        EndpointResolution::Unknown => FlowEndpoint {
            query: query.into(),
            selected: None,
            candidates: Vec::new(),
        },
    }
}

fn endpoint_failure_state<'a>(
    resolutions: impl IntoIterator<Item = &'a EndpointResolution>,
) -> Option<FlowState> {
    let resolutions = resolutions.into_iter().collect::<Vec<_>>();
    if resolutions
        .iter()
        .any(|resolution| matches!(resolution, EndpointResolution::Unknown))
    {
        Some(FlowState::UnknownEndpoint)
    } else if resolutions
        .iter()
        .any(|resolution| matches!(resolution, EndpointResolution::Ambiguous(_)))
    {
        Some(FlowState::AmbiguousEndpoint)
    } else {
        None
    }
}

fn result_from_paths(
    paths: Vec<GraphPath>,
    highest_confidence: Option<GraphPath>,
    expansions: usize,
    truncated: bool,
    endpoints: Vec<FlowEndpoint>,
    status: &AtlasStatus,
) -> FlowResult {
    let shortest = paths.first().cloned();
    let state = if !paths.is_empty() {
        FlowState::Found
    } else if truncated {
        FlowState::Truncated
    } else if status.scip.state != LayerState::Fresh {
        FlowState::CapabilityUnavailable
    } else {
        FlowState::NoPath
    };
    let mut diagnostics = Vec::new();
    if truncated {
        diagnostics.push(FlowDiagnostic {
            code: "atlas-flow-truncated".into(),
            message: "flow traversal exhausted a configured limit".into(),
        });
    }
    if paths.is_empty() && status.scip.state != LayerState::Fresh {
        diagnostics.push(FlowDiagnostic {
            code: "atlas-flow-scip-unavailable".into(),
            message: format!(
                "SCIP evidence is {:?}; absence of a path is not definitive",
                status.scip.state
            ),
        });
    }
    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.message.cmp(&right.message))
    });
    FlowResult {
        schema: "agent-spec/rust-atlas/flow-v1".into(),
        state,
        endpoints,
        shortest,
        highest_confidence,
        alternatives: paths,
        expansions,
        truncated,
        diagnostics,
        runtime_boundaries: Vec::new(),
        runtime_boundary_truncated: false,
        status: status.clone(),
        stale: status.syn.stale_files.clone(),
    }
}

fn empty_result(
    state: FlowState,
    endpoints: Vec<FlowEndpoint>,
    status: &AtlasStatus,
) -> FlowResult {
    let (code, message) = match state {
        FlowState::UnknownEndpoint => (
            "atlas-flow-unknown-endpoint",
            "one or more flow endpoints did not resolve",
        ),
        FlowState::AmbiguousEndpoint => (
            "atlas-flow-ambiguous-endpoint",
            "one or more flow endpoints have multiple candidates",
        ),
        _ => ("atlas-flow-empty", "flow ended before traversal"),
    };
    FlowResult {
        schema: "agent-spec/rust-atlas/flow-v1".into(),
        state,
        endpoints,
        shortest: None,
        highest_confidence: None,
        alternatives: Vec::new(),
        expansions: 0,
        truncated: false,
        diagnostics: vec![FlowDiagnostic {
            code: code.into(),
            message: message.into(),
        }],
        runtime_boundaries: Vec::new(),
        runtime_boundary_truncated: false,
        status: status.clone(),
        stale: status.syn.stale_files.clone(),
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        AtlasStatus, DispatchKind, Edge, EdgeConfidence, EdgeKind, EdgeResolution,
        ExtractorIdentity, FlowState, GraphIdentity, LayerState, LayerStatus, Node, NodeKind,
        Provenance, QueryIndex, TraversalLimits,
    };

    fn node(id: &str) -> Node {
        Node {
            id: id.into(),
            symbol: id.into(),
            kind: NodeKind::Fn,
            file: format!("{}.rs", id.replace("::", "_")),
            line_start: 1,
            line_end: 1,
            visibility: "pub".into(),
            signature: format!("fn {id}()"),
            doc: None,
            cfg: None,
        }
    }

    fn edge(from: &str, to: &str, confidence: EdgeConfidence) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            target_text: Some(to.into()),
            resolution: EdgeResolution::Resolved,
            kind: EdgeKind::Calls,
            provenance: Provenance::Scip,
            site: None,
            extractor: Some(ExtractorIdentity {
                name: "test".into(),
                version: Some("1".into()),
            }),
            dispatch: Some(DispatchKind::Static),
            confidence: Some(confidence),
            candidates: Vec::new(),
            evidence: Some(format!("{from} calls {to}")),
            generic: false,
        }
    }

    fn index(nodes: Vec<Node>, edges: Vec<Edge>) -> QueryIndex {
        QueryIndex::from_test_parts("flow-test", nodes, edges)
    }

    fn status(scip: LayerState) -> AtlasStatus {
        let identity = GraphIdentity {
            repository_root: "/repo".into(),
            git_common_dir: Some("/repo/.git".into()),
            worktree_root: "/repo".into(),
            graph_root: "/repo/.agent-spec/graph".into(),
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
            generation: Some("g-flow-test".into()),
            graph_fingerprint: "flow-test".into(),
            recorded_identity: identity.clone(),
            current_identity: identity,
            worktree_mismatch: None,
            syn: layer(LayerState::Fresh),
            scip: layer(scip),
            mir: layer(LayerState::Unavailable),
        }
    }

    fn between(from: &str, to: &str) -> FlowQuery {
        FlowQuery::Between {
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn test_atlas_flow_returns_shortest_and_highest_confidence_paths() {
        let graph = index(
            vec![node("a"), node("b"), node("c"), node("d"), node("z")],
            vec![
                edge("a", "b", EdgeConfidence::BoundedCandidates),
                edge("b", "z", EdgeConfidence::Exact),
                edge("a", "c", EdgeConfidence::Exact),
                edge("c", "d", EdgeConfidence::Exact),
                edge("d", "z", EdgeConfidence::Exact),
            ],
        );
        let result = flow_index(
            &graph,
            between("a", "z"),
            FlowOptions::default(),
            &status(LayerState::Fresh),
        )
        .unwrap();
        assert_eq!(result.state, FlowState::Found);
        assert_eq!(result.shortest.as_ref().unwrap().hops.len(), 2);
        assert_eq!(result.highest_confidence.as_ref().unwrap().hops.len(), 3);
        assert_eq!(
            result.highest_confidence.as_ref().unwrap().confidence,
            crate::PathConfidence::Exact
        );
        assert!(
            result.shortest.as_ref().unwrap().hops[0]
                .edge
                .evidence
                .is_some()
        );
        let repeated = flow_index(
            &graph,
            between("a", "z"),
            FlowOptions::default(),
            &status(LayerState::Fresh),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&result).unwrap(),
            serde_json::to_vec(&repeated).unwrap()
        );

        let limited = flow_index(
            &graph,
            between("a", "z"),
            FlowOptions {
                limits: TraversalLimits {
                    max_paths: 1,
                    ..TraversalLimits::flow_default()
                },
                frozen: false,
            },
            &status(LayerState::Fresh),
        )
        .unwrap();
        assert_eq!(limited.shortest.as_ref().unwrap().hops.len(), 2);
        assert_eq!(limited.highest_confidence.as_ref().unwrap().hops.len(), 3);
        assert_eq!(limited.alternatives.len(), 1);
        assert!(limited.truncated);
    }

    #[test]
    fn test_atlas_flow_preserves_bounded_candidate_alternatives() {
        let mut candidate = edge(
            "crate::Through",
            "unresolved",
            EdgeConfidence::BoundedCandidates,
        );
        candidate.resolution = EdgeResolution::Unresolved;
        candidate.candidates = vec!["crate::C".into(), "crate::B".into()];
        let graph = index(
            vec![
                node("crate::Caller"),
                node("crate::Through"),
                node("crate::B"),
                node("crate::C"),
            ],
            vec![
                edge("crate::Caller", "crate::Through", EdgeConfidence::Exact),
                candidate,
            ],
        );
        let result = flow_index(
            &graph,
            FlowQuery::Through {
                symbol: "crate::Through".into(),
            },
            FlowOptions::default(),
            &status(LayerState::Fresh),
        )
        .unwrap();
        let targets = result
            .alternatives
            .iter()
            .map(|path| path.hops.last().unwrap().chosen_target.as_str())
            .collect::<Vec<_>>();
        assert_eq!(targets, vec!["crate::B", "crate::C"]);
        for path in &result.alternatives {
            assert_eq!(path.nodes[0].id, "crate::Caller");
            assert_eq!(path.nodes[1].id, "crate::Through");
            assert_eq!(path.hops.len(), 2);
            assert_eq!(path.hops[0].chosen_target, "crate::Through");
            assert!(path.hops[1].candidate);
            assert_eq!(path.hops[1].edge.resolution, EdgeResolution::Unresolved);
        }
    }

    #[test]
    fn test_atlas_flow_distinguishes_no_path_unavailable_and_truncated() {
        let disconnected = index(vec![node("a"), node("z")], Vec::new());
        assert_eq!(
            flow_index(
                &disconnected,
                between("a", "z"),
                FlowOptions::default(),
                &status(LayerState::Fresh),
            )
            .unwrap()
            .state,
            FlowState::NoPath
        );
        assert_eq!(
            flow_index(
                &disconnected,
                between("a", "z"),
                FlowOptions::default(),
                &status(LayerState::Unavailable),
            )
            .unwrap()
            .state,
            FlowState::CapabilityUnavailable
        );

        let limited = index(
            vec![node("a"), node("b"), node("z")],
            vec![
                edge("a", "b", EdgeConfidence::Exact),
                edge("b", "z", EdgeConfidence::Exact),
            ],
        );
        let result = flow_index(
            &limited,
            between("a", "z"),
            FlowOptions {
                limits: TraversalLimits {
                    max_depth: 8,
                    max_expansions: 1,
                    max_paths: 8,
                },
                frozen: false,
            },
            &status(LayerState::Fresh),
        )
        .unwrap();
        assert_eq!(result.state, FlowState::Truncated);
        assert!(result.truncated);

        let cycle = index(
            vec![node("caller"), node("through")],
            vec![
                edge("caller", "through", EdgeConfidence::Exact),
                edge("through", "caller", EdgeConfidence::Exact),
            ],
        );
        let cycle = flow_index(
            &cycle,
            FlowQuery::Through {
                symbol: "through".into(),
            },
            FlowOptions {
                limits: TraversalLimits {
                    max_depth: 1,
                    ..TraversalLimits::flow_default()
                },
                frozen: false,
            },
            &status(LayerState::Fresh),
        )
        .unwrap();
        assert_eq!(cycle.state, FlowState::NoPath);
        assert!(!cycle.truncated);
    }

    #[test]
    fn test_atlas_flow_handles_ambiguous_endpoints_and_syn_paths_without_scip() {
        let graph = index(
            vec![
                node("a"),
                node("z"),
                node("crate::a::run"),
                node("crate::b::run"),
            ],
            vec![edge("a", "z", EdgeConfidence::Exact)],
        );
        assert_eq!(
            flow_index(
                &graph,
                between("missing", "z"),
                FlowOptions::default(),
                &status(LayerState::Fresh),
            )
            .unwrap()
            .state,
            FlowState::UnknownEndpoint
        );
        let ambiguous = flow_index(
            &graph,
            between("run", "z"),
            FlowOptions::default(),
            &status(LayerState::Fresh),
        )
        .unwrap();
        assert_eq!(ambiguous.state, FlowState::AmbiguousEndpoint);
        assert_eq!(
            ambiguous.endpoints[0]
                .candidates
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["crate::a::run", "crate::b::run"]
        );
        assert_eq!(
            flow_index(
                &graph,
                between("a", "z"),
                FlowOptions::default(),
                &status(LayerState::Unavailable),
            )
            .unwrap()
            .state,
            FlowState::Found
        );
        let stale = flow_index(
            &graph,
            between("a", "z"),
            FlowOptions::default(),
            &status(LayerState::Stale),
        )
        .unwrap();
        assert_eq!(stale.state, FlowState::Found);
        assert_eq!(stale.status.scip.state, LayerState::Stale);
    }
}
