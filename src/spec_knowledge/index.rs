//! Reverse index: decision id -> spec files that declare `satisfies:` it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Map of UPPERCASE decision id -> spec paths satisfying it.
pub type SatisfiesIndex = BTreeMap<String, Vec<PathBuf>>;

/// Scan `specs_dir` recursively for `*.spec.md` / `*.spec`, parse each, and
/// index its `satisfies:` ids. Unparseable specs are skipped (best-effort).
pub fn build_satisfies_index(specs_dir: &Path) -> SatisfiesIndex {
    let mut index: SatisfiesIndex = BTreeMap::new();
    for path in spec_files(specs_dir) {
        let Ok(doc) = crate::spec_parser::parse_spec(&path) else {
            continue;
        };
        for id in &doc.meta.satisfies {
            index.entry(id.clone()).or_default().push(path.clone());
        }
    }
    index
}

fn spec_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect(&p, out);
        } else if is_spec_file(&p) {
            out.push(p);
        }
    }
}

fn is_spec_file(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    name.ends_with(".spec.md") || name.ends_with(".spec")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_index_maps_decision_to_specs() {
        let dir = std::env::temp_dir().join(format!("kll-idx-{}", std::process::id()));
        let specs = dir.join("specs");
        std::fs::create_dir_all(&specs).unwrap();
        std::fs::write(
            specs.join("task-a.spec.md"),
            "spec: task\nname: \"A\"\nsatisfies: [ADR-001]\n---\n## Intent\nx\n",
        )
        .unwrap();
        std::fs::write(
            specs.join("task-b.spec.md"),
            "spec: task\nname: \"B\"\n---\n## Intent\nx\n",
        )
        .unwrap();

        let idx = build_satisfies_index(&specs);
        assert_eq!(idx.get("ADR-001").map(|v| v.len()), Some(1));
        assert!(!idx.contains_key("ADR-999"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
