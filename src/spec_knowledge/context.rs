//! Free-form context serve (KLL P2, §6.5). `knowledge/context/` holds arbitrary
//! Markdown — not typed, not linted, no schema. Served read-only via MCP
//! `context.read(path)`. This module is the safe reader behind it.

use std::path::{Component, Path, PathBuf};

/// Resolve a caller-supplied relative path against `context_dir`, rejecting any
/// traversal (`..`, absolute, or rooted components). Returns the safe joined
/// path, or an error describing why it was rejected.
pub fn safe_join(context_dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel_path = Path::new(rel);
    let mut out = context_dir.to_path_buf();
    for comp in rel_path.components() {
        match comp {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!("path escapes context root: {rel}"));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("absolute paths are not allowed: {rel}"));
            }
        }
    }
    Ok(out)
}

/// Read a context file under `context_dir` by relative path. The lexical guard
/// rejects `..`/absolute paths, and the canonical guard rejects symlink escapes.
pub fn read_context(context_dir: &Path, rel: &str) -> Result<String, String> {
    let root = context_dir
        .canonicalize()
        .map_err(|e| format!("cannot resolve context root {}: {e}", context_dir.display()))?;
    let path = safe_join(context_dir, rel)?;
    reject_symlink_components(context_dir, &path)?;
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", path.display()))?;
    if !canonical.starts_with(&root) {
        return Err(format!("path escapes context root: {rel}"));
    }
    std::fs::read_to_string(&canonical)
        .map_err(|e| format!("cannot read {}: {e}", canonical.display()))
}

fn reject_symlink_components(root: &Path, path: &Path) -> Result<(), String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| format!("path escapes context root: {}", path.display()))?;
    let mut current = root.to_path_buf();
    for comp in rel.components() {
        current.push(comp.as_os_str());
        let meta = std::fs::symlink_metadata(&current)
            .map_err(|e| format!("cannot inspect {}: {e}", current.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("symlinks are not allowed: {}", current.display()));
        }
    }
    Ok(())
}

/// List context files (relative paths, sorted) under `context_dir`.
pub fn list_context(context_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(root) = context_dir.canonicalize() else {
        return out;
    };
    collect(&root, &root, &mut out);
    out.sort();
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let Ok(canonical) = p.canonicalize() else {
            continue;
        };
        if !canonical.starts_with(root) {
            continue;
        }
        if file_type.is_dir() {
            collect(root, &p, out);
        } else if canonical.is_file()
            && let Ok(rel) = p.strip_prefix(root)
            && let Some(s) = rel.to_str()
        {
            out.push(s.to_string());
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_join_rejects_traversal() {
        let root = Path::new("/k/context");
        assert!(safe_join(root, "../secrets.md").is_err());
        assert!(safe_join(root, "/etc/passwd").is_err());
        assert_eq!(
            safe_join(root, "notes/a.md").unwrap(),
            PathBuf::from("/k/context/notes/a.md")
        );
    }

    #[test]
    fn test_read_and_list_context() {
        let dir = std::env::temp_dir().join(format!("kll-ctx-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        std::fs::write(dir.join("a.md"), "alpha").unwrap();
        std::fs::write(dir.join("notes/b.md"), "beta").unwrap();

        assert_eq!(read_context(&dir, "a.md").unwrap(), "alpha");
        assert_eq!(read_context(&dir, "notes/b.md").unwrap(), "beta");
        assert!(read_context(&dir, "../x").is_err());

        let listing = list_context(&dir);
        assert!(listing.contains(&"a.md".to_string()));
        assert!(listing.contains(&"notes/b.md".to_string()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn test_read_context_rejects_symlink_escape() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("kll-ctx-root-{stamp}"));
        let outside = std::env::temp_dir().join(format!("kll-ctx-outside-{stamp}.md"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, "outside secret").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("leak.md")).unwrap();

        let result = read_context(&root, "leak.md");

        assert!(result.is_err(), "symlink escape must be rejected");

        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_file(&outside).ok();
    }

    #[cfg(unix)]
    #[test]
    fn test_read_context_rejects_internal_symlink() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("kll-ctx-root-{stamp}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("target.md"), "inside").unwrap();
        std::os::unix::fs::symlink(root.join("target.md"), root.join("alias.md")).unwrap();

        let result = read_context(&root, "alias.md");

        assert!(result.is_err(), "context symlinks should be rejected");

        std::fs::remove_dir_all(&root).ok();
    }
}
