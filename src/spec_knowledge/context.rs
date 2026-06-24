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

/// Read a context file under `context_dir` by relative path (traversal-guarded).
pub fn read_context(context_dir: &Path, rel: &str) -> Result<String, String> {
    let path = safe_join(context_dir, rel)?;
    std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// List context files (relative paths, sorted) under `context_dir`.
pub fn list_context(context_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect(context_dir, context_dir, &mut out);
    out.sort();
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect(root, &p, out);
        } else if let Ok(rel) = p.strip_prefix(root)
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
}
