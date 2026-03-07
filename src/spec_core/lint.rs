use super::Span;
use serde::{Deserialize, Serialize};

/// Lint diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A single lint diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintDiagnostic {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub suggestion: Option<String>,
}

/// Quality score for a spec document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    pub determinism: f64,
    pub testability: f64,
    pub coverage: f64,
    pub overall: f64,
}

impl QualityScore {
    pub fn compute(determinism: f64, testability: f64, coverage: f64) -> Self {
        let overall = (determinism + testability + coverage) / 3.0;
        Self {
            determinism,
            testability,
            coverage,
            overall,
        }
    }
}

/// Complete lint report for a spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintReport {
    pub spec_name: String,
    pub diagnostics: Vec<LintDiagnostic>,
    pub quality_score: QualityScore,
}

impl LintReport {
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diag| diag.severity == Severity::Error)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}
