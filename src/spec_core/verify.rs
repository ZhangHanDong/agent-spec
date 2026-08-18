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

/// Who settled a verdict, recorded only as a class rather than an identity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JudgmentSource {
    Human,
    Model,
}

/// A non-mechanical judgment bound to the evidence available before it was
/// applied. The core deliberately records no approver identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanJudgment {
    pub source: JudgmentSource,
    pub verdict: Verdict,
    pub reasoning: String,
    pub scenario_id: String,
    pub evidence_digest: String,
}

impl HumanJudgment {
    pub fn new(
        source: JudgmentSource,
        verdict: Verdict,
        reasoning: impl Into<String>,
        scenario_id: impl Into<String>,
        evidence: &[String],
    ) -> Self {
        Self {
            source,
            verdict,
            reasoning: reasoning.into(),
            scenario_id: scenario_id.into(),
            evidence_digest: blake3::hash(evidence.join("\n").as_bytes())
                .to_hex()
                .to_string(),
        }
    }
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
        /// Present only when an answered verification envelope explicitly
        /// supplied a human verdict. Keeping this inside the evidence item
        /// lets the resolved report carry the judgment into the requirement
        /// trace writer without changing the legacy `ScenarioResult` shape.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        human_judgment: Option<HumanJudgment>,
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
    /// Content fingerprint of the spec the checkpoint was taken from (same
    /// algorithm as the run log). Empty for checkpoints written before this
    /// field existed; such checkpoints are treated as stale.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub spec_fingerprint: String,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiDecision {
    pub model: String,
    pub confidence: f64,
    pub verdict: Verdict,
    pub reasoning: String,
}

/// Summary of a full verification run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationSummary {
    /// Gate counts: every result including synthetic layer rows
    /// (`[boundaries] …`, `[atlas-symbols] …`, `[complexity] …`). These feed
    /// `is_passing` and existing consumers; their meaning is unchanged.
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub uncertain: usize,
    #[serde(default)]
    pub pending_review: usize,
    /// Counts over genuine scenarios only (names not starting with `[`).
    /// Declared after the gate counts so JSON field order for the legacy
    /// fields is unchanged.
    #[serde(default, skip_serializing_if = "ScenarioCounts::is_empty")]
    pub scenarios: ScenarioCounts,
    /// One entry per synthetic verifier layer present in the results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LayerVerdict>,
}

/// Verdict counts over genuine scenarios (synthetic layer rows excluded).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioCounts {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub uncertain: usize,
    pub pending_review: usize,
}

impl ScenarioCounts {
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

/// Verdict of one synthetic verifier layer (`[name] …` result row).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerVerdict {
    pub name: String,
    pub verdict: Verdict,
}

fn verdict_word(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
        Verdict::Skip => "skip",
        Verdict::Uncertain => "uncertain",
        Verdict::PendingReview => "pending_review",
    }
}

/// Layer name of a synthetic result row (`[boundaries] …` → `boundaries`),
/// or `None` for a genuine scenario.
pub fn layer_name_of(scenario_name: &str) -> Option<&str> {
    let rest = scenario_name.strip_prefix('[')?;
    let end = rest.find(']')?;
    let name = &rest[..end];
    (!name.is_empty()).then_some(name)
}

impl VerificationSummary {
    /// One human-readable line: genuine scenario counts, then each layer's
    /// verdict when any layer ran.
    pub fn human_line(&self) -> String {
        let mut line = format!(
            "{}/{} scenarios passed, {} failed, {} skipped, {} uncertain",
            self.scenarios.passed,
            self.scenarios.total,
            self.scenarios.failed,
            self.scenarios.skipped,
            self.scenarios.uncertain,
        );
        if self.scenarios.pending_review > 0 {
            line.push_str(&format!(
                ", {} pending_review",
                self.scenarios.pending_review
            ));
        }
        if !self.layers.is_empty() {
            let layers: Vec<String> = self
                .layers
                .iter()
                .map(|l| format!("{}={}", l.name, verdict_word(l.verdict)))
                .collect();
            line.push_str(" · layers: ");
            line.push_str(&layers.join(", "));
        }
        line
    }

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

        let mut scenarios = ScenarioCounts::default();
        let mut layers = Vec::new();
        for r in &results {
            match layer_name_of(&r.scenario_name) {
                Some(name) => layers.push(LayerVerdict {
                    name: name.to_string(),
                    verdict: r.verdict,
                }),
                None => {
                    scenarios.total += 1;
                    match r.verdict {
                        Verdict::Pass => scenarios.passed += 1,
                        Verdict::Fail => scenarios.failed += 1,
                        Verdict::Skip => scenarios.skipped += 1,
                        Verdict::Uncertain => scenarios.uncertain += 1,
                        Verdict::PendingReview => scenarios.pending_review += 1,
                    }
                }
            }
        }

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
                scenarios,
                layers,
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn row(name: &str, verdict: Verdict) -> ScenarioResult {
        ScenarioResult {
            scenario_name: name.into(),
            verdict,
            step_results: vec![],
            evidence: vec![],
            duration_ms: 0,
            provenance: None,
        }
    }

    #[test]
    fn test_summary_splits_scenarios_and_layers() {
        let mut results: Vec<ScenarioResult> = (0..10)
            .map(|i| row(&format!("s{i}"), Verdict::Pass))
            .collect();
        results.push(row(
            "[boundaries] explicit change set respects declared paths",
            Verdict::Fail,
        ));
        let report = VerificationReport::from_results("x".into(), results);
        // Gate counts unchanged: layer rows still count.
        assert_eq!(report.summary.total, 11);
        assert_eq!(report.summary.passed, 10);
        assert_eq!(report.summary.failed, 1);
        // Genuine scenarios only.
        assert_eq!(report.summary.scenarios.total, 10);
        assert_eq!(report.summary.scenarios.passed, 10);
        assert_eq!(report.summary.scenarios.failed, 0);
        assert_eq!(
            report.summary.layers,
            vec![LayerVerdict {
                name: "boundaries".into(),
                verdict: Verdict::Fail
            }]
        );
    }

    #[test]
    fn test_summary_without_layers_serializes_like_before() {
        let report = VerificationReport::from_results("x".into(), vec![row("a", Verdict::Pass)]);
        let json = serde_json::to_string(&report.summary).unwrap();
        assert!(!json.contains("layers"), "{json}");
        assert!(report.summary.layers.is_empty());
        assert_eq!(report.summary.scenarios.total, 1);
    }

    #[test]
    fn test_summary_json_keeps_legacy_field_order() {
        let report = VerificationReport::from_results(
            "x".into(),
            vec![
                row("a", Verdict::Pass),
                row("[complexity] code quality gate", Verdict::Pass),
            ],
        );
        let json = serde_json::to_string(&report.summary).unwrap();
        let failed_at = json.find("\"failed\"").unwrap();
        let scenarios_at = json.find("\"scenarios\"").unwrap();
        let layers_at = json.find("\"layers\"").unwrap();
        assert!(failed_at < scenarios_at, "{json}");
        assert!(scenarios_at < layers_at, "{json}");
    }

    #[test]
    fn test_text_summary_line_separates_scenarios_and_layers() {
        let mut results: Vec<ScenarioResult> = (0..10)
            .map(|i| row(&format!("s{i}"), Verdict::Pass))
            .collect();
        results.push(row("[boundaries] ok", Verdict::Pass));
        let report = VerificationReport::from_results("x".into(), results);
        let line = report.summary.human_line();
        assert!(line.contains("10/10 scenarios passed"), "{line}");
        assert!(line.contains("boundaries=pass"), "{line}");
        // No layers → no layer suffix.
        let plain = VerificationReport::from_results("y".into(), vec![row("a", Verdict::Skip)]);
        assert_eq!(
            plain.summary.human_line(),
            "0/1 scenarios passed, 0 failed, 1 skipped, 0 uncertain"
        );
    }

    #[test]
    fn test_layer_name_of_recognizes_bracket_prefix() {
        assert_eq!(layer_name_of("[boundaries] x"), Some("boundaries"));
        assert_eq!(layer_name_of("[atlas-symbols] y"), Some("atlas-symbols"));
        assert_eq!(layer_name_of("plain scenario"), None);
        assert_eq!(layer_name_of("[] empty"), None);
    }

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
        assert!(
            !json.contains("provenance"),
            "None must skip the key: {json}"
        );

        // When set, it serializes lowercased.
        let some = ScenarioResult {
            provenance: Some(EvidenceProvenance::Computational),
            ..none
        };
        let json = serde_json::to_string(&some).unwrap();
        assert!(json.contains("\"provenance\":\"computational\""));
    }

    // ---- Phase 3: Rule event log ----

    #[test]
    fn test_rule_events_additive_empty_by_default() {
        use crate::spec_core::{BehaviorRule, RuleKey, RuleScope, Span};
        let rule = BehaviorRule {
            key: RuleKey {
                scope: RuleScope::Task("t".into()),
                id: "r".into(),
            },
            name: "r".into(),
            scenario_names: vec![],
            events: vec![],
            span: Span::line(1),
        };
        let json = serde_json::to_string(&rule).unwrap();
        assert!(
            !json.contains("\"events\""),
            "empty events must skip key: {json}"
        );
    }

    #[test]
    fn test_rule_event_roundtrips() {
        use crate::spec_core::{RuleEvent, RuleEventKind};
        let ev = RuleEvent {
            kind: RuleEventKind::Promoted,
            note: "from task-foo".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: RuleEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ev);
        assert!(json.contains("\"promoted\""));
    }
}
