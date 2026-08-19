//! Single source of truth for boundary path expressions.
//!
//! `### Allowed Changes` entries are path expressions by definition: every
//! entry is normalized and takes part in matching — nothing is silently
//! dropped by a "looks like a path" heuristic. `### Forbidden` / general
//! entries mix natural-language prohibitions with paths, so only an entry
//! that is a single path token becomes a forbidden pattern; paths quoted
//! inside prose are never extracted.
//!
//! The boundaries verifier, plan scanning and the MCP `spec_allows_path`
//! tool all go through this module so they agree on every entry and path.

use std::path::Path;

use super::{BoundaryCategory, Section};

/// Split a raw boundary entry into its path body and an optional trailing
/// note. A note starts with ` — ` (em dash) or ` #` outside any backtick
/// span and preceded by whitespace. Parentheses are never treated as a note.
pub fn split_boundary_note(raw: &str) -> (&str, Option<&str>) {
    let mut in_code = false;
    let mut prev_ws = false;
    for (idx, ch) in raw.char_indices() {
        if ch == '`' {
            in_code = !in_code;
            prev_ws = false;
            continue;
        }
        if !in_code && prev_ws && (ch == '—' || ch == '#') {
            let after = &raw[idx + ch.len_utf8()..];
            if ch == '#' || after.starts_with(char::is_whitespace) || after.is_empty() {
                let note = after.trim();
                return (
                    raw[..idx].trim_end(),
                    if note.is_empty() { None } else { Some(note) },
                );
            }
        }
        prev_ws = ch.is_whitespace();
    }
    (raw.trim_end(), None)
}

/// Normalize a boundary entry into a repo-root-relative pattern:
/// strip note → trim → strip backticks → `\` to `/` → strip leading `./`
/// → strip surrounding `/`. A bare filename stays as-is and means a
/// repo-root file.
pub fn normalize_boundary_pattern(raw: &str) -> String {
    let (body, _) = split_boundary_note(raw);
    body.trim()
        .trim_matches('`')
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

/// Whether a normalized entry is a single path token: non-empty, no
/// whitespace, and carrying at least one path-ish character.
pub fn is_path_token(normalized: &str) -> bool {
    !normalized.is_empty()
        && !normalized.chars().any(char::is_whitespace)
        && normalized.contains(['/', '*', '.', '?'])
}

/// Collect `(allowed, forbidden)` patterns from a contract's boundary
/// sections. Every Allow entry becomes a pattern; Deny/General entries only
/// when they are a single path token; Symbols entries are never paths.
pub fn collect_boundary_patterns(sections: &[Section]) -> (Vec<String>, Vec<String>) {
    let mut allowed = Vec::new();
    let mut forbidden = Vec::new();
    for section in sections {
        let Section::Boundaries { items, .. } = section else {
            continue;
        };
        for item in items {
            let normalized = normalize_boundary_pattern(&item.text);
            match item.category {
                BoundaryCategory::Allow => {
                    if !normalized.is_empty() {
                        allowed.push(normalized);
                    }
                }
                BoundaryCategory::Symbols => {}
                BoundaryCategory::Deny | BoundaryCategory::General => {
                    if is_path_token(&normalized) {
                        forbidden.push(normalized);
                    }
                }
            }
        }
    }
    (allowed, forbidden)
}

/// Normalize a changed path against an optional workspace root into the
/// same shape patterns use.
pub fn normalize_change_path(path: &Path, workspace_root: Option<&Path>) -> String {
    let candidate = workspace_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    candidate
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

/// Segment-wise glob match: `*` matches within one segment, `**` spans
/// segments. Both sides are expected in normalized form.
pub fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&pattern_segments, &path_segments)
}

/// Whether an unnormalized entry (as written in the spec) allows `path`.
pub fn entry_allows_path(raw_entry: &str, path: &str) -> bool {
    let pattern = normalize_boundary_pattern(raw_entry);
    !pattern.is_empty()
        && path_matches_pattern(&pattern, &normalize_change_path(Path::new(path), None))
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return (0..=path.len()).any(|index| match_segments(&pattern[1..], &path[index..]));
    }
    if path.is_empty() {
        return false;
    }
    segment_matches(pattern[0], path[0]) && match_segments(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == segment;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let mut cursor = 0usize;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 && anchored_start {
            if !segment[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            continue;
        }
        if let Some(found) = segment[cursor..].find(part) {
            cursor += found + part.len();
        } else {
            return false;
        }
    }
    if anchored_end && let Some(last_part) = parts.iter().rev().find(|part| !part.is_empty()) {
        return segment.ends_with(last_part);
    }
    true
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_boundary_pattern_strips_backticks_and_dot_slash() {
        // Order: note → trim → backticks → slashes → leading ./ → surrounding /
        assert_eq!(
            normalize_boundary_pattern("`./src\\lib.rs` — note"),
            "src/lib.rs"
        );
        assert_eq!(normalize_boundary_pattern("./Cargo.toml"), "Cargo.toml");
        assert_eq!(normalize_boundary_pattern("`CLAUDE.md`"), "CLAUDE.md");
        assert_eq!(normalize_boundary_pattern("crates/foo/"), "crates/foo");
    }

    #[test]
    fn test_boundary_note_split_keeps_parentheses() {
        assert_eq!(
            normalize_boundary_pattern("`Cargo.toml` — dev-dep only"),
            "Cargo.toml"
        );
        assert_eq!(
            normalize_boundary_pattern("src/a.rs # new file"),
            "src/a.rs"
        );
        assert_eq!(
            normalize_boundary_pattern("docs/foo (copy).md"),
            "docs/foo (copy).md"
        );
        assert_eq!(
            split_boundary_note("`Cargo.toml` — dev-dep only"),
            ("`Cargo.toml`", Some("dev-dep only"))
        );
        assert_eq!(
            split_boundary_note("docs/foo (copy).md"),
            ("docs/foo (copy).md", None)
        );
    }

    #[test]
    fn test_boundary_note_ignores_delimiters_inside_backticks() {
        assert_eq!(
            normalize_boundary_pattern("`docs/a — b.md`"),
            "docs/a — b.md"
        );
        assert_eq!(normalize_boundary_pattern("`src/#tag.rs`"), "src/#tag.rs");
        // A dash glued to a word is not a note delimiter either.
        assert_eq!(normalize_boundary_pattern("docs/a—b.md"), "docs/a—b.md");
    }

    #[test]
    fn test_is_path_token_requires_pathish_char_and_no_whitespace() {
        assert!(is_path_token("src/lib.rs"));
        assert!(is_path_token("Cargo.toml"));
        assert!(is_path_token("src/**"));
        assert!(!is_path_token("LICENSE"));
        assert!(!is_path_token("Do not modify src/x.rs"));
        assert!(!is_path_token("不新增依赖"));
        assert!(!is_path_token(""));
    }

    #[test]
    fn matches_double_star_path_patterns() {
        assert!(path_matches_pattern(
            "crates/spec-parser/**",
            "crates/spec-parser/src/parser.rs"
        ));
        assert!(path_matches_pattern("specs/**", "specs/task.spec"));
        assert!(!path_matches_pattern(
            "crates/spec-parser/**",
            "crates/spec-gateway/src/lib.rs"
        ));
        assert!(path_matches_pattern("docs/*.md", "docs/x.md"));
        assert!(!path_matches_pattern("docs/*.md", "docs/sub/x.md"));
    }

    #[test]
    fn test_boundary_no_local_looks_like_path_helpers_remain() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for rel in ["src/spec_verify/boundaries.rs", "src/spec_gateway/plan.rs"] {
            let src = std::fs::read_to_string(root.join(rel)).unwrap();
            assert!(
                !src.contains("fn looks_like_path"),
                "{rel} still carries a local path heuristic"
            );
        }
    }
}
