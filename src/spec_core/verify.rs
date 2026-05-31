use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Verification verdict for a scenario or step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Fail,
    Skip,
    Uncertain,
    PendingReview,
}

/// Whether a verdict came from mechanical execution or AI inference.
/// Phase 2 (coverage matrix): makes the unified verdict channel auditable —
/// a mechanically-proven pass is distinguishable from an AI-inferred one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceProvenance {
    /// Produced by a mechanical verifier (test / boundaries / structural / complexity).
    Computational,
    /// Produced by AI inference (ai verifier or caller-mode resolved decision).
    Inferential,
}

/// Result of verifying a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub verdict: Verdict,
    pub step_results: Vec<StepVerdict>,
    pub evidence: Vec<Evidence>,
    pub duration_ms: u64,
    /// Whether this verdict is mechanical or inferential. Additive (Phase 2);
    /// `None` for uncovered/skip results and legacy reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<EvidenceProvenance>,
}

/// Verdict for a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepVerdict {
    pub step_text: String,
    pub verdict: Verdict,
    pub reason: String,
}

/// Evidence supporting a verification verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Evidence {
    TestOutput {
        test_name: String,
        stdout: String,
        passed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        package: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        test_double: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        targets: Option<String>,
    },
    CodeSnippet {
        file: String,
        line: usize,
        content: String,
    },
    AiAnalysis {
        model: String,
        confidence: f64,
        reasoning: String,
    },
    PatternMatch {
        pattern: String,
        matched: bool,
        locations: Vec<String>,
    },
}

/// Checkpoint data for incremental/conservative resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub spec_name: String,
    pub timestamp: u64,
    pub vcs_ref: Option<String>,
    pub scenarios: HashMap<String, CheckpointEntry>,
}

/// Entry for a single scenario in a checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointEntry {
    pub verdict: Verdict,
    pub vcs_ref: Option<String>,
}

/// Structured request sent to an AI verifier backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequest {
    pub spec_name: String,
    pub scenario_name: String,
    pub steps: Vec<String>,
    pub code_paths: Vec<String>,
    /// Contract intent for additional context.
    #[serde(default)]
    pub contract_intent: String,
    /// Relevant contract constraints (must / must-not).
    #[serde(default)]
    pub contract_constraints: Vec<String>,
    /// Explicit change paths in scope.
    #[serde(default)]
    pub change_paths: Vec<String>,
    /// Prior evidence summaries from other verifiers.
    #[serde(default)]
    pub prior_evidence: Vec<String>,
}

/// Structured response returned by an AI verifier backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDecision {
    pub model: String,
    pub confidence: f64,
    pub verdict: Verdict,
    pub reasoning: String,
}

/// Summary of a full verification run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub uncertain: usize,
    #[serde(default)]
    pub pending_review: usize,
}

impl VerificationSummary {
    pub fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.passed as f64 / self.total as f64
    }
}

/// Full verification report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub spec_name: String,
    pub results: Vec<ScenarioResult>,
    pub summary: VerificationSummary,
}

impl VerificationReport {
    pub fn from_results(spec_name: String, results: Vec<ScenarioResult>) -> Self {
        let total = results.len();
        let passed = results
            .iter()
            .filter(|r| r.verdict == Verdict::Pass)
            .count();
        let failed = results
            .iter()
            .filter(|r| r.verdict == Verdict::Fail)
            .count();
        let skipped = results
            .iter()
            .filter(|r| r.verdict == Verdict::Skip)
            .count();
        let uncertain = results
            .iter()
            .filter(|r| r.verdict == Verdict::Uncertain)
            .count();
        let pending_review = results
            .iter()
            .filter(|r| r.verdict == Verdict::PendingReview)
            .count();

        Self {
            spec_name,
            results,
            summary: VerificationSummary {
                total,
                passed,
                failed,
                skipped,
                uncertain,
                pending_review,
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_json_provenance_additive_only() {
        // provenance == None must not emit a `provenance` key (legacy shape).
        let none = ScenarioResult {
            scenario_name: "s".into(),
            verdict: Verdict::Pass,
            step_results: vec![],
            evidence: vec![],
            duration_ms: 0,
            provenance: None,
        };
        let json = serde_json::to_string(&none).unwrap();
        assert!(!json.contains("provenance"), "None must skip the key: {json}");

        // When set, it serializes lowercased.
        let some = ScenarioResult {
            provenance: Some(EvidenceProvenance::Computational),
            ..none
        };
        let json = serde_json::to_string(&some).unwrap();
        assert!(json.contains("\"provenance\":\"computational\""));
    }
}
