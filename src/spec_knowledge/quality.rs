//! Boundary 4: typed quality provider roles, normalized outcomes, and the
//! Execution Bundle — one complete, verifiable execution context per work
//! unit (work unit + contract + code bindings + quality profile + required
//! skills + fast checks + acceptance gates).
//!
//! Hard rules: a required provider that is unavailable never counts as
//! passing evidence; provider configuration is executable + argv arrays
//! (no interpolated shell strings); skill receipts are provenance, never
//! acceptance evidence — deterministic tool output and lifecycle verdicts
//! remain the only acceptance evidence.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::provenance::blake3_hex;

pub const EXECUTION_BUNDLE_SCHEMA_ID: &str = "agent-spec/intent-compiler/execution-bundle-v1";

/// Typed provider roles in a quality profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRole {
    CodeIntelligence,
    Diagnostic,
    Verification,
    Transformation,
    AgentGuidance,
}

/// Normalized quality outcome. `Skip` carries the authorizing policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "outcome")]
pub enum QualityOutcome {
    Pass,
    Fail,
    Unavailable,
    Error,
    Skip { policy: String },
}

/// One provider in the quality profile: executable + argv arrays with
/// explicit cwd, timeout, and output limits — never a shell string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityProvider {
    pub id: String,
    pub role: ProviderRole,
    pub required: bool,
    pub executable: String,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub timeout_secs: u64,
    pub max_output_bytes: u64,
}

impl QualityProvider {
    /// Capability detection: the executable resolves to a runnable file.
    pub fn is_available(&self) -> bool {
        let exe = Path::new(&self.executable);
        if exe.components().count() > 1 {
            return exe.is_file();
        }
        std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths).any(|dir| dir.join(&self.executable).is_file())
            })
            .unwrap_or(false)
    }

    /// Normalize this provider's outcome given its availability and an
    /// optional raw exit status from execution.
    pub fn normalize(&self, exit_ok: Option<bool>) -> QualityOutcome {
        if !self.is_available() {
            return QualityOutcome::Unavailable;
        }
        match exit_ok {
            Some(true) => QualityOutcome::Pass,
            Some(false) => QualityOutcome::Fail,
            None => QualityOutcome::Error,
        }
    }
}

/// Gate evaluation for one provider outcome. A required provider that is
/// unavailable (or errored, or failed) never passes; an optional provider
/// passes the gate only through `pass` or an authorized skip.
pub fn outcome_passes_gate(outcome: &QualityOutcome, required: bool) -> bool {
    match outcome {
        QualityOutcome::Pass => true,
        QualityOutcome::Skip { .. } => !required,
        QualityOutcome::Fail | QualityOutcome::Error => false,
        QualityOutcome::Unavailable => false,
    }
}

/// Skill receipt: provenance of an instruction file handed to the agent.
/// Never acceptance evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillReceipt {
    pub id: String,
    pub version: String,
    pub source: String,
    pub content_hash: String,
}

/// Acceptance evidence evaluation: only passing verification outcomes count.
/// Receipts are deliberately not an input — passing them anywhere else is a
/// type error by construction.
pub fn acceptance_from_outcomes(verification_outcomes: &[QualityOutcome]) -> bool {
    !verification_outcomes.is_empty()
        && verification_outcomes
            .iter()
            .all(|outcome| matches!(outcome, QualityOutcome::Pass))
}

/// Baseline profile: cargo test (verification), clippy (diagnostic),
/// rustfmt (transformation). Array-based, no shell strings.
pub fn baseline_quality_profile() -> Vec<QualityProvider> {
    let provider = |id: &str, role, required, args: &[&str]| QualityProvider {
        id: id.to_string(),
        role,
        required,
        executable: "cargo".to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        cwd: None,
        timeout_secs: 600,
        max_output_bytes: 1_048_576,
    };
    vec![
        provider(
            "cargo-test",
            ProviderRole::Verification,
            true,
            &["test", "--quiet"],
        ),
        provider(
            "cargo-clippy",
            ProviderRole::Diagnostic,
            true,
            &["clippy", "--all-targets", "--quiet"],
        ),
        provider(
            "rustfmt-check",
            ProviderRole::Transformation,
            false,
            &["fmt", "--check"],
        ),
    ]
}

/// Acceptance gate entry in the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceGate {
    pub kind: String,
    pub reference: String,
}

/// Contract embedded by path + digest + content so the bundle is
/// self-contained and pinnable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleContract {
    pub path: String,
    pub blake3: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionBundle {
    pub schema: String,
    pub work_unit: crate::spec_knowledge::WorkUnit,
    pub contracts: Vec<BundleContract>,
    /// Code-binding entries for this work unit (from a fresh bind run);
    /// empty when no bindings artifact exists.
    pub code_bindings: Vec<serde_json::Value>,
    pub quality_profile: Vec<QualityProvider>,
    pub required_skills: Vec<String>,
    pub skill_receipts: Vec<SkillReceipt>,
    /// Fast pre-checks (diagnostic/transformation roles) an agent runs early.
    pub fast_checks: Vec<String>,
    pub acceptance_gates: Vec<AcceptanceGate>,
}

/// Build the Execution Bundle for one work unit id.
pub fn build_execution_bundle(
    knowledge: &Path,
    specs: &Path,
    bindings_path: &Path,
    unit_id: &str,
) -> Result<ExecutionBundle, String> {
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    let units = crate::spec_knowledge::build_work_units(&graph);
    let wanted = unit_id.trim().to_ascii_uppercase();
    let Some(unit) = units.units.iter().find(|unit| unit.id == wanted) else {
        return Err(format!(
            "unknown work unit `{wanted}`; known units: {}",
            units
                .units
                .iter()
                .map(|unit| unit.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };

    // Contracts: every spec satisfying the unit's requirement, embedded.
    let index = crate::spec_knowledge::build_satisfies_index(specs);
    let mut contracts = Vec::new();
    if let Some(paths) = index.get(&unit.requirement_id) {
        let mut sorted = paths.clone();
        sorted.sort();
        for path in sorted {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            contracts.push(BundleContract {
                path: path.to_string_lossy().replace('\\', "/"),
                blake3: blake3_hex(content.as_bytes()),
                content,
            });
        }
    }

    // Code bindings for this unit, when a bindings artifact exists.
    let code_bindings = read_unit_bindings(bindings_path, &wanted)?;

    let quality_profile = baseline_quality_profile();
    let fast_checks = quality_profile
        .iter()
        .filter(|provider| {
            matches!(
                provider.role,
                ProviderRole::Diagnostic | ProviderRole::Transformation
            )
        })
        .map(|provider| provider.id.clone())
        .collect();

    // Required skills from guidance docs; receipts hash resolvable files.
    let (required_skills, skill_receipts) = collect_skills(knowledge);

    let mut acceptance_gates = contracts
        .iter()
        .map(|contract| AcceptanceGate {
            kind: "lifecycle".to_string(),
            reference: contract.path.clone(),
        })
        .collect::<Vec<_>>();
    acceptance_gates.extend(
        quality_profile
            .iter()
            .filter(|provider| provider.required && provider.role == ProviderRole::Verification)
            .map(|provider| AcceptanceGate {
                kind: "quality-provider".to_string(),
                reference: provider.id.clone(),
            }),
    );

    Ok(ExecutionBundle {
        schema: EXECUTION_BUNDLE_SCHEMA_ID.to_string(),
        work_unit: unit.clone(),
        contracts,
        code_bindings,
        quality_profile,
        required_skills,
        skill_receipts,
        fast_checks,
        acceptance_gates,
    })
}

fn read_unit_bindings(
    bindings_path: &Path,
    unit_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    if !bindings_path.is_file() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(bindings_path)
        .map_err(|e| format!("cannot read {}: {e}", bindings_path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        format!(
            "{} is not valid bindings JSON: {e}",
            bindings_path.display()
        )
    })?;
    Ok(value
        .get("entries")
        .and_then(|entries| entries.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry.get("work_unit_id").and_then(|id| id.as_str()) == Some(unit_id)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default())
}

fn collect_skills(knowledge: &Path) -> (Vec<String>, Vec<SkillReceipt>) {
    let collection = crate::spec_knowledge::collect_knowledge(knowledge);
    let mut skills: Vec<String> = collection
        .iter()
        .flat_map(crate::spec_knowledge::skills)
        .collect();
    skills.sort();
    skills.dedup();
    let receipts = skills
        .iter()
        .filter_map(|skill| {
            let source = PathBuf::from("skills").join(skill).join("SKILL.md");
            let content = std::fs::read_to_string(&source).ok()?;
            let version = content
                .lines()
                .find_map(|line| line.split("**Version:**").nth(1))
                .map(|rest| {
                    rest.split('|')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_string()
                })
                .unwrap_or_else(|| "unknown".to_string());
            Some(SkillReceipt {
                id: skill.clone(),
                version,
                source: source.to_string_lossy().replace('\\', "/"),
                content_hash: blake3_hex(content.as_bytes()),
            })
        })
        .collect();
    (skills, receipts)
}

/// Render the bundle as pretty JSON with a trailing newline.
pub fn render_execution_bundle(bundle: &ExecutionBundle) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(bundle).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn make_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-q.md"),
            "---\nkind: requirement\nid: REQ-QUALITY-DEMO\ntitle: \"Quality Demo\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Quality Demo\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-QUALITY-DEMO-ONE] The system MUST hold one obligation.\n\n## Scenarios\n\nScenario: holds\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-q.spec.md"),
            "spec: task\nname: \"Quality Demo Contract\"\nsatisfies: [REQ-QUALITY-DEMO]\n---\n\n## Intent\n\nx\n\n## Boundaries\n\n### Allowed Changes\n- src/**\n\n## Completion Criteria\n\nScenario: holds\n  Test: test_holds\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n",
        )
        .unwrap();
        fs::write(
            dir.join("bindings.json"),
            "{\n  \"schema\": \"agent-spec/intent-compiler/code-bindings-v1\",\n  \"entries\": [\n    {\n      \"requirement_id\": \"REQ-QUALITY-DEMO\",\n      \"work_unit_id\": \"WU-REQ-QUALITY-DEMO\",\n      \"provider\": \"rust-atlas\",\n      \"graph_fingerprint\": \"0000000000000000000000000000000000000000000000000000000000000000\",\n      \"targets\": []\n    }\n  ]\n}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_execution_bundle_packages_context() {
        let dir = make_tree("bundle-ok");
        let bundle = build_execution_bundle(
            &dir.join("knowledge"),
            &dir.join("specs"),
            &dir.join("bindings.json"),
            "WU-REQ-QUALITY-DEMO",
        )
        .unwrap();

        assert_eq!(bundle.schema, EXECUTION_BUNDLE_SCHEMA_ID);
        assert_eq!(bundle.work_unit.id, "WU-REQ-QUALITY-DEMO");
        assert_eq!(bundle.contracts.len(), 1);
        assert!(
            bundle.contracts[0]
                .content
                .contains("Quality Demo Contract")
        );
        assert_eq!(bundle.contracts[0].blake3.len(), 64);
        assert_eq!(bundle.code_bindings.len(), 1, "unit bindings must embed");
        assert_eq!(bundle.quality_profile.len(), 3);
        assert!(
            bundle
                .quality_profile
                .iter()
                .all(|provider| !provider.executable.contains(' ')),
            "argv arrays only — no interpolated shell strings"
        );
        assert!(bundle.fast_checks.contains(&"cargo-clippy".to_string()));
        assert!(
            bundle
                .acceptance_gates
                .iter()
                .any(|gate| gate.kind == "lifecycle"),
            "lifecycle stays an acceptance gate"
        );
        assert!(
            bundle
                .acceptance_gates
                .iter()
                .any(|gate| gate.kind == "quality-provider" && gate.reference == "cargo-test")
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_required_provider_unavailable_is_not_pass() {
        let provider = QualityProvider {
            id: "ghost-verifier".into(),
            role: ProviderRole::Verification,
            required: true,
            executable: "definitely-not-a-real-binary-a7f3".into(),
            args: vec!["--version".into()],
            cwd: None,
            timeout_secs: 10,
            max_output_bytes: 1024,
        };
        let outcome = provider.normalize(Some(true));
        assert_eq!(outcome, QualityOutcome::Unavailable);
        assert!(
            !outcome_passes_gate(&outcome, provider.required),
            "a required unavailable provider must fail the gate"
        );
        assert!(
            !acceptance_from_outcomes(&[outcome]),
            "unavailability must never count as passing evidence"
        );
    }

    #[test]
    fn test_skill_receipt_is_not_acceptance_evidence() {
        let receipt = SkillReceipt {
            id: "agent-spec-tool-first".into(),
            version: "3.5.0".into(),
            source: "skills/agent-spec-tool-first/SKILL.md".into(),
            content_hash: "ab".repeat(32),
        };
        assert!(!receipt.content_hash.is_empty());
        // Acceptance is a function of verification outcomes only; with no
        // passing outcomes, receipts alone produce no acceptance.
        assert!(!acceptance_from_outcomes(&[]));
        assert!(!acceptance_from_outcomes(&[QualityOutcome::Skip {
            policy: "policy-x".into()
        }]));
    }

    #[test]
    fn test_bundle_rejects_unknown_work_unit() {
        let dir = make_tree("bundle-unknown");
        let err = build_execution_bundle(
            &dir.join("knowledge"),
            &dir.join("specs"),
            &dir.join("bindings.json"),
            "WU-GHOST",
        )
        .unwrap_err();
        assert!(err.contains("WU-GHOST"), "{err}");
        fs::remove_dir_all(dir).ok();
    }
}
