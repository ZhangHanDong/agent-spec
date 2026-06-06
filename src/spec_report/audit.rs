//! Spec-library health audit (Phase 8): mechanical aggregation across a whole
//! spec library. Observability only — never gates.

use serde::Serialize;

use crate::spec_core::{Section, SpecDocument};

/// Aggregated health metrics for a spec library.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AuditReport {
    pub spec_count: usize,
    pub rule_count: usize,
    pub scenario_count: usize,
    /// Rules declared with no proving Example (scenario_names empty).
    pub unproven_rules: usize,
    /// Scenarios not grouped under any Rule.
    pub ungrouped_scenarios: usize,
    /// Unresolved Discovery questions.
    pub open_questions: usize,
    /// `Rule:` lines whose id was not valid kebab-case.
    pub malformed_rules: usize,
}

fn question_is_resolved(item: &str) -> bool {
    let t = item.trim();
    t.starts_with("[x]")
        || t.starts_with("[X]")
        || t.starts_with("[已解决]")
        || t.contains("RESOLVED")
        || t.contains("已解决")
}

/// Mechanically aggregate library-level health metrics.
pub fn audit_specs(docs: &[SpecDocument]) -> AuditReport {
    let mut r = AuditReport {
        spec_count: docs.len(),
        ..Default::default()
    };
    for doc in docs {
        for section in &doc.sections {
            match section {
                Section::AcceptanceCriteria {
                    scenarios,
                    rules,
                    malformed_rules,
                    ..
                } => {
                    r.scenario_count += scenarios.len();
                    r.rule_count += rules.len();
                    r.unproven_rules +=
                        rules.iter().filter(|x| x.scenario_names.is_empty()).count();
                    r.ungrouped_scenarios += scenarios.iter().filter(|s| s.rule.is_none()).count();
                    r.malformed_rules += malformed_rules.len();
                }
                Section::Questions { items, .. } => {
                    r.open_questions += items.iter().filter(|i| !question_is_resolved(i)).count();
                }
                _ => {}
            }
        }
    }
    r
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_parser::parse_spec_from_str;

    fn doc(s: &str) -> SpecDocument {
        parse_spec_from_str(s).unwrap()
    }

    #[test]
    fn test_audit_counts_specs_rules_scenarios() {
        let a = doc(
            "spec: task\nname: \"a\"\n---\n\n## 完成条件\n\n### Rule: r-one — 一\n场景: s1\n  测试: t1\n  当 a\n  那么 b\n场景: s2\n  测试: t2\n  当 a\n  那么 b\n",
        );
        let b = doc(
            "spec: task\nname: \"b\"\n---\n\n## 完成条件\n\n### Rule: r-two — 二\n场景: s3\n  测试: t3\n  当 a\n  那么 b\n",
        );
        let rep = audit_specs(&[a, b]);
        assert_eq!(rep.spec_count, 2);
        assert_eq!(rep.rule_count, 2);
        assert_eq!(rep.scenario_count, 3);
    }

    #[test]
    fn test_audit_counts_unproven_rules() {
        let a = doc(
            "spec: capability\nname: \"cap\"\n---\n\n## 完成条件\n\n### Rule: empty-one\n### Rule: proven\n场景: s\n  测试: t\n  当 a\n  那么 b\n",
        );
        // In a capability spec, both rules take Capability scope; empty-one has no scenarios.
        assert_eq!(audit_specs(&[a]).unproven_rules, 1);
    }

    #[test]
    fn test_audit_counts_ungrouped_scenarios() {
        let a = doc(
            "spec: task\nname: \"a\"\n---\n\n## 完成条件\n\n场景: s1\n  测试: t1\n  当 a\n  那么 b\n场景: s2\n  测试: t2\n  当 a\n  那么 b\n",
        );
        assert_eq!(audit_specs(&[a]).ungrouped_scenarios, 2);
    }

    #[test]
    fn test_audit_counts_open_questions() {
        let a = doc("spec: task\nname: \"a\"\n---\n\n## Questions\n\n- 未决\n- [x] 已决\n");
        assert_eq!(audit_specs(&[a]).open_questions, 1);
    }

    #[test]
    fn test_audit_counts_malformed_rules() {
        let a = doc(
            "spec: task\nname: \"a\"\n---\n\n## 完成条件\n\n规则: NOT kebab\n场景: s\n  测试: t\n  当 a\n  那么 b\n",
        );
        assert_eq!(audit_specs(&[a]).malformed_rules, 1);
    }

    #[test]
    fn test_audit_empty_library() {
        let rep = audit_specs(&[]);
        assert_eq!(rep.spec_count, 0);
        assert_eq!(rep.rule_count, 0);
        assert_eq!(rep.scenario_count, 0);
    }

    #[test]
    fn test_audit_json_serializes() {
        let a = doc(
            "spec: task\nname: \"a\"\n---\n\n## 完成条件\n\n场景: s\n  测试: t\n  当 a\n  那么 b\n",
        );
        let json = serde_json::to_string(&audit_specs(&[a])).unwrap();
        assert!(json.contains("spec_count"));
        assert!(json.contains("unproven_rules"));
    }
}
