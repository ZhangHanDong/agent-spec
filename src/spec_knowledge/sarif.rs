//! SARIF 2.1.0 writer (§13 P3) — net-new. Renders knowledge-lint diagnostics
//! into a SARIF log for GitHub Code Scanning. No existing writer to reuse.

use crate::spec_core::{LintDiagnostic, Severity};
use serde_json::{Value, json};

/// A diagnostic paired with the artifact URI it applies to (empty = no location,
/// e.g. corpus-level findings).
#[derive(Debug, Clone)]
pub struct Finding {
    pub uri: String,
    pub diag: LintDiagnostic,
}

fn sarif_level(sev: Severity) -> &'static str {
    match sev {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

/// Render findings into a SARIF 2.1.0 log value.
pub fn render_sarif(findings: &[Finding]) -> Value {
    let results: Vec<Value> = findings.iter().map(result).collect();
    json!({
        "version": "2.1.0",
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "runs": [ {
            "tool": { "driver": {
                "name": "agent-spec",
                "informationUri": env!("CARGO_PKG_REPOSITORY"),
                "version": env!("CARGO_PKG_VERSION"),
                "rules": rules(findings),
            } },
            "results": results,
        } ]
    })
}

fn result(f: &Finding) -> Value {
    let mut r = json!({
        "ruleId": f.diag.rule,
        "level": sarif_level(f.diag.severity),
        "message": { "text": f.diag.message },
    });
    if !f.uri.is_empty() {
        r["locations"] = json!([ {
            "physicalLocation": {
                "artifactLocation": { "uri": f.uri },
            }
        } ]);
    }
    r
}

/// Unique rule descriptors referenced by the findings (SARIF `tool.driver.rules`).
fn rules(findings: &[Finding]) -> Value {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for f in findings {
        if seen.insert(f.diag.rule.clone()) {
            out.push(json!({ "id": f.diag.rule }));
        }
    }
    Value::Array(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_core::Span;

    fn diag(rule: &str, sev: Severity) -> LintDiagnostic {
        LintDiagnostic {
            rule: rule.into(),
            severity: sev,
            message: "msg".into(),
            span: Span::default(),
            suggestion: None,
        }
    }

    #[test]
    fn test_render_sarif_shape() {
        let findings = vec![
            Finding {
                uri: "knowledge/decisions/adr-001.md".into(),
                diag: diag("decision-required-section", Severity::Error),
            },
            Finding {
                uri: String::new(),
                diag: diag("knowledge-id-conflict", Severity::Error),
            },
        ];
        let log = render_sarif(&findings);
        assert_eq!(log["version"], "2.1.0");
        let results = log["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["level"], "error");
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "knowledge/decisions/adr-001.md"
        );
        // corpus-level finding has no locations
        assert!(results[1].get("locations").is_none());
        // rules deduped
        assert_eq!(
            log["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn test_render_sarif_uses_package_repository_url() {
        let log = render_sarif(&[]);
        assert_eq!(
            log["runs"][0]["tool"]["driver"]["informationUri"],
            "https://github.com/ZhangHanDong/agent-spec"
        );
    }
}
