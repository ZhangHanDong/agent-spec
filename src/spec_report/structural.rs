//! Mechanical structural checks (Phase 7): dependency-cruiser-lite layering /
//! forbidden-reference enforcement. Realizes the `Probe::Static` execution
//! semantics. Pure and self-contained — no external tooling.

use std::path::{Path, PathBuf};

/// Return relative paths of files matching `file_glob` (under `code_paths`)
/// whose contents contain the `forbidden` substring. `target/` and `.git/`
/// directories are skipped.
pub fn structural_violations(
    code_paths: &[PathBuf],
    forbidden: &str,
    file_glob: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    for base in code_paths {
        let mut files = Vec::new();
        collect_files(base, &mut files);
        for f in &files {
            let rel = f
                .strip_prefix(base)
                .unwrap_or(f)
                .to_string_lossy()
                .replace('\\', "/");
            if !glob_matches(file_glob, &rel) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(f)
                && content.contains(forbidden)
            {
                violations.push(rel);
            }
        }
    }
    violations.sort();
    violations.dedup();
    violations
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == ".git" {
                continue;
            }
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Minimal glob match supporting `**` (any depth) and a trailing `/**`,
/// plus a `*` wildcard within a path segment. `**` alone matches everything.
fn glob_matches(glob: &str, path: &str) -> bool {
    if glob == "**" || glob == "**/*" {
        return true;
    }
    if let Some(prefix) = glob.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = glob.strip_suffix("**") {
        return path.starts_with(prefix);
    }
    // Fall back to exact or simple prefix-with-* match.
    if let Some(prefix) = glob.strip_suffix('*') {
        return path.starts_with(prefix);
    }
    glob == path
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_tree(files: &[(&str, &str)]) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent_spec_struct_{stamp}"));
        for (rel, content) in files {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        root
    }

    #[test]
    fn test_structural_flags_forbidden_reference() {
        let root = temp_tree(&[("clients/a.rs", "use crate::services::X;\nfn f() {}\n")]);
        let v = structural_violations(&[root.clone()], "crate::services", "clients/**");
        assert!(v.iter().any(|p| p == "clients/a.rs"), "got {v:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_structural_no_violation_returns_empty() {
        let root = temp_tree(&[("clients/a.rs", "fn clean() {}\n")]);
        let v = structural_violations(&[root.clone()], "crate::services", "clients/**");
        assert!(v.is_empty(), "got {v:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_structural_respects_glob_scope() {
        let root = temp_tree(&[("services/b.rs", "use crate::services::X;\n")]);
        let v = structural_violations(&[root.clone()], "crate::services", "clients/**");
        assert!(!v.iter().any(|p| p == "services/b.rs"), "got {v:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_structural_skips_target_dir() {
        let root = temp_tree(&[("target/x.rs", "crate::services\n")]);
        let v = structural_violations(&[root.clone()], "crate::services", "**");
        assert!(!v.iter().any(|p| p.contains("target")), "got {v:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_check_structure_reports_violations() {
        let root = temp_tree(&[("clients/a.rs", "use crate::services::X;\n")]);
        let v = structural_violations(&[root.clone()], "crate::services", "clients/**");
        assert!(!v.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
