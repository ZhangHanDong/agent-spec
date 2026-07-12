use crate::spec_wiki::{
    WikiDiagnostic, WikiSource, WikiSourceKind, WikiSourceOptions, WikiSourceSet,
};
use std::path::{Path, PathBuf};

pub fn discover_wiki_sources(root: &Path, opts: &WikiSourceOptions) -> WikiSourceSet {
    let mut sources = Vec::new();
    let mut diagnostics = Vec::new();

    if !root.exists() {
        diagnostics.push(WikiDiagnostic {
            code: "wiki-root-missing".into(),
            severity: "error".into(),
            path: Some(root.to_path_buf()),
            message: format!("wiki source root does not exist: {}", root.display()),
        });
        return WikiSourceSet {
            sources,
            diagnostics,
        };
    }

    visit_sources(root, root, opts, &mut sources, &mut diagnostics);
    sources.sort_by(|left, right| left.path.cmp(&right.path).then(left.kind.cmp(&right.kind)));

    WikiSourceSet {
        sources,
        diagnostics,
    }
}

pub fn fingerprint_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = std::fs::read(path)?;
    Ok(fingerprint_bytes(&bytes))
}

pub fn fingerprint_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn visit_sources(
    root: &Path,
    dir: &Path,
    opts: &WikiSourceOptions,
    sources: &mut Vec<WikiSource>,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        diagnostics.push(WikiDiagnostic {
            code: "wiki-source-dir-unreadable".into(),
            severity: "warning".into(),
            path: Some(relative_path(root, dir)),
            message: format!("wiki source directory could not be read: {}", dir.display()),
        });
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if should_skip(root, &path, opts) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-source-type-unreadable".into(),
                severity: "error".into(),
                path: Some(relative_path(root, &path)),
                message: "wiki source file type could not be read".into(),
            });
            continue;
        };
        if file_type.is_symlink() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-source-symlink-rejected".into(),
                severity: "error".into(),
                path: Some(relative_path(root, &path)),
                message: "wiki source traversal rejects symbolic links".into(),
            });
            continue;
        }
        if file_type.is_dir() {
            visit_sources(root, &path, opts, sources, diagnostics);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Some(kind) = classify_source(root, &path, opts) else {
            continue;
        };
        match fingerprint_file(&path) {
            Ok(fingerprint) => sources.push(WikiSource {
                kind,
                path: relative_path(root, &path),
                fingerprint,
            }),
            Err(err) => diagnostics.push(WikiDiagnostic {
                code: "wiki-source-unreadable".into(),
                severity: "warning".into(),
                path: Some(relative_path(root, &path)),
                message: format!("wiki source could not be read: {err}"),
            }),
        }
    }
}

fn should_skip(root: &Path, path: &Path, opts: &WikiSourceOptions) -> bool {
    let rel = relative_path(root, path);
    let parts = rel
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();

    if parts
        .iter()
        .any(|part| *part == ".git" || *part == "target")
    {
        return true;
    }
    if parts.first() == Some(&"docs") && parts.get(1) == Some(&"wiki") {
        return true;
    }
    if !opts.include_archives && parts.contains(&"archive") {
        return true;
    }
    false
}

fn classify_source(root: &Path, path: &Path, opts: &WikiSourceOptions) -> Option<WikiSourceKind> {
    let rel = relative_path(root, path);
    let rel_text = path_to_slash(&rel);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    if rel_text.starts_with("src/") && path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
        return Some(WikiSourceKind::Code);
    }
    if file_name == "Cargo.toml" || file_name == "Cargo.lock" {
        return Some(WikiSourceKind::Cargo);
    }
    if rel_text.starts_with("knowledge/")
        && path.extension().and_then(|ext| ext.to_str()) == Some("md")
    {
        return Some(WikiSourceKind::Knowledge);
    }
    if rel_text.starts_with("specs/")
        && (file_name.ends_with(".spec") || file_name.ends_with(".spec.md"))
    {
        return Some(WikiSourceKind::Spec);
    }
    if rel_text.starts_with(".agent-spec/trace/")
        || (rel_text.starts_with(".agent-spec/runs/")
            && rel_text.contains("/.agent-spec/trace/")
            && file_name.ends_with(".json"))
    {
        return Some(WikiSourceKind::Trace);
    }
    if opts.include_archives
        && (rel_text.starts_with(".agent-spec/archive/") || rel_text.starts_with("specs/archive/"))
    {
        return Some(WikiSourceKind::Archive);
    }
    if rel_text.starts_with("docs/")
        && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        && !rel_text.starts_with("docs/wiki/")
    {
        return Some(WikiSourceKind::Documentation);
    }
    if is_image_path(path) {
        return Some(WikiSourceKind::Asset);
    }
    None
}

fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase()),
        Some(ext)
            if matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"
            )
    )
}

pub(crate) fn relative_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

pub fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepoPathIssue {
    Absolute,
    ParentTraversal,
    Missing,
    OutsideRoot,
}

pub(crate) fn repo_path_issue(root: &Path, path: &Path) -> Option<RepoPathIssue> {
    if path.is_absolute() {
        return Some(RepoPathIssue::Absolute);
    }
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Some(RepoPathIssue::ParentTraversal);
    }

    let candidate = root.join(path);
    if !candidate.exists() {
        return Some(RepoPathIssue::Missing);
    }
    if let (Ok(root), Ok(candidate)) = (root.canonicalize(), candidate.canonicalize())
        && candidate.strip_prefix(root).is_err()
    {
        return Some(RepoPathIssue::OutsideRoot);
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_wiki_sources_sorts_and_classifies_inputs() {
        let dir =
            std::env::temp_dir().join(format!("agent-spec-wiki-sources-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::create_dir_all(dir.join(".agent-spec/trace")).unwrap();
        fs::write(
            dir.join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-add.md"),
            "---\nkind: requirement\nid: REQ-ADD\ntitle: \"Add\"\nliveness: auto\n---\n## Problem\nAdd.\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-add.spec.md"),
            "spec: task\nname: \"Add\"\nsatisfies: [REQ-ADD]\n---\n## Intent\nAdd.\n",
        )
        .unwrap();
        fs::write(
            dir.join(".agent-spec/trace/run.json"),
            "{\"version\":1,\"records\":[],\"diagnostics\":[]}",
        )
        .unwrap();

        let opts = WikiSourceOptions::default();
        let set = discover_wiki_sources(&dir, &opts);

        assert!(set.sources.iter().any(|s| s.kind == WikiSourceKind::Code));
        assert!(
            set.sources
                .iter()
                .any(|s| s.kind == WikiSourceKind::Knowledge)
        );
        assert!(set.sources.iter().any(|s| s.kind == WikiSourceKind::Spec));
        assert!(set.sources.iter().any(|s| s.kind == WikiSourceKind::Trace));
        assert!(set.sources.windows(2).all(|w| w[0].path <= w[1].path));

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_discover_wiki_sources_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = std::env::temp_dir().join(format!(
            "agent-spec-wiki-source-symlink-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("outside.rs"), "pub fn outside() {}\n").unwrap();
        symlink(dir.join("outside.rs"), dir.join("src/linked.rs")).unwrap();

        let set = discover_wiki_sources(&dir, &WikiSourceOptions::default());

        assert!(
            !set.sources
                .iter()
                .any(|source| source.path == Path::new("src/linked.rs"))
        );
        assert!(set.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-source-symlink-rejected" && diagnostic.severity == "error"
        }));
        let _ = fs::remove_dir_all(dir);
    }
}
