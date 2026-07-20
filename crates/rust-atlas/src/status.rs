use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{
    AtlasError, Meta, PersistedMeta, io_err, read_persisted_meta, rel_path, walk_rs_files,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphIdentity {
    pub repository_root: String,
    pub git_common_dir: Option<String>,
    pub worktree_root: String,
    pub graph_root: String,
    pub toolchain: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerState {
    Fresh,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerStatus {
    pub state: LayerState,
    pub recorded_fingerprint: Option<String>,
    pub current_fingerprint: Option<String>,
    pub stale_files: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasStatus {
    pub graph_fingerprint: String,
    pub recorded_identity: GraphIdentity,
    pub current_identity: GraphIdentity,
    pub worktree_mismatch: Option<String>,
    pub syn: LayerStatus,
    pub scip: LayerStatus,
    pub mir: LayerStatus,
}

/// Return graph identity and independent source, SCIP, and MIR freshness.
///
/// Metadata is fully schema-validated before inspecting the current worktree,
/// which keeps schema mismatch as the first actionable failure.
pub fn status(code_root: &Path, graph_dir: &Path) -> Result<AtlasStatus, AtlasError> {
    let recorded = read_persisted_meta(graph_dir)?;
    status_with_meta(code_root, graph_dir, &recorded)
}

pub(crate) fn status_with_meta(
    code_root: &Path,
    graph_dir: &Path,
    recorded: &PersistedMeta,
) -> Result<AtlasStatus, AtlasError> {
    let current_identity = capture_identity(code_root, graph_dir)?;
    let current_files = source_hashes(code_root)?;
    let stale_files = stale_files(&recorded.meta.files, &current_files);
    let recorded_source_fingerprint = source_fingerprint(&recorded.meta.files)?;
    let current_source_fingerprint = source_fingerprint(&current_files)?;

    let syn = LayerStatus {
        state: if stale_files.is_empty() {
            LayerState::Fresh
        } else {
            LayerState::Stale
        },
        recorded_fingerprint: Some(recorded_source_fingerprint.clone()),
        current_fingerprint: Some(current_source_fingerprint.clone()),
        stale_files,
        diagnostics: Vec::new(),
    };
    let scip = scip_status(&recorded.meta, &current_source_fingerprint);
    let worktree_mismatch = (recorded.identity.worktree_root != current_identity.worktree_root)
        .then(|| {
            format!(
                "atlas-worktree-mismatch: graph was built in {}; current worktree is {}",
                recorded.identity.worktree_root, current_identity.worktree_root
            )
        });

    Ok(AtlasStatus {
        graph_fingerprint: recorded.meta.graph_fingerprint.clone(),
        recorded_identity: recorded.identity.clone(),
        current_identity,
        worktree_mismatch,
        syn,
        scip,
        mir: unavailable("MIR layer is unavailable"),
    })
}

/// Reject graph status that cannot provide definitive evidence.
///
/// An unavailable semantic layer is not evidence and therefore does not block
/// syn-only consumers. Any available-but-stale layer does.
pub fn require_authority(status: &AtlasStatus) -> Result<(), AtlasError> {
    require_worktree_match(status)?;
    let mut diagnostics = Vec::new();
    if status.syn.state == LayerState::Stale {
        if status.syn.stale_files.is_empty() {
            diagnostics.push("syn layer is stale".to_string());
        } else {
            diagnostics.push(format!(
                "syn graph lags the code for {}",
                status.syn.stale_files.join(", ")
            ));
        }
        diagnostics.extend(status.syn.diagnostics.iter().cloned());
    }
    for (name, layer) in [("SCIP", &status.scip), ("MIR", &status.mir)] {
        if layer.state == LayerState::Stale {
            if layer.diagnostics.is_empty() {
                diagnostics.push(format!("{name} layer is stale"));
            } else {
                diagnostics.extend(layer.diagnostics.iter().cloned());
            }
        }
    }
    diagnostics.sort();
    diagnostics.dedup();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(AtlasError::StaleAuthority {
            detail: diagnostics.join("; "),
        })
    }
}

pub(crate) fn require_worktree_match(status: &AtlasStatus) -> Result<(), AtlasError> {
    if status.recorded_identity.worktree_root == status.current_identity.worktree_root {
        return Ok(());
    }
    Err(AtlasError::WorktreeMismatch {
        recorded: status.recorded_identity.worktree_root.clone(),
        current: status.current_identity.worktree_root.clone(),
    })
}

fn stale_files(
    recorded: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> Vec<String> {
    recorded
        .keys()
        .chain(current.keys())
        .filter(|file| recorded.get(*file) != current.get(*file))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn capture_identity(
    code_root: &Path,
    graph_dir: &Path,
) -> Result<GraphIdentity, AtlasError> {
    let code_root = canonical_path(code_root)?;
    let graph_root = canonical_path(graph_dir)?;
    let toolchain = rustc_version()?;

    let (repository_root, worktree_root, git_common_dir) = match git_worktree(&code_root)? {
        Some((root, common_dir)) => {
            let root = display_path(&root);
            (root.clone(), root, Some(display_path(&common_dir)))
        }
        None => {
            let root = display_path(&code_root);
            (root.clone(), root, None)
        }
    };

    Ok(GraphIdentity {
        repository_root,
        git_common_dir,
        worktree_root,
        graph_root: display_path(&graph_root),
        toolchain,
    })
}

pub(crate) fn source_fingerprint(files: &BTreeMap<String, String>) -> Result<String, AtlasError> {
    let bytes = serde_json::to_vec(files).map_err(|error| AtlasError::Io(error.to_string()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn scip_status(meta: &Meta, current_source_fingerprint: &str) -> LayerStatus {
    let capability = &meta.capability;
    let authority = [
        ("scip_index", capability.scip_index.as_ref()),
        ("scip_fingerprint", capability.scip_fingerprint.as_ref()),
        (
            "scip_source_fingerprint",
            capability.scip_source_fingerprint.as_ref(),
        ),
    ];
    if !capability.scip {
        let diagnostics = authority
            .iter()
            .filter_map(|(field, value)| {
                value.as_ref().map(|_| {
                    format!("SCIP capability is false but authority field is present: {field}")
                })
            })
            .collect::<Vec<_>>();
        if diagnostics.is_empty() {
            return unavailable("SCIP layer is unavailable: no explicit SCIP overlay");
        }
        return LayerStatus {
            state: LayerState::Stale,
            recorded_fingerprint: capability.scip_fingerprint.clone(),
            current_fingerprint: None,
            stale_files: Vec::new(),
            diagnostics,
        };
    }

    let missing = authority
        .iter()
        .filter(|(_, value)| value.is_none())
        .map(|(field, _)| format!("SCIP authority missing field: {field}"))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return LayerStatus {
            state: LayerState::Stale,
            recorded_fingerprint: capability.scip_fingerprint.clone(),
            current_fingerprint: None,
            stale_files: Vec::new(),
            diagnostics: missing,
        };
    }

    let (Some(recorded_index), Some(recorded_sources), Some(index_path)) = (
        capability.scip_fingerprint.as_ref(),
        capability.scip_source_fingerprint.as_ref(),
        capability.scip_index.as_ref(),
    ) else {
        unreachable!("complete SCIP authority was validated above");
    };

    let current_index = std::fs::read(index_path)
        .ok()
        .map(|bytes| blake3::hash(&bytes).to_hex().to_string());
    let mut diagnostics = Vec::new();
    if current_index.as_deref() != Some(recorded_index) {
        diagnostics.push(format!(
            "SCIP index fingerprint mismatch: recorded {recorded_index}, current {}",
            current_index.as_deref().unwrap_or("missing")
        ));
    }
    if current_source_fingerprint != recorded_sources {
        diagnostics.push(format!(
            "SCIP source-set fingerprint mismatch: recorded {recorded_sources}, current {current_source_fingerprint}"
        ));
    }
    diagnostics.sort();

    LayerStatus {
        state: if diagnostics.is_empty() {
            LayerState::Fresh
        } else {
            LayerState::Stale
        },
        recorded_fingerprint: Some(recorded_index.clone()),
        current_fingerprint: current_index,
        stale_files: Vec::new(),
        diagnostics,
    }
}

fn unavailable(diagnostic: &str) -> LayerStatus {
    LayerStatus {
        state: LayerState::Unavailable,
        recorded_fingerprint: None,
        current_fingerprint: None,
        stale_files: Vec::new(),
        diagnostics: vec![diagnostic.to_string()],
    }
}

fn source_hashes(code_root: &Path) -> Result<BTreeMap<String, String>, AtlasError> {
    let code_root = canonical_path(code_root)?;
    let mut files = BTreeMap::new();
    for path in walk_rs_files(&code_root) {
        let relative = rel_path(&code_root, &path);
        let bytes = std::fs::read(path).map_err(io_err)?;
        files.insert(relative, blake3::hash(&bytes).to_hex().to_string());
    }
    Ok(files)
}

fn git_worktree(code_root: &Path) -> Result<Option<(PathBuf, PathBuf)>, AtlasError> {
    let root = git_rev_parse(code_root, "--show-toplevel")?;
    let Some(root) = root else {
        return Ok(None);
    };
    let common_dir = git_rev_parse(code_root, "--git-common-dir")?.ok_or_else(|| {
        AtlasError::Cargo("git returned a worktree root without a common directory".to_string())
    })?;
    let common_dir = if common_dir.is_absolute() {
        common_dir
    } else {
        code_root.join(common_dir)
    };
    Ok(Some((canonical_path(&root)?, canonical_path(&common_dir)?)))
}

fn git_rev_parse(code_root: &Path, argument: &str) -> Result<Option<PathBuf>, AtlasError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(code_root)
        .arg("rev-parse")
        .arg(argument)
        .output()
        .map_err(|error| AtlasError::Cargo(format!("cannot run git rev-parse: {error}")))?;
    if !output.status.success() {
        return Ok(None);
    }
    let value = String::from_utf8(output.stdout).map_err(|error| {
        AtlasError::Cargo(format!("git rev-parse returned invalid UTF-8: {error}"))
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(AtlasError::Cargo(
            "git rev-parse succeeded without returning a path".to_string(),
        ));
    }
    Ok(Some(PathBuf::from(value)))
}

fn rustc_version() -> Result<String, AtlasError> {
    let output = Command::new("rustc")
        .arg("-Vv")
        .output()
        .map_err(|error| AtlasError::Cargo(format!("cannot run rustc -Vv: {error}")))?;
    if !output.status.success() {
        return Err(AtlasError::Cargo(format!(
            "rustc -Vv failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| AtlasError::Cargo(format!("rustc -Vv returned invalid UTF-8: {error}")))
}

fn canonical_path(path: &Path) -> Result<PathBuf, AtlasError> {
    std::fs::canonicalize(path).map_err(io_err)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::{
        AtlasError, BuildOptions, EdgeKind, EdgeSite, LayerState, Provenance, QueryOptions, build,
        impls,
    };

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas/basic")
    }

    fn scip_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas/scip/index.json")
    }

    fn fixture(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let code = base.join("code");
        fs::create_dir_all(code.join("src")).unwrap();
        for relative in ["Cargo.toml", "src/lib.rs", "src/store.rs", "src/service.rs"] {
            fs::copy(fixture_root().join(relative), code.join(relative)).unwrap();
        }
        (code, base.join("graph"))
    }

    fn copied_scip_index(code: &Path) -> PathBuf {
        let index = code.join("index.scip.json");
        fs::copy(scip_fixture(), &index).unwrap();
        index
    }

    fn init_git_repository(code: &Path) {
        for args in [
            ["init"].as_slice(),
            ["add", "."].as_slice(),
            [
                "-c",
                "user.name=Atlas Test",
                "-c",
                "user.email=atlas@example.test",
                "commit",
                "-m",
                "initial",
            ]
            .as_slice(),
        ] {
            let output = std::process::Command::new("git")
                .args(args)
                .current_dir(code)
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
        }
    }

    #[test]
    fn test_atlas_status_reports_fresh_syn_scip_and_unavailable_mir() {
        let (code, graph) = fixture("atlas-status-fresh");
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
            },
        )
        .unwrap();

        let report = crate::status(&code, &graph).unwrap();
        assert_eq!(report.syn.state, LayerState::Fresh);
        assert_eq!(report.scip.state, LayerState::Fresh);
        assert_eq!(report.mir.state, LayerState::Unavailable);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_identity_falls_back_outside_git() {
        let (code, graph) = fixture("atlas-identity-no-git");
        build(&code, &graph, &BuildOptions::default()).unwrap();

        let report = crate::status(&code, &graph).unwrap();
        assert_eq!(
            report.current_identity.worktree_root,
            fs::canonicalize(&code).unwrap().to_string_lossy()
        );
        assert_eq!(report.current_identity.git_common_dir, None);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_identity_distinguishes_linked_worktrees_with_shared_common_dir() {
        let (code, graph) = fixture("atlas-identity-linked-worktree");
        init_git_repository(&code);
        let linked = code.parent().unwrap().join("linked");
        let output = std::process::Command::new("git")
            .args(["worktree", "add", "-b", "linked", &linked.to_string_lossy()])
            .current_dir(&code)
            .output()
            .unwrap();
        assert!(output.status.success(), "git worktree add: {output:?}");

        build(&code, &graph, &BuildOptions::default()).unwrap();
        let report = crate::status(&linked, &graph).unwrap();
        assert_ne!(
            report.recorded_identity.worktree_root,
            report.current_identity.worktree_root
        );
        assert_eq!(
            report.recorded_identity.git_common_dir,
            report.current_identity.git_common_dir
        );
        assert!(report.worktree_mismatch.is_some());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_status_reports_source_edit_before_refresh() {
        let (code, graph) = fixture("atlas-status-source-edit");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let service = code.join("src/service.rs");
        fs::write(
            &service,
            format!("{}\n// changed\n", fs::read_to_string(&service).unwrap()),
        )
        .unwrap();

        let report = crate::status(&code, &graph).unwrap();
        assert_eq!(report.syn.state, LayerState::Stale);
        assert_eq!(report.syn.stale_files, vec!["src/service.rs"]);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_refresh_keeps_reused_scip_layer_stale() {
        let (code, graph) = fixture("atlas-status-refresh-scip");
        let index = copied_scip_index(&code);
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(index),
            },
        )
        .unwrap();
        let service = code.join("src/service.rs");
        fs::write(
            &service,
            format!("{}\n// changed\n", fs::read_to_string(&service).unwrap()),
        )
        .unwrap();

        let before_refresh = crate::status(&code, &graph).unwrap();
        assert_eq!(before_refresh.syn.state, LayerState::Stale);
        assert_eq!(before_refresh.scip.state, LayerState::Stale);

        impls(&code, &graph, "Store", &QueryOptions::default()).unwrap();
        let report = crate::status(&code, &graph).unwrap();
        assert_eq!(report.syn.state, LayerState::Fresh);
        assert_eq!(report.scip.state, LayerState::Stale);
        assert!(
            report
                .scip
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("source-set fingerprint mismatch"))
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_status_refresh_retains_explicit_scip_index_fingerprint() {
        let (code, graph) = fixture("atlas-status-refresh-scip-index-authority");
        let index = copied_scip_index(&code);
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(index.clone()),
            },
        )
        .unwrap();
        let explicit = crate::status(&code, &graph).unwrap();
        let explicit_index_fingerprint = explicit.scip.recorded_fingerprint.unwrap();
        let explicit_source_fingerprint = crate::load_graph(&graph)
            .unwrap()
            .0
            .capability
            .scip_source_fingerprint
            .unwrap();

        let mut changed_index: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index).unwrap()).unwrap();
        changed_index["metadata"]["tool_info"]["version"] =
            serde_json::json!("2026-07-20-authority-regression");
        changed_index["documents"][0]["occurrences"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "symbol": "rust-analyzer cargo atlas-basic 0.1.0 store/Store#",
                "symbol_roles": 0,
                "range": [6, 5, 6, 10]
            }));
        fs::write(&index, serde_json::to_vec_pretty(&changed_index).unwrap()).unwrap();
        let service = code.join("src/service.rs");
        fs::write(
            &service,
            format!("{}\n// changed\n", fs::read_to_string(&service).unwrap()),
        )
        .unwrap();

        let refreshed_impls = impls(&code, &graph, "Store", &QueryOptions::default()).unwrap();
        let relation = refreshed_impls
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::ImplsTrait)
            .unwrap();
        let (_, refreshed_shards) = crate::load_graph(&graph).unwrap();
        let refreshed_scip_edge = refreshed_shards
            .iter()
            .flat_map(|shard| &shard.edges)
            .find(|edge| {
                edge.from == relation.from
                    && edge.to == relation.to
                    && edge.kind == EdgeKind::References
                    && edge.provenance == Provenance::Scip
            })
            .unwrap();
        assert_eq!(
            refreshed_scip_edge.site,
            Some(EdgeSite {
                file: "src/store.rs".to_string(),
                line_start: 7,
                column_start: 6,
                line_end: 7,
                column_end: 11,
            })
        );
        assert_eq!(
            refreshed_scip_edge.evidence.as_deref(),
            Some(
                "rust-analyzer-scip occurrence at src/store.rs:7:6-7:11 for \
                 `rust-analyzer cargo atlas-basic 0.1.0 store/Store#`: one target"
            )
        );

        let persisted = crate::load_graph(&graph).unwrap().0.capability;
        assert_eq!(
            persisted.scip_fingerprint.as_deref(),
            Some(explicit_index_fingerprint.as_str())
        );
        assert_eq!(
            persisted.scip_source_fingerprint.as_deref(),
            Some(explicit_source_fingerprint.as_str())
        );
        let report = crate::status(&code, &graph).unwrap();
        assert_eq!(report.syn.state, LayerState::Fresh);
        assert_eq!(report.scip.state, LayerState::Stale);
        assert!(
            report
                .scip
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("index fingerprint mismatch"))
        );
        assert!(
            report
                .scip
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("source-set fingerprint mismatch"))
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_status_reports_changed_and_deleted_scip_index() {
        let (code, graph) = fixture("atlas-status-scip-index");
        let index = copied_scip_index(&code);
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(index.clone()),
            },
        )
        .unwrap();
        fs::write(&index, b"changed").unwrap();
        let changed = crate::status(&code, &graph).unwrap();
        assert_eq!(changed.scip.state, LayerState::Stale);
        assert!(
            changed
                .scip
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("index fingerprint mismatch"))
        );

        fs::remove_file(&index).unwrap();
        let deleted = crate::status(&code, &graph).unwrap();
        assert_eq!(deleted.scip.state, LayerState::Stale);
        assert_eq!(deleted.scip.current_fingerprint, None);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_status_reports_scip_unavailable_without_explicit_overlay() {
        let (code, graph) = fixture("atlas-status-no-scip");
        build(&code, &graph, &BuildOptions::default()).unwrap();

        let report = crate::status(&code, &graph).unwrap();
        assert_eq!(report.scip.state, LayerState::Unavailable);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_partial_scip_authority_fails_closed() {
        #[derive(Clone, Copy)]
        enum Case {
            Missing(&'static str),
            DisabledWithLeftover(&'static str),
        }

        let (code, graph) = fixture("atlas-status-partial-scip-authority");
        let index = copied_scip_index(&code);
        build(
            &code,
            &graph,
            &BuildOptions {
                full: false,
                scip_index: Some(index),
            },
        )
        .unwrap();
        let meta_path = graph.join("meta.json");
        let original: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        let authority_fields = ["scip_index", "scip_fingerprint", "scip_source_fingerprint"];
        let cases = authority_fields.iter().copied().map(Case::Missing).chain(
            authority_fields
                .iter()
                .copied()
                .map(Case::DisabledWithLeftover),
        );

        for case in cases {
            let mut meta = original.clone();
            let (label, field) = match case {
                Case::Missing(field) => {
                    meta["capability"][field] = serde_json::Value::Null;
                    ("missing", field)
                }
                Case::DisabledWithLeftover(field) => {
                    meta["capability"]["scip"] = serde_json::json!(false);
                    meta["capability"]["scip_tool"] = serde_json::Value::Null;
                    for authority_field in authority_fields {
                        meta["capability"][authority_field] = serde_json::Value::Null;
                    }
                    meta["capability"][field] = original["capability"][field].clone();
                    ("leftover", field)
                }
            };
            fs::write(&meta_path, serde_json::to_vec_pretty(&meta).unwrap()).unwrap();

            let report = crate::status(&code, &graph).unwrap();
            assert_eq!(report.scip.state, LayerState::Stale, "{label} {field}");
            assert!(
                report
                    .scip
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.contains(field)),
                "{label} {field}: {:?}",
                report.scip.diagnostics
            );
            let error = crate::require_authority(&report).unwrap_err().to_string();
            assert!(error.contains("atlas-stale"), "{label} {field}: {error}");
            assert!(error.contains(field), "{label} {field}: {error}");
        }

        let mut absent = original;
        absent["capability"]["scip"] = serde_json::json!(false);
        absent["capability"]["scip_tool"] = serde_json::Value::Null;
        for authority_field in authority_fields {
            absent["capability"][authority_field] = serde_json::Value::Null;
        }
        fs::write(&meta_path, serde_json::to_vec_pretty(&absent).unwrap()).unwrap();
        let report = crate::status(&code, &graph).unwrap();
        assert_eq!(report.scip.state, LayerState::Unavailable);
        crate::require_authority(&report).unwrap();
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_status_rejects_v5_before_identity_deserialization() {
        let (code, graph) = fixture("atlas-status-schema-v5");
        fs::create_dir_all(&graph).unwrap();
        fs::write(graph.join("meta.json"), r#"{"schema_version":5}"#).unwrap();

        let error = crate::status(&code, &graph).unwrap_err();
        assert!(matches!(
            error,
            AtlasError::SchemaMismatch {
                found: 5,
                expected: crate::SCHEMA_VERSION
            }
        ));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_status_serialization_and_diagnostics_are_deterministic() {
        let (code, graph) = fixture("atlas-status-deterministic");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        fs::remove_file(code.join("src/store.rs")).unwrap();
        fs::write(code.join("src/new.rs"), "pub fn new_file() {}\n").unwrap();
        let first = crate::status(&code, &graph).unwrap();
        let second = crate::status(&code, &graph).unwrap();

        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert_eq!(first.syn.stale_files, vec!["src/new.rs", "src/store.rs"]);
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }
}
