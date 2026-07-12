//! Hand-written knowledge parser (mirrors spec_parser/meta.rs). No serde_yaml.

use crate::spec_knowledge::model::{
    DecisionStatus, KSection, KnowledgeDoc, KnowledgeKind, KnowledgeMeta, LivenessDeclared,
};
use std::path::Path;

/// Validate stable knowledge ids before they can participate in trace links or
/// generated filesystem paths. IDs are ASCII alphanumeric segments separated
/// by single hyphens and must start with a letter.
pub fn validate_knowledge_id(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty()
        || !id.as_bytes()[0].is_ascii_alphabetic()
        || !id.contains('-')
        || id.starts_with('-')
        || id.ends_with('-')
        || id.contains("--")
        || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err(format!(
            "invalid knowledge id `{id}`; expected ASCII alphanumeric segments separated by single hyphens"
        ));
    }
    Ok(())
}

/// Resolve a decision id (§6.0): frontmatter `id:` is canonical; else the
/// filename prefix `<letters>-<digits>` normalized to UPPERCASE. Returns the
/// normalized id, or `None` when neither source yields one.
pub fn resolve_decision_id(frontmatter_id: Option<&str>, path: &Path) -> Option<String> {
    if let Some(id) = frontmatter_id {
        let id = id.trim();
        if validate_knowledge_id(id).is_ok() {
            return Some(id.to_ascii_uppercase());
        }
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    // take leading <letters>-<digits>
    let mut parts = stem.splitn(3, '-');
    let letters = parts.next()?;
    let digits = parts.next()?;
    if !letters.is_empty()
        && letters.chars().all(|c| c.is_ascii_alphabetic())
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
    {
        Some(format!("{}-{}", letters.to_ascii_uppercase(), digits))
    } else {
        None
    }
}

/// Parse a knowledge document of any kind from a string. `path` is used for id
/// fallback. The kind is read from frontmatter `kind:` (defaults to decision).
pub fn parse_knowledge_str(input: &str, path: &Path) -> Result<KnowledgeDoc, String> {
    let lines: Vec<&str> = input.lines().collect();
    if lines.first().map(|line| line.trim()) != Some("---") {
        return Err("front-matter must start on line 1 with '---'".to_string());
    }
    // front-matter is between the first `---` and the next `---`
    let rest = &lines[1..];
    let close = rest
        .iter()
        .position(|l| l.trim() == "---")
        .ok_or_else(|| "missing closing front-matter '---'".to_string())?;
    let meta_lines = &rest[..close];
    let body_lines = &rest[close + 1..];

    let meta = parse_knowledge_meta(meta_lines, path)?;
    let sections = parse_sections(body_lines);
    Ok(KnowledgeDoc {
        meta,
        sections,
        source_path: path.to_path_buf(),
    })
}

/// Parse a knowledge document of any kind from disk.
pub fn parse_knowledge(path: &Path) -> Result<KnowledgeDoc, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_knowledge_str(&content, path)
}

/// Parse a string, requiring it to be of the given `kind`.
fn parse_kind_str(input: &str, path: &Path, kind: KnowledgeKind) -> Result<KnowledgeDoc, String> {
    let doc = parse_knowledge_str(input, path)?;
    if doc.meta.kind != kind {
        return Err(format!(
            "expected kind {:?}, found {:?}",
            kind, doc.meta.kind
        ));
    }
    Ok(doc)
}

/// Parse a decision document from a string (strict: kind must be decision).
pub fn parse_decision_str(input: &str, path: &Path) -> Result<KnowledgeDoc, String> {
    parse_kind_str(input, path, KnowledgeKind::Decision)
}

/// Parse a decision document from disk (strict: kind must be decision).
pub fn parse_decision(path: &Path) -> Result<KnowledgeDoc, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_decision_str(&content, path)
}

/// Parse a requirement document from a string (strict: kind must be requirement).
pub fn parse_requirement_str(input: &str, path: &Path) -> Result<KnowledgeDoc, String> {
    parse_kind_str(input, path, KnowledgeKind::Requirement)
}

/// Parse a requirement document from disk (strict: kind must be requirement).
pub fn parse_requirement(path: &Path) -> Result<KnowledgeDoc, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_requirement_str(&content, path)
}

fn parse_knowledge_meta(lines: &[&str], path: &Path) -> Result<KnowledgeMeta, String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut id_field: Option<String> = None;
    let mut title: Option<String> = None;
    let mut status: Option<DecisionStatus> = None;
    let mut supersedes: Option<String> = None;
    let mut liveness = LivenessDeclared::Auto;
    let mut kind = KnowledgeKind::Decision;
    let mut tags = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once(':') else {
            return Err(format!(
                "front-matter line {} must use `key: value` syntax",
                index + 2
            ));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("front-matter line {} has an empty key", index + 2));
        }
        if !seen.insert(key.to_string()) {
            return Err(format!(
                "front-matter line {} duplicates key `{key}`",
                index + 2
            ));
        }
        let val = val.trim().trim_matches('"').trim();
        match key {
            "kind" => {
                kind = KnowledgeKind::parse(val)
                    .ok_or_else(|| format!("unsupported knowledge kind '{val}'"))?;
            }
            "id" => {
                validate_knowledge_id(val)?;
                id_field = Some(val.to_string());
            }
            "title" => title = Some(val.to_string()),
            "status" => {
                status = Some(match val.to_ascii_lowercase().as_str() {
                    "proposed" => DecisionStatus::Proposed,
                    "accepted" => DecisionStatus::Accepted,
                    "superseded" => DecisionStatus::Superseded,
                    "deprecated" => DecisionStatus::Deprecated,
                    "rejected" => DecisionStatus::Rejected,
                    other => return Err(format!("unknown status '{other}'")),
                });
            }
            "supersedes" => {
                validate_knowledge_id(val)?;
                supersedes = Some(val.to_ascii_uppercase());
            }
            "liveness" => {
                liveness = match val.to_ascii_lowercase().as_str() {
                    "auto" => LivenessDeclared::Auto,
                    "n/a" | "na" => LivenessDeclared::Na,
                    other => return Err(format!("unknown liveness '{other}'")),
                };
            }
            "tags" => {
                let val = val.trim_start_matches('[').trim_end_matches(']');
                for tag in val.split(',') {
                    let t = tag.trim().trim_matches('"');
                    if !t.is_empty() {
                        tags.push(t.to_string());
                    }
                }
            }
            _ => {} // unknown keys ignored (forward-compat), like spec meta
        }
    }

    let id = resolve_decision_id(id_field.as_deref(), path).ok_or_else(|| {
        "knowledge doc has no resolvable id (frontmatter id: or <letters>-<digits> filename)"
            .to_string()
    })?;

    Ok(KnowledgeMeta {
        kind,
        id,
        title,
        status,
        supersedes,
        liveness,
        tags,
    })
}

/// Split body into `## Heading` sections (level-2 only for P1).
fn parse_sections(lines: &[&str]) -> Vec<KSection> {
    let mut sections: Vec<KSection> = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    for line in lines {
        if let Some(h) = line.strip_prefix("## ") {
            if let Some((heading, body)) = current.take() {
                sections.push(KSection {
                    heading,
                    body: body.join("\n").trim().to_string(),
                });
            }
            current = Some((h.trim().to_string(), Vec::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push((*line).to_string());
        }
    }
    if let Some((heading, body)) = current.take() {
        sections.push(KSection {
            heading,
            body: body.join("\n").trim().to_string(),
        });
    }
    sections
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_id_from_frontmatter_is_uppercased() {
        let p = PathBuf::from("knowledge/decisions/adr-001-soft-delete.md");
        assert_eq!(
            resolve_decision_id(Some("adr-001"), &p),
            Some("ADR-001".into())
        );
    }

    #[test]
    fn test_id_falls_back_to_filename_prefix() {
        let p = PathBuf::from("knowledge/decisions/adr-007-x.md");
        assert_eq!(resolve_decision_id(None, &p), Some("ADR-007".into()));
    }

    #[test]
    fn test_id_none_when_no_prefix_and_no_frontmatter() {
        let p = PathBuf::from("knowledge/decisions/notes.md");
        assert_eq!(resolve_decision_id(None, &p), None);
    }

    #[test]
    fn test_parse_decision_minimal() {
        let input = "---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n\n## Context\n\nWhy.\n\n## Decision\n\nDo X.\n\n## Consequences\n\nGood, because A. Bad, because B.\n";
        let doc = parse_decision_str(input, Path::new("adr-001-x.md")).unwrap();
        assert_eq!(doc.meta.id, "ADR-001");
        assert_eq!(doc.meta.status, Some(DecisionStatus::Accepted));
        assert_eq!(doc.meta.liveness, LivenessDeclared::Auto);
        assert!(doc.section("Context").is_some());
        assert!(doc.section("Decision").is_some());
        assert!(doc.section("Consequences").is_some());
    }

    #[test]
    fn test_parse_requirement_title_metadata() {
        let input = "---\nkind: requirement\nid: REQ-101\ntitle: \"User Login\"\n---\n\n## Problem\np\n## Requirements\n[REQ-101] The system MUST log users in.\n";
        let doc = parse_requirement_str(input, Path::new("req-101-user-login.md")).unwrap();
        assert_eq!(doc.meta.title.as_deref(), Some("User Login"));
    }

    #[test]
    fn test_parse_liveness_na() {
        let input = "---\nkind: decision\nid: ADR-009\nliveness: n/a\n---\n\n## Context\n\nLicense.\n\n## Decision\n\nMIT.\n\n## Consequences\n\nGood. Bad.\n";
        let doc = parse_decision_str(input, Path::new("adr-009.md")).unwrap();
        assert_eq!(doc.meta.liveness, LivenessDeclared::Na);
    }

    #[test]
    fn test_knowledge_parser_rejects_unsafe_ids_and_malformed_frontmatter() {
        for id in [
            "../../REQ-ESCAPE",
            "/tmp/REQ-ESCAPE",
            "REQ--EMPTY",
            "REQ.BAD",
        ] {
            let input = format!(
                "---\nkind: requirement\nid: {id}\ntitle: Bad\n---\n## Problem\np\n## Requirements\n[{id}] The system MUST work.\n"
            );
            assert!(
                parse_requirement_str(&input, Path::new("req-safe.md")).is_err(),
                "unsafe id must be rejected: {id}"
            );
        }

        let duplicate = "---\nkind: requirement\nid: REQ-A\nid: REQ-B\ntitle: Duplicate\n---\n## Problem\np\n## Requirements\n[REQ-A] The system MUST work.\n";
        assert!(parse_requirement_str(duplicate, Path::new("req-a.md")).is_err());

        let malformed = "---\nkind: requirement\nid: REQ-A\ntitle without colon\n---\n## Problem\np\n## Requirements\n[REQ-A] The system MUST work.\n";
        assert!(parse_requirement_str(malformed, Path::new("req-a.md")).is_err());

        let leading_content = "# Heading\n---\nkind: requirement\nid: REQ-A\n---\n## Problem\np\n## Requirements\n[REQ-A] The system MUST work.\n";
        assert!(parse_requirement_str(leading_content, Path::new("req-a.md")).is_err());
    }
}
