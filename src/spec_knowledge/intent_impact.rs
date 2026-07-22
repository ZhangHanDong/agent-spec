//! Deterministic join from provider-neutral code impact to governed intent.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::spec_knowledge::{
    CodeImpactInput, ProviderImpact, ProviderImpactEntry, ProviderImpactError,
};

pub const INTENT_IMPACT_SCHEMA_ID: &str = "agent-spec/intent-compiler/intent-impact-v1";
pub const AFFECTED_EXECUTION_BUNDLE_SCHEMA_ID: &str =
    "agent-spec/intent-compiler/affected-execution-bundle-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentImpactGap {
    pub code: String,
    pub severity: String,
    pub node_id: Option<String>,
    pub requirement_id: Option<String>,
    pub spec_path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentTestObligationLink {
    pub requirement_id: String,
    pub scenario_name: String,
    pub suggested_selector: String,
    pub selector_authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentScenarioLink {
    pub name: String,
    pub authoritative_selector: Option<String>,
    pub test_candidate: Option<String>,
    pub test_obligation: Option<IntentTestObligationLink>,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentSpecLink {
    pub path: PathBuf,
    pub risk: Option<String>,
    pub scenarios: Vec<IntentScenarioLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedWorktreeLink {
    pub path: PathBuf,
    pub branch: String,
    pub base_branch: String,
    pub batch: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentBindingLink {
    pub requirement_id: String,
    pub work_unit_id: String,
    pub provider: String,
    pub graph_fingerprint: String,
    pub specs: Vec<IntentSpecLink>,
    pub worktree: Option<PlannedWorktreeLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentAffectedNode {
    pub impact: ProviderImpactEntry,
    pub links: Vec<IntentBindingLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentImpactReport {
    pub schema: String,
    pub provider: String,
    pub graph_fingerprint: Option<String>,
    pub input: CodeImpactInput,
    pub affected: Vec<IntentAffectedNode>,
    pub truncated: bool,
    pub gaps: Vec<IntentImpactGap>,
    pub provider_diagnostics: Vec<crate::spec_knowledge::ProviderImpactDiagnostic>,
    pub observed_vcs: Option<crate::vcs::VcsContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionSelection {
    pub id: String,
    pub kind: String,
    pub role: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectedTest {
    pub requirement_id: String,
    pub spec_path: PathBuf,
    pub scenario: String,
    pub selector: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuidanceSelection {
    pub guidance_id: String,
    pub source: PathBuf,
    pub matched_paths: Vec<String>,
    pub skills: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AffectedExecutionBundle {
    pub schema: String,
    pub intent_impact_digest: String,
    pub risk: Option<String>,
    pub required_evidence: Vec<String>,
    pub quality_profile: Vec<crate::spec_knowledge::QualityProvider>,
    pub fast_checks: Vec<ExecutionSelection>,
    pub acceptance_gates: Vec<ExecutionSelection>,
    pub authoritative_tests: Vec<SelectedTest>,
    pub test_candidates: Vec<SelectedTest>,
    pub guidance: Vec<GuidanceSelection>,
    pub required_skills: Vec<String>,
    pub skill_receipts: Vec<crate::spec_knowledge::SkillReceipt>,
    pub gaps: Vec<IntentImpactGap>,
}

#[allow(clippy::too_many_arguments)]
pub fn build_intent_impact(
    provider: &str,
    input: CodeImpactInput,
    impact: Result<ProviderImpact, ProviderImpactError>,
    knowledge: &Path,
    specs: &Path,
    bindings_path: &Path,
    worktree_manifest_path: Option<&Path>,
    observed_vcs: Option<crate::vcs::VcsContext>,
) -> Result<IntentImpactReport, String> {
    let worktree_manifest_missing = worktree_manifest_path.is_none_or(|path| !path.is_file());
    let mut gaps = Vec::new();
    if worktree_manifest_missing {
        gaps.push(gap(
            "worktree-manifest-missing",
            "warning",
            None,
            None,
            None,
            "no worktree manifest was available for the affected projection",
        ));
    }
    if observed_vcs.is_none() {
        gaps.push(gap(
            "vcs-unobserved",
            "warning",
            None,
            None,
            None,
            "no VCS context was observed for the affected projection",
        ));
    }
    let impact = match impact {
        Ok(impact) => impact,
        Err(error) => {
            gaps.push(gap(&error.code, "error", None, None, None, error.message));
            sort_gaps(&mut gaps);
            return Ok(IntentImpactReport {
                schema: INTENT_IMPACT_SCHEMA_ID.into(),
                provider: provider.into(),
                graph_fingerprint: None,
                input,
                affected: Vec::new(),
                truncated: false,
                gaps,
                provider_diagnostics: Vec::new(),
                observed_vcs,
            });
        }
    };

    let bindings_missing = !bindings_path.is_file();
    let bindings = read_bindings(bindings_path)?;
    let plan = crate::spec_knowledge::build_requirement_plan(knowledge, specs);
    let requirement_graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    let obligations = crate::spec_knowledge::build_test_obligations(knowledge, specs);
    let worktrees = read_worktrees(worktree_manifest_path)?;
    if bindings_missing {
        gaps.push(gap(
            "bindings-missing",
            "error",
            None,
            None,
            None,
            format!(
                "code bindings artifact is missing: {}",
                bindings_path.display()
            ),
        ));
    }
    if impact.truncated {
        gaps.push(gap(
            "impact-truncated",
            "warning",
            None,
            None,
            None,
            "provider impact reached a configured traversal or output limit",
        ));
    }

    let mut affected = Vec::new();
    for entry in impact.entries {
        let matching = bindings
            .entries
            .iter()
            .filter(|binding| {
                binding.provider == impact.provider
                    && binding
                        .targets
                        .iter()
                        .any(|target| target.node_id == entry.node.node_id)
            })
            .collect::<Vec<_>>();
        let mut links = Vec::new();
        if matching.is_empty() {
            gaps.push(gap(
                "affected-node-unbound",
                "warning",
                Some(&entry.node.node_id),
                None,
                None,
                format!(
                    "affected node `{}` has no current code binding",
                    entry.node.node_id
                ),
            ));
        }
        for binding in matching {
            if binding.graph_fingerprint != impact.graph_fingerprint {
                gaps.push(gap(
                    "binding-fingerprint-mismatch",
                    "error",
                    Some(&entry.node.node_id),
                    Some(&binding.requirement_id),
                    None,
                    format!(
                        "binding graph {} differs from provider graph {}",
                        binding.graph_fingerprint, impact.graph_fingerprint
                    ),
                ));
            }
            if !plan
                .requirements
                .iter()
                .any(|requirement| requirement.id == binding.requirement_id)
            {
                gaps.push(gap(
                    "requirement-not-in-plan",
                    "error",
                    Some(&entry.node.node_id),
                    Some(&binding.requirement_id),
                    None,
                    format!(
                        "{} is absent from the requirement plan",
                        binding.requirement_id
                    ),
                ));
            }

            let spec_paths = plan
                .coverage
                .iter()
                .find(|coverage| coverage.requirement_id == binding.requirement_id)
                .map(|coverage| coverage.spec_paths.clone())
                .unwrap_or_default();
            if spec_paths.is_empty() {
                gaps.push(gap(
                    "scenario-unmapped",
                    "error",
                    Some(&entry.node.node_id),
                    Some(&binding.requirement_id),
                    None,
                    "requirement has no satisfying Task Contract",
                ));
            }
            let mut spec_links = Vec::new();
            for spec_path in spec_paths {
                let doc = crate::spec_parser::parse_spec(&spec_path)
                    .map_err(|error| format!("cannot parse {}: {error}", spec_path.display()))?;
                let requirement_scenarios = requirement_graph
                    .node(&binding.requirement_id)
                    .map(|node| {
                        node.scenarios
                            .iter()
                            .map(|scenario| scenario.name.as_str())
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                let mut scenarios = Vec::new();
                for scenario in contract_scenarios(&doc) {
                    if doc.meta.satisfies.len() > 1
                        && !requirement_scenarios.contains(scenario.name.as_str())
                    {
                        continue;
                    }
                    let explicit = scenario
                        .test_selector
                        .as_ref()
                        .map(|selector| selector.filter.clone());
                    let obligation = obligations.obligations.iter().find(|obligation| {
                        obligation.requirement_id == binding.requirement_id
                            && obligation.scenario_name == scenario.name
                    });
                    let candidate = explicit
                        .is_none()
                        .then(|| {
                            obligation
                                .filter(|item| item.selector_authority == "candidate")
                                .map(|item| item.suggested_selector.clone())
                        })
                        .flatten();
                    let required_evidence = obligation
                        .map(|item| {
                            item.required_evidence
                                .iter()
                                .map(enum_name)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if obligation.is_none() {
                        gaps.push(gap(
                            "obligation-unmapped",
                            "error",
                            Some(&entry.node.node_id),
                            Some(&binding.requirement_id),
                            Some(&spec_path),
                            format!(
                                "scenario `{}` has no matching test obligation",
                                scenario.name
                            ),
                        ));
                    }
                    if explicit.is_none() {
                        gaps.push(gap(
                            "selector-missing",
                            "error",
                            Some(&entry.node.node_id),
                            Some(&binding.requirement_id),
                            Some(&spec_path),
                            format!(
                                "scenario `{}` has no explicit Task Contract selector",
                                scenario.name
                            ),
                        ));
                    }
                    scenarios.push(IntentScenarioLink {
                        name: scenario.name.clone(),
                        authoritative_selector: explicit,
                        test_candidate: candidate,
                        test_obligation: obligation.map(|item| IntentTestObligationLink {
                            requirement_id: item.requirement_id.clone(),
                            scenario_name: item.scenario_name.clone(),
                            suggested_selector: item.suggested_selector.clone(),
                            selector_authority: item.selector_authority.clone(),
                        }),
                        required_evidence,
                    });
                }
                if scenarios.is_empty() {
                    gaps.push(gap(
                        "scenario-unmapped",
                        "error",
                        Some(&entry.node.node_id),
                        Some(&binding.requirement_id),
                        Some(&spec_path),
                        "no contract scenario maps to the requirement",
                    ));
                }
                scenarios.sort_by(|left, right| left.name.cmp(&right.name));
                spec_links.push(IntentSpecLink {
                    path: spec_path,
                    risk: doc.meta.risk,
                    scenarios,
                });
            }
            spec_links.sort_by(|left, right| left.path.cmp(&right.path));
            let worktree = worktrees.as_ref().and_then(|manifest| {
                manifest
                    .entries
                    .iter()
                    .find(|item| item.requirement_id == binding.requirement_id)
                    .map(|item| PlannedWorktreeLink {
                        path: item.path.clone(),
                        branch: item.branch.clone(),
                        base_branch: item.base_branch.clone(),
                        batch: item.batch,
                    })
            });
            if worktree.is_none() {
                gaps.push(gap(
                    "worktree-unobserved",
                    "warning",
                    Some(&entry.node.node_id),
                    Some(&binding.requirement_id),
                    None,
                    "requirement has no planned worktree entry",
                ));
            }
            links.push(IntentBindingLink {
                requirement_id: binding.requirement_id.clone(),
                work_unit_id: binding.work_unit_id.clone(),
                provider: binding.provider.clone(),
                graph_fingerprint: binding.graph_fingerprint.clone(),
                specs: spec_links,
                worktree,
            });
        }
        links.sort_by(|left, right| {
            left.requirement_id
                .cmp(&right.requirement_id)
                .then_with(|| left.work_unit_id.cmp(&right.work_unit_id))
        });
        affected.push(IntentAffectedNode {
            impact: entry,
            links,
        });
    }
    affected.sort_by(|left, right| {
        left.impact
            .distance
            .cmp(&right.impact.distance)
            .then_with(|| left.impact.node.node_id.cmp(&right.impact.node.node_id))
    });
    sort_gaps(&mut gaps);

    Ok(IntentImpactReport {
        schema: INTENT_IMPACT_SCHEMA_ID.into(),
        provider: impact.provider,
        graph_fingerprint: Some(impact.graph_fingerprint),
        input: impact.input,
        affected,
        truncated: impact.truncated,
        gaps,
        provider_diagnostics: impact.diagnostics,
        observed_vcs,
    })
}

fn read_bindings(path: &Path) -> Result<crate::spec_knowledge::CodeBindings, String> {
    if !path.is_file() {
        return Ok(crate::spec_knowledge::CodeBindings {
            schema: crate::spec_knowledge::CODE_BINDINGS_SCHEMA_ID.into(),
            entries: Vec::new(),
        });
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn read_worktrees(
    path: Option<&Path>,
) -> Result<Option<crate::spec_knowledge::WorktreeManifest>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn contract_scenarios(doc: &crate::spec_core::SpecDocument) -> Vec<&crate::spec_core::Scenario> {
    doc.sections
        .iter()
        .filter_map(|section| match section {
            crate::spec_core::Section::AcceptanceCriteria { scenarios, .. } => Some(scenarios),
            _ => None,
        })
        .flatten()
        .collect()
}

fn gap(
    code: &str,
    severity: &str,
    node_id: Option<&str>,
    requirement_id: Option<&str>,
    spec_path: Option<&Path>,
    message: impl Into<String>,
) -> IntentImpactGap {
    IntentImpactGap {
        code: code.into(),
        severity: severity.into(),
        node_id: node_id.map(str::to_string),
        requirement_id: requirement_id.map(str::to_string),
        spec_path: spec_path.map(Path::to_path_buf),
        message: message.into(),
    }
}

fn sort_gaps(gaps: &mut Vec<IntentImpactGap>) {
    gaps.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.node_id.cmp(&right.node_id))
            .then_with(|| left.requirement_id.cmp(&right.requirement_id))
            .then_with(|| left.spec_path.cmp(&right.spec_path))
            .then_with(|| left.message.cmp(&right.message))
    });
    gaps.dedup();
}

fn enum_name(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

pub fn render_intent_impact(report: &IntentImpactReport) -> Result<String, String> {
    serde_json::to_string_pretty(report)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| error.to_string())
}

pub fn build_affected_execution_bundle(
    report: &IntentImpactReport,
    knowledge: &Path,
    project_root: &Path,
    quality_profile: &[crate::spec_knowledge::QualityProvider],
) -> Result<AffectedExecutionBundle, String> {
    let intent_bytes = serde_json::to_vec(report).map_err(|error| error.to_string())?;
    let risk = report
        .affected
        .iter()
        .flat_map(|node| &node.links)
        .flat_map(|link| &link.specs)
        .filter_map(|spec| spec.risk.as_deref())
        .min_by_key(|risk| risk_rank(risk))
        .map(str::to_string);
    let risk_label = risk.as_deref().unwrap_or("unclassified");
    let qa_class = risk
        .as_deref()
        .map(crate::spec_qa::QaClass::try_parse)
        .transpose()?
        .unwrap_or(crate::spec_qa::QaClass::B);

    let mut required_evidence = report
        .affected
        .iter()
        .flat_map(|node| &node.links)
        .flat_map(|link| &link.specs)
        .flat_map(|spec| &spec.scenarios)
        .flat_map(|scenario| scenario.required_evidence.iter().cloned())
        .collect::<Vec<_>>();
    required_evidence.extend(
        crate::spec_qa::required_evidence_for(qa_class)
            .iter()
            .map(enum_name),
    );
    required_evidence.sort();
    required_evidence.dedup();
    required_evidence.sort();
    required_evidence.dedup();

    let mut fast_checks = quality_profile
        .iter()
        .filter(|provider| match qa_class {
            crate::spec_qa::QaClass::A => matches!(
                provider.role,
                crate::spec_knowledge::ProviderRole::Diagnostic
                    | crate::spec_knowledge::ProviderRole::Transformation
            ),
            crate::spec_qa::QaClass::B => {
                provider.required
                    && provider.role == crate::spec_knowledge::ProviderRole::Diagnostic
            }
            crate::spec_qa::QaClass::C => false,
        })
        .map(|provider| ExecutionSelection {
            id: provider.id.clone(),
            kind: "quality-provider".into(),
            role: Some(enum_name(&provider.role)),
            reason: format!(
                "{} provider selected as an early check for risk {risk_label}",
                enum_name(&provider.role)
            ),
        })
        .collect::<Vec<_>>();

    let mut acceptance_gates = Vec::new();
    let mut authoritative_tests = Vec::new();
    let mut test_candidates = Vec::new();
    for link in report.affected.iter().flat_map(|node| &node.links) {
        for spec in &link.specs {
            acceptance_gates.push(ExecutionSelection {
                id: spec.path.to_string_lossy().replace('\\', "/"),
                kind: "lifecycle".into(),
                role: Some("verification".into()),
                reason: format!(
                    "Task Contract lifecycle is required for {} at risk {risk_label}",
                    link.requirement_id
                ),
            });
            if matches!(
                qa_class,
                crate::spec_qa::QaClass::A | crate::spec_qa::QaClass::B
            ) {
                acceptance_gates.push(ExecutionSelection {
                    id: spec.path.to_string_lossy().replace('\\', "/"),
                    kind: "trace".into(),
                    role: Some("verification".into()),
                    reason: format!(
                        "stored lifecycle trace is required for {} at risk {risk_label}",
                        link.requirement_id
                    ),
                });
            }
            if qa_class == crate::spec_qa::QaClass::A {
                acceptance_gates.push(ExecutionSelection {
                    id: spec.path.to_string_lossy().replace('\\', "/"),
                    kind: "adversarial-review".into(),
                    role: Some("verification".into()),
                    reason: format!(
                        "independent adversarial review is required for {} at risk A",
                        link.requirement_id
                    ),
                });
            }
            for scenario in &spec.scenarios {
                if let Some(selector) = &scenario.authoritative_selector {
                    authoritative_tests.push(SelectedTest {
                        requirement_id: link.requirement_id.clone(),
                        spec_path: spec.path.clone(),
                        scenario: scenario.name.clone(),
                        selector: selector.clone(),
                        reason: "explicit Task Contract selector".into(),
                    });
                    if qa_class == crate::spec_qa::QaClass::A {
                        acceptance_gates.push(ExecutionSelection {
                            id: selector.clone(),
                            kind: "test".into(),
                            role: Some("verification".into()),
                            reason: format!(
                                "targeted explicit selector for risk A scenario `{}` in {}",
                                scenario.name,
                                spec.path.display()
                            ),
                        });
                    }
                }
                if let Some(candidate) = &scenario.test_candidate {
                    test_candidates.push(SelectedTest {
                        requirement_id: link.requirement_id.clone(),
                        spec_path: spec.path.clone(),
                        scenario: scenario.name.clone(),
                        selector: candidate.clone(),
                        reason: "heuristic test-obligation candidate; not acceptance evidence"
                            .into(),
                    });
                }
            }
        }
    }
    acceptance_gates.extend(
        quality_profile
            .iter()
            .filter(|provider| {
                qa_class != crate::spec_qa::QaClass::C
                    && provider.required
                    && provider.role == crate::spec_knowledge::ProviderRole::Verification
            })
            .map(|provider| ExecutionSelection {
                id: provider.id.clone(),
                kind: "quality-provider".into(),
                role: Some(enum_name(&provider.role)),
                reason: format!("required verification provider selected for risk {risk_label}"),
            }),
    );

    let affected_paths = report
        .affected
        .iter()
        .map(|node| node.impact.node.file.clone())
        .collect::<BTreeSet<_>>();
    let mut guidance = Vec::new();
    let mut required_skills = Vec::new();
    for doc in crate::spec_knowledge::collect_knowledge(knowledge) {
        if doc.meta.kind != crate::spec_knowledge::KnowledgeKind::Guidance {
            continue;
        }
        let skills = crate::spec_knowledge::skills(&doc);
        if skills.is_empty() {
            continue;
        }
        let matched_paths = affected_paths
            .iter()
            .filter(|path| crate::spec_knowledge::applies_to_path(&doc, path))
            .cloned()
            .collect::<Vec<_>>();
        if matched_paths.is_empty() {
            continue;
        }
        required_skills.extend(skills.iter().cloned());
        guidance.push(GuidanceSelection {
            guidance_id: doc.meta.id.clone(),
            source: doc.source_path.clone(),
            matched_paths: matched_paths.clone(),
            skills,
            reason: format!("guidance applies to {}", matched_paths.join(", ")),
        });
    }
    required_skills.sort();
    required_skills.dedup();

    let mut gaps = report.gaps.clone();
    let mut skill_receipts = Vec::new();
    for skill in &required_skills {
        let Some(source) = safe_skill_source(project_root, skill) else {
            gaps.push(gap(
                "skill-unresolved",
                "error",
                None,
                None,
                None,
                format!("skill id `{skill}` is not a safe relative identifier"),
            ));
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&source) else {
            gaps.push(gap(
                "skill-unresolved",
                "warning",
                None,
                None,
                None,
                format!(
                    "required skill `{skill}` is not installed at {}",
                    source.display()
                ),
            ));
            continue;
        };
        let version = content
            .lines()
            .find_map(|line| line.split("**Version:**").nth(1))
            .map(|value| {
                value
                    .split('|')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|| "unknown".into());
        skill_receipts.push(crate::spec_knowledge::SkillReceipt {
            id: skill.clone(),
            version,
            source: source.to_string_lossy().replace('\\', "/"),
            content_hash: crate::spec_knowledge::blake3_hex(content.as_bytes()),
        });
    }

    fast_checks.sort_by(|left, right| left.id.cmp(&right.id));
    fast_checks.dedup();
    acceptance_gates.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.id.cmp(&right.id))
    });
    acceptance_gates.dedup();
    sort_tests(&mut authoritative_tests);
    sort_tests(&mut test_candidates);
    guidance.sort_by(|left, right| left.guidance_id.cmp(&right.guidance_id));
    skill_receipts.sort_by(|left, right| left.id.cmp(&right.id));
    sort_gaps(&mut gaps);
    let selected_provider_ids = fast_checks
        .iter()
        .chain(&acceptance_gates)
        .filter(|selection| selection.kind == "quality-provider")
        .map(|selection| selection.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut selected_quality_profile = quality_profile
        .iter()
        .filter(|provider| selected_provider_ids.contains(provider.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    selected_quality_profile.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(AffectedExecutionBundle {
        schema: AFFECTED_EXECUTION_BUNDLE_SCHEMA_ID.into(),
        intent_impact_digest: crate::spec_knowledge::blake3_hex(&intent_bytes),
        risk,
        required_evidence,
        quality_profile: selected_quality_profile,
        fast_checks,
        acceptance_gates,
        authoritative_tests,
        test_candidates,
        guidance,
        required_skills,
        skill_receipts,
        gaps,
    })
}

fn risk_rank(risk: &str) -> u8 {
    match risk {
        "A" => 0,
        "B" => 1,
        "C" => 2,
        _ => 3,
    }
}

fn safe_skill_source(project_root: &Path, skill: &str) -> Option<PathBuf> {
    let relative = Path::new(skill);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(project_root.join("skills").join(relative).join("SKILL.md"))
}

fn sort_tests(tests: &mut Vec<SelectedTest>) {
    tests.sort_by(|left, right| {
        left.requirement_id
            .cmp(&right.requirement_id)
            .then_with(|| left.spec_path.cmp(&right.spec_path))
            .then_with(|| left.scenario.cmp(&right.scenario))
            .then_with(|| left.selector.cmp(&right.selector))
    });
    tests.dedup();
}

pub fn render_affected_execution_bundle(
    bundle: &AffectedExecutionBundle,
) -> Result<String, String> {
    serde_json::to_string_pretty(bundle)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::{
        CODE_IMPACT_SCHEMA_ID, ImpactCodeNode, ImpactPath, ProviderImpactDiagnostic,
    };
    use crate::vcs::{VcsContext, VcsType};
    use std::fs;

    fn fixture(name: &str, selector: bool) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "agent-spec-intent-impact-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(root.join("specs")).unwrap();
        fs::create_dir_all(root.join(".agent-spec")).unwrap();
        fs::write(
            root.join("knowledge/requirements/req-demo.md"),
            "---\nkind: requirement\nid: REQ-IMPACT-DEMO\ntitle: \"Impact Demo\"\nstatus: accepted\nliveness: auto\n---\n\n## Problem\np\n\n## Requirements\n\n[REQ-IMPACT-DEMO-ONE] The system MUST preserve one chain.\n\n## Scenarios\n\nScenario: Full chain\n  Given one node\n  When impact is joined\n  Then the chain is visible\n",
        )
        .unwrap();
        let test_line = if selector {
            "  Test: test_full_chain\n"
        } else {
            ""
        };
        fs::write(
            root.join("specs/task-impact.spec.md"),
            format!(
                "spec: task\nname: \"Impact Demo\"\nsatisfies: [REQ-IMPACT-DEMO]\nrisk: A\n---\n\n## Intent\nx\n\n## Boundaries\n\n### Allowed Changes\n- src/**\n\n## Completion Criteria\n\nScenario: Full chain\n{test_line}  Given one node\n  When impact is joined\n  Then the chain is visible\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join(".agent-spec/code-bindings.json"),
            "{\n  \"schema\": \"agent-spec/intent-compiler/code-bindings-v1\",\n  \"entries\": [{\n    \"requirement_id\": \"REQ-IMPACT-DEMO\",\n    \"work_unit_id\": \"WU-REQ-IMPACT-DEMO\",\n    \"provider\": \"rust-atlas\",\n    \"graph_fingerprint\": \"graph-1\",\n    \"targets\": [{\n      \"node_id\": \"node-1\", \"kind\": \"fn\", \"file\": \"src/lib.rs\", \"provenance\": \"syn\"\n    }]\n  }]\n}\n",
        )
        .unwrap();
        fs::write(
            root.join(".agent-spec/worktrees.json"),
            "{\n  \"version\": 1,\n  \"entries\": [{\n    \"work_unit_id\": \"WU-REQ-IMPACT-DEMO\",\n    \"requirement_id\": \"REQ-IMPACT-DEMO\",\n    \"batch\": 1,\n    \"base_branch\": \"main\",\n    \"branch\": \"feat/wu-impact-demo\",\n    \"path\": \"../worktrees/wu-impact-demo\",\n    \"spec_path\": \"specs/task-impact.spec.md\",\n    \"depends_on\": []\n  }],\n  \"diagnostics\": []\n}\n",
        )
        .unwrap();
        root
    }

    fn impact(node_id: &str, file: &str, truncated: bool) -> ProviderImpact {
        let node = ImpactCodeNode {
            node_id: node_id.into(),
            symbol: "demo::run".into(),
            kind: "fn".into(),
            file: file.into(),
            line_start: 1,
            line_end: 3,
            provenance: "syn".into(),
        };
        ProviderImpact {
            schema: CODE_IMPACT_SCHEMA_ID.into(),
            provider: "rust-atlas".into(),
            graph_fingerprint: "graph-1".into(),
            input: CodeImpactInput::Paths {
                paths: vec![file.into()],
            },
            entries: vec![ProviderImpactEntry {
                node: node.clone(),
                distance: 0,
                path: ImpactPath {
                    nodes: vec![node],
                    hops: Vec::new(),
                    confidence: "exact".into(),
                },
            }],
            truncated,
            diagnostics: Vec::new(),
        }
    }

    fn build(
        root: &Path,
        impact: Result<ProviderImpact, ProviderImpactError>,
    ) -> IntentImpactReport {
        build_intent_impact(
            "rust-atlas",
            CodeImpactInput::Paths {
                paths: vec!["src/lib.rs".into()],
            },
            impact,
            &root.join("knowledge"),
            &root.join("specs"),
            &root.join(".agent-spec/code-bindings.json"),
            Some(&root.join(".agent-spec/worktrees.json")),
            Some(VcsContext {
                vcs_type: VcsType::Git,
                change_ref: "abc1234".into(),
                operation_ref: None,
            }),
        )
        .unwrap()
    }

    #[test]
    fn test_intent_aware_affected_projects_full_chain_deterministically() {
        let root = fixture("full", true);
        let left = build(&root, Ok(impact("node-1", "src/lib.rs", false)));
        let right = build(&root, Ok(impact("node-1", "src/lib.rs", false)));
        assert_eq!(
            serde_json::to_vec(&left).unwrap(),
            serde_json::to_vec(&right).unwrap()
        );
        let link = &left.affected[0].links[0];
        assert_eq!(link.requirement_id, "REQ-IMPACT-DEMO");
        assert_eq!(link.work_unit_id, "WU-REQ-IMPACT-DEMO");
        assert_eq!(
            link.specs[0].scenarios[0].authoritative_selector.as_deref(),
            Some("test_full_chain")
        );
        assert!(link.specs[0].scenarios[0].test_obligation.is_some());
        assert_eq!(
            link.worktree.as_ref().unwrap().branch,
            "feat/wu-impact-demo"
        );
        assert_eq!(left.observed_vcs.as_ref().unwrap().change_ref, "abc1234");
        assert!(left.gaps.is_empty(), "unexpected gaps: {:?}", left.gaps);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn intent_impact_schema_matches_tagged_input_serialization() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/intent-compiler/schemas/intent-impact-v1.schema.json"
        ))
        .unwrap();
        let variants = schema["properties"]["input"]["oneOf"].as_array().unwrap();
        let expected = [
            serde_json::json!({"kind": "paths", "paths": ["src/lib.rs"]}),
            serde_json::json!({"kind": "symbol", "symbol": "crate::run"}),
        ];
        for (variant, serialized) in variants.iter().zip(&expected) {
            assert!(
                variant["required"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|field| field == "kind")
            );
            assert_eq!(variant["properties"]["kind"]["const"], serialized["kind"]);
        }
        assert_eq!(
            serde_json::to_value(CodeImpactInput::Paths {
                paths: vec!["src/lib.rs".into()]
            })
            .unwrap(),
            expected[0]
        );
        assert_eq!(
            serde_json::to_value(CodeImpactInput::Symbol {
                symbol: "crate::run".into()
            })
            .unwrap(),
            expected[1]
        );
    }

    #[test]
    fn test_intent_aware_affected_reports_unbound_node_gap() {
        let root = fixture("unbound", true);
        let report = build(&root, Ok(impact("node-unbound", "src/lib.rs", false)));
        assert_eq!(report.affected.len(), 1);
        assert!(report.affected[0].links.is_empty());
        assert!(
            report
                .gaps
                .iter()
                .any(|gap| gap.code == "affected-node-unbound")
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_intent_aware_affected_reports_missing_explicit_selector() {
        let root = fixture("selector", false);
        let report = build(&root, Ok(impact("node-1", "src/lib.rs", false)));
        let scenario = &report.affected[0].links[0].specs[0].scenarios[0];
        assert_eq!(scenario.authoritative_selector, None);
        assert_eq!(scenario.test_candidate.as_deref(), Some("test_full_chain"));
        assert!(report.gaps.iter().any(|gap| gap.code == "selector-missing"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_intent_aware_affected_preserves_provider_and_truncation_gaps() {
        let root = fixture("authority", true);
        let unavailable = build(
            &root,
            Err(ProviderImpactError {
                code: "provider-unavailable".into(),
                provider: "rust-atlas".into(),
                message: "graph missing".into(),
            }),
        );
        assert!(
            unavailable
                .gaps
                .iter()
                .any(|gap| gap.code == "provider-unavailable")
        );

        let mut truncated = impact("node-1", "src/lib.rs", true);
        truncated.diagnostics.push(ProviderImpactDiagnostic {
            code: "atlas-impact-truncated".into(),
            message: "bounded".into(),
        });
        let report = build(&root, Ok(truncated));
        assert!(report.gaps.iter().any(|gap| gap.code == "impact-truncated"));

        let bindings = root.join(".agent-spec/code-bindings.json");
        let text = fs::read_to_string(&bindings)
            .unwrap()
            .replace("graph-1", "graph-old");
        fs::write(&bindings, text).unwrap();
        let mismatch = build(&root, Ok(impact("node-1", "src/lib.rs", false)));
        assert!(
            mismatch
                .gaps
                .iter()
                .any(|gap| gap.code == "binding-fingerprint-mismatch")
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_intent_aware_affected_never_infers_tests_from_filenames() {
        let root = fixture("filename", false);
        let report = build(&root, Ok(impact("node-1", "tests/feature_test.rs", false)));
        assert_eq!(report.affected[0].impact.node.file, "tests/feature_test.rs");
        assert!(
            report
                .affected
                .iter()
                .flat_map(|node| &node.links)
                .flat_map(|link| &link.specs)
                .flat_map(|spec| &spec.scenarios)
                .all(|scenario| {
                    scenario.authoritative_selector.is_none()
                        && scenario
                            .test_candidate
                            .as_deref()
                            .is_none_or(|candidate| !candidate.contains("feature_test"))
                })
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn intent_aware_affected_reports_missing_worktree_vcs_and_obligation_context() {
        let root = fixture("missing-context", true);
        let requirement = root.join("knowledge/requirements/req-demo.md");
        let content = fs::read_to_string(&requirement).unwrap().replace(
            "Scenario: Full chain",
            "Scenario: Requirement-only scenario",
        );
        fs::write(&requirement, content).unwrap();

        let report = build_intent_impact(
            "rust-atlas",
            CodeImpactInput::Paths {
                paths: vec!["src/lib.rs".into()],
            },
            Ok(impact("node-1", "src/lib.rs", false)),
            &root.join("knowledge"),
            &root.join("specs"),
            &root.join(".agent-spec/code-bindings.json"),
            None,
            None,
        )
        .unwrap();

        let scenario = &report.affected[0].links[0].specs[0].scenarios[0];
        assert!(scenario.test_obligation.is_none());
        for code in [
            "obligation-unmapped",
            "worktree-manifest-missing",
            "worktree-unobserved",
            "vcs-unobserved",
        ] {
            assert!(
                report.gaps.iter().any(|gap| gap.code == code),
                "missing {code}: {:?}",
                report.gaps
            );
        }
        fs::remove_dir_all(root).ok();
    }

    fn add_guidance(root: &Path) {
        fs::create_dir_all(root.join("knowledge/guidance")).unwrap();
        fs::create_dir_all(root.join("skills/rust-review")).unwrap();
        fs::write(
            root.join("skills/rust-review/SKILL.md"),
            "# Rust Review\n\n**Version:** 1.2.3 | local\n",
        )
        .unwrap();
        fs::write(
            root.join("knowledge/guidance/g-rust.md"),
            "---\nkind: guidance\nid: G-RUST-IMPACT\ntitle: \"Rust Impact Guidance\"\nliveness: n/a\ntags: [rust]\n---\n\n## Scope\nRust source.\n\n## Instructions\nReview affected paths.\n\n## Applies To\nsrc/**\n\n## Skills\n- rust-review\n- missing-skill\n",
        )
        .unwrap();
        fs::write(
            root.join("knowledge/guidance/g-docs.md"),
            "---\nkind: guidance\nid: G-DOCS\ntitle: \"Docs Guidance\"\nliveness: n/a\ntags: [docs]\n---\n\n## Scope\nDocs.\n\n## Instructions\nReview docs.\n\n## Applies To\ndocs/**\n\n## Skills\n- docs-only\n",
        )
        .unwrap();
    }

    fn execution_bundle(root: &Path, selector: bool) -> AffectedExecutionBundle {
        let report = build(root, Ok(impact("node-1", "src/lib.rs", false)));
        if selector {
            assert!(
                report.affected[0].links[0].specs[0].scenarios[0]
                    .authoritative_selector
                    .is_some()
            );
        }
        build_affected_execution_bundle(
            &report,
            &root.join("knowledge"),
            root,
            &crate::spec_knowledge::baseline_quality_profile(),
        )
        .unwrap()
    }

    #[test]
    fn test_affected_execution_bundle_selects_checks_and_gates_with_reasons() {
        let root = fixture("bundle-selection", true);
        add_guidance(&root);
        let bundle = execution_bundle(&root, true);
        assert_eq!(bundle.risk.as_deref(), Some("A"));
        assert!(
            bundle
                .fast_checks
                .iter()
                .any(|item| item.id == "cargo-clippy" && item.reason.contains("risk A"))
        );
        assert!(
            bundle
                .acceptance_gates
                .iter()
                .all(|item| !item.reason.is_empty() && item.role.is_some())
        );
        assert!(bundle.quality_profile.iter().any(|provider| {
            provider.id == "cargo-clippy"
                && provider.executable == "cargo"
                && provider.args == ["clippy", "--all-targets", "--quiet"]
                && provider.timeout_secs > 0
                && provider.max_output_bytes > 0
        }));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn affected_execution_bundle_applies_distinct_risk_policies() {
        let root = fixture("bundle-risk", true);
        add_guidance(&root);
        let spec_path = root.join("specs/task-impact.spec.md");
        let original = fs::read_to_string(&spec_path).unwrap();

        let bundle_for = |risk: &str| {
            fs::write(
                &spec_path,
                original.replace("risk: A", &format!("risk: {risk}")),
            )
            .unwrap();
            execution_bundle(&root, true)
        };
        let class_a = bundle_for("A");
        let class_b = bundle_for("B");
        let class_c = bundle_for("C");

        assert!(
            class_a
                .acceptance_gates
                .iter()
                .any(|gate| gate.kind == "adversarial-review")
        );
        assert!(
            class_a
                .acceptance_gates
                .iter()
                .any(|gate| gate.kind == "test")
        );
        assert!(
            class_b
                .acceptance_gates
                .iter()
                .any(|gate| gate.kind == "trace")
        );
        assert!(
            class_b
                .acceptance_gates
                .iter()
                .all(|gate| gate.kind != "adversarial-review" && gate.kind != "test")
        );
        assert!(class_c.fast_checks.is_empty());
        assert!(
            class_c
                .acceptance_gates
                .iter()
                .all(|gate| gate.kind == "lifecycle")
        );
        assert_ne!(class_a.required_evidence, class_b.required_evidence);
        assert_ne!(class_b.required_evidence, class_c.required_evidence);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_affected_execution_bundle_uses_only_authoritative_tests() {
        let root = fixture("bundle-tests", true);
        add_guidance(&root);
        let bundle = execution_bundle(&root, true);
        assert_eq!(bundle.authoritative_tests.len(), 1);
        assert_eq!(bundle.authoritative_tests[0].selector, "test_full_chain");
        assert!(bundle.test_candidates.is_empty());
        assert!(
            bundle
                .acceptance_gates
                .iter()
                .any(|gate| gate.kind == "test" && gate.id == "test_full_chain")
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_affected_execution_bundle_keeps_heuristic_test_candidates_separate() {
        let root = fixture("bundle-candidates", false);
        add_guidance(&root);
        let bundle = execution_bundle(&root, false);
        assert!(bundle.authoritative_tests.is_empty());
        assert_eq!(bundle.test_candidates[0].selector, "test_full_chain");
        assert!(
            bundle
                .acceptance_gates
                .iter()
                .all(|gate| gate.id != "test_full_chain")
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_affected_execution_bundle_preserves_missing_selector_and_provider_gaps() {
        let root = fixture("bundle-gaps", false);
        add_guidance(&root);
        let mut report = build(&root, Ok(impact("node-1", "src/lib.rs", false)));
        report.gaps.push(gap(
            "provider-unavailable",
            "error",
            None,
            None,
            None,
            "missing provider",
        ));
        let bundle = build_affected_execution_bundle(
            &report,
            &root.join("knowledge"),
            &root,
            &crate::spec_knowledge::baseline_quality_profile(),
        )
        .unwrap();
        assert!(bundle.gaps.iter().any(|gap| gap.code == "selector-missing"));
        assert!(
            bundle
                .gaps
                .iter()
                .any(|gap| gap.code == "provider-unavailable")
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_affected_execution_bundle_scopes_guidance_to_affected_paths() {
        let root = fixture("bundle-guidance", true);
        add_guidance(&root);
        let bundle = execution_bundle(&root, true);
        assert_eq!(bundle.guidance.len(), 1);
        assert_eq!(bundle.guidance[0].guidance_id, "G-RUST-IMPACT");
        assert_eq!(bundle.guidance[0].matched_paths, vec!["src/lib.rs"]);
        assert!(!bundle.required_skills.contains(&"docs-only".into()));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_affected_execution_bundle_keeps_skill_receipts_separate_from_evidence() {
        let root = fixture("bundle-receipts", true);
        add_guidance(&root);
        let bundle = execution_bundle(&root, true);
        assert_eq!(bundle.skill_receipts.len(), 1);
        assert_eq!(bundle.skill_receipts[0].id, "rust-review");
        assert_eq!(bundle.skill_receipts[0].content_hash.len(), 64);
        assert!(bundle.gaps.iter().any(|gap| gap.code == "skill-unresolved"));
        let gates = serde_json::to_string(&bundle.acceptance_gates).unwrap();
        assert!(!gates.contains("rust-review"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_affected_execution_bundle_is_byte_stable() {
        let root = fixture("bundle-stable", true);
        add_guidance(&root);
        let left = execution_bundle(&root, true);
        let right = execution_bundle(&root, true);
        assert_eq!(
            serde_json::to_vec(&left).unwrap(),
            serde_json::to_vec(&right).unwrap()
        );
        fs::remove_dir_all(root).ok();
    }
}
