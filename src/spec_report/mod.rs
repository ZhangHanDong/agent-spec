


use crate::spec_core::{LintReport, Severity, Verdict, VerificationReport};

/// Output format.
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
}

/// Format a verification report.
pub fn format_verification(report: &VerificationReport, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_verification_text(report),
        OutputFormat::Json => format_json(report),
        OutputFormat::Markdown => format_verification_md(report),
    }
}

/// Lightweight input for the explain renderer.
///
/// `spec-report` cannot depend on `spec-gateway` (circular dep), so callers
/// build this from their own `TaskContract`.
pub struct ExplainInput {
    pub name: String,
    pub intent: String,
    pub must: Vec<String>,
    pub must_not: Vec<String>,
    pub decisions: Vec<String>,
    pub allowed_changes: Vec<String>,
    pub forbidden: Vec<String>,
    pub out_of_scope: Vec<String>,
}

/// Format an explain (contract review) summary.
pub fn format_explain(
    input: &ExplainInput,
    report: &VerificationReport,
    format: &OutputFormat,
) -> String {
    match format {
        OutputFormat::Text => format_explain_text(input, report),
        OutputFormat::Json => format_json(report),
        OutputFormat::Markdown => format_explain_md(input, report),
    }
}

/// Format a lint report.
pub fn format_lint(report: &LintReport, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_lint_text(report),
        OutputFormat::Json => format_lint_json(report),
        OutputFormat::Markdown => format_lint_md(report),
    }
}

// === Text formatters ===

fn format_verification_text(report: &VerificationReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Spec: {}\n", report.spec_name));
    out.push_str(&format!(
        "Results: {} total, {} passed, {} failed, {} skipped, {} uncertain\n\n",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.skipped,
        report.summary.uncertain,
    ));

    for result in &report.results {
        let icon = match result.verdict {
            Verdict::Pass => "[PASS]",
            Verdict::Fail => "[FAIL]",
            Verdict::Skip => "[SKIP]",
            Verdict::Uncertain => "[????]",
        };
        out.push_str(&format!("  {icon} {}\n", result.scenario_name));

        for step in &result.step_results {
            let step_icon = match step.verdict {
                Verdict::Pass => "+",
                Verdict::Fail => "x",
                Verdict::Skip => "-",
                Verdict::Uncertain => "?",
            };
            out.push_str(&format!("    {step_icon} {}\n", step.step_text));
            if step.verdict == Verdict::Fail {
                out.push_str(&format!("      reason: {}\n", step.reason));
            }
        }

        for ev in &result.evidence {
            match ev {
                crate::spec_core::Evidence::CodeSnippet {
                    file,
                    line,
                    content,
                } => {
                    out.push_str(&format!("    > {file}:{line}: {content}\n"));
                }
                crate::spec_core::Evidence::PatternMatch {
                    pattern,
                    matched,
                    locations,
                } => {
                    out.push_str(&format!(
                        "    > pattern '{pattern}': matched={matched}, locations={}\n",
                        locations.join(", ")
                    ));
                }
                crate::spec_core::Evidence::TestOutput {
                    test_name, passed, ..
                } => {
                    out.push_str(&format!("    > test '{test_name}': passed={passed}\n"));
                }
                crate::spec_core::Evidence::AiAnalysis {
                    model,
                    confidence,
                    reasoning,
                } => {
                    out.push_str(&format!(
                        "    > ai '{model}': confidence={confidence:.2}, reasoning={reasoning}\n"
                    ));
                }
            }
        }
        out.push('\n');
    }

    let rate = report.summary.pass_rate() * 100.0;
    out.push_str(&format!("Pass rate: {rate:.1}%\n"));

    out
}

fn format_lint_text(report: &LintReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Spec: {}\n", report.spec_name));
    out.push_str(&format!(
        "Quality: {:.0}% (determinism: {:.0}%, testability: {:.0}%, coverage: {:.0}%)\n\n",
        report.quality_score.overall * 100.0,
        report.quality_score.determinism * 100.0,
        report.quality_score.testability * 100.0,
        report.quality_score.coverage * 100.0,
    ));

    if report.diagnostics.is_empty() {
        out.push_str("  No issues found.\n");
    } else {
        for diag in &report.diagnostics {
            let icon = match diag.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "WARN ",
                Severity::Info => "INFO ",
            };
            out.push_str(&format!(
                "  [{icon}] line {}: [{}] {}\n",
                diag.span.start_line, diag.rule, diag.message,
            ));
            if let Some(ref suggestion) = diag.suggestion {
                out.push_str(&format!("         suggestion: {suggestion}\n"));
            }
        }
    }

    out
}

// === Explain formatters ===

fn format_explain_text(input: &ExplainInput, report: &VerificationReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "=== Contract Review: {} ===\n\n",
        input.name
    ));

    out.push_str("Intent\n");
    out.push_str(&format!("  {}\n\n", input.intent));

    if !input.decisions.is_empty() {
        out.push_str("Decisions\n");
        for d in &input.decisions {
            out.push_str(&format!("  - {d}\n"));
        }
        out.push('\n');
    }

    out.push_str("Boundaries\n");
    if !input.allowed_changes.is_empty() {
        out.push_str("  Allowed:\n");
        for a in &input.allowed_changes {
            out.push_str(&format!("    - {a}\n"));
        }
    }
    if !input.forbidden.is_empty() {
        out.push_str("  Forbidden:\n");
        for f in &input.forbidden {
            out.push_str(&format!("    - {f}\n"));
        }
    }
    if !input.out_of_scope.is_empty() {
        out.push_str("  Out of Scope:\n");
        for o in &input.out_of_scope {
            out.push_str(&format!("    - {o}\n"));
        }
    }
    out.push('\n');

    out.push_str("Verification Summary\n");
    let rate = report.summary.pass_rate() * 100.0;
    out.push_str(&format!(
        "  {}/{} passed, {} failed, {} skipped, {} uncertain  ({rate:.1}%)\n",
        report.summary.passed,
        report.summary.total,
        report.summary.failed,
        report.summary.skipped,
        report.summary.uncertain,
    ));
    for result in &report.results {
        let icon = match result.verdict {
            Verdict::Pass => "[PASS]",
            Verdict::Fail => "[FAIL]",
            Verdict::Skip => "[SKIP]",
            Verdict::Uncertain => "[????]",
        };
        out.push_str(&format!("  {icon} {}\n", result.scenario_name));
    }

    out
}

fn format_explain_md(input: &ExplainInput, report: &VerificationReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Contract Review: {}\n\n", input.name));

    out.push_str("## Intent\n\n");
    out.push_str(&format!("{}\n\n", input.intent));

    if !input.decisions.is_empty() {
        out.push_str("## Decisions\n\n");
        for d in &input.decisions {
            out.push_str(&format!("- {d}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Boundaries\n\n");
    if !input.allowed_changes.is_empty() {
        out.push_str("**Allowed:**\n");
        for a in &input.allowed_changes {
            out.push_str(&format!("- {a}\n"));
        }
        out.push('\n');
    }
    if !input.forbidden.is_empty() {
        out.push_str("**Forbidden:**\n");
        for f in &input.forbidden {
            out.push_str(&format!("- {f}\n"));
        }
        out.push('\n');
    }
    if !input.out_of_scope.is_empty() {
        out.push_str("**Out of Scope:**\n");
        for o in &input.out_of_scope {
            out.push_str(&format!("- {o}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Verification Summary\n\n");
    out.push_str("| Total | Passed | Failed | Skipped | Uncertain | Pass Rate |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    let rate = report.summary.pass_rate() * 100.0;
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {rate:.1}% |\n\n",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.skipped,
        report.summary.uncertain,
    ));

    for result in &report.results {
        let icon = match result.verdict {
            Verdict::Pass => "✅",
            Verdict::Fail => "❌",
            Verdict::Skip => "⏭️",
            Verdict::Uncertain => "❓",
        };
        out.push_str(&format!("- {icon} {}\n", result.scenario_name));
    }

    out
}

// === Orchestrator JSON (Phase 5) ===

/// Structured JSON output combining contract + verification for external orchestrators.
pub fn format_orchestrator_json(input: &ExplainInput, report: &VerificationReport) -> String {
    let contract = serde_json::json!({
        "name": input.name,
        "intent": input.intent,
        "must": input.must,
        "must_not": input.must_not,
        "decisions": input.decisions,
        "allowed_changes": input.allowed_changes,
        "forbidden": input.forbidden,
        "out_of_scope": input.out_of_scope,
    });

    let verification = serde_json::json!({
        "spec_name": report.spec_name,
        "summary": {
            "total": report.summary.total,
            "passed": report.summary.passed,
            "failed": report.summary.failed,
            "skipped": report.summary.skipped,
            "uncertain": report.summary.uncertain,
            "pass_rate": report.summary.pass_rate(),
        },
        "results": report.results.iter().map(|r| {
            serde_json::json!({
                "scenario_name": r.scenario_name,
                "verdict": format!("{:?}", r.verdict).to_lowercase(),
            })
        }).collect::<Vec<_>>(),
    });

    let output = serde_json::json!({
        "contract": contract,
        "verification": verification,
    });

    serde_json::to_string_pretty(&output).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

// === Cost Report (Phase 6) ===

/// A single entry in a cost breakdown by verification layer.
#[derive(Debug, Clone)]
pub struct CostEntry {
    pub layer: String,
    pub scenarios_hit: usize,
    pub duration_ms: u64,
    pub token_count: u64,
}

/// Cost report breaking down resources by verification layer.
#[derive(Debug, Clone)]
pub struct CostReport {
    pub spec_name: String,
    pub entries: Vec<CostEntry>,
}

/// Format a cost report.
pub fn format_cost_report(report: &CostReport, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_cost_text(report),
        OutputFormat::Json => {
            let json = serde_json::json!({
                "spec_name": report.spec_name,
                "layers": report.entries.iter().map(|e| {
                    serde_json::json!({
                        "layer": e.layer,
                        "scenarios_hit": e.scenarios_hit,
                        "duration_ms": e.duration_ms,
                        "token_count": e.token_count,
                    })
                }).collect::<Vec<_>>(),
                "total_duration_ms": report.entries.iter().map(|e| e.duration_ms).sum::<u64>(),
                "total_tokens": report.entries.iter().map(|e| e.token_count).sum::<u64>(),
            });
            serde_json::to_string_pretty(&json).unwrap_or_default()
        }
        OutputFormat::Markdown => format_cost_md(report),
    }
}

fn format_cost_text(report: &CostReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Cost Report: {}\n\n", report.spec_name));
    for entry in &report.entries {
        out.push_str(&format!(
            "  [{}] scenarios={}, duration={}ms, tokens={}\n",
            entry.layer, entry.scenarios_hit, entry.duration_ms, entry.token_count,
        ));
    }
    let total_time: u64 = report.entries.iter().map(|e| e.duration_ms).sum();
    let total_tokens: u64 = report.entries.iter().map(|e| e.token_count).sum();
    out.push_str(&format!(
        "\n  Total: duration={}ms, tokens={}\n",
        total_time, total_tokens,
    ));
    out
}

fn format_cost_md(report: &CostReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Cost Report: {}\n\n", report.spec_name));
    out.push_str("| Layer | Scenarios | Duration (ms) | Tokens |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for entry in &report.entries {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            entry.layer, entry.scenarios_hit, entry.duration_ms, entry.token_count,
        ));
    }
    let total_time: u64 = report.entries.iter().map(|e| e.duration_ms).sum();
    let total_tokens: u64 = report.entries.iter().map(|e| e.token_count).sum();
    out.push_str(&format!(
        "| **Total** | | **{}** | **{}** |\n",
        total_time, total_tokens,
    ));
    out
}

// === JSON formatters ===

fn format_json<T: serde::Serialize>(report: &T) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

fn format_lint_json(report: &LintReport) -> String {
    format_json(report)
}

// === Markdown formatters ===

fn format_verification_md(report: &VerificationReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Verification: {}\n\n", report.spec_name));
    out.push_str("| Total | Passed | Failed | Skipped | Uncertain | Pass Rate |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {:.1}% |\n\n",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.skipped,
        report.summary.uncertain,
        report.summary.pass_rate() * 100.0,
    ));

    out.push_str("## Scenarios\n\n");
    for result in &report.results {
        let icon = match result.verdict {
            Verdict::Pass => "✅",
            Verdict::Fail => "❌",
            Verdict::Skip => "⏭️",
            Verdict::Uncertain => "❓",
        };
        out.push_str(&format!("### {icon} {}\n\n", result.scenario_name));

        for step in &result.step_results {
            let s = match step.verdict {
                Verdict::Pass => "✅",
                Verdict::Fail => "❌",
                Verdict::Skip => "⏭️",
                Verdict::Uncertain => "❓",
            };
            out.push_str(&format!("- {s} {}\n", step.step_text));
        }
        out.push('\n');
    }

    out
}

fn format_lint_md(report: &LintReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Lint: {}\n\n", report.spec_name));
    out.push_str(&format!(
        "**Quality Score: {:.0}%** (determinism: {:.0}%, testability: {:.0}%, coverage: {:.0}%)\n\n",
        report.quality_score.overall * 100.0,
        report.quality_score.determinism * 100.0,
        report.quality_score.testability * 100.0,
        report.quality_score.coverage * 100.0,
    ));

    if report.diagnostics.is_empty() {
        out.push_str("No issues found.\n");
    } else {
        out.push_str("| Severity | Rule | Line | Message |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for diag in &report.diagnostics {
            let sev = match diag.severity {
                Severity::Error => "🔴 Error",
                Severity::Warning => "🟡 Warning",
                Severity::Info => "🔵 Info",
            };
            out.push_str(&format!(
                "| {sev} | {} | {} | {} |\n",
                diag.rule, diag.span.start_line, diag.message,
            ));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_core::{Evidence, ScenarioResult, StepVerdict, VerificationReport, VerificationSummary};

    #[test]
    fn test_format_verification_text() {
        let report = VerificationReport {
            spec_name: "test".into(),
            results: vec![ScenarioResult {
                scenario_name: "test scenario".into(),
                verdict: Verdict::Pass,
                step_results: vec![StepVerdict {
                    step_text: "user exists".into(),
                    verdict: Verdict::Pass,
                    reason: "ok".into(),
                }],
                evidence: vec![],
                duration_ms: 10,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
            },
        };
        let text = format_verification(&report, &OutputFormat::Text);
        assert!(text.contains("[PASS]"));
        assert!(text.contains("100.0%"));
    }

    #[test]
    fn test_format_verification_text_includes_ai_analysis_evidence() {
        let report = VerificationReport {
            spec_name: "ai".into(),
            results: vec![ScenarioResult {
                scenario_name: "needs ai".into(),
                verdict: Verdict::Uncertain,
                step_results: vec![StepVerdict {
                    step_text: "review code intent".into(),
                    verdict: Verdict::Uncertain,
                    reason: "manual review required".into(),
                }],
                evidence: vec![Evidence::AiAnalysis {
                    model: "stub".into(),
                    confidence: 0.0,
                    reasoning: "ai verifier stub enabled".into(),
                }],
                duration_ms: 0,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 0,
                failed: 0,
                skipped: 0,
                uncertain: 1,
            },
        };

        let text = format_verification(&report, &OutputFormat::Text);
        assert!(text.contains("ai 'stub'"));
        assert!(text.contains("confidence=0.00"));
        assert!(text.contains("ai verifier stub enabled"));
    }

    #[test]
    fn test_report_json_exposes_contract_and_verification_summary_for_orchestrators() {
        let input = ExplainInput {
            name: "Orchestrator Test".into(),
            intent: "Validate JSON output for orchestrators".into(),
            must: vec!["Return structured data".into()],
            must_not: vec![],
            decisions: vec!["Use JSON".into()],
            allowed_changes: vec!["crates/**".into()],
            forbidden: vec![],
            out_of_scope: vec![],
        };
        let report = VerificationReport {
            spec_name: "orch".into(),
            results: vec![ScenarioResult {
                scenario_name: "happy path".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 5,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
            },
        };

        let json = format_orchestrator_json(&input, &report);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Contract section present
        assert!(parsed["contract"]["name"].is_string());
        assert_eq!(parsed["contract"]["name"], "Orchestrator Test");
        assert!(parsed["contract"]["intent"].is_string());
        assert!(parsed["contract"]["must"].is_array());
        assert!(parsed["contract"]["decisions"].is_array());

        // Verification summary present
        assert!(parsed["verification"]["summary"]["total"].is_number());
        assert_eq!(parsed["verification"]["summary"]["passed"], 1);
        assert!(parsed["verification"]["summary"]["pass_rate"].is_number());
        assert!(parsed["verification"]["results"].is_array());
    }

    #[test]
    fn test_cost_report_breaks_down_tokens_time_and_layers() {
        let report = CostReport {
            spec_name: "cost test".into(),
            entries: vec![
                CostEntry {
                    layer: "test".into(),
                    scenarios_hit: 3,
                    duration_ms: 150,
                    token_count: 0,
                },
                CostEntry {
                    layer: "ai".into(),
                    scenarios_hit: 2,
                    duration_ms: 500,
                    token_count: 1200,
                },
            ],
        };

        let text = format_cost_report(&report, &OutputFormat::Text);
        assert!(text.contains("[test]"), "should show test layer");
        assert!(text.contains("[ai]"), "should show ai layer");
        assert!(text.contains("duration="), "should show duration");
        assert!(text.contains("tokens="), "should show tokens");
        assert!(text.contains("Total:"), "should show total");

        let json = format_cost_report(&report, &OutputFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["layers"].is_array());
        assert_eq!(parsed["layers"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["total_duration_ms"], 650);
        assert_eq!(parsed["total_tokens"], 1200);
    }
}
