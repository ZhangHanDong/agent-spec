use crate::spec_wiki::{
    ArchitectureDependency, ArchitectureEntrypoint, ArchitectureInventory, ArchitectureModule,
    ArchitectureModuleEdge, ArchitecturePackage, ArchitectureTarget, WikiDiagnostic, path_to_slash,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_architecture_inventory(root: &Path) -> ArchitectureInventory {
    if root.join("Cargo.toml").exists() {
        return build_rust_inventory(root);
    }
    build_generic_inventory(root, Vec::new())
}

pub fn render_architecture_mermaid(inventory: &ArchitectureInventory) -> String {
    let mut out = String::from("graph TD\n");
    if inventory.packages.is_empty() {
        out.push_str("  repo[\"Repository\"]\n");
        return out;
    }

    for package in &inventory.packages {
        out.push_str(&format!(
            "  {}[\"{}\"]\n",
            mermaid_id(&package.name),
            package.name
        ));
    }
    for dependency in &inventory.dependencies {
        out.push_str(&format!(
            "  {} --> {}\n",
            mermaid_id(&dependency.from),
            mermaid_id(&dependency.to)
        ));
    }
    out
}

pub fn render_architecture_module_mermaid(inventory: &ArchitectureInventory) -> String {
    let mut out = String::from("graph TD\n");
    if inventory.modules.is_empty() {
        out.push_str("  repo[\"Repository\"]\n");
        return out;
    }
    for module in &inventory.modules {
        out.push_str(&format!(
            "  {}[\"{}\"]\n",
            mermaid_id(&module.name),
            module.name
        ));
    }
    for edge in &inventory.module_edges {
        out.push_str(&format!(
            "  {} --> {}\n",
            mermaid_id(&edge.from),
            mermaid_id(&edge.to)
        ));
    }
    out
}

fn build_rust_inventory(root: &Path) -> ArchitectureInventory {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output();

    let Ok(output) = output else {
        return build_generic_inventory(
            root,
            vec![WikiDiagnostic {
                code: "wiki-cargo-metadata-unavailable".into(),
                severity: "warning".into(),
                path: Some(PathBuf::from("Cargo.toml")),
                message: "cargo metadata could not be executed; falling back to generic inventory"
                    .into(),
            }],
        );
    };
    if !output.status.success() {
        return build_generic_inventory(
            root,
            vec![WikiDiagnostic {
                code: "wiki-cargo-metadata-failed".into(),
                severity: "warning".into(),
                path: Some(PathBuf::from("Cargo.toml")),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            }],
        );
    }

    let parsed = serde_json::from_slice::<Value>(&output.stdout);
    let Ok(metadata) = parsed else {
        return build_generic_inventory(
            root,
            vec![WikiDiagnostic {
                code: "wiki-cargo-metadata-invalid".into(),
                severity: "warning".into(),
                path: Some(PathBuf::from("Cargo.toml")),
                message: "cargo metadata output was not valid JSON".into(),
            }],
        );
    };

    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .map(|members| {
            members
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();

    let packages_json = metadata
        .get("packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut package_names = BTreeSet::new();
    let mut package_paths = BTreeMap::new();
    let mut packages = Vec::new();
    let mut targets = Vec::new();
    let mut source_files = BTreeSet::new();
    source_files.insert(PathBuf::from("Cargo.toml"));
    if root.join("Cargo.lock").exists() {
        source_files.insert(PathBuf::from("Cargo.lock"));
    }

    for package in &packages_json {
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !workspace_members.is_empty() && !workspace_members.contains(&id) {
            continue;
        }
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let version = package
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string);
        let manifest_path = package
            .get("manifest_path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("Cargo.toml"));
        let package_dir = manifest_path.parent().unwrap_or(root);
        let rel_package_dir = relative_path(root, package_dir);
        let portable_id = portable_package_id(&id, &rel_package_dir, version.as_deref());

        package_names.insert(name.clone());
        package_paths.insert(name.clone(), rel_package_dir.clone());
        source_files.insert(relative_path(root, &manifest_path));

        packages.push(ArchitecturePackage {
            id: portable_id,
            name: name.clone(),
            version,
            path: rel_package_dir,
            language: "rust".into(),
            kind: "workspace-member".into(),
        });

        if let Some(target_array) = package.get("targets").and_then(Value::as_array) {
            for target in target_array {
                let target_name = target
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&name)
                    .to_string();
                let target_kind = target
                    .get("kind")
                    .and_then(Value::as_array)
                    .and_then(|kinds| kinds.first())
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let src_path = target
                    .get("src_path")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| package_dir.join("src/lib.rs"));
                let rel_src_path = relative_path(root, &src_path);
                source_files.insert(rel_src_path.clone());
                targets.push(ArchitectureTarget {
                    package: name.clone(),
                    name: target_name,
                    kind: target_kind,
                    src_path: rel_src_path,
                });
            }
        }
    }

    let mut dependencies = Vec::new();
    for package in &packages_json {
        let from = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        if !package_names.contains(&from) {
            continue;
        }
        let Some(deps) = package.get("dependencies").and_then(Value::as_array) else {
            continue;
        };
        for dep in deps {
            let dep_name = dep
                .get("rename")
                .and_then(Value::as_str)
                .or_else(|| dep.get("name").and_then(Value::as_str))
                .unwrap_or("unknown")
                .to_string();
            let kind = if package_names.contains(&dep_name) || dep.get("path").is_some() {
                "local"
            } else {
                "external"
            };
            dependencies.push(ArchitectureDependency {
                from: from.clone(),
                to: dep_name,
                kind: kind.into(),
                source: "cargo-metadata".into(),
            });
        }
    }

    packages.sort_by(|left, right| left.name.cmp(&right.name));
    targets.sort_by(|left, right| {
        left.package
            .cmp(&right.package)
            .then(left.name.cmp(&right.name))
    });
    dependencies.sort_by(|left, right| left.from.cmp(&right.from).then(left.to.cmp(&right.to)));
    let (modules, module_edges, diagnostics) = build_rust_module_graph(root);
    let mut entrypoints = targets
        .iter()
        .map(|target| ArchitectureEntrypoint {
            name: target.name.clone(),
            path: target.src_path.clone(),
            kind: target.kind.clone(),
        })
        .collect::<Vec<_>>();
    entrypoints.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.name.cmp(&right.name))
    });

    ArchitectureInventory {
        provider: "rust-cargo".into(),
        root: root.to_path_buf(),
        packages,
        targets,
        dependencies,
        modules,
        module_edges,
        entrypoints,
        source_files: source_files.into_iter().collect(),
        diagnostics,
    }
}

fn build_generic_inventory(
    root: &Path,
    mut diagnostics: Vec<WikiDiagnostic>,
) -> ArchitectureInventory {
    let mut source_files = Vec::new();
    collect_generic_sources(root, root, &mut source_files, &mut diagnostics);
    source_files.sort();

    ArchitectureInventory {
        provider: "generic-files".into(),
        root: root.to_path_buf(),
        packages: vec![ArchitecturePackage {
            id: project_name(root),
            name: project_name(root),
            version: None,
            path: PathBuf::from("."),
            language: "generic".into(),
            kind: "repository".into(),
        }],
        targets: Vec::new(),
        dependencies: Vec::new(),
        modules: Vec::new(),
        module_edges: Vec::new(),
        entrypoints: Vec::new(),
        source_files,
        diagnostics,
    }
}

fn collect_generic_sources(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = relative_path(root, &path);
        let rel_text = path_to_slash(&rel);
        if rel_text.starts_with(".git/")
            || rel_text.starts_with("target/")
            || rel_text.starts_with(".agent-spec/wiki/")
        {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-inventory-symlink-rejected".into(),
                severity: "error".into(),
                path: Some(rel),
                message: "architecture inventory traversal rejects symbolic links".into(),
            });
        } else if file_type.is_dir() {
            collect_generic_sources(root, &path, out, diagnostics);
        } else if file_type.is_file() && is_generic_source(&path) {
            out.push(rel);
        }
    }
}

fn is_generic_source(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase()),
        Some(ext)
            if matches!(
                ext.as_str(),
                "rs" | "toml" | "md" | "json" | "js" | "ts" | "py" | "go" | "java"
            )
    )
}

fn build_rust_module_graph(
    root: &Path,
) -> (
    Vec<ArchitectureModule>,
    Vec<ArchitectureModuleEdge>,
    Vec<WikiDiagnostic>,
) {
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    collect_rust_files(root, &root.join("src"), &mut files, &mut diagnostics);
    files.sort();
    let mut modules = Vec::new();
    let mut module_names = BTreeSet::new();
    for file in &files {
        let rel = relative_path(root, file);
        let Some(name) = rust_module_name(&rel) else {
            continue;
        };
        module_names.insert(name.clone());
        modules.push(ArchitectureModule {
            name,
            path: rel,
            kind: "rust-module".into(),
        });
    }
    modules.sort_by(|left, right| left.name.cmp(&right.name));

    let mut edges = BTreeSet::<ArchitectureModuleEdge>::new();
    for module in &modules {
        let path = root.join(&module.path);
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(child) = parse_module_decl(trimmed) {
                let target = child_module_name(&module.name, &child);
                if module_names.contains(&target) && target != module.name {
                    edges.insert(ArchitectureModuleEdge {
                        from: module.name.clone(),
                        to: target,
                        kind: "declares".into(),
                        source: path_to_slash(&module.path),
                    });
                }
            }
            if let Some(target) = parse_crate_use(trimmed, &module_names)
                && target != module.name
            {
                edges.insert(ArchitectureModuleEdge {
                    from: module.name.clone(),
                    to: target,
                    kind: "uses".into(),
                    source: path_to_slash(&module.path),
                });
            }
        }
    }
    (modules, edges.into_iter().collect(), diagnostics)
}

fn collect_rust_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-inventory-symlink-rejected".into(),
                severity: "error".into(),
                path: Some(relative_path(root, &path)),
                message: "architecture inventory traversal rejects symbolic links".into(),
            });
        } else if file_type.is_dir() {
            collect_rust_files(root, &path, out, diagnostics);
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs")
        {
            out.push(path);
        }
    }
}

fn rust_module_name(path: &Path) -> Option<String> {
    let mut components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    if components.first() != Some(&"src") {
        return None;
    }
    components.remove(0);
    if components.is_empty() {
        return None;
    }
    let file = components.pop()?;
    let stem = file.strip_suffix(".rs")?;
    if components.is_empty() && (stem == "main" || stem == "lib") {
        return Some(stem.to_string());
    }
    if stem != "mod" {
        components.push(stem);
    }
    if components.is_empty() {
        None
    } else {
        Some(components.join("::"))
    }
}

fn parse_module_decl(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("pub mod ")
        .or_else(|| line.strip_prefix("mod "))?;
    let name = rest
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()
        .unwrap_or_default();
    (!name.is_empty()).then(|| name.to_string())
}

fn child_module_name(current: &str, child: &str) -> String {
    if current == "main" || current == "lib" {
        child.to_string()
    } else {
        format!("{current}::{child}")
    }
}

fn parse_crate_use(line: &str, module_names: &BTreeSet<String>) -> Option<String> {
    let rest = line.strip_prefix("use crate::")?;
    let first = rest
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()
        .unwrap_or_default();
    if first.is_empty() {
        return None;
    }
    if module_names.contains(first) {
        return Some(first.to_string());
    }
    module_names
        .iter()
        .find(|name| name.starts_with(&format!("{first}::")))
        .cloned()
}

fn relative_path(root: &Path, path: &Path) -> PathBuf {
    if let (Ok(root), Ok(path)) = (root.canonicalize(), path.canonicalize())
        && let Ok(rel) = path.strip_prefix(root)
    {
        return non_empty_relative_path(rel);
    }
    path.strip_prefix(root)
        .map(non_empty_relative_path)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn non_empty_relative_path(path: &Path) -> PathBuf {
    if path.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        path.to_path_buf()
    }
}

fn portable_package_id(id: &str, package_path: &Path, version: Option<&str>) -> String {
    if !id.contains("file://") {
        return id.to_string();
    }

    let path = path_to_slash(package_path);
    match version {
        Some(version) => format!("path+{path}#{version}"),
        None => format!("path+{path}"),
    }
}

fn project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository")
        .replace('-', "_")
}

fn mermaid_id(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    if out.is_empty() {
        out.push_str("node");
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_inventory_source_files_are_repo_relative_for_relative_root() {
        let inventory = build_architecture_inventory(Path::new("."));

        assert_eq!(inventory.provider, "rust-cargo");
        assert!(
            inventory.source_files.iter().all(|path| path.is_relative()),
            "expected repo-relative source files, got {:?}",
            inventory.source_files
        );
        assert!(
            inventory
                .packages
                .iter()
                .all(|package| package.path.is_relative()),
            "expected repo-relative package paths, got {:?}",
            inventory.packages
        );
        assert!(
            inventory
                .targets
                .iter()
                .all(|target| target.src_path.is_relative()),
            "expected repo-relative target paths, got {:?}",
            inventory.targets
        );
        assert!(
            inventory
                .source_files
                .iter()
                .map(|path| path_to_slash(path))
                .all(|path| !path.starts_with("//")),
            "expected source files without double-slash roots, got {:?}",
            inventory.source_files
        );
        assert!(
            inventory
                .packages
                .iter()
                .all(|package| !package.id.contains("file://")),
            "expected portable package ids without file URLs, got {:?}",
            inventory.packages
        );
    }
}
