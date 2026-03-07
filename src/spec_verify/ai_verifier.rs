use std::sync::Arc;

use crate::spec_core::{
    AiDecision, AiRequest, Evidence, Scenario, ScenarioResult, SpecResult, StepVerdict, Verdict,
};

use super::{AiMode, VerificationContext, Verifier};

/// AI verifier backed by a pluggable AI backend.
pub struct AiVerifier {
    backend: Option<Arc<dyn AiBackend>>,
}

pub trait AiBackend: Send + Sync {
    fn name(&self) -> &str;
    fn analyze(&self, request: &AiRequest) -> SpecResult<AiDecision>;
}

pub struct StubAiBackend;

impl AiBackend for StubAiBackend {
    fn name(&self) -> &str {
        "stub"
    }

    fn analyze(&self, _request: &AiRequest) -> SpecResult<AiDecision> {
        Ok(AiDecision {
            model: self.name().into(),
            confidence: 0.0,
            verdict: Verdict::Uncertain,
            reasoning: "ai verifier stub enabled; no model backend configured, manual review required".into(),
        })
    }
}

impl AiVerifier {
    pub fn from_mode(mode: AiMode) -> Self {
        let backend = match mode {
            AiMode::Off => None,
            AiMode::Stub => Some(Arc::new(StubAiBackend) as Arc<dyn AiBackend>),
            AiMode::External => None,
            AiMode::Caller => None,
        };
        Self { backend }
    }

    pub fn with_backend(backend: Arc<dyn AiBackend>) -> Self {
        Self {
            backend: Some(backend),
        }
    }
}

impl Default for AiVerifier {
    fn default() -> Self {
        Self::from_mode(AiMode::Off)
    }
}

impl Verifier for AiVerifier {
    fn name(&self) -> &str {
        "ai"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        let Some(backend) = self.backend.as_ref() else {
            return Ok(Vec::new());
        };

        let mut results = Vec::new();
        for scenario in &ctx.resolved_spec.all_scenarios {
            let request = build_ai_request(&ctx.resolved_spec.task.meta.name, scenario, ctx);
            let decision = backend.analyze(&request)?;
            let step_results = scenario
                .steps
                .iter()
                .map(|step| StepVerdict {
                    step_text: step.text.clone(),
                    verdict: decision.verdict,
                    reason: decision.reasoning.clone(),
                })
                .collect();

            results.push(ScenarioResult {
                scenario_name: scenario.name.clone(),
                verdict: decision.verdict,
                step_results,
                evidence: vec![Evidence::AiAnalysis {
                    model: decision.model,
                    confidence: decision.confidence,
                    reasoning: decision.reasoning,
                }],
                duration_ms: 0,
            });
        }

        Ok(results)
    }
}

pub fn build_ai_request(
    spec_name: &str,
    scenario: &Scenario,
    ctx: &VerificationContext,
) -> AiRequest {
    // Extract intent from resolved spec sections
    let contract_intent = ctx
        .resolved_spec
        .task
        .sections
        .iter()
        .find_map(|s| match s {
            crate::spec_core::Section::Intent { content, .. } => Some(content.clone()),
            _ => None,
        })
        .unwrap_or_default();

    // Extract constraints for context
    let contract_constraints: Vec<String> = ctx
        .resolved_spec
        .task
        .sections
        .iter()
        .filter_map(|s| match s {
            crate::spec_core::Section::Constraints { items, .. } => {
                Some(items.iter().map(|c| c.text.clone()).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .chain(
            ctx.resolved_spec
                .inherited_constraints
                .iter()
                .map(|c| c.text.clone()),
        )
        .collect();

    AiRequest {
        spec_name: spec_name.into(),
        scenario_name: scenario.name.clone(),
        steps: scenario
            .steps
            .iter()
            .map(|step| step.text.clone())
            .collect(),
        code_paths: ctx
            .code_paths
            .iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect(),
        contract_intent,
        contract_constraints,
        change_paths: ctx
            .change_paths
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect(),
        prior_evidence: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::spec_core::{
        AiDecision, Evidence, ResolvedSpec, Scenario, Section, SpecDocument, SpecLevel, SpecMeta,
        Span, Step, StepKind, Verdict,
    };

    use super::{AiBackend, AiVerifier, StubAiBackend, build_ai_request};
    use super::{AiMode, VerificationContext, Verifier};
    use crate::spec_core::SpecResult;

    struct FakeBackend;

    impl AiBackend for FakeBackend {
        fn name(&self) -> &str {
            "fake"
        }

        fn analyze(&self, _request: &crate::spec_core::AiRequest) -> SpecResult<AiDecision> {
            Ok(AiDecision {
                model: self.name().into(),
                confidence: 0.42,
                verdict: Verdict::Uncertain,
                reasoning: "custom backend response".into(),
            })
        }
    }

    fn sample_context() -> VerificationContext {
        use crate::spec_core::{Constraint, ConstraintCategory};

        let scenario = Scenario {
            name: "AI 场景".into(),
            steps: vec![
                Step {
                    kind: StepKind::Given,
                    text: "存在代码上下文".into(),
                    params: vec![],
                    table: vec![],
                    span: Span::line(1),
                },
                Step {
                    kind: StepKind::Then,
                    text: "需要 AI 判断".into(),
                    params: vec![],
                    table: vec![],
                    span: Span::line(2),
                },
            ],
            test_selector: None,
            tags: vec![],
            span: Span::line(1),
        };

        VerificationContext {
            code_paths: vec![PathBuf::from("crates/spec-verify/src/ai_verifier.rs")],
            change_paths: vec![PathBuf::from("src/changed.rs")],
            ai_mode: AiMode::Stub,
            resolved_spec: ResolvedSpec {
                task: SpecDocument {
                    meta: SpecMeta {
                        level: SpecLevel::Task,
                        name: "AI 验证".into(),
                        inherits: None,
                        lang: vec![],
                        tags: vec![],
                    },
                    sections: vec![
                        Section::Intent {
                            content: "验证 AI 请求包含完整上下文".into(),
                            span: Span::line(1),
                        },
                        Section::Constraints {
                            items: vec![Constraint {
                                text: "所有错误必须返回 Result".into(),
                                category: ConstraintCategory::Must,
                                span: Span::line(2),
                            }],
                            span: Span::line(2),
                        },
                        Section::AcceptanceCriteria {
                            scenarios: vec![scenario.clone()],
                            span: Span::line(3),
                        },
                    ],
                    source_path: PathBuf::new(),
                },
                inherited_constraints: vec![Constraint {
                    text: "禁止使用 unwrap".into(),
                    category: ConstraintCategory::MustNot,
                    span: Span::line(1),
                }],
                inherited_decisions: vec![],
                all_scenarios: vec![scenario],
            },
        }
    }

    #[test]
    fn test_stub_ai_backend_returns_uncertain_decision() {
        let backend = StubAiBackend;
        let request = crate::spec_core::AiRequest {
            spec_name: "AI 验证".into(),
            scenario_name: "AI 场景".into(),
            steps: vec!["需要 AI 判断".into()],
            code_paths: vec!["src/lib.rs".into()],
            contract_intent: String::new(),
            contract_constraints: Vec::new(),
            change_paths: Vec::new(),
            prior_evidence: Vec::new(),
        };

        let decision = backend.analyze(&request).unwrap();
        assert_eq!(decision.model, "stub");
        assert_eq!(decision.verdict, Verdict::Uncertain);
        assert_eq!(decision.confidence, 0.0);
    }

    #[test]
    fn test_ai_verifier_with_custom_backend_uses_backend_response() {
        let ctx = sample_context();
        let verifier = AiVerifier::with_backend(Arc::new(FakeBackend));
        let results = verifier.verify(&ctx).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Uncertain);
        assert!(matches!(
            results[0].evidence.first(),
            Some(Evidence::AiAnalysis {
                model,
                confidence,
                reasoning,
            }) if model == "fake" && (*confidence - 0.42).abs() < f64::EPSILON && reasoning == "custom backend response"
        ));
    }

    #[test]
    fn test_build_ai_request_includes_scenario_and_code_paths() {
        let ctx = sample_context();
        let scenario = &ctx.resolved_spec.all_scenarios[0];
        let request = build_ai_request(&ctx.resolved_spec.task.meta.name, scenario, &ctx);

        assert_eq!(request.spec_name, "AI 验证");
        assert_eq!(request.scenario_name, "AI 场景");
        assert_eq!(request.steps, vec!["存在代码上下文", "需要 AI 判断"]);
        assert!(
            request
                .code_paths
                .iter()
                .any(|path| path.ends_with("crates/spec-verify/src/ai_verifier.rs"))
        );
    }

    #[test]
    fn test_build_ai_request_includes_contract_change_set_and_evidence_context() {
        let ctx = sample_context();
        let scenario = &ctx.resolved_spec.all_scenarios[0];
        let request = build_ai_request(&ctx.resolved_spec.task.meta.name, scenario, &ctx);

        // Contract intent is populated from the spec's Intent section
        assert!(
            request.contract_intent.contains("验证 AI 请求包含完整上下文"),
            "contract_intent should contain the spec intent"
        );

        // Contract constraints include both local and inherited
        assert!(
            request
                .contract_constraints
                .iter()
                .any(|c| c.contains("所有错误必须返回 Result")),
            "should include local constraints"
        );
        assert!(
            request
                .contract_constraints
                .iter()
                .any(|c| c.contains("禁止使用 unwrap")),
            "should include inherited constraints"
        );

        // Change paths are populated from ctx
        assert!(
            request
                .change_paths
                .iter()
                .any(|p| p.contains("src/changed.rs")),
            "should include change paths"
        );
    }
}
