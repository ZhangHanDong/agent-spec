use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{
    AtlasError, Meta, check_with_meta, io_err, read_persisted_meta, rel_path, walk_rs_files,
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
    let current_identity = capture_identity(code_root, graph_dir)?;
    let current_files = source_hashes(code_root)?;
    let stale_files = check_with_meta(&canonical_path(code_root)?, &recorded.meta)?;
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
                "worktree mismatch: graph was built in {}; current worktree is {}",
                recorded.identity.worktree_root, current_identity.worktree_root
            )
        });

    Ok(AtlasStatus {
        graph_fingerprint: recorded.meta.graph_fingerprint,
        recorded_identity: recorded.identity,
        current_identity,
        worktree_mismatch,
        syn,
        scip,
        mir: unavailable("MIR layer is unavailable"),
    })
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
    let (Some(recorded_index), Some(recorded_sources), Some(index_path)) = (
        capability.scip_fingerprint.as_ref(),
        capability.scip_source_fingerprint.as_ref(),
        capability.scip_index.as_ref(),
    ) else {
        return unavailable("SCIP layer is unavailable: no explicit SCIP overlay");
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

    use crate::{AtlasError, BuildOptions, LayerState, QueryOptions, build, impls};

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
