use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::impact::{impact_many_index, validate_options};
use crate::{
    AtlasError, AtlasStatus, ImpactEntry, ImpactOptions, Node, QueryIndex, QueryOptions,
    indexed_query_state,
};

#[derive(Debug, Clone, Default)]
pub struct AffectedOptions {
    pub impact: ImpactOptions,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedSeed {
    pub file: String,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffectedDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedResult {
    pub schema: String,
    pub files: Vec<String>,
    pub seeds: Vec<AffectedSeed>,
    pub affected: Vec<ImpactEntry>,
    pub truncated: bool,
    pub diagnostics: Vec<AffectedDiagnostic>,
    pub status: AtlasStatus,
    pub stale: Vec<String>,
}

pub fn affected_paths(
    code_root: &Path,
    graph_dir: &Path,
    paths: &[PathBuf],
    options: &AffectedOptions,
) -> Result<AffectedResult, AtlasError> {
    let (_, index, status) = indexed_query_state(
        code_root,
        graph_dir,
        &QueryOptions {
            frozen: options.impact.frozen,
        },
    )?;
    let files = paths
        .iter()
        .map(|path| normalize_affected_path(code_root, path))
        .collect::<Result<BTreeSet<_>, _>>()?
        .into_iter()
        .collect::<Vec<_>>();
    let mut result = affected_index(&index, files, options, &status)?;
    let mut represented = result.files.iter().cloned().collect::<BTreeSet<_>>();
    represented.extend(
        result
            .seeds
            .iter()
            .flat_map(|seed| seed.nodes.iter().map(|node| node.file.clone())),
    );
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

pub(crate) fn affected_index(
    index: &QueryIndex,
    files: Vec<String>,
    options: &AffectedOptions,
    status: &AtlasStatus,
) -> Result<AffectedResult, AtlasError> {
    validate_options(&options.impact)?;
    let mut seeds = Vec::with_capacity(files.len());
    let mut traversal_seeds = BTreeMap::<String, Node>::new();
    let mut diagnostics = BTreeSet::new();
    let mut seeds_truncated = false;

    for file in &files {
        let remaining = options
            .impact
            .max_nodes
            .saturating_sub(traversal_seeds.len());
        let (nodes, has_nodes, truncated) = nodes_for_file(index, file, remaining);
        seeds_truncated |= truncated;
        if !has_nodes {
            diagnostics.insert((
                "atlas-affected-no-symbols".to_string(),
                format!("changed file `{file}` has no code nodes in the current graph"),
            ));
        }
        for node in &nodes {
            traversal_seeds.insert(node.id.clone(), node.clone());
        }
        seeds.push(AffectedSeed {
            file: file.clone(),
            nodes,
        });
    }

    if seeds_truncated {
        diagnostics.insert((
            "atlas-affected-seeds-truncated".to_string(),
            format!(
                "changed-file seed selection reached the global node limit {}",
                options.impact.max_nodes
            ),
        ));
    }

    let traversal_seeds = traversal_seeds.into_values().collect::<Vec<_>>();
    let traversal = impact_many_index(index, &traversal_seeds, &options.impact)?;
    diagnostics.extend(
        traversal
            .diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message)),
    );
    Ok(AffectedResult {
        schema: "agent-spec/rust-atlas/affected-v1".into(),
        files,
        seeds,
        affected: traversal.affected,
        truncated: seeds_truncated || traversal.truncated,
        diagnostics: diagnostics
            .into_iter()
            .map(|(code, message)| AffectedDiagnostic { code, message })
            .collect(),
        status: status.clone(),
        stale: status.syn.stale_files.clone(),
    })
}

fn nodes_for_file(index: &QueryIndex, file: &str, limit: usize) -> (Vec<Node>, bool, bool) {
    let mut nodes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut has_nodes = false;
    let mut truncated = false;
    for node in index
        .file
        .get(file)
        .into_iter()
        .flatten()
        .filter_map(|position| index.nodes.get(*position))
    {
        has_nodes = true;
        if !seen.insert(node.id.as_str()) {
            continue;
        }
        if nodes.len() == limit {
            truncated = true;
            break;
        }
        nodes.push(node.clone());
    }
    (nodes, has_nodes, truncated)
}

pub(crate) fn normalize_affected_path(code_root: &Path, path: &Path) -> Result<String, AtlasError> {
    let display = path.to_string_lossy().into_owned();
    let canonical_root = std::fs::canonicalize(code_root).map_err(|error| {
        affected_path_error(
            path,
            format!(
                "cannot resolve code root `{}`: {error}",
                code_root.display()
            ),
        )
    })?;
    let portable = PathBuf::from(display.replace('\\', "/"));
    reject_parent_components(path, &portable)?;
    let relative = if portable.is_absolute() {
        absolute_relative_path(path, &portable, &canonical_root)?
    } else {
        lexical_relative(path, &portable)?
    };
    if relative.as_os_str().is_empty() {
        return Err(affected_path_error(
            path,
            "path must name a repository file",
        ));
    }
    reject_symlink_escape(path, &canonical_root, &relative)?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn absolute_relative_path(
    original: &Path,
    path: &Path,
    canonical_root: &Path,
) -> Result<PathBuf, AtlasError> {
    let resolved = resolve_with_missing_tail(original, path)?;
    resolved
        .strip_prefix(canonical_root)
        .map(Path::to_path_buf)
        .map_err(|_| affected_path_error(original, "absolute path is outside the code root"))
}

fn resolve_with_missing_tail(original: &Path, path: &Path) -> Result<PathBuf, AtlasError> {
    let mut ancestor = path;
    let mut missing = Vec::<OsString>::new();
    loop {
        match std::fs::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let Some(name) = ancestor.file_name() else {
                    return Err(affected_path_error(
                        original,
                        format!("cannot resolve any existing path ancestor: {error}"),
                    ));
                };
                missing.push(name.to_os_string());
                let Some(parent) = ancestor.parent() else {
                    return Err(affected_path_error(
                        original,
                        "absolute path has no resolvable ancestor",
                    ));
                };
                ancestor = parent;
            }
            Err(error) => {
                return Err(affected_path_error(
                    original,
                    format!("cannot resolve `{}`: {error}", ancestor.display()),
                ));
            }
        }
    }
}

fn reject_parent_components(original: &Path, path: &Path) -> Result<(), AtlasError> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(affected_path_error(
            original,
            "parent-directory components are not allowed",
        ));
    }
    Ok(())
}

fn lexical_relative(original: &Path, path: &Path) -> Result<PathBuf, AtlasError> {
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => relative.push(value),
            Component::ParentDir => unreachable!("parent components were rejected"),
            Component::Prefix(_) | Component::RootDir => {
                return Err(affected_path_error(
                    original,
                    "path must be repository-relative or an absolute path inside the code root",
                ));
            }
        }
    }
    Ok(relative)
}

fn reject_symlink_escape(
    original: &Path,
    canonical_root: &Path,
    relative: &Path,
) -> Result<(), AtlasError> {
    let mut candidate = canonical_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            continue;
        };
        candidate.push(value);
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = std::fs::canonicalize(&candidate).map_err(|error| {
                    affected_path_error(
                        original,
                        format!("cannot resolve symlink `{}`: {error}", candidate.display()),
                    )
                })?;
                if target.strip_prefix(canonical_root).is_err() {
                    return Err(affected_path_error(
                        original,
                        format!("symlink `{}` escapes the code root", candidate.display()),
                    ));
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(affected_path_error(
                    original,
                    format!("cannot inspect `{}`: {error}", candidate.display()),
                ));
            }
        }
    }
    Ok(())
}

fn affected_path_error(path: &Path, detail: impl Into<String>) -> AtlasError {
    AtlasError::AffectedPath {
        path: path.to_string_lossy().into_owned(),
        detail: detail.into(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{BuildOptions, build};

    struct Fixture {
        root: PathBuf,
        code: PathBuf,
        graph: PathBuf,
        out_of_root: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture(name: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "rust-atlas-affected-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        let code = root.join("code");
        let graph = root.join("graph");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::create_dir_all(code.join("tests")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"affected-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            code.join("src/lib.rs"),
            "pub mod other;\npub fn changed() {}\npub fn dependent() { changed(); }\n",
        )
        .unwrap();
        fs::write(
            code.join("src/other.rs"),
            "pub fn first() {}\npub fn second() {}\n",
        )
        .unwrap();
        fs::write(
            code.join("tests/feature_test.rs"),
            "#[test]\nfn feature_test() {}\n",
        )
        .unwrap();
        build(
            &code,
            &graph,
            &BuildOptions {
                full: true,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let out_of_root = root.join("outside.rs");
        fs::write(&out_of_root, "pub fn outside() {}\n").unwrap();
        Fixture {
            root,
            code,
            graph,
            out_of_root,
        }
    }

    fn opts() -> AffectedOptions {
        AffectedOptions::default()
    }

    fn serialized(fixture: &Fixture, path: &Path) -> Vec<u8> {
        serde_json::to_vec(
            &affected_paths(
                &fixture.code,
                &fixture.graph,
                &[path.to_path_buf()],
                &opts(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_atlas_affected_normalizes_repo_relative_dot_and_absolute_paths() {
        let fixture = fixture("normalize");
        let relative = serialized(&fixture, Path::new("src/lib.rs"));
        let dotted = serialized(&fixture, Path::new("./src/lib.rs"));
        let absolute = serialized(&fixture, &fixture.code.join("src/lib.rs"));
        assert_eq!(relative, dotted);
        assert_eq!(relative, absolute);
        assert!(
            affected_paths(
                &fixture.code,
                &fixture.graph,
                &[PathBuf::from("../escape.rs")],
                &opts(),
            )
            .unwrap_err()
            .to_string()
            .contains("atlas-affected-path")
        );
        let deleted = affected_paths(
            &fixture.code,
            &fixture.graph,
            &[PathBuf::from("src/deleted.rs")],
            &opts(),
        )
        .unwrap();
        assert_eq!(deleted.files, vec!["src/deleted.rs"]);
        assert!(deleted.seeds[0].nodes.is_empty());
        let deleted_absolute = affected_paths(
            &fixture.code,
            &fixture.graph,
            &[fixture.code.join("src/deleted.rs")],
            &opts(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&deleted).unwrap(),
            serde_json::to_vec(&deleted_absolute).unwrap()
        );
        let canonical_code = fixture.code.canonicalize().unwrap();
        let deleted_alias = affected_paths(
            &canonical_code,
            &fixture.graph,
            &[fixture.code.join("src/deleted.rs")],
            &opts(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&deleted).unwrap(),
            serde_json::to_vec(&deleted_alias).unwrap()
        );
        assert!(
            affected_paths(
                &fixture.code,
                &fixture.graph,
                std::slice::from_ref(&fixture.out_of_root),
                &opts(),
            )
            .unwrap_err()
            .to_string()
            .contains("atlas-affected-path")
        );

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&fixture.out_of_root, fixture.code.join("escape.rs"))
                .unwrap();
            assert!(
                affected_paths(
                    &fixture.code,
                    &fixture.graph,
                    &[PathBuf::from("escape.rs")],
                    &opts(),
                )
                .unwrap_err()
                .to_string()
                .contains("atlas-affected-path")
            );
        }
    }

    #[test]
    fn test_atlas_affected_does_not_infer_tests_from_filenames() {
        let fixture = fixture("test-honesty");
        let result = affected_paths(
            &fixture.code,
            &fixture.graph,
            &[PathBuf::from("tests/feature_test.rs")],
            &opts(),
        )
        .unwrap();
        let value = serde_json::to_value(result).unwrap();
        let serialized = value.to_string();
        assert!(serialized.contains("tests/feature_test.rs"));
        assert!(value.get("test_selectors").is_none());
        assert!(value.get("coverage").is_none());
        assert!(!serialized.contains("inferred_test"));
    }

    #[test]
    fn affected_validates_and_applies_a_global_node_limit() {
        let fixture = fixture("global-limit");
        let options = AffectedOptions {
            impact: ImpactOptions {
                max_nodes: 1,
                ..ImpactOptions::default()
            },
        };
        let result = affected_paths(
            &fixture.code,
            &fixture.graph,
            &[PathBuf::from("src/lib.rs")],
            &options,
        )
        .unwrap();
        assert!(
            result
                .seeds
                .iter()
                .map(|seed| seed.nodes.len())
                .sum::<usize>()
                <= 1
        );
        assert!(result.affected.len() <= 1);
        assert!(result.truncated);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "atlas-affected-seeds-truncated")
        );

        let invalid = AffectedOptions {
            impact: ImpactOptions {
                max_nodes: 0,
                ..ImpactOptions::default()
            },
        };
        assert!(matches!(
            affected_paths(&fixture.code, &fixture.graph, &[], &invalid),
            Err(AtlasError::TraversalLimit { .. })
        ));
    }
}
