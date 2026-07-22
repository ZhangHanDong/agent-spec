use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{
    AtlasError, BuildDiagnostic, DispatchKind, Edge, EdgeConfidence, EdgeKind, EdgeResolution,
    ExtractorIdentity, Node, NodeKind, Provenance, Shard, commit_shard_generation, read_shard,
};

pub(super) const MAX_CANDIDATES: usize = 64;
const EXTRACTOR_NAME: &str = "rust-atlas-dynamic-dispatch";
const EXTRACTOR_VERSION: &str = "v1";

#[derive(Debug, Default)]
pub(super) struct EnrichmentOutcome {
    pub edges_added: usize,
    pub diagnostics: Vec<BuildDiagnostic>,
}

pub(super) fn is_dynamic_edge(edge: &Edge) -> bool {
    edge.extractor
        .as_ref()
        .is_some_and(|extractor| extractor.name == EXTRACTOR_NAME)
}

pub(super) fn remove(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
) -> Result<(), AtlasError> {
    let mut shards = BTreeMap::new();
    let mut changed = false;
    for rel in files.keys() {
        let mut shard = read_shard(shards_dir, rel)?;
        let old_len = shard.edges.len();
        shard.edges.retain(|edge| !is_dynamic_edge(edge));
        changed |= shard.edges.len() != old_len;
        shards.insert(rel.clone(), shard);
    }
    if changed {
        commit_shard_generation(shards_dir, &shards)?;
    }
    Ok(())
}

pub(super) fn enrich(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
    max_candidates: usize,
) -> Result<EnrichmentOutcome, AtlasError> {
    let mut shards = load_shards(shards_dir, files)?;
    let mut changed = false;
    for shard in shards.values_mut() {
        let old_len = shard.edges.len();
        shard.edges.retain(|edge| !is_dynamic_edge(edge));
        changed |= shard.edges.len() != old_len;
    }

    let nodes = node_index(&shards);
    let children = containment_index(&shards);
    let trait_methods = trait_method_index(&nodes, &children);
    let implementation_methods = implementation_method_index(&shards, &nodes, &children);
    let mut additions: BTreeMap<String, Vec<Edge>> = BTreeMap::new();
    let mut outcome = EnrichmentOutcome::default();

    for (rel, shard) in &shards {
        for anchor in &shard.edges {
            if anchor.kind != EdgeKind::Calls
                || anchor.provenance != Provenance::Scip
                || anchor.resolution != EdgeResolution::Resolved
                || anchor.confidence != Some(EdgeConfidence::Exact)
                || is_dynamic_edge(anchor)
            {
                continue;
            }
            let Some((trait_id, method_name, trait_method_symbol)) = trait_methods.get(&anchor.to)
            else {
                continue;
            };
            let mut candidates = implementation_methods
                .get(&(trait_id.clone(), method_name.clone()))
                .cloned()
                .unwrap_or_default();
            candidates.sort();
            candidates.dedup();
            if candidates.is_empty() {
                continue;
            }
            if candidates.len() > max_candidates {
                outcome.diagnostics.push(BuildDiagnostic {
                    code: "dynamic-dispatch-truncated".to_string(),
                    severity: "warning".to_string(),
                    message: format!(
                        "trait call `{trait_method_symbol}` has {} candidates, exceeding the limit {max_candidates}; no inferred edge was written",
                        candidates.len()
                    ),
                });
                continue;
            }

            let mut inferred = anchor.clone();
            inferred.resolution = EdgeResolution::Unresolved;
            inferred.dispatch = Some(DispatchKind::Trait);
            inferred.confidence = Some(EdgeConfidence::BoundedCandidates);
            inferred.candidates = candidates;
            inferred.extractor = Some(ExtractorIdentity {
                name: EXTRACTOR_NAME.to_string(),
                version: Some(EXTRACTOR_VERSION.to_string()),
            });
            inferred.evidence = Some(format!(
                "whole-graph trait dispatch: `{trait_method_symbol}` has {} resolved implementation candidates",
                inferred.candidates.len()
            ));
            inferred.generic = false;
            additions.entry(rel.clone()).or_default().push(inferred);
        }
    }

    for (rel, mut edges) in additions {
        let Some(shard) = shards.get_mut(&rel) else {
            continue;
        };
        let old_len = shard.edges.len();
        shard.edges.append(&mut edges);
        shard.edges.sort();
        shard.edges.dedup();
        outcome.edges_added += shard.edges.len() - old_len;
        changed |= shard.edges.len() != old_len;
    }
    outcome.diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.message.cmp(&right.message))
    });

    if changed {
        commit_shard_generation(shards_dir, &shards)?;
    }
    Ok(outcome)
}

fn load_shards(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Shard>, AtlasError> {
    files
        .keys()
        .map(|rel| Ok((rel.clone(), read_shard(shards_dir, rel)?)))
        .collect()
}

fn node_index(shards: &BTreeMap<String, Shard>) -> BTreeMap<String, Node> {
    shards
        .values()
        .flat_map(|shard| shard.nodes.iter().cloned())
        .map(|node| (node.id.clone(), node))
        .collect()
}

fn containment_index(shards: &BTreeMap<String, Shard>) -> BTreeMap<String, BTreeSet<String>> {
    let mut children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in shards
        .values()
        .flat_map(|shard| &shard.edges)
        .filter(|edge| {
            edge.kind == EdgeKind::Contains && edge.resolution == EdgeResolution::Resolved
        })
    {
        children
            .entry(edge.from.clone())
            .or_default()
            .insert(edge.to.clone());
    }
    children
}

fn trait_method_index(
    nodes: &BTreeMap<String, Node>,
    children: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, (String, String, String)> {
    let mut methods = BTreeMap::new();
    for trait_node in nodes.values().filter(|node| node.kind == NodeKind::Trait) {
        for child_id in children.get(&trait_node.id).into_iter().flatten() {
            let Some(method) = nodes.get(child_id).filter(|node| node.kind == NodeKind::Fn) else {
                continue;
            };
            let Some(method_name) = method.symbol.rsplit("::").next() else {
                continue;
            };
            methods.insert(
                method.id.clone(),
                (
                    trait_node.id.clone(),
                    method_name.to_string(),
                    method.symbol.clone(),
                ),
            );
        }
    }
    methods
}

fn implementation_method_index(
    shards: &BTreeMap<String, Shard>,
    nodes: &BTreeMap<String, Node>,
    children: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<(String, String), Vec<String>> {
    let mut methods: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for edge in shards
        .values()
        .flat_map(|shard| &shard.edges)
        .filter(|edge| {
            edge.kind == EdgeKind::ImplsTrait && edge.resolution == EdgeResolution::Resolved
        })
    {
        let Some(impl_node) = nodes
            .get(&edge.from)
            .filter(|node| node.kind == NodeKind::Impl)
        else {
            continue;
        };
        for child_id in children.get(&impl_node.id).into_iter().flatten() {
            let Some(method) = nodes.get(child_id).filter(|node| node.kind == NodeKind::Fn) else {
                continue;
            };
            let Some(method_name) = method.symbol.rsplit("::").next() else {
                continue;
            };
            methods
                .entry((edge.to.clone(), method_name.to_string()))
                .or_default()
                .push(method.id.clone());
        }
    }
    methods
}
