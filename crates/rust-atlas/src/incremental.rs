use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::{
    AtlasError, BuildOptions, EdgeKind, Provenance, Shard, check_cancelled, dynamic_dispatch,
    read_shard,
};

const ORPHAN_SCHEMA_VERSION: u32 = 1;
const ORPHAN_FILE: &str = "orphans.json";
const MAX_ORPHAN_ITEMS: usize = 100_000;
const MAX_VALUE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct FrontierPlan {
    pub(crate) files: Vec<String>,
    pub(crate) changed_symbols: Vec<String>,
    pub(crate) fallback_reason: Option<String>,
}

pub(crate) struct FrontierRequest<'a> {
    pub(crate) shards_dir: &'a Path,
    pub(crate) files: &'a BTreeMap<String, String>,
    pub(crate) old_changed_shards: &'a BTreeMap<String, Shard>,
    pub(crate) rebuilt: &'a [String],
    pub(crate) removed: &'a [String],
    pub(crate) prior: Option<&'a OrphanQueue>,
    pub(crate) full: bool,
    pub(crate) input_plan_changed: bool,
    pub(crate) capability_change_reason: Option<&'a str>,
    pub(crate) opts: &'a BuildOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrphanQueue {
    schema_version: u32,
    pub(crate) base_generation: Option<String>,
    pub(crate) source_plan_fingerprint: String,
    pub(crate) affected_files: Vec<String>,
    pub(crate) changed_symbols: Vec<String>,
    pub(crate) reason: String,
}

impl OrphanQueue {
    pub(crate) fn new(
        base_generation: Option<String>,
        source_plan_fingerprint: String,
        affected_files: Vec<String>,
        changed_symbols: Vec<String>,
        reason: String,
    ) -> Result<Self, AtlasError> {
        let queue = Self {
            schema_version: ORPHAN_SCHEMA_VERSION,
            base_generation,
            source_plan_fingerprint,
            affected_files,
            changed_symbols,
            reason,
        };
        queue.validate()?;
        Ok(queue)
    }

    fn validate(&self) -> Result<(), AtlasError> {
        if self.schema_version != ORPHAN_SCHEMA_VERSION {
            return Err(orphan_error(format!(
                "schema v{} != v{}",
                self.schema_version, ORPHAN_SCHEMA_VERSION
            )));
        }
        if self.affected_files.len() > MAX_ORPHAN_ITEMS
            || self.changed_symbols.len() > MAX_ORPHAN_ITEMS
        {
            return Err(orphan_error("queue exceeds the bounded item limit"));
        }
        if self.source_plan_fingerprint.len() != 64
            || !self
                .source_plan_fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(orphan_error("source plan fingerprint is invalid"));
        }
        if !matches!(
            self.reason.as_str(),
            "source-change"
                | "full-build"
                | "input-plan-change"
                | "orphan-recovery"
                | "dependency-frontier-overflow"
                | "semantic-overlay-change"
                | "dynamic-dispatch-full-recompute"
        ) {
            return Err(orphan_error(format!(
                "unsupported queue reason `{}`",
                self.reason
            )));
        }
        for file in &self.affected_files {
            validate_relative_file(file)?;
        }
        for symbol in &self.changed_symbols {
            if symbol.is_empty()
                || symbol.len() > MAX_VALUE_BYTES
                || symbol.chars().any(char::is_control)
            {
                return Err(orphan_error("queue contains an unsafe symbol"));
            }
        }
        Ok(())
    }
}

pub(crate) fn plan_frontier(request: FrontierRequest<'_>) -> Result<FrontierPlan, AtlasError> {
    let FrontierRequest {
        shards_dir,
        files,
        old_changed_shards,
        rebuilt,
        removed,
        prior,
        full,
        input_plan_changed,
        capability_change_reason,
        opts,
    } = request;
    check_cancelled(opts)?;

    let mut changed_symbols = BTreeSet::new();
    let mut changed_ids = BTreeSet::new();
    let mut impl_relations_changed = false;
    let changed_files = rebuilt.iter().chain(removed).collect::<Vec<_>>();
    for batch in changed_files.chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            let old = old_changed_shards.get(*rel);
            let new = files
                .contains_key(*rel)
                .then(|| read_shard(shards_dir, rel))
                .transpose()?;
            let old_nodes = declaration_map(old);
            let new_nodes = declaration_map(new.as_ref());
            for id in old_nodes.keys().chain(new_nodes.keys()) {
                if old_nodes.get(id) != new_nodes.get(id) {
                    changed_ids.insert(id.clone());
                    if let Some(symbol) = old_nodes.get(id) {
                        changed_symbols.insert(symbol.clone());
                    }
                    if let Some(symbol) = new_nodes.get(id) {
                        changed_symbols.insert(symbol.clone());
                    }
                }
            }
            impl_relations_changed |= impl_relations(old) != impl_relations(new.as_ref());
        }
    }

    let mut frontier = rebuilt
        .iter()
        .filter(|rel| files.contains_key(*rel))
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(prior) = prior {
        frontier.extend(
            prior
                .affected_files
                .iter()
                .filter(|rel| files.contains_key(*rel))
                .cloned(),
        );
        changed_symbols.extend(prior.changed_symbols.iter().cloned());
    }
    let changed_last = changed_symbols
        .iter()
        .filter_map(|symbol| symbol.rsplit("::").next().map(str::to_string))
        .collect::<BTreeSet<_>>();

    for batch in files.keys().collect::<Vec<_>>().chunks(opts.batch_size) {
        check_cancelled(opts)?;
        for rel in batch {
            let shard = read_shard(shards_dir, rel)?;
            let is_dependent = shard.edges.iter().any(|edge| {
                if edge.provenance != Provenance::Syn || edge.kind == EdgeKind::Contains {
                    return impl_relations_changed && dynamic_dispatch::is_dynamic_edge(edge);
                }
                let target = edge.target_text.as_deref().unwrap_or(&edge.to);
                changed_ids.contains(&edge.to)
                    || changed_symbols.contains(target)
                    || (!target.contains("::") && changed_last.contains(target))
                    || (impl_relations_changed
                        && matches!(edge.kind, EdgeKind::ImplsTrait | EdgeKind::ImplFor))
            });
            let has_dispatch_anchor = impl_relations_changed
                && shard.edges.iter().any(|edge| {
                    dynamic_dispatch::is_dynamic_edge(edge)
                        || (edge.provenance == Provenance::Scip && edge.kind == EdgeKind::Calls)
                });
            if is_dependent || has_dispatch_anchor {
                frontier.insert((*rel).clone());
            }
        }
    }

    let mut fallback_reason = None;
    if full {
        frontier.extend(files.keys().cloned());
    } else if let Some(reason) = capability_change_reason {
        frontier.extend(files.keys().cloned());
        fallback_reason = Some(reason.to_string());
    } else if input_plan_changed && !files.is_empty() {
        frontier.extend(files.keys().cloned());
        fallback_reason = Some("input-plan-change".to_string());
    } else if frontier.len() > opts.frontier_limit {
        frontier = files.keys().cloned().collect();
        fallback_reason = Some("dependency-frontier-overflow".to_string());
    } else if opts.dynamic_dispatch && (!rebuilt.is_empty() || !removed.is_empty()) {
        frontier.extend(files.keys().cloned());
        fallback_reason = Some("dynamic-dispatch-full-recompute".to_string());
    }

    Ok(FrontierPlan {
        files: frontier.into_iter().collect(),
        changed_symbols: changed_symbols.into_iter().collect(),
        fallback_reason,
    })
}

pub(crate) fn load_orphan(
    graph_root: &Path,
    expected_base: Option<&str>,
) -> Result<Option<OrphanQueue>, AtlasError> {
    let path = graph_root.join(ORPHAN_FILE);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AtlasError::Io(error.to_string())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(orphan_error("queue path is not a regular file"));
    }
    if metadata.len() > 16 * 1024 * 1024 {
        return Err(orphan_error("queue file exceeds 16 MiB"));
    }
    let text = std::fs::read_to_string(&path).map_err(|error| AtlasError::Io(error.to_string()))?;
    let queue: OrphanQueue = serde_json::from_str(&text)
        .map_err(|error| orphan_error(format!("invalid queue JSON: {error}")))?;
    queue.validate()?;
    if queue.base_generation.as_deref() != expected_base {
        return Err(orphan_error(format!(
            "base generation {:?} does not match active generation {:?}",
            queue.base_generation, expected_base
        )));
    }
    Ok(Some(queue))
}

pub(crate) fn write_orphan(graph_root: &Path, queue: &OrphanQueue) -> Result<(), AtlasError> {
    queue.validate()?;
    crate::index::write_json_atomic(&graph_root.join(ORPHAN_FILE), queue)
}

pub(crate) fn clear_orphan(graph_root: &Path) -> Result<(), AtlasError> {
    #[cfg(test)]
    if TEST_CLEAR_FAILURE.with(std::cell::Cell::get) {
        return Err(AtlasError::Io(
            "injected orphan queue clear failure".to_string(),
        ));
    }
    match std::fs::remove_file(graph_root.join(ORPHAN_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AtlasError::Io(error.to_string())),
    }
}

pub(crate) fn rebase_orphan_after_commit(
    graph_root: &Path,
    generation: &str,
) -> Result<(), AtlasError> {
    let path = graph_root.join(ORPHAN_FILE);
    let text = std::fs::read_to_string(&path).map_err(|error| AtlasError::Io(error.to_string()))?;
    let mut queue: OrphanQueue = serde_json::from_str(&text)
        .map_err(|error| orphan_error(format!("invalid queue JSON: {error}")))?;
    queue.validate()?;
    queue.base_generation = Some(generation.to_string());
    queue.reason = "orphan-recovery".to_string();
    write_orphan(graph_root, &queue)
}

#[cfg(test)]
std::thread_local! {
    static TEST_CLEAR_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn set_test_clear_failure(value: bool) {
    TEST_CLEAR_FAILURE.with(|enabled| enabled.set(value));
}

fn declaration_map(shard: Option<&Shard>) -> BTreeMap<String, String> {
    shard
        .into_iter()
        .flat_map(|shard| &shard.nodes)
        .map(|node| (node.id.clone(), node.symbol.clone()))
        .collect()
}

fn impl_relations(shard: Option<&Shard>) -> BTreeSet<(String, String, EdgeKind)> {
    shard
        .into_iter()
        .flat_map(|shard| &shard.edges)
        .filter(|edge| matches!(edge.kind, EdgeKind::ImplsTrait | EdgeKind::ImplFor))
        .map(|edge| {
            (
                edge.from.clone(),
                edge.target_text.clone().unwrap_or_else(|| edge.to.clone()),
                edge.kind,
            )
        })
        .collect()
}

fn validate_relative_file(value: &str) -> Result<(), AtlasError> {
    let path = Path::new(value);
    let safe = !value.is_empty()
        && value.len() <= MAX_VALUE_BYTES
        && !value.chars().any(char::is_control)
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if safe {
        Ok(())
    } else {
        Err(orphan_error(format!("unsafe queue path `{value}`")))
    }
}

fn orphan_error(detail: impl Into<String>) -> AtlasError {
    AtlasError::OrphanQueue {
        detail: detail.into(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn orphan_queue_rejects_unsafe_paths() {
        let error = OrphanQueue::new(
            None,
            "a".repeat(64),
            vec!["../outside.rs".to_string()],
            Vec::new(),
            "source-change".to_string(),
        )
        .unwrap_err();
        assert!(matches!(error, AtlasError::OrphanQueue { .. }));
    }

    #[test]
    fn shard_file_names_remain_relative_queue_values() {
        assert_eq!(crate::shard_file_name("src/lib.rs"), "src%2Flib.rs.json");
    }
}
