mod ai_verifier;
mod boundaries;
mod complexity;
mod structural;
mod test_verifier;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::spec_core::{
    ResolvedSpec, ScenarioResult, SpecResult, StepVerdict, Verdict, VerificationReport,
};

pub use ai_verifier::{AiBackend, AiVerifier, build_ai_request};
pub use boundaries::BoundariesVerifier;
pub use complexity::ComplexityVerifier;
pub use structural::StructuralVerifier;
pub use test_verifier::TestVerifier;

/// AI verifier mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiMode {
    Off,
    Stub,
    External,
    /// Caller mode: emit AiRequests for the calling agent to resolve externally.
    Caller,
}

/// Context for verification.
pub struct VerificationContext {
    pub code_paths: Vec<PathBuf>,
    pub change_paths: Vec<PathBuf>,
    pub ai_mode: AiMode,
    pub resolved_spec: ResolvedSpec,
}

/// Trait for scenario verifiers.
pub trait Verifier: Send + Sync {
    fn name(&self) -> &str;
    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>>;
}

/// Run verification with a set of verifiers.
pub fn run_verification(
    ctx: &VerificationContext,
    verifiers: &[&dyn Verifier],
) -> SpecResult<VerificationReport> {
    let mut all_results = Vec::new();
    let mut covered_scenarios = HashSet::new();

    for verifier in verifiers {
        // Stamp provenance by producing verifier: the `ai` verifier is
        // inferential, every mechanical verifier is computational. This is the
        // single place mechanical results are distinguished from AI ones.
        let provenance = if verifier.name() == "ai" {
            crate::spec_core::EvidenceProvenance::Inferential
        } else {
            crate::spec_core::EvidenceProvenance::Computational
        };
        let results = verifier.verify(ctx)?;
        for mut result in results {
            if !covered_scenarios.insert(result.scenario_name.clone()) {
                continue;
            }
            result.provenance = Some(provenance);
            all_results.push(result);
        }
    }

    for scenario in &ctx.resolved_spec.all_scenarios {
        if covered_scenarios.contains(&scenario.name) {
            continue;
        }

        let step_results: Vec<StepVerdict> = scenario
            .steps
            .iter()
            .map(|step| StepVerdict {
                step_text: step.text.clone(),
                verdict: Verdict::Skip,
                reason: "no verifier covered this step".into(),
            })
            .collect();

        all_results.push(ScenarioResult {
            scenario_name: scenario.name.clone(),
            verdict: Verdict::Skip,
            step_results,
            evidence: Vec::new(),
            duration_ms: 0,
            provenance: None,
        });
    }

    Ok(VerificationReport::from_results(
        ctx.resolved_spec.task.meta.name.clone(),
        all_results,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use crate::spec_core::{
        ResolvedSpec, Scenario, ScenarioResult, Section, Span, SpecDocument, SpecLevel, SpecMeta,
        Step, StepKind, Verdict,
    };

    use super::{AiMode, VerificationContext, Verifier, run_verification};

    struct FirstVerifier;
    struct SecondVerifier;

    impl Verifier for FirstVerifier {
        fn name(&self) -> &str {
            "first"
        }

        fn verify(
            &self,
            _ctx: &VerificationContext,
        ) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "同一场景".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            }])
        }
    }

    impl Verifier for SecondVerifier {
        fn name(&self) -> &str {
            "second"
        }

        fn verify(
            &self,
            _ctx: &VerificationContext,
        ) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "同一场景".into(),
                verdict: Verdict::Uncertain,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            }])
        }
    }

    #[test]
    fn run_verification_keeps_first_result_for_same_scenario() {
        let scenario = Scenario {
            name: "同一场景".into(),
            steps: vec![Step {
                kind: StepKind::Given,
                text: "前置条件".into(),
                params: vec![],
                table: vec![],
                span: Span::line(1),
            }],
            test_selector: None,
            tags: vec![],
            review: Default::default(),
            mode: Default::default(),
            depends_on: vec![],
            rule: None,
            span: Span::line(1),
        };
        let ctx = VerificationContext {
            code_paths: vec![PathBuf::from(".")],
            change_paths: vec![],
            ai_mode: AiMode::Off,
            resolved_spec: ResolvedSpec {
                task: SpecDocument {
                    meta: SpecMeta {
                        level: SpecLevel::Task,
                        name: "test".into(),
                        inherits: None,
                        lang: vec![],
                        tags: vec![],
                        depends: vec![],
                        estimate: None,
                    },
                    sections: vec![Section::AcceptanceCriteria {
                        scenarios: vec![scenario.clone()],
                        rules: vec![],
                        malformed_rules: vec![],
                        span: Span::line(1),
                    }],
                    source_path: PathBuf::new(),
                },
                inherited_constraints: vec![],
                inherited_decisions: vec![],
                all_scenarios: vec![scenario],
            },
        };

        let first = FirstVerifier;
        let second = SecondVerifier;
        let report = run_verification(&ctx, &[&first, &second]).unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].verdict, Verdict::Pass);
    }

    // ---- Phase 2: provenance stamping ----

    struct MechVerifier;
    struct AiNamedVerifier;

    impl Verifier for MechVerifier {
        fn name(&self) -> &str {
            "test"
        }
        fn verify(
            &self,
            _ctx: &VerificationContext,
        ) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "机械场景".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            }])
        }
    }

    impl Verifier for AiNamedVerifier {
        fn name(&self) -> &str {
            "ai"
        }
        fn verify(
            &self,
            _ctx: &VerificationContext,
        ) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "推理场景".into(),
                verdict: Verdict::Uncertain,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            }])
        }
    }

    fn empty_ctx() -> VerificationContext {
        VerificationContext {
            code_paths: vec![PathBuf::from(".")],
            change_paths: vec![],
            ai_mode: AiMode::Off,
            resolved_spec: ResolvedSpec {
                task: SpecDocument {
                    meta: SpecMeta {
                        level: SpecLevel::Task,
                        name: "t".into(),
                        inherits: None,
                        lang: vec![],
                        tags: vec![],
                        depends: vec![],
                        estimate: None,
                    },
                    sections: vec![],
                    source_path: PathBuf::new(),
                },
                inherited_constraints: vec![],
                inherited_decisions: vec![],
                all_scenarios: vec![],
            },
        }
    }

    #[test]
    fn test_provenance_test_verifier_is_computational() {
        use crate::spec_core::EvidenceProvenance;
        let report = run_verification(&empty_ctx(), &[&MechVerifier]).unwrap();
        assert_eq!(
            report.results[0].provenance,
            Some(EvidenceProvenance::Computational)
        );
    }

    #[test]
    fn test_provenance_ai_verifier_is_inferential() {
        use crate::spec_core::EvidenceProvenance;
        let report = run_verification(&empty_ctx(), &[&AiNamedVerifier]).unwrap();
        assert_eq!(
            report.results[0].provenance,
            Some(EvidenceProvenance::Inferential)
        );
    }
}
