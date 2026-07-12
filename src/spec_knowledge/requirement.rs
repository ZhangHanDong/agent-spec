//! Requirement artifacts (KLL P2, §6.2): EARS/29148-style normative clauses.
//!
//! A `requirement` doc carries a `## Requirements` section, one normative
//! clause per line, ideally `[REQ-NNN] … MUST/SHOULD/MAY …`. This module turns
//! that prose into structured clauses so the lint (§6.2) can check them.

use crate::spec_knowledge::model::KnowledgeDoc;

/// A BCP-14 (RFC 2119/8174) normative keyword, normalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormativeKeyword {
    Must,
    MustNot,
    Should,
    ShouldNot,
    May,
}

impl NormativeKeyword {
    /// Detect the first normative keyword in a clause (case-sensitive on the
    /// uppercase BCP-14 spelling, which is what the standard mandates). Checks
    /// the negative forms before the positive ones so `MUST NOT` is not read as
    /// `MUST`.
    pub fn detect(text: &str) -> Option<Self> {
        // (needle, keyword) in priority order.
        const TABLE: &[(&str, NormativeKeyword)] = &[
            ("MUST NOT", NormativeKeyword::MustNot),
            ("SHALL NOT", NormativeKeyword::MustNot),
            ("SHOULD NOT", NormativeKeyword::ShouldNot),
            ("NOT RECOMMENDED", NormativeKeyword::ShouldNot),
            ("MUST", NormativeKeyword::Must),
            ("SHALL", NormativeKeyword::Must),
            ("REQUIRED", NormativeKeyword::Must),
            ("SHOULD", NormativeKeyword::Should),
            ("RECOMMENDED", NormativeKeyword::Should),
            ("MAY", NormativeKeyword::May),
            ("OPTIONAL", NormativeKeyword::May),
        ];
        // Find the earliest-position match; ties broken by table order (negatives first).
        let mut best: Option<(usize, NormativeKeyword)> = None;
        for (needle, kw) in TABLE {
            if let Some(pos) = text.find(needle) {
                match best {
                    Some((bp, _)) if bp <= pos => {}
                    _ => best = Some((pos, *kw)),
                }
            }
        }
        best.map(|(_, kw)| kw)
    }
}

/// Count whole-word BCP-14 base tokens in a clause (case-sensitive uppercase).
/// Used by the 29148 single-statement lint: >1 token = a compound requirement.
pub fn normative_token_count(text: &str) -> usize {
    const TOKENS: &[&str] = &[
        "MUST",
        "SHALL",
        "SHOULD",
        "MAY",
        "REQUIRED",
        "RECOMMENDED",
        "OPTIONAL",
    ];
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| TOKENS.contains(w))
        .count()
}

/// One normative clause from a requirement's `## Requirements` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementClause {
    /// `REQ-NNN` id if the line begins with `[REQ-NNN]`, normalized UPPERCASE.
    pub id: Option<String>,
    /// The detected normative keyword, if any.
    pub keyword: Option<NormativeKeyword>,
    /// The full clause text (without the leading `[id]` marker).
    pub text: String,
}

/// Extract normative clauses from a requirement doc's `## Requirements` section.
/// One clause per non-empty, non-bullet-only line. Lines may start with a `-`
/// bullet which is stripped.
pub fn extract_requirements(doc: &KnowledgeDoc) -> Vec<RequirementClause> {
    let Some(section) = doc.section("Requirements") else {
        return Vec::new();
    };
    let mut clauses = Vec::new();
    for raw in section.body.lines() {
        let line = raw.trim().trim_start_matches('-').trim();
        if line.is_empty() {
            continue;
        }
        let (id, text) = split_id_prefix(line);
        let keyword = NormativeKeyword::detect(&text);
        clauses.push(RequirementClause { id, keyword, text });
    }
    clauses
}

/// Split a leading `[REQ-NNN]` marker off a clause, returning (id, rest).
fn split_id_prefix(line: &str) -> (Option<String>, String) {
    if let Some(rest) = line.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        let id = rest[..end].trim().to_ascii_uppercase();
        let text = rest[end + 1..].trim().to_string();
        if !id.is_empty() {
            return (Some(id), text);
        }
    }
    (None, line.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::parser::parse_requirement_str;
    use std::path::Path;

    fn parse(input: &str) -> KnowledgeDoc {
        parse_requirement_str(input, Path::new("req-001-x.md")).unwrap()
    }

    #[test]
    fn test_detect_keyword_negatives_before_positives() {
        assert_eq!(
            NormativeKeyword::detect("the system MUST NOT log secrets"),
            Some(NormativeKeyword::MustNot)
        );
        assert_eq!(
            NormativeKeyword::detect("the system MUST retry"),
            Some(NormativeKeyword::Must)
        );
        assert_eq!(NormativeKeyword::detect("it should be lowercase"), None);
    }

    #[test]
    fn test_extract_requirements_ids_and_keywords() {
        let doc = parse(
            "---\nkind: requirement\nid: REQ-001\n---\n## Problem\np\n## Requirements\n[REQ-001] The API MUST return 429 on rate limit.\n[REQ-002] The client SHOULD back off exponentially.\nThe cache MAY be warmed at boot.\n",
        );
        let clauses = extract_requirements(&doc);
        assert_eq!(clauses.len(), 3);
        assert_eq!(clauses[0].id.as_deref(), Some("REQ-001"));
        assert_eq!(clauses[0].keyword, Some(NormativeKeyword::Must));
        assert_eq!(clauses[1].id.as_deref(), Some("REQ-002"));
        assert_eq!(clauses[1].keyword, Some(NormativeKeyword::Should));
        assert_eq!(clauses[2].id, None);
        assert_eq!(clauses[2].keyword, Some(NormativeKeyword::May));
    }

    #[test]
    fn test_extract_empty_when_no_requirements_section() {
        let doc = parse("---\nkind: requirement\nid: REQ-009\n---\n## Problem\nonly a problem\n");
        assert!(extract_requirements(&doc).is_empty());
    }
}
