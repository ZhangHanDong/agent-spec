//! Coverage matrix (Phase 2): Rule × Scenario × Test × Verdict × Provenance.
//!
//! Mechanically assembled from a [`ResolvedSpec`], an optional
//! [`VerificationReport`], and a test-function-name index. Never calls an LLM;
//! the matrix is observability, not a gate.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::spec_core::{EvidenceProvenance, Evidence, ResolvedSpec, Verdict, VerificationReport};

/// Whether a scenario's `Test:` selector resolves to a real test function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TestFound {
    /// The selector exactly matches a collected test-function name.
    Found,
    /// The selector is present but matches no test function (dangling).
    Missing,
    /// The scenario declares no `Test:` selector.
    None,
}

/// One coverage-matrix row, one per scenario.
#[derive(Debug, Clone, Serialize)]
pub struct CoverageRow {
    /// Owning behavior-rule id, or `None` for ungrouped scenarios.
    pub rule: Option<String>,
    pub scenario: String,
    /// The `Test:` selector filter, or `None`.
    pub test_selector: Option<String>,
    pub test_found: TestFound,
    /// Verdict from the verification report, or `None` if no report was given.
    pub verdict: Option<Verdict>,
    /// Whether the verdict is mechanical or inferential, or `None`.
    pub provenance: Option<EvidenceProvenance>,
}

/// The full coverage matrix.
#[derive(Debug, Clone, Serialize)]
pub struct CoverageMatrix {
    pub rows: Vec<CoverageRow>,
}

/// Mechanically assemble the coverage matrix. Pure: depends only on its
/// arguments, never mutates the report, never calls an LLM.
pub fn build_coverage_matrix(
    resolved: &ResolvedSpec,
    report: Option<&VerificationReport>,
    test_index: &HashSet<String>,
) -> CoverageMatrix {
    let rows = resolved
        .all_scenarios
        .iter()
        .map(|scenario| {
            let test_selector = scenario.test_selector.as_ref().map(|s| s.filter.clone());
            let test_found = match &test_selector {
                None => TestFound::None,
                Some(filter) if test_index.contains(filter) => TestFound::Found,
                Some(_) => TestFound::Missing,
            };

            let result = report.and_then(|r| {
                r.results
                    .iter()
                    .find(|res| res.scenario_name == scenario.name)
            });
            let verdict = result.map(|r| r.verdict);
            let provenance = result.and_then(provenance_of);

            CoverageRow {
                rule: scenario.rule.clone(),
                scenario: scenario.name.clone(),
                test_selector,
                test_found,
                verdict,
                provenance,
            }
        })
        .collect();

    CoverageMatrix { rows }
}

impl CoverageMatrix {
    /// Render as a markdown table (Rule | Scenario | Test | Found | Verdict | Provenance).
    pub fn to_markdown(&self) -> String {
        let mut out = String::from(
            "| Rule | Scenario | Test | Found | Verdict | Provenance |\n\
             |------|----------|------|-------|---------|------------|\n",
        );
        for r in &self.rows {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                dash(r.rule.as_deref()),
                r.scenario,
                dash(r.test_selector.as_deref()),
                test_found_str(r.test_found),
                r.verdict.map(verdict_str).unwrap_or("—"),
                r.provenance.map(prov_str).unwrap_or("—"),
            ));
        }
        out
    }

    /// Render as a plain-text table-ish list.
    pub fn to_text(&self) -> String {
        let mut out = String::from("Coverage Matrix (Rule × Scenario × Test × Verdict)\n");
        for r in &self.rows {
            out.push_str(&format!(
                "- [{}] {} → {} ({}) :: {} / {}\n",
                dash(r.rule.as_deref()),
                r.scenario,
                dash(r.test_selector.as_deref()),
                test_found_str(r.test_found),
                r.verdict.map(verdict_str).unwrap_or("—"),
                r.provenance.map(prov_str).unwrap_or("—"),
            ));
        }
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

fn dash(s: Option<&str>) -> &str {
    s.unwrap_or("—")
}

fn test_found_str(f: TestFound) -> &'static str {
    match f {
        TestFound::Found => "found",
        TestFound::Missing => "missing",
        TestFound::None => "none",
    }
}

fn verdict_str(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
        Verdict::Skip => "skip",
        Verdict::Uncertain => "uncertain",
        Verdict::PendingReview => "pending_review",
    }
}

fn prov_str(p: EvidenceProvenance) -> &'static str {
    match p {
        EvidenceProvenance::Computational => "computational",
        EvidenceProvenance::Inferential => "inferential",
    }
}

/// Mechanically collect every Rust test-function name (`#[test]` /
/// `#[tokio::test]`) under the given code paths. This is a NAME-existence
/// index, intentionally distinct from cargo's run-time filter semantics.
pub fn collect_test_function_names(code_paths: &[PathBuf]) -> HashSet<String> {
    let mut files = Vec::new();
    for p in code_paths {
        collect_rust_files(p, &mut files);
    }
    let mut names = HashSet::new();
    for f in &files {
        if let Ok(src) = std::fs::read_to_string(f) {
            collect_from_source(&src, &mut names);
        }
    }
    names
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if dir.is_file() {
        if dir.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(dir.to_path_buf());
        }
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
            collect_rust_files(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn collect_from_source(src: &str, names: &mut HashSet<String>) {
    let mut saw_test_attr = false;
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with("#[test]") || t.contains("tokio::test") {
            saw_test_attr = true;
            continue;
        }
        if t.starts_with("#[") || t.starts_with("//") || t.is_empty() {
            // other attributes / comments may sit between the attr and the fn
            continue;
        }
        if saw_test_attr {
            if let Some(name) = fn_name(t) {
                names.insert(name);
            }
            saw_test_attr = false;
        }
    }
}

/// Extract `name` from a line like `fn name(...)` / `async fn name(...)` / `pub fn name`.
fn fn_name(line: &str) -> Option<String> {
    let after_fn = line.split_once("fn ")?.1;
    let name: String = after_fn
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

/// Provenance of a result: its stamped value, falling back to `Inferential`
/// when it carries AI-analysis evidence but was never stamped (defensive — e.g.
/// reports produced by a path that forgot to stamp).
fn provenance_of(result: &crate::spec_core::ScenarioResult) -> Option<EvidenceProvenance> {
    if let Some(p) = result.provenance {
        return Some(p);
    }
    if result
        .evidence
        .iter()
        .any(|e| matches!(e, Evidence::AiAnalysis { .. }))
    {
        return Some(EvidenceProvenance::Inferential);
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_parser::parse_spec_from_str;
    use crate::spec_core::{
        ResolvedSpec, ScenarioResult, StepVerdict, VerificationReport,
    };

    fn resolved_of(input: &str) -> ResolvedSpec {
        let doc = parse_spec_from_str(input).unwrap();
        crate::spec_parser::resolve_spec(doc, &[]).unwrap()
    }

    fn idx(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    const TWO_RULE_SCENARIOS: &str = r#"spec: task
name: "x"
---

## 完成条件

### Rule: refund-idempotent — 退款幂等
场景: 首次退款
  测试: test_first_refund
  当 退款
  那么 成功
场景: 重复退款
  测试: test_dup_refund
  当 再次退款
  那么 不重复
"#;

    #[test]
    fn test_matrix_has_one_row_per_scenario() {
        let resolved = resolved_of(TWO_RULE_SCENARIOS);
        let index = idx(&["test_first_refund", "test_dup_refund"]);
        let report = VerificationReport::from_results(
            "x".into(),
            vec![
                pass_result("首次退款"),
                pass_result("重复退款"),
            ],
        );
        let m = build_coverage_matrix(&resolved, Some(&report), &index);
        assert_eq!(m.rows.len(), 2);
        assert_eq!(m.rows[0].rule.as_deref(), Some("refund-idempotent"));
        assert_eq!(m.rows[0].test_selector.as_deref(), Some("test_first_refund"));
        assert_eq!(m.rows[0].test_found, TestFound::Found);
        assert_eq!(m.rows[0].verdict, Some(Verdict::Pass));
    }

    #[test]
    fn test_matrix_flags_dangling_selector_as_missing() {
        let input = r#"spec: task
name: "x"
---

## 完成条件

场景: 悬挂
  测试: test_does_not_exist_anywhere
  当 a
  那么 b
"#;
        let resolved = resolved_of(input);
        let m = build_coverage_matrix(&resolved, None, &idx(&["test_other"]));
        assert_eq!(m.rows[0].test_found, TestFound::Missing);
    }

    #[test]
    fn test_matrix_test_found_requires_exact_function_name() {
        let input = r#"spec: task
name: "x"
---

## 完成条件

场景: 子串
  测试: register
  当 a
  那么 b
场景: 精确
  测试: test_register_returns_201
  当 a
  那么 b
"#;
        let resolved = resolved_of(input);
        // Index has the full function name; "register" is only a substring.
        let m = build_coverage_matrix(&resolved, None, &idx(&["test_register_returns_201"]));
        let substr = m.rows.iter().find(|r| r.scenario == "子串").unwrap();
        let exact = m.rows.iter().find(|r| r.scenario == "精确").unwrap();
        assert_eq!(substr.test_found, TestFound::Missing);
        assert_eq!(exact.test_found, TestFound::Found);
    }

    #[test]
    fn test_matrix_marks_scenario_without_selector_as_none() {
        let input = r#"spec: task
name: "x"
---

## 完成条件

场景: 无绑定
  当 a
  那么 b
"#;
        let resolved = resolved_of(input);
        let report = VerificationReport::from_results(
            "x".into(),
            vec![skip_result("无绑定")],
        );
        let m = build_coverage_matrix(&resolved, Some(&report), &HashSet::new());
        assert_eq!(m.rows[0].test_selector, None);
        assert_eq!(m.rows[0].test_found, TestFound::None);
        assert_eq!(m.rows[0].verdict, Some(Verdict::Skip));
    }

    #[test]
    fn test_matrix_ungrouped_scenario_rule_column_is_dash() {
        let input = r#"spec: task
name: "x"
---

## 完成条件

场景: 未分组
  测试: test_x
  当 a
  那么 b
"#;
        let resolved = resolved_of(input);
        let m = build_coverage_matrix(&resolved, None, &idx(&["test_x"]));
        assert_eq!(m.rows[0].rule, None);
    }

    #[test]
    fn test_matrix_derives_inferential_from_ai_evidence() {
        use crate::spec_core::Evidence;
        let input = r#"spec: task
name: "x"
---

## 完成条件

场景: AI 场景
  当 a
  那么 b
"#;
        let resolved = resolved_of(input);
        // Result has AiAnalysis evidence but provenance was never stamped.
        let result = ScenarioResult {
            scenario_name: "AI 场景".into(),
            verdict: Verdict::Uncertain,
            step_results: vec![],
            evidence: vec![Evidence::AiAnalysis {
                model: "m".into(),
                confidence: 0.5,
                reasoning: "r".into(),
            }],
            duration_ms: 0,
            provenance: None,
        };
        let report = VerificationReport::from_results("x".into(), vec![result]);
        let m = build_coverage_matrix(&resolved, Some(&report), &HashSet::new());
        assert_eq!(
            m.rows[0].provenance,
            Some(EvidenceProvenance::Inferential),
            "AiAnalysis evidence must derive inferential when unstamped"
        );
    }

    fn pass_result(name: &str) -> ScenarioResult {
        ScenarioResult {
            scenario_name: name.into(),
            verdict: Verdict::Pass,
            step_results: vec![StepVerdict {
                step_text: "s".into(),
                verdict: Verdict::Pass,
                reason: "ok".into(),
            }],
            evidence: vec![],
            duration_ms: 0,
            provenance: Some(EvidenceProvenance::Computational),
        }
    }

    fn skip_result(name: &str) -> ScenarioResult {
        ScenarioResult {
            scenario_name: name.into(),
            verdict: Verdict::Skip,
            step_results: vec![],
            evidence: vec![],
            duration_ms: 0,
            provenance: None,
        }
    }

    #[test]
    fn test_matrix_markdown_renders_table() {
        let resolved = resolved_of(TWO_RULE_SCENARIOS);
        let m = build_coverage_matrix(&resolved, None, &idx(&["test_first_refund"]));
        let md = m.to_markdown();
        assert!(md.contains("| Rule | Scenario | Test | Found | Verdict | Provenance |"));
        assert!(md.contains("refund-idempotent"));
        assert!(md.contains("首次退款"));
    }

    #[test]
    fn test_matrix_json_is_machine_parseable() {
        let resolved = resolved_of(TWO_RULE_SCENARIOS);
        let report = VerificationReport::from_results("x".into(), vec![pass_result("首次退款")]);
        let m = build_coverage_matrix(&resolved, Some(&report), &idx(&["test_first_refund"]));
        let json = m.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let rows = parsed.get("rows").and_then(|r| r.as_array()).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].get("scenario").is_some());
        assert!(rows[0].get("test_found").is_some());
        assert!(rows[0].get("verdict").is_some());
    }
}
