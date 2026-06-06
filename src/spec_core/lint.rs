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

/// Quality dimension a lint rule belongs to (Phase 5; Spec Kit /checklist's
/// five dimensions adapted to agent-spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dimension {
    Completeness,
    Clarity,
    Consistency,
    Coverage,
    Boundary,
}

/// Classify a lint rule code into one of the five quality dimensions.
/// Unknown codes fall back to `Consistency` (never panics).
pub fn dimension_of(rule_code: &str) -> Dimension {
    match rule_code {
        "coverage"
        | "decision-coverage"
        | "observable-decision-coverage"
        | "output-mode-coverage"
        | "precedence-fallback-coverage"
        | "flag-combination-coverage"
        | "error-path"
        | "bdd-rule-grouping" => Dimension::Coverage,
        "vague-verb"
        | "unquantified"
        | "determinism"
        | "testability"
        | "implicit-dep"
        | "bdd-scenario-shape"
        | "bdd-implementation-detail-step"
        | "bdd-rule-id"
        | "open-question" => Dimension::Clarity,
        "boundary-entry-point"
        | "platform-decision-tag"
        | "cross-check-boundary"
        | "external-io-error-strength"
        | "verification-metadata-suggestion" => Dimension::Boundary,
        "scenario-presence" | "universal-claim" => Dimension::Completeness,
        _ => Dimension::Consistency,
    }
}

/// Complete lint report for a spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintReport {
    pub spec_name: String,
    pub diagnostics: Vec<LintDiagnostic>,
    /// Diagnostics waived via lint-ack (Phase 5). Additive; empty when none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledged: Vec<LintDiagnostic>,
    pub quality_score: QualityScore,
}

impl LintReport {
    /// Count of remaining (non-acknowledged) diagnostics per dimension.
    pub fn dimension_counts(&self) -> [(Dimension, usize); 5] {
        let dims = [
            Dimension::Completeness,
            Dimension::Clarity,
            Dimension::Consistency,
            Dimension::Coverage,
            Dimension::Boundary,
        ];
        dims.map(|d| {
            let n = self
                .diagnostics
                .iter()
                .filter(|diag| dimension_of(&diag.rule) == d)
                .count();
            (d, n)
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_of_known_rules() {
        assert_eq!(dimension_of("coverage"), Dimension::Coverage);
        assert_eq!(dimension_of("bdd-scenario-shape"), Dimension::Clarity);
        assert_eq!(dimension_of("cross-check-boundary"), Dimension::Boundary);
        assert_eq!(dimension_of("scenario-presence"), Dimension::Completeness);
    }

    #[test]
    fn test_dimension_of_unknown_falls_back() {
        assert_eq!(
            dimension_of("totally-unregistered-rule"),
            Dimension::Consistency
        );
    }
}
