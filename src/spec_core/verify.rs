use serde::{Deserialize, Serialize};

/// Verification verdict for a scenario or step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Fail,
    Skip,
    Uncertain,
}

/// Result of verifying a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub verdict: Verdict,
    pub step_results: Vec<StepVerdict>,
    pub evidence: Vec<Evidence>,
    pub duration_ms: u64,
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

        Self {
            spec_name,
            results,
            summary: VerificationSummary {
                total,
                passed,
                failed,
                skipped,
                uncertain,
            },
        }
    }
}
