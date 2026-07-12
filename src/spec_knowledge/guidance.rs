//! Guidance artifacts (KLL P2, §6.4): agent-facing instructions + skill
//! designation. `liveness` is always `n/a` (never enters the code gate);
//! consumed via `gen-integrations` projection and MCP `guidance.for`.

use crate::spec_knowledge::model::KnowledgeDoc;

/// Lines of a `##`-section body as trimmed, bullet-stripped, non-empty items.
fn section_items(doc: &KnowledgeDoc, heading: &str) -> Vec<String> {
    let Some(section) = doc.section(heading) else {
        return Vec::new();
    };
    section
        .body
        .lines()
        .map(|l| l.trim().trim_start_matches('-').trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Path/stack globs the guidance applies to (from `## Applies To`).
pub fn applies_to(doc: &KnowledgeDoc) -> Vec<String> {
    section_items(doc, "Applies To")
}

/// Skills this guidance designates for its scope (from `## Skills`).
pub fn skills(doc: &KnowledgeDoc) -> Vec<String> {
    section_items(doc, "Skills")
}

/// Whether the guidance applies to `path`. A guidance with no `## Applies To`
/// globs is global (matches everything); otherwise any glob match wins.
pub fn applies_to_path(doc: &KnowledgeDoc, path: &str) -> bool {
    let globs = applies_to(doc);
    if globs.is_empty() {
        return true;
    }
    globs.iter().any(|g| glob_match(g, path))
}

/// Whether the guidance applies to a `stack` (e.g. "rust"): matched against
/// `tags` (case-insensitive) or an exact `## Applies To` token.
pub fn applies_to_stack(doc: &KnowledgeDoc, stack: &str) -> bool {
    let stack_lc = stack.to_ascii_lowercase();
    if doc
        .meta
        .tags
        .iter()
        .any(|t| t.eq_ignore_ascii_case(&stack_lc))
    {
        return true;
    }
    applies_to(doc)
        .iter()
        .any(|g| g.eq_ignore_ascii_case(&stack_lc))
}

/// Minimal path-glob matcher: `?` = one non-slash char, `*` = any run of
/// non-slash chars, `**` = any run including slashes. Matches the whole text.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    matches_from(&p, 0, &t, 0)
}

fn matches_from(p: &[char], pi: usize, t: &[char], ti: usize) -> bool {
    if pi == p.len() {
        return ti == t.len();
    }
    match p[pi] {
        '*' => {
            // `**` crosses slashes; `*` does not.
            let double = pi + 1 < p.len() && p[pi + 1] == '*';
            let next_pi = if double { pi + 2 } else { pi + 1 };
            // zero-width match
            if matches_from(p, next_pi, t, ti) {
                return true;
            }
            // consume one char and retry, respecting slash boundary for single `*`.
            if ti < t.len() && (double || t[ti] != '/') {
                return matches_from(p, pi, t, ti + 1);
            }
            false
        }
        '?' => {
            if ti < t.len() && t[ti] != '/' {
                matches_from(p, pi + 1, t, ti + 1)
            } else {
                false
            }
        }
        c => {
            if ti < t.len() && t[ti] == c {
                matches_from(p, pi + 1, t, ti + 1)
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::parser::parse_knowledge_str;
    use std::path::Path;

    fn parse(input: &str) -> KnowledgeDoc {
        parse_knowledge_str(input, Path::new("g-001-x.md")).unwrap()
    }

    #[test]
    fn test_glob_match_segments_and_globstar() {
        assert!(glob_match("src/*.rs", "src/main.rs"));
        assert!(!glob_match("src/*.rs", "src/a/main.rs"));
        assert!(glob_match("src/**", "src/a/b/main.rs"));
        assert!(glob_match("**/*.rs", "src/a/main.rs"));
        assert!(!glob_match("*.rs", "main.py"));
    }

    #[test]
    fn test_applies_to_path_and_skills() {
        let doc = parse(
            "---\nkind: guidance\nid: G-001\nliveness: n/a\ntags: [rust]\n---\n## Scope\ns\n## Instructions\ni\n## Applies To\nsrc/**\n## Skills\n- tdd\n- debugging\n",
        );
        assert!(applies_to_path(&doc, "src/spec_knowledge/mod.rs"));
        assert!(!applies_to_path(&doc, "docs/readme.md"));
        assert!(applies_to_stack(&doc, "Rust"));
        assert_eq!(
            skills(&doc),
            vec!["tdd".to_string(), "debugging".to_string()]
        );
    }

    #[test]
    fn test_no_applies_to_is_global() {
        let doc = parse(
            "---\nkind: guidance\nid: G-002\nliveness: n/a\n---\n## Scope\ns\n## Instructions\ni\n",
        );
        assert!(applies_to_path(&doc, "anything/at/all.rs"));
    }
}
