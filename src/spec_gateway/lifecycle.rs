use std::sync::Arc;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::spec_core::{LintReport, SpecResult, Verdict, VerificationReport};
use crate::spec_lint::LintPipeline;
use crate::spec_report::OutputFormat;
use crate::spec_verify::{
    AiBackend, AiMode, AiVerifier, BoundariesVerifier, ComplexityVerifier, StructuralVerifier,
    TestVerifier, VerificationContext, Verifier, run_verification,
};

use super::TaskContract;

/// The main entry point for agent-spec lifecycle integration.
pub struct SpecGateway {
    doc: crate::spec_core::SpecDocument,
    resolved: crate::spec_core::ResolvedSpec,
}

impl SpecGateway {
    /// Load a spec file from disk.
    pub fn load(spec_path: impl AsRef<Path>) -> SpecResult<Self> {
        let doc = crate::spec_parser::parse_spec(spec_path.as_ref())?;
        let resolved = crate::spec_parser::resolve_spec(doc.clone(), &[])?;
        Ok(Self { doc, resolved })
    }

    /// Load a spec from a string (for tests or inline specs).
    pub fn from_input(input: &str) -> SpecResult<Self> {
        let doc = crate::spec_parser::parse_spec_from_str(input)?;
        let resolved = crate::spec_parser::resolve_spec(doc.clone(), &[])?;
        Ok(Self { doc, resolved })
    }

    /// Access the resolved spec (for caller-mode AI request building).
    pub fn resolved(&self) -> &crate::spec_core::ResolvedSpec {
        &self.resolved
    }

    // ── Stage 1: PLAN ───────────────────────────────────────────

    pub fn plan(&self) -> TaskContract {
        TaskContract::from_resolved(&self.resolved)
    }

    pub fn contract(&self) -> TaskContract {
        self.plan()
    }

    #[allow(deprecated)]
    #[deprecated(note = "Use SpecGateway::plan()/contract() instead")]
    pub fn brief(&self) -> super::SpecBrief {
        super::SpecBrief::from_contract(&self.plan())
    }

    pub fn ast_json(&self) -> String {
        serde_json::to_string_pretty(&self.doc).unwrap_or_default()
    }

    // ── Stage 2: GATE ───────────────────────────────────────────

    pub fn lint(&self) -> LintReport {
        let pipeline = LintPipeline::with_defaults();
        pipeline.run(&self.doc)
    }

    pub fn quality_gate(&self, min_score: f64) -> Result<LintReport, GateFailure> {
        let report = self.lint();
        if report.has_errors() || report.quality_score.overall < min_score {
            Err(GateFailure {
                actual_score: report.quality_score.overall,
                required_score: min_score,
                report,
            })
        } else {
            Ok(report)
        }
    }

    // ── Stage 3: VERIFY ─────────────────────────────────────────

    pub fn verify(&self, code_path: impl AsRef<Path>) -> SpecResult<VerificationReport> {
        self.verify_with_ai_mode(code_path, AiMode::Off)
    }

    pub fn verify_with_changes(
        &self,
        code_path: impl AsRef<Path>,
        change_paths: &[PathBuf],
    ) -> SpecResult<VerificationReport> {
        self.verify_with_changes_and_ai_mode(code_path, change_paths, AiMode::Off)
    }

    pub fn verify_with_ai_mode(
        &self,
        code_path: impl AsRef<Path>,
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.verify_with_changes_and_ai_mode(code_path, &[], ai_mode)
    }

    pub fn verify_with_ai_backend(
        &self,
        code_path: impl AsRef<Path>,
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.verify_with_changes_and_ai_backend(code_path, &[], backend)
    }

    pub fn verify_with_changes_and_ai_mode(
        &self,
        code_path: impl AsRef<Path>,
        change_paths: &[PathBuf],
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            vec![code_path.as_ref().to_path_buf()],
            change_paths.to_vec(),
            ai_mode,
            AiVerifier::from_mode(ai_mode),
        )
    }

    pub fn verify_with_changes_and_ai_backend(
        &self,
        code_path: impl AsRef<Path>,
        change_paths: &[PathBuf],
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            vec![code_path.as_ref().to_path_buf()],
            change_paths.to_vec(),
            AiMode::External,
            AiVerifier::with_backend(backend),
        )
    }

    pub fn verify_paths(&self, code_paths: &[PathBuf]) -> SpecResult<VerificationReport> {
        self.verify_paths_with_ai_mode(code_paths, AiMode::Off)
    }

    pub fn verify_paths_with_changes(
        &self,
        code_paths: &[PathBuf],
        change_paths: &[PathBuf],
    ) -> SpecResult<VerificationReport> {
        self.verify_paths_with_changes_and_ai_mode(code_paths, change_paths, AiMode::Off)
    }

    pub fn verify_paths_with_ai_mode(
        &self,
        code_paths: &[PathBuf],
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.verify_paths_with_changes_and_ai_mode(code_paths, &[], ai_mode)
    }

    pub fn verify_paths_with_ai_backend(
        &self,
        code_paths: &[PathBuf],
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.verify_paths_with_changes_and_ai_backend(code_paths, &[], backend)
    }

    pub fn verify_paths_with_changes_and_ai_mode(
        &self,
        code_paths: &[PathBuf],
        change_paths: &[PathBuf],
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            code_paths.to_vec(),
            change_paths.to_vec(),
            ai_mode,
            AiVerifier::from_mode(ai_mode),
        )
    }

    pub fn verify_paths_with_changes_and_ai_backend(
        &self,
        code_paths: &[PathBuf],
        change_paths: &[PathBuf],
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            code_paths.to_vec(),
            change_paths.to_vec(),
            AiMode::External,
            AiVerifier::with_backend(backend),
        )
    }

    fn run_verify(
        &self,
        code_paths: Vec<PathBuf>,
        change_paths: Vec<PathBuf>,
        ai_mode: AiMode,
        ai: AiVerifier,
    ) -> SpecResult<VerificationReport> {
        let ctx = VerificationContext {
            code_paths,
            change_paths,
            ai_mode,
            resolved_spec: self.resolved.clone(),
        };

        let structural = StructuralVerifier;
        let boundaries = BoundariesVerifier;
        let test = TestVerifier;
        let complexity = ComplexityVerifier;
        let verifiers: Vec<&dyn Verifier> =
            vec![&structural, &boundaries, &test, &ai, &complexity];
        run_verification(&ctx, &verifiers)
    }

    // ── Stage 4: DECIDE ─────────────────────────────────────────

    pub fn is_passing(&self, report: &VerificationReport) -> bool {
        self.is_passing_with_review_mode(report, "auto")
    }

    /// Check if verification is passing, with review mode support.
    ///
    /// - `"auto"` (default): PendingReview counts as passing
    /// - `"strict"`: PendingReview counts as non-passing
    pub fn is_passing_with_review_mode(
        &self,
        report: &VerificationReport,
        review_mode: &str,
    ) -> bool {
        let base = report.summary.total > 0
            && report.summary.failed == 0
            && report.summary.skipped == 0
            && report.summary.uncertain == 0;

        if review_mode == "strict" {
            base && report.summary.pending_review == 0
        } else {
            // "auto" mode: PendingReview counts as pass
            base
        }
    }

    /// Compute gate status by checking whether any critical scenario has a
    /// non-pass verdict.
    pub fn gate_status(&self, report: &VerificationReport) -> GateStatus {
        let mut blocked_gates = Vec::new();

        for scenario in &self.resolved.all_scenarios {
            if !scenario.is_critical() {
                continue;
            }
            let display = scenario.display_name();
            // Find the matching result in the report
            let is_non_pass = report.results.iter().any(|r| {
                (r.scenario_name == scenario.name || r.scenario_name == display)
                    && r.verdict != Verdict::Pass
                    && r.verdict != Verdict::PendingReview
            });
            if is_non_pass {
                blocked_gates.push(display.to_owned());
            }
        }

        GateStatus {
            gate_blocked: !blocked_gates.is_empty(),
            blocked_gates,
        }
    }

    pub fn failure_summary(&self, report: &VerificationReport) -> String {
        let mut out = String::new();
        out.push_str("## Verification Failed\n\n");
        out.push_str(&format!(
            "{} of {} scenarios are non-passing.\n\n",
            report.summary.failed
                + report.summary.skipped
                + report.summary.uncertain
                + report.summary.pending_review,
            report.summary.total,
        ));

        out.push_str("### Non-passing Scenarios\n\n");
        for result in &report.results {
            if result.verdict != Verdict::Pass {
                out.push_str(&format!("**{}**\n", result.scenario_name));
                out.push_str(&format!("- verdict: {:?}\n", result.verdict));
                for step in &result.step_results {
                    if step.verdict != Verdict::Pass {
                        out.push_str(&format!(
                            "- {:?}: {} ({})\n",
                            step.verdict, step.step_text, step.reason
                        ));
                    }
                }
                for ev in &result.evidence {
                    match ev {
                        crate::spec_core::Evidence::CodeSnippet {
                            file,
                            line,
                            content,
                        } => {
                            out.push_str(&format!("  > {file}:{line}: `{content}`\n"));
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
                            out.push_str(&format!("  > test `{test_name}`: passed={passed}\n"));
                            if let Some(package) = package {
                                out.push_str(&format!("    package={package}\n"));
                            }
                            if let Some(level) = level {
                                out.push_str(&format!("    level={level}\n"));
                            }
                            if let Some(test_double) = test_double {
                                out.push_str(&format!("    test_double={test_double}\n"));
                            }
                            if let Some(targets) = targets {
                                out.push_str(&format!("    targets={targets}\n"));
                            }
                        }
                        crate::spec_core::Evidence::AiAnalysis {
                            model,
                            confidence,
                            reasoning,
                        } => {
                            out.push_str(&format!(
                                "  > ai `{model}`: confidence={confidence:.2}; {reasoning}\n"
                            ));
                        }
                        _ => {}
                    }
                }
                out.push('\n');
            }
        }

        out.push_str("### Action Required\n\n");
        out.push_str("Resolve the failures or missing verification coverage above and re-run verification.\n");

        out
    }

    pub fn format_report(&self, report: &VerificationReport, format: &str) -> String {
        let fmt = match format {
            "json" => OutputFormat::Json,
            "md" | "markdown" => OutputFormat::Markdown,
            _ => OutputFormat::Text,
        };
        crate::spec_report::format_verification(report, &fmt)
    }

    pub fn format_lint_report(&self, report: &LintReport, format: &str) -> String {
        let fmt = match format {
            "json" => OutputFormat::Json,
            "md" | "markdown" => OutputFormat::Markdown,
            _ => OutputFormat::Text,
        };
        crate::spec_report::format_lint(report, &fmt)
    }
}

/// Result of the goal-gate check: whether any critical scenario is blocked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStatus {
    pub gate_blocked: bool,
    pub blocked_gates: Vec<String>,
}

/// Gate failure when spec quality is below threshold.
pub struct GateFailure {
    pub actual_score: f64,
    pub required_score: f64,
    pub report: LintReport,
}

impl std::fmt::Display for GateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error_count = self.report.error_count();
        if error_count > 0 {
            write!(
                f,
                "spec has {error_count} error-level lint issue(s) (quality {:.0}%, required {:.0}%)",
                self.actual_score * 100.0,
                self.required_score * 100.0,
            )
        } else {
            write!(
                f,
                "spec quality {:.0}% below required {:.0}% ({} issues)",
                self.actual_score * 100.0,
                self.required_score * 100.0,
                self.report.diagnostics.len(),
            )
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_verify::AiMode;
    use std::fs;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SAMPLE: &str = r#"spec: task
name: "测试任务"
tags: [test]
---

## 意图

实现一个简单的功能。

## 约束

### 禁止做
- 禁止使用 `.unwrap()`
- 禁止使用 `panic!`

### 必须做
- 所有错误必须返回 Result

## 已定决策

- 使用 thiserror 处理错误类型

## 验收标准

场景: 正常路径
  测试: test_full_lifecycle
  假设 输入有效
  当 调用函数
  那么 返回 Ok

场景: 错误路径
  测试: test_plan_returns_task_contract
  假设 输入无效
  当 调用函数
  那么 返回 Err

## 排除范围

- 日志系统
"#;

    #[test]
    fn test_full_lifecycle() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let contract = gw.plan();

        assert_eq!(contract.name, "测试任务");
        assert_eq!(contract.intent, "实现一个简单的功能。");
        assert_eq!(contract.must, vec!["所有错误必须返回 Result"]);
        assert_eq!(
            contract.must_not,
            vec!["禁止使用 `.unwrap()`", "禁止使用 `panic!`"]
        );
        assert_eq!(contract.decisions, vec!["使用 thiserror 处理错误类型"]);
        assert!(contract.forbidden.is_empty());
        assert_eq!(contract.out_of_scope, vec!["日志系统"]);
        assert_eq!(contract.completion_criteria.len(), 2);

        let contract_prompt = contract.to_prompt();
        assert!(contract_prompt.contains("# Task Contract: 测试任务"));
        assert!(contract_prompt.contains("## Must"));
        assert!(contract_prompt.contains("## Must NOT"));
        assert!(contract_prompt.contains("## Boundaries"));
        assert!(contract_prompt.contains(".unwrap()"));
        assert!(contract_prompt.contains("## Completion Criteria"));

        let lint = gw.lint();
        assert!(lint.quality_score.overall > 0.0);

        let json = contract.to_json();
        assert!(json.contains("测试任务"));
    }

    #[test]
    fn test_plan_returns_task_contract() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let plan = gw.plan();
        let contract = gw.contract();

        assert_eq!(plan.name, contract.name);
        assert_eq!(plan.must, contract.must);
        assert_eq!(plan.must_not, contract.must_not);
        assert_eq!(plan.decisions, contract.decisions);
        assert_eq!(plan.forbidden, contract.forbidden);
        assert_eq!(plan.completion_criteria.len(), 2);
        assert!(plan.to_prompt().contains("# Task Contract: 测试任务"));
    }

    #[test]
    fn test_quality_gate() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        assert!(gw.quality_gate(0.0).is_ok());
        let result = gw.quality_gate(1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_quality_gate_fails_on_error_lint_issue() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "缺少测试绑定"
---

## 完成条件

场景: 缺少 selector
  假设 存在一个任务规格
  当 质量闸门检查该规格
  那么 质量闸门应失败
"#,
        )
        .unwrap();

        let result = gw.quality_gate(0.0);
        assert!(result.is_err());
        let failure = result.unwrap_err();
        assert!(failure.to_string().contains("error-level lint issue"));
        assert!(failure.report.has_errors());
    }

    #[test]
    fn test_contract_prompt_format() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let prompt = gw.plan().to_prompt();
        assert!(prompt.contains("## Intent"));
        assert!(prompt.contains("## Must"));
        assert!(prompt.contains("## Must NOT"));
        assert!(prompt.contains("## Decisions"));
        assert!(prompt.contains("## Boundaries"));
        assert!(prompt.contains("## Completion Criteria"));
        assert!(prompt.contains("Scenario: 正常路径"));
        assert!(prompt.contains("Scenario: 错误路径"));
    }

    #[test]
    fn test_skip_is_not_passing() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let report = VerificationReport {
            spec_name: "测试任务".into(),
            results: vec![crate::spec_core::ScenarioResult {
                scenario_name: "未验证场景".into(),
                verdict: Verdict::Skip,
                step_results: vec![crate::spec_core::StepVerdict {
                    step_text: "等待 verifier".into(),
                    verdict: Verdict::Skip,
                    reason: "no verifier covered this step".into(),
                }],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            }],
            summary: crate::spec_core::VerificationSummary {
                total: 1,
                passed: 0,
                failed: 0,
                skipped: 1,
                uncertain: 0,
                pending_review: 0,
            },
        };

        assert!(!gw.is_passing(&report));
        let summary = gw.failure_summary(&report);
        assert!(summary.contains("non-passing"));
        assert!(summary.contains("Skip"));
    }

    #[test]
    fn test_load_resolves_inherited_constraints_from_spec_directory() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-gateway-{stamp}"));
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("project.spec"),
            r#"spec: project
name: "项目规则"
---

## 约束

### 禁止做
- 禁止使用 `panic!`
"#,
        )
        .unwrap();

        fs::write(
            root.join("task.spec"),
            r#"spec: task
name: "任务"
inherits: project
---

## 意图

实现功能。

## 约束

### 必须做
- 返回 Result

## 验收标准

场景: 正常路径
  假设 输入有效
  当 调用函数
  那么 返回 Ok
"#,
        )
        .unwrap();

        let gw = SpecGateway::load(root.join("task.spec")).unwrap();
        let prompt = gw.plan().to_prompt();
        let contract = gw.contract();

        assert!(prompt.contains("禁止使用 `panic!`"));
        assert!(prompt.contains("返回 Result"));
        assert!(contract.must_not.contains(&"禁止使用 `panic!`".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_load_resolves_full_project_contract_from_spec_directory() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-gateway-full-{stamp}"));
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("project.spec"),
            r#"spec: project
name: "项目规则"
---

## 约束

### 必须做
- 所有公共 API 返回结构化错误

### 禁止做
- 禁止使用 `panic!`

## 已定决策

- 使用 `thiserror` 统一错误类型
"#,
        )
        .unwrap();

        fs::write(
            root.join("task.spec"),
            r#"spec: task
name: "任务"
inherits: project
---

## 意图

实现功能。

## 已定决策

- 返回值保持现有 JSON 格式

## 验收标准

场景: 正常路径
  假设 输入有效
  当 调用函数
  那么 返回 Ok
"#,
        )
        .unwrap();

        let gw = SpecGateway::load(root.join("task.spec")).unwrap();
        let contract = gw.plan();

        assert!(
            contract
                .must
                .contains(&"所有公共 API 返回结构化错误".to_string())
        );
        assert!(contract.must_not.contains(&"禁止使用 `panic!`".to_string()));
        assert!(
            contract
                .decisions
                .contains(&"使用 `thiserror` 统一错误类型".to_string())
        );
        assert!(
            contract
                .decisions
                .contains(&"返回值保持现有 JSON 格式".to_string())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_task_contract_keeps_must_must_not_and_decisions_distinct() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "Contract fidelity"
---

## 意图

修正 Task Contract 语义。

## 约束

### 必须做
- 所有公共函数返回 `Result`

### 禁止做
- 禁止使用 `panic!`

## 已定决策

- 使用 `thiserror` 统一错误类型

## 验收标准

场景: 正常路径
  假设 合同包含多类约束
  当 gateway 构造 Task Contract
  那么 不同约束保持独立
"#,
        )
        .unwrap();

        let contract = gw.plan();
        assert_eq!(contract.must, vec!["所有公共函数返回 `Result`"]);
        assert_eq!(contract.must_not, vec!["禁止使用 `panic!`"]);
        assert_eq!(contract.decisions, vec!["使用 `thiserror` 统一错误类型"]);
    }

    #[allow(deprecated)]
    #[test]
    fn test_legacy_brief_stays_derived_from_contract() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let brief = gw.brief();
        let contract = gw.plan();

        assert_eq!(brief.name, contract.name);
        assert_eq!(brief.must, contract.must);
        assert!(brief.must_not.contains(&"禁止使用 `.unwrap()`".to_string()));
        assert_eq!(brief.decided, contract.decisions);
        assert_eq!(
            brief.scenario_names.len(),
            contract.completion_criteria.len()
        );
    }

    #[test]
    fn test_pass_plus_skip_is_not_passing() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let report = VerificationReport {
            spec_name: "测试任务".into(),
            results: vec![
                crate::spec_core::ScenarioResult {
                    scenario_name: "[structural] 禁止使用 `panic!`".into(),
                    verdict: Verdict::Pass,
                    step_results: vec![crate::spec_core::StepVerdict {
                        step_text: "禁止使用 `panic!`".into(),
                        verdict: Verdict::Pass,
                        reason: "no violations found".into(),
                    }],
                    evidence: vec![],
                    duration_ms: 0,
                    provenance: None,
                },
                crate::spec_core::ScenarioResult {
                    scenario_name: "未验证场景".into(),
                    verdict: Verdict::Skip,
                    step_results: vec![crate::spec_core::StepVerdict {
                        step_text: "等待 verifier".into(),
                        verdict: Verdict::Skip,
                        reason: "no verifier covered this step".into(),
                    }],
                    evidence: vec![],
                    duration_ms: 0,
                    provenance: None,
                },
            ],
            summary: crate::spec_core::VerificationSummary {
                total: 2,
                passed: 1,
                failed: 0,
                skipped: 1,
                uncertain: 0,
                pending_review: 0,
            },
        };

        assert!(!gw.is_passing(&report));
        let summary = gw.failure_summary(&report);
        assert!(summary.contains("verdict: Skip"));
    }

    #[test]
    fn test_new_bdd_lints_do_not_affect_lifecycle_verdict() {
        // Triggers bdd-rule-grouping (3 ungrouped scenarios) and
        // bdd-implementation-detail-step (点击), all warning/info.
        let input = r#"spec: task
name: "lint-not-gating"
---

## 完成条件

场景: 一
  测试: t1
  当 用户点击按钮
  那么 成功
场景: 二
  测试: t2
  当 a
  那么 b
场景: 三
  测试: t3
  当 a
  那么 b
"#;
        let doc = crate::spec_parser::parse_spec_from_str(input).unwrap();
        let report = crate::spec_lint::LintPipeline::with_defaults().run(&doc);
        let bdd: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.rule.starts_with("bdd-"))
            .collect();
        assert!(!bdd.is_empty(), "expected bdd-* diagnostics to fire");
        assert!(
            bdd.iter()
                .all(|d| d.severity != crate::spec_core::Severity::Error),
            "bdd-* lints must never be Error (would gate lifecycle)"
        );

        // is_passing reads only the verification summary — never lint — so an
        // all-pass report passes regardless of the bdd diagnostics above.
        let gw = SpecGateway::from_input(input).unwrap();
        let all_pass = VerificationReport {
            spec_name: "lint-not-gating".into(),
            results: vec![crate::spec_core::ScenarioResult {
                scenario_name: "一".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            }],
            summary: crate::spec_core::VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
            },
        };
        assert!(gw.is_passing(&all_pass));
    }

    #[test]
    fn test_existing_specs_pass_lifecycle_after_v1_changes() {
        // Every existing spec/example must still parse, and the new bdd-* lints
        // must never gate them (no Error severity from v1 rules).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0usize;
        for dir in ["specs", "examples"] {
            let base = root.join(dir);
            let mut stack = vec![base];
            while let Some(d) = stack.pop() {
                let Ok(entries) = fs::read_dir(&d) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !(name.ends_with(".spec") || name.ends_with(".spec.md")) {
                        continue;
                    }
                    let doc = crate::spec_parser::parse_spec(&path)
                        .unwrap_or_else(|e| panic!("failed to parse {}: {e:?}", path.display()));
                    let report = crate::spec_lint::LintPipeline::with_defaults().run(&doc);
                    for d in &report.diagnostics {
                        if d.rule.starts_with("bdd-") {
                            assert_ne!(
                                d.severity,
                                crate::spec_core::Severity::Error,
                                "bdd lint gated {}: {}",
                                path.display(),
                                d.message
                            );
                        }
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "expected to check existing specs");
    }

    #[test]
    fn test_verify_with_ai_mode_stub_marks_uncovered_scenarios_uncertain() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "AI skeleton"
---

## 完成条件

场景: 需要 AI 判断
  假设 存在一个未被机械 verifier 覆盖的场景
  当 gateway 使用 stub 模式验证
  那么 返回 uncertain
"#,
        )
        .unwrap();

        let report = gw.verify_with_ai_mode(".", AiMode::Stub).unwrap();
        assert_eq!(report.summary.uncertain, 1);
        assert_eq!(report.summary.skipped, 0);
        assert_eq!(report.results[0].verdict, Verdict::Uncertain);
    }

    #[test]
    fn test_provenance_ai_stub_is_inferential() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "AI provenance"
---

## 完成条件

场景: 未覆盖场景
  假设 存在一个未被机械 verifier 覆盖的场景
  当 gateway 使用 stub 模式验证
  那么 返回 uncertain
"#,
        )
        .unwrap();

        let report = gw.verify_with_ai_mode(".", AiMode::Stub).unwrap();
        assert_eq!(
            report.results[0].provenance,
            Some(crate::spec_core::EvidenceProvenance::Inferential),
            "stub AI verdict must be stamped inferential"
        );
    }

    #[test]
    fn test_verify_default_keeps_uncovered_scenarios_skipped() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "AI skeleton"
---

## 完成条件

场景: 需要 AI 判断
  假设 存在一个未被机械 verifier 覆盖的场景
  当 gateway 使用默认模式验证
  那么 返回 skip
"#,
        )
        .unwrap();

        let report = gw.verify(".").unwrap();
        assert_eq!(report.summary.uncertain, 0);
        assert_eq!(report.summary.skipped, 1);
        assert_eq!(report.results[0].verdict, Verdict::Skip);
    }

    struct HostBackend;

    impl AiBackend for HostBackend {
        fn name(&self) -> &str {
            "host-backend"
        }

        fn analyze(
            &self,
            _request: &crate::spec_core::AiRequest,
        ) -> SpecResult<crate::spec_core::AiDecision> {
            Ok(crate::spec_core::AiDecision {
                model: self.name().into(),
                confidence: 0.88,
                verdict: Verdict::Uncertain,
                reasoning: "host backend supplied the analysis".into(),
            })
        }
    }

    #[test]
    fn test_load_resolves_org_project_task_chain() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-gw-org-{stamp}"));
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("org.spec"),
            "spec: org\nname: \"组织规则\"\n---\n\n## Constraints\n\n- No hardcoded credentials\n",
        )
        .unwrap();
        fs::write(root.join("project.spec"), "spec: project\nname: \"项目规则\"\ninherits: org\n---\n\n## Constraints\n\n### Must\n- All public APIs return structured errors\n\n### Must NOT\n- 禁止使用 `panic!`\n\n## Decisions\n\n- Use thiserror for error types\n").unwrap();
        fs::write(root.join("task.spec"), "spec: task\nname: \"任务\"\ninherits: project\n---\n\n## Intent\n\n实现功能。\n\n## Completion Criteria\n\nScenario: happy path\n  Given valid input\n  When function is called\n  Then returns Ok\n").unwrap();

        let gw = SpecGateway::load(root.join("task.spec")).unwrap();
        let contract = gw.plan();

        assert!(
            contract
                .must
                .iter()
                .any(|c| c.contains("No hardcoded credentials"))
        );
        assert!(
            contract
                .must
                .iter()
                .any(|c| c.contains("All public APIs return structured errors"))
        );
        assert!(
            contract
                .must_not
                .iter()
                .any(|c| c.contains("禁止使用 `panic!`"))
        );
        assert!(
            contract
                .decisions
                .iter()
                .any(|d| d.contains("Use thiserror"))
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_verify_with_injected_ai_backend_uses_host_backend() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "AI host backend"
---

## 完成条件

场景: 交给宿主 backend
  假设 宿主 agent 提供了自定义 backend
  当 gateway 执行验证
  那么 返回 backend 的分析结果
"#,
        )
        .unwrap();

        let report = gw
            .verify_with_ai_backend(".", Arc::new(HostBackend))
            .unwrap();
        assert_eq!(report.summary.uncertain, 1);
        assert_eq!(report.results[0].verdict, Verdict::Uncertain);
    }

    // ── Goal-Gate tests ─────────────────────────────────────────

    /// Helper: build a minimal VerificationReport from (name, verdict) pairs.
    fn make_report(spec_name: &str, items: &[(&str, Verdict)]) -> VerificationReport {
        let results: Vec<crate::spec_core::ScenarioResult> = items
            .iter()
            .map(|(name, verdict)| crate::spec_core::ScenarioResult {
                scenario_name: (*name).to_owned(),
                verdict: *verdict,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            })
            .collect();
        VerificationReport::from_results(spec_name.to_owned(), results)
    }

    const CRITICAL_SAMPLE: &str = r#"spec: task
name: "门禁测试"
tags: [test]
---

## 意图

测试门禁功能。

## 验收标准

场景: 普通场景
  测试: test_critical_scenario_fail_sets_gate_blocked
  假设 输入有效
  当 调用函数
  那么 返回 Ok

场景: 关键场景
  标签: [critical]
  测试: test_critical_scenario_pass_no_gate_block
  假设 输入有效
  当 调用函数
  那么 返回 Ok
"#;

    #[test]
    fn test_critical_scenario_fail_sets_gate_blocked() {
        let gw = SpecGateway::from_input(CRITICAL_SAMPLE).unwrap();
        let report = make_report(
            "门禁测试",
            &[
                ("普通场景", Verdict::Pass),
                ("关键场景", Verdict::Fail),
            ],
        );

        let gate = gw.gate_status(&report);
        assert!(gate.gate_blocked);
        assert_eq!(gate.blocked_gates, vec!["关键场景"]);
    }

    #[test]
    fn test_critical_scenario_pass_no_gate_block() {
        let gw = SpecGateway::from_input(CRITICAL_SAMPLE).unwrap();
        let report = make_report(
            "门禁测试",
            &[
                ("普通场景", Verdict::Pass),
                ("关键场景", Verdict::Pass),
            ],
        );

        let gate = gw.gate_status(&report);
        assert!(!gate.gate_blocked);
        assert!(gate.blocked_gates.is_empty());
    }

    #[test]
    fn test_no_critical_tag_preserves_existing_behavior() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let report = make_report(
            "测试任务",
            &[
                ("正常路径", Verdict::Pass),
                ("错误路径", Verdict::Fail),
            ],
        );

        let gate = gw.gate_status(&report);
        assert!(!gate.gate_blocked);
        assert!(gate.blocked_gates.is_empty());

        // Existing behaviour: is_passing returns false when there are failures
        assert!(!gw.is_passing(&report));
    }

    #[test]
    fn test_critical_suffix_in_scenario_name() {
        let spec_with_suffix = r#"spec: task
name: "后缀测试"
tags: [test]
---

## 意图

测试名称后缀。

## 验收标准

场景: 用户注册成功（critical）
  测试: test_critical_suffix_in_scenario_name
  假设 输入有效
  当 调用函数
  那么 返回 Ok
"#;
        let gw = SpecGateway::from_input(spec_with_suffix).unwrap();

        // Verify the scenario is recognized as critical
        let scenario = &gw.resolved().all_scenarios[0];
        assert!(scenario.is_critical());
        assert_eq!(scenario.display_name(), "用户注册成功");

        // Verify gate_status works with the display name
        let report = make_report(
            "后缀测试",
            &[("用户注册成功（critical）", Verdict::Fail)],
        );
        let gate = gw.gate_status(&report);
        assert!(gate.gate_blocked);
        assert_eq!(gate.blocked_gates, vec!["用户注册成功"]);
    }

    #[test]
    fn test_critical_fail_exit_code_is_2() {
        // We test the gate_status method which drives exit code 2 in main.
        let gw = SpecGateway::from_input(CRITICAL_SAMPLE).unwrap();
        let report = make_report(
            "门禁测试",
            &[
                ("普通场景", Verdict::Fail),
                ("关键场景", Verdict::Fail),
            ],
        );

        let passing = gw.is_passing(&report);
        let gate = gw.gate_status(&report);

        // Not passing AND gate blocked → exit code should be 2
        assert!(!passing);
        assert!(gate.gate_blocked);
        assert_eq!(gate.blocked_gates, vec!["关键场景"]);
    }

    // ── Human Review tests ─────────────────────────────────────

    #[test]
    fn test_human_review_scenario_produces_pending_review() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "人类审核"
---

## 完成条件

场景: 需要人类审核
  审核: human
  测试: test_human_review_scenario_produces_pending_review
  假设 某个场景声明审核为 human 且测试通过
  当 lifecycle 执行该场景
  那么 verdict 为 pending_review
"#,
        )
        .unwrap();

        // Simulate a PendingReview result (test verifier would produce this)
        let report = make_report(
            "人类审核",
            &[("需要人类审核", Verdict::PendingReview)],
        );

        assert_eq!(report.summary.pending_review, 1);
        assert_eq!(report.results[0].verdict, Verdict::PendingReview);

        // In auto mode, pending_review counts as passing
        assert!(gw.is_passing_with_review_mode(&report, "auto"));
    }

    #[test]
    fn test_auto_review_mode_treats_pending_as_pass() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "审核模式"
---

## 完成条件

场景: 审核场景
  审核: human
  测试: test_auto_review_mode_treats_pending_as_pass
  假设 某个场景 verdict 为 pending_review
  当 lifecycle 使用默认 review-mode auto
  那么 最终 passed 为 true
"#,
        )
        .unwrap();

        let report = make_report(
            "审核模式",
            &[("审核场景", Verdict::PendingReview)],
        );

        // Auto mode: PendingReview counts as pass
        assert!(gw.is_passing_with_review_mode(&report, "auto"));
        // Default is_passing also uses auto
        assert!(gw.is_passing(&report));
    }

    #[test]
    fn test_strict_review_mode_treats_pending_as_not_pass() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "严格模式"
---

## 完成条件

场景: 严格审核
  审核: human
  测试: test_strict_review_mode_treats_pending_as_not_pass
  假设 某个场景 verdict 为 pending_review
  当 lifecycle 使用 review-mode strict
  那么 最终 passed 为 false
"#,
        )
        .unwrap();

        let report = make_report(
            "严格模式",
            &[("严格审核", Verdict::PendingReview)],
        );

        // Strict mode: PendingReview counts as NOT passing
        assert!(!gw.is_passing_with_review_mode(&report, "strict"));
    }

    // ── Optimize Scenario Mode tests ──────────────────────────────

    const OPTIMIZE_SAMPLE: &str = r#"spec: task
name: "优化模式测试"
tags: [test]
---

## 意图

测试优化场景模式。

## 验收标准

场景: 普通场景
  测试: test_optimize_scenario_pass_listed_as_candidate
  假设 输入有效
  当 调用函数
  那么 返回 Ok

场景: 优化场景
  模式: optimize
  测试: test_optimize_scenario_fail_blocks_pass
  假设 输入有效
  当 调用函数
  那么 性能达标
"#;

    #[test]
    fn test_optimize_scenario_pass_listed_as_candidate() {
        let gw = SpecGateway::from_input(OPTIMIZE_SAMPLE).unwrap();

        // Verify the optimize scenario is parsed correctly
        let opt_scenario = gw
            .resolved()
            .all_scenarios
            .iter()
            .find(|s| s.name == "优化场景")
            .expect("should find optimize scenario");
        assert_eq!(opt_scenario.mode, crate::spec_core::ScenarioMode::Optimize);

        // Build a report where optimize scenario passes
        let report = make_report(
            "优化模式测试",
            &[
                ("普通场景", Verdict::Pass),
                ("优化场景", Verdict::Pass),
            ],
        );

        // is_passing should be true (all pass)
        assert!(gw.is_passing(&report));

        // Collect optimization candidates (same logic as cmd_lifecycle)
        let candidates: Vec<String> = gw
            .resolved()
            .all_scenarios
            .iter()
            .filter(|s| s.mode == crate::spec_core::ScenarioMode::Optimize)
            .filter(|s| {
                report
                    .results
                    .iter()
                    .any(|r| r.scenario_name == s.name && r.verdict == Verdict::Pass)
            })
            .map(|s| s.name.clone())
            .collect();

        assert_eq!(candidates, vec!["优化场景"]);
    }

    #[test]
    fn test_optimize_scenario_fail_blocks_pass() {
        let gw = SpecGateway::from_input(OPTIMIZE_SAMPLE).unwrap();
        let report = make_report(
            "优化模式测试",
            &[
                ("普通场景", Verdict::Pass),
                ("优化场景", Verdict::Fail),
            ],
        );

        // Optimize scenario failing should cause is_passing to be false
        assert!(!gw.is_passing(&report));
    }

    // ── Scenario Dependencies tests ──────────────────────────────

    const DEPENDS_SAMPLE: &str = r#"spec: task
name: "依赖测试"
tags: [test]
---

## 意图

测试场景依赖。

## 验收标准

场景: 场景 A
  测试: test_dependency_skip_on_prerequisite_fail
  假设 前置条件 A
  当 执行 A
  那么 A 完成

场景: 场景 B
  前置: 场景 A
  测试: test_topological_sort_execution_order
  假设 前置条件 B
  当 执行 B
  那么 B 完成
"#;

    #[test]
    fn test_dependency_skip_on_prerequisite_fail() {
        let gw = SpecGateway::from_input(DEPENDS_SAMPLE).unwrap();

        // Verify depends_on is parsed
        let scenario_b = gw
            .resolved()
            .all_scenarios
            .iter()
            .find(|s| s.name == "场景 B")
            .expect("should find scenario B");
        assert_eq!(scenario_b.depends_on, vec!["场景 A"]);

        // Build report where A fails
        let mut report = make_report(
            "依赖测试",
            &[
                ("场景 A", Verdict::Fail),
                ("场景 B", Verdict::Pass),
            ],
        );

        // Apply dependency skips
        crate::apply_dependency_skips(&mut report, &gw.resolved().all_scenarios);

        // B should be skipped because A failed
        let b_result = report
            .results
            .iter()
            .find(|r| r.scenario_name == "场景 B")
            .expect("should find scenario B result");
        assert_eq!(b_result.verdict, Verdict::Skip);

        // Evidence should mention the failed dependency
        let has_dep_evidence = b_result.evidence.iter().any(|e| {
            matches!(e, crate::spec_core::Evidence::PatternMatch { pattern, .. }
                if pattern == "dependency-skip")
        });
        assert!(has_dep_evidence, "should have dependency-skip evidence");
    }

    #[test]
    fn test_topological_sort_execution_order() {
        let input = r#"spec: task
name: "拓扑排序"
---

## 完成条件

场景: C
  前置: B
  假设 C
  当 C
  那么 C

场景: B
  前置: A
  假设 B
  当 B
  那么 B

场景: A
  假设 A
  当 A
  那么 A
"#;
        let gw = SpecGateway::from_input(input).unwrap();
        let order = crate::topological_sort_scenarios(&gw.resolved().all_scenarios);

        // A (index 2) should come before B (index 1) which should come before C (index 0)
        let pos_a = order.iter().position(|&i| i == 2).unwrap();
        let pos_b = order.iter().position(|&i| i == 1).unwrap();
        let pos_c = order.iter().position(|&i| i == 0).unwrap();
        assert!(pos_a < pos_b, "A should come before B");
        assert!(pos_b < pos_c, "B should come before C");
    }

    #[test]
    fn test_no_dependency_preserves_original_order() {
        let input = r#"spec: task
name: "原始顺序"
---

## 完成条件

场景: 第一个
  假设 A
  当 A
  那么 A

场景: 第二个
  假设 B
  当 B
  那么 B

场景: 第三个
  假设 C
  当 C
  那么 C
"#;
        let gw = SpecGateway::from_input(input).unwrap();
        let order = crate::topological_sort_scenarios(&gw.resolved().all_scenarios);

        // Should preserve original order: 0, 1, 2
        assert_eq!(order, vec![0, 1, 2]);
    }
}
