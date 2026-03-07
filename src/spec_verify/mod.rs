mod ai_verifier;
mod boundaries;
mod structural;
mod test_verifier;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::spec_core::{
    ResolvedSpec, ScenarioResult, SpecResult, StepVerdict, Verdict, VerificationReport,
};

pub use boundaries::BoundariesVerifier;
pub use structural::StructuralVerifier;
pub use test_verifier::TestVerifier;
pub use ai_verifier::{AiBackend, AiVerifier, StubAiBackend, build_ai_request};

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
        let results = verifier.verify(ctx)?;
        for result in results {
            if !covered_scenarios.insert(result.scenario_name.clone()) {
                continue;
            }
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
        });
    }

    Ok(VerificationReport::from_results(
        ctx.resolved_spec.task.meta.name.clone(),
        all_results,
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::spec_core::{
        ResolvedSpec, Scenario, ScenarioResult, Section, SpecDocument, SpecLevel, Span, SpecMeta,
        Step, StepKind, Verdict,
    };

    use super::{AiMode, VerificationContext, Verifier, run_verification};

    struct FirstVerifier;
    struct SecondVerifier;

    impl Verifier for FirstVerifier {
        fn name(&self) -> &str {
            "first"
        }

        fn verify(&self, _ctx: &VerificationContext) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "同一场景".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
            }])
        }
    }

    impl Verifier for SecondVerifier {
        fn name(&self) -> &str {
            "second"
        }

        fn verify(&self, _ctx: &VerificationContext) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "同一场景".into(),
                verdict: Verdict::Uncertain,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
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
                    },
                    sections: vec![Section::AcceptanceCriteria {
                        scenarios: vec![scenario.clone()],
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
}
