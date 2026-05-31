use crate::spec_core::{LintReport, Severity, Verdict, VerificationReport};
use serde::Serialize;

pub mod coverage;
pub use coverage::{CoverageMatrix, build_coverage_matrix, collect_test_function_names};

/// Output format.
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
    Compact,
    Diagnostic,
}

// === Status File Contract ===

/// Machine-readable status report for CI/CD and external agent consumption.
#[derive(Debug, Clone, Serialize)]
pub struct StatusReport {
    pub spec: String,
    pub outcome: String,
    pub gate_blocked: bool,
    pub scenarios: serde_json::Map<String, serde_json::Value>,
    pub context_updates: serde_json::Map<String, serde_json::Value>,
    pub timestamp: u64,
    pub notes: String,
}

/// Build a [`StatusReport`] from a verification report.
///
/// `gate_blocked` should be `true` when a critical (gate) scenario has failed;
/// callers that have not yet implemented Goal Gate logic may pass `false`.
pub fn build_status_report(
    spec_name: &str,
    report: &VerificationReport,
    gate_blocked: bool,
) -> StatusReport {
    // Determine outcome
    let all_pass = report.summary.failed == 0
        && report.summary.skipped == 0
        && report.summary.uncertain == 0
        && report.summary.passed > 0;
    let all_fail = report.summary.passed == 0 && report.summary.total > 0;

    let outcome = if gate_blocked {
        "gate_blocked".to_string()
    } else if all_pass {
        "success".to_string()
    } else if all_fail {
        "fail".to_string()
    } else {
        "partial_success".to_string()
    };

    // Populate scenarios map
    let mut scenarios = serde_json::Map::new();
    for r in &report.results {
        let verdict_str = match r.verdict {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
            Verdict::Skip => "skip",
            Verdict::Uncertain => "uncertain",
            Verdict::PendingReview => "pending_review",
        };
        scenarios.insert(
            r.scenario_name.clone(),
            serde_json::json!({
                "verdict": verdict_str,
                "duration_ms": r.duration_ms,
            }),
        );
    }

    // Populate context_updates
    let mut context_updates = serde_json::Map::new();
    context_updates.insert(
        "tests_passing".to_string(),
        serde_json::Value::Number(report.summary.passed.into()),
    );
    context_updates.insert(
        "tests_failing".to_string(),
        serde_json::Value::Number(report.summary.failed.into()),
    );
    context_updates.insert(
        "tests_skipped".to_string(),
        serde_json::Value::Number(report.summary.skipped.into()),
    );

    // Human-readable notes
    let notes = format!(
        "{}/{} passed, {} failed, {} skipped, {} uncertain, {} pending_review",
        report.summary.passed,
        report.summary.total,
        report.summary.failed,
        report.summary.skipped,
        report.summary.uncertain,
        report.summary.pending_review,
    );

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    StatusReport {
        spec: spec_name.to_string(),
        outcome,
        gate_blocked,
        scenarios,
        context_updates,
        timestamp,
        notes,
    }
}

/// Format a verification report.
pub fn format_verification(report: &VerificationReport, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_verification_text(report),
        OutputFormat::Json => format_json(report),
        OutputFormat::Markdown => format_verification_md(report),
        OutputFormat::Compact => format_verification_compact(report),
        OutputFormat::Diagnostic => format_verification_diagnostic(report),
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
        OutputFormat::Text | OutputFormat::Compact | OutputFormat::Diagnostic => {
            format_explain_text(input, report)
        }
        OutputFormat::Json => format_json(report),
        OutputFormat::Markdown => format_explain_md(input, report),
    }
}

/// Format a lint report.
pub fn format_lint(report: &LintReport, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Text | OutputFormat::Compact | OutputFormat::Diagnostic => {
            format_lint_text(report)
        }
        OutputFormat::Json => format_lint_json(report),
        OutputFormat::Markdown => format_lint_md(report),
    }
}

// === Text formatters ===

fn format_verification_text(report: &VerificationReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Spec: {}\n", report.spec_name));
    out.push_str(&format!(
        "Results: {} total, {} passed, {} failed, {} skipped, {} uncertain, {} pending_review\n\n",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.skipped,
        report.summary.uncertain,
        report.summary.pending_review,
    ));

    for result in &report.results {
        let icon = match result.verdict {
            Verdict::Pass => "[PASS]",
            Verdict::Fail => "[FAIL]",
            Verdict::Skip => "[SKIP]",
            Verdict::Uncertain => "[????]",
            Verdict::PendingReview => "[REVIEW]",
        };
        out.push_str(&format!("  {icon} {}\n", result.scenario_name));

        for step in &result.step_results {
            let step_icon = match step.verdict {
                Verdict::Pass => "+",
                Verdict::Fail => "x",
                Verdict::Skip => "-",
                Verdict::Uncertain => "?",
                Verdict::PendingReview => "R",
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
                    test_name,
                    passed,
                    package,
                    level,
                    test_double,
                    targets,
                    ..
                } => {
                    out.push_str(&format!("    > test '{test_name}': passed={passed}\n"));
                    if let Some(package) = package {
                        out.push_str(&format!("      package={package}\n"));
                    }
                    if let Some(level) = level {
                        out.push_str(&format!("      level={level}\n"));
                    }
                    if let Some(test_double) = test_double {
                        out.push_str(&format!("      test_double={test_double}\n"));
                    }
                    if let Some(targets) = targets {
                        out.push_str(&format!("      targets={targets}\n"));
                    }
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
    out.push_str(&format!("=== Contract Review: {} ===\n\n", input.name));

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
            Verdict::PendingReview => "[REVIEW]",
        };
        out.push_str(&format!("  {icon} {}\n", result.scenario_name));
        for ev in &result.evidence {
            if let crate::spec_core::Evidence::TestOutput {
                test_name,
                package,
                level,
                test_double,
                targets,
                ..
            } = ev
            {
                out.push_str(&format!("    test: {test_name}\n"));
                if let Some(package) = package {
                    out.push_str(&format!("    package: {package}\n"));
                }
                if let Some(level) = level {
                    out.push_str(&format!("    level: {level}\n"));
                }
                if let Some(test_double) = test_double {
                    out.push_str(&format!("    test double: {test_double}\n"));
                }
                if let Some(targets) = targets {
                    out.push_str(&format!("    targets: {targets}\n"));
                }
            }
        }
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
            Verdict::PendingReview => "👁️",
        };
        out.push_str(&format!("- {icon} {}\n", result.scenario_name));
        for ev in &result.evidence {
            if let crate::spec_core::Evidence::TestOutput {
                test_name,
                package,
                level,
                test_double,
                targets,
                ..
            } = ev
            {
                out.push_str(&format!("  - test: `{test_name}`\n"));
                if let Some(package) = package {
                    out.push_str(&format!("    - package: `{package}`\n"));
                }
                if let Some(level) = level {
                    out.push_str(&format!("    - level: `{level}`\n"));
                }
                if let Some(test_double) = test_double {
                    out.push_str(&format!("    - test double: `{test_double}`\n"));
                }
                if let Some(targets) = targets {
                    out.push_str(&format!("    - targets: `{targets}`\n"));
                }
            }
        }
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
            let test_output = r.evidence.iter().find_map(|ev| match ev {
                crate::spec_core::Evidence::TestOutput {
                    test_name,
                    package,
                    level,
                    test_double,
                    targets,
                    ..
                } => Some(serde_json::json!({
                    "test_name": test_name,
                    "package": package,
                    "level": level,
                    "test_double": test_double,
                    "targets": targets,
                })),
                _ => None,
            });
            serde_json::json!({
                "scenario_name": r.scenario_name,
                "verdict": format!("{:?}", r.verdict).to_lowercase(),
                "test_binding": test_output,
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
        OutputFormat::Compact | OutputFormat::Diagnostic => format_cost_text(report),
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

// === Compact / Diagnostic formatters ===

/// Single-line compact summary: `✓ Scenario1  ✗ Scenario2  ⊘ Scenario3 | 2/3 pass`
fn format_verification_compact(report: &VerificationReport) -> String {
    let mut parts: Vec<String> = Vec::new();
    for r in &report.results {
        let icon = match r.verdict {
            Verdict::Pass => "\u{2713}",  // ✓
            Verdict::Fail => "\u{2717}",  // ✗
            Verdict::Skip => "\u{2298}",  // ⊘
            Verdict::Uncertain => "?",
            Verdict::PendingReview => "\u{2299}",  // ⊙
        };
        parts.push(format!("{icon} {}", r.scenario_name));
    }
    let summary = format!(
        "{}/{} pass",
        report.summary.passed, report.summary.total,
    );
    format!("{}  | {}", parts.join("  "), summary)
}

/// Diagnostic format: JSON with full stdout in evidence (wrapper envelope).
fn format_verification_diagnostic(report: &VerificationReport) -> String {
    let json = serde_json::json!({
        "format": "diagnostic",
        "note": "Full evidence including raw test stdout is embedded in each scenario result.",
        "report": serde_json::to_value(report).unwrap_or_default(),
    });
    serde_json::to_string_pretty(&json).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
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
            Verdict::PendingReview => "👁️",
        };
        out.push_str(&format!("### {icon} {}\n\n", result.scenario_name));

        for step in &result.step_results {
            let s = match step.verdict {
                Verdict::Pass => "✅",
                Verdict::Fail => "❌",
                Verdict::Skip => "⏭️",
                Verdict::Uncertain => "❓",
                Verdict::PendingReview => "👁️",
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_core::{
        Evidence, ScenarioResult, StepVerdict, VerificationReport, VerificationSummary,
    };

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
                provenance: None,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
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
                provenance: None,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 0,
                failed: 0,
                skipped: 0,
                uncertain: 1,
                pending_review: 0,
            },
        };

        let text = format_verification(&report, &OutputFormat::Text);
        assert!(text.contains("ai 'stub'"));
        assert!(text.contains("confidence=0.00"));
        assert!(text.contains("ai verifier stub enabled"));
    }

    #[test]
    fn test_format_verification_text_includes_test_binding_metadata() {
        let report = VerificationReport {
            spec_name: "verify-meta".into(),
            results: vec![ScenarioResult {
                scenario_name: "http path".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![Evidence::TestOutput {
                    test_name: "test_http_path".into(),
                    stdout: String::new(),
                    passed: true,
                    package: Some("agent-spec".into()),
                    level: Some("integration".into()),
                    test_double: Some("local_http_stub".into()),
                    targets: Some("commands/update".into()),
                }],
                duration_ms: 5,
                provenance: None,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
            },
        };

        let text = format_verification(&report, &OutputFormat::Text);
        assert!(text.contains("package=agent-spec"));
        assert!(text.contains("level=integration"));
        assert!(text.contains("test_double=local_http_stub"));
        assert!(text.contains("targets=commands/update"));
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
                evidence: vec![Evidence::TestOutput {
                    test_name: "test_happy_path".into(),
                    stdout: String::new(),
                    passed: true,
                    package: Some("agent-spec".into()),
                    level: Some("integration".into()),
                    test_double: Some("fixture_fs".into()),
                    targets: Some("spec_gateway/brief".into()),
                }],
                duration_ms: 5,
                provenance: None,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
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
        assert_eq!(
            parsed["verification"]["results"][0]["test_binding"]["level"],
            "integration"
        );
        assert_eq!(
            parsed["verification"]["results"][0]["test_binding"]["test_double"],
            "fixture_fs"
        );
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

    // === Status File Contract tests ===

    fn make_all_pass_report() -> VerificationReport {
        VerificationReport {
            spec_name: "status-test".into(),
            results: vec![
                ScenarioResult {
                    scenario_name: "scenario A".into(),
                    verdict: Verdict::Pass,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 10,
                    provenance: None,
                },
                ScenarioResult {
                    scenario_name: "scenario B".into(),
                    verdict: Verdict::Pass,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 20,
                    provenance: None,
                },
            ],
            summary: VerificationSummary {
                total: 2,
                passed: 2,
                failed: 0,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
            },
        }
    }

    fn make_mixed_report() -> VerificationReport {
        VerificationReport {
            spec_name: "mixed-test".into(),
            results: vec![
                ScenarioResult {
                    scenario_name: "pass scenario".into(),
                    verdict: Verdict::Pass,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 10,
                    provenance: None,
                },
                ScenarioResult {
                    scenario_name: "fail scenario".into(),
                    verdict: Verdict::Fail,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 20,
                    provenance: None,
                },
                ScenarioResult {
                    scenario_name: "skip scenario".into(),
                    verdict: Verdict::Skip,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 0,
                    provenance: None,
                },
            ],
            summary: VerificationSummary {
                total: 3,
                passed: 1,
                failed: 1,
                skipped: 1,
                uncertain: 0,
                pending_review: 0,
            },
        }
    }

    #[test]
    fn test_status_file_writes_success_on_all_pass() {
        let report = make_all_pass_report();
        let status = build_status_report("status-test", &report, false);

        assert_eq!(status.outcome, "success");
        assert!(!status.gate_blocked);
        assert_eq!(status.context_updates["tests_failing"], 0);
        assert_eq!(status.context_updates["tests_passing"], 2);
        assert_eq!(status.context_updates["tests_skipped"], 0);
        assert!(status.scenarios.contains_key("scenario A"));
        assert!(status.scenarios.contains_key("scenario B"));

        // Ensure it serializes to JSON without error
        let json = serde_json::to_string_pretty(&status).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["outcome"], "success");
    }

    #[test]
    fn test_status_file_writes_partial_success_on_mixed() {
        let report = make_mixed_report();
        let status = build_status_report("mixed-test", &report, false);

        assert_eq!(status.outcome, "partial_success");
        assert!(!status.gate_blocked);
        let passing = status.context_updates["tests_passing"].as_u64().unwrap();
        let failing = status.context_updates["tests_failing"].as_u64().unwrap();
        assert!(passing > 0, "tests_passing should be > 0");
        assert!(failing > 0, "tests_failing should be > 0");
    }

    #[test]
    fn test_status_file_outcome_reflects_gate_blocked() {
        let report = make_mixed_report();
        let status = build_status_report("gate-test", &report, true);

        assert_eq!(status.outcome, "gate_blocked");
        assert!(status.gate_blocked);
    }

    // === Context Fidelity tests ===

    #[test]
    fn test_compact_format_outputs_single_line_summary() {
        let report = make_mixed_report();
        let output = format_verification_compact(&report);

        // Should contain the marker icons
        assert!(output.contains('\u{2713}'), "should contain ✓ for pass");
        assert!(output.contains('\u{2717}'), "should contain ✗ for fail");
        assert!(output.contains('\u{2298}'), "should contain ⊘ for skip");
        // Should contain pass count
        assert!(output.contains("1/3 pass"), "should contain pass count summary");
        // Should be a single line
        let line_count = output.lines().count();
        assert!(
            line_count <= 3,
            "compact output should be at most 3 lines, got {line_count}"
        );
    }

    #[test]
    fn test_diagnostic_format_includes_raw_test_output() {
        let report = VerificationReport {
            spec_name: "diag-test".into(),
            results: vec![ScenarioResult {
                scenario_name: "test with stdout".into(),
                verdict: Verdict::Fail,
                step_results: vec![],
                evidence: vec![Evidence::TestOutput {
                    test_name: "test_something".into(),
                    stdout: "thread 'test_something' panicked at 'assertion failed'".into(),
                    passed: false,
                    package: Some("my-crate".into()),
                    level: None,
                    test_double: None,
                    targets: None,
                }],
                duration_ms: 42,
                provenance: None,
            }],
            summary: VerificationSummary {
                total: 1,
                passed: 0,
                failed: 1,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
            },
        };

        let output = format_verification_diagnostic(&report);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["format"], "diagnostic");
        // Ensure raw stdout is present in evidence
        let results = &parsed["report"]["results"];
        assert!(results.is_array());
        let evidence = &results[0]["evidence"][0];
        assert_eq!(evidence["type"], "test_output");
        let stdout = evidence["stdout"].as_str().unwrap();
        assert!(
            stdout.contains("assertion failed"),
            "diagnostic output should include raw test stdout"
        );
    }
}
