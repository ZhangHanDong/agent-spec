//! Requirement artifacts (KLL P2, §6.2): EARS/29148-style normative clauses.
//!
//! A `requirement` doc carries a `## Requirements` section, one normative
//! clause per line, ideally `[REQ-NNN] … MUST/SHOULD/MAY …`. This module turns
//! that prose into structured clauses so the lint (§6.2) can check them.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::spec_core::Verdict;
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

/// Explicit coverage of identifiable MUST/MUST NOT clauses by requirement
/// scenarios grouped under `Rule: <clause-id>`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClauseCoverage {
    pub covered: Vec<String>,
    pub uncovered: Vec<String>,
    pub attributed_but_skipped: Vec<String>,
    pub unknown_rule_ids: Vec<String>,
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
        // HTML comments (e.g. `<!-- source-id: ... -->`) are annotations,
        // never normative clauses.
        if line.starts_with("<!--") {
            continue;
        }
        let (id, text) = split_id_prefix(line);
        let keyword = NormativeKeyword::detect(&text);
        clauses.push(RequirementClause { id, keyword, text });
    }
    clauses
}

/// Compute static clause coverage. An explicitly attributed scenario counts as
/// covered when no runtime verdicts are available.
pub fn clause_coverage(doc: &KnowledgeDoc) -> ClauseCoverage {
    clause_coverage_with_verdicts(doc, &BTreeMap::new())
}

/// Compute clause coverage with optional scenario verdicts keyed by scenario
/// name. A clause whose attributed scenarios all resolve to `Skip` remains
/// distinct from a clause with no attribution at all.
pub fn clause_coverage_with_verdicts(
    doc: &KnowledgeDoc,
    verdicts: &BTreeMap<String, Verdict>,
) -> ClauseCoverage {
    let clauses = extract_requirements(doc);
    let known_ids: BTreeSet<String> = clauses
        .iter()
        .filter_map(|clause| clause.id.clone())
        .collect();
    let must_ids: Vec<String> = clauses
        .iter()
        .filter(|clause| is_must_clause(clause))
        .filter_map(|clause| clause.id.clone())
        .collect();
    let (attributions, encountered_rule_ids) = scenario_attributions(doc);

    let mut coverage = ClauseCoverage {
        unknown_rule_ids: encountered_rule_ids
            .difference(&known_ids)
            .cloned()
            .collect(),
        ..ClauseCoverage::default()
    };

    for clause_id in must_ids {
        let scenarios = attributions.get(&clause_id).cloned().unwrap_or_default();
        if scenarios.is_empty() {
            coverage.uncovered.push(clause_id);
            continue;
        }

        let matched_verdicts: Vec<Verdict> = scenarios
            .iter()
            .filter_map(|scenario| verdicts.get(scenario).copied())
            .collect();
        if matched_verdicts.len() == scenarios.len()
            && matched_verdicts
                .iter()
                .all(|verdict| *verdict == Verdict::Skip)
        {
            coverage.attributed_but_skipped.push(clause_id);
        } else {
            coverage.covered.push(clause_id);
        }
    }

    coverage
}

fn is_must_clause(clause: &RequirementClause) -> bool {
    matches!(
        clause.keyword,
        Some(NormativeKeyword::Must | NormativeKeyword::MustNot)
    ) || clause
        .text
        .split(|character: char| !character.is_ascii_alphabetic())
        .any(|word| matches!(word, "MUST" | "SHALL" | "REQUIRED"))
}

fn scenario_attributions(doc: &KnowledgeDoc) -> (BTreeMap<String, Vec<String>>, BTreeSet<String>) {
    let Some(section) = doc.section("Scenarios") else {
        return (BTreeMap::new(), BTreeSet::new());
    };
    let mut current_rule: Option<String> = None;
    let mut attributions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut encountered_rule_ids = BTreeSet::new();

    for raw in section.body.lines() {
        let line = raw.trim().trim_start_matches('#').trim();
        if let Some(rest) = line.strip_prefix("Rule:") {
            let id = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches('`')
                .to_ascii_uppercase();
            if id.is_empty() {
                current_rule = None;
            } else {
                encountered_rule_ids.insert(id.clone());
                attributions.entry(id.clone()).or_default();
                current_rule = Some(id);
            }
            continue;
        }

        let scenario_name = line
            .strip_prefix("Scenario:")
            .or_else(|| line.strip_prefix("场景:"))
            .map(str::trim)
            .filter(|name| !name.is_empty());
        if let (Some(rule_id), Some(name)) = (current_rule.as_ref(), scenario_name) {
            attributions
                .entry(rule_id.clone())
                .or_default()
                .push(name.to_string());
        }
    }

    (attributions, encountered_rule_ids)
}

pub(crate) fn attributed_scenario_names(doc: &KnowledgeDoc) -> Vec<String> {
    let (attributions, _) = scenario_attributions(doc);
    attributions.into_values().flatten().collect()
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
    use crate::spec_core::Verdict;
    use crate::spec_knowledge::parser::parse_requirement_str;
    use std::collections::BTreeMap;
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

    #[test]
    fn test_clause_coverage_attributes_by_rule_id() {
        let doc = parse(
            "---\nkind: requirement\nid: REQ-X\n---\n## Problem\np\n## Requirements\n[REQ-X-ALPHA] The system MUST emit alpha.\n## Scenarios\nRule: REQ-X-ALPHA\nScenario: Alpha\n  Given input\n  When alpha runs\n  Then output is visible\n",
        );
        let coverage = clause_coverage(&doc);
        assert_eq!(coverage.covered, vec!["REQ-X-ALPHA"]);
        assert!(coverage.uncovered.is_empty());
    }

    #[test]
    fn test_clause_coverage_ignores_textual_similarity() {
        let doc = parse(
            "---\nkind: requirement\nid: REQ-X\n---\n## Problem\np\n## Requirements\n[REQ-X-ALPHA] The system MUST emit the alpha result.\n## Scenarios\nScenario: Alpha result\n  Given alpha input\n  When the system emits the alpha result\n  Then the alpha result is visible\n",
        );
        let coverage = clause_coverage(&doc);
        assert_eq!(coverage.uncovered, vec!["REQ-X-ALPHA"]);
        assert!(coverage.covered.is_empty());
    }

    #[test]
    fn test_clause_coverage_excludes_skipped_scenarios() {
        let doc = parse(
            "---\nkind: requirement\nid: REQ-X\n---\n## Problem\np\n## Requirements\n[REQ-X-ALPHA] The system MUST emit alpha.\n## Scenarios\nRule: REQ-X-ALPHA\nScenario: Alpha\n  Given input\n  When alpha runs\n  Then output is visible\n",
        );
        let verdicts = BTreeMap::from([("Alpha".to_string(), Verdict::Skip)]);
        let coverage = clause_coverage_with_verdicts(&doc, &verdicts);
        assert!(coverage.covered.is_empty());
        assert_eq!(coverage.attributed_but_skipped, vec!["REQ-X-ALPHA"]);
    }
}
