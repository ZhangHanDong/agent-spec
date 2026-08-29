use std::path::{Path, PathBuf};

use crate::spec_core::{
    Evidence, ScenarioResult, SpecResult, StepVerdict, Verdict, collect_boundary_patterns,
    normalize_change_path, path_matches_pattern,
};

use super::{VerificationContext, Verifier};

/// Mechanical verifier for file-level task boundaries.
///
/// This verifier only runs when callers provide an explicit `change_paths` set.
/// It does not infer diffs from VCS state.
pub struct BoundariesVerifier;

impl Verifier for BoundariesVerifier {
    fn name(&self) -> &str {
        "boundaries"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        let (allowed, forbidden) = collect_boundary_patterns(&ctx.resolved_spec.task.sections);
        if ctx.change_paths.is_empty() || (allowed.is_empty() && forbidden.is_empty()) {
            return Ok(Vec::new());
        }

        let workspace_root =
            find_workspace_root(&ctx.code_paths).or_else(|| find_workspace_root(&ctx.change_paths));
        let changes = normalize_change_paths(&ctx.change_paths, workspace_root.as_deref());
        if changes.is_empty() {
            return Ok(Vec::new());
        }

        let mut step_results = Vec::new();
        let mut evidence = Vec::new();
        let mut has_failure = false;

        for change in changes {
            if let Some(pattern) = forbidden
                .iter()
                .find(|pattern| path_matches_pattern(pattern, &change))
            {
                has_failure = true;
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Fail,
                    reason: format!("matches forbidden boundary `{pattern}`"),
                });
                evidence.push(Evidence::PatternMatch {
                    pattern: pattern.clone(),
                    matched: true,
                    locations: vec![change],
                });
                continue;
            }

            if let Some(pattern) = allowed
                .iter()
                .find(|pattern| path_matches_pattern(pattern, &change))
            {
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Pass,
                    reason: format!("matches allowed boundary `{pattern}`"),
                });
                evidence.push(Evidence::PatternMatch {
                    pattern: pattern.clone(),
                    matched: true,
                    locations: vec![change],
                });
                continue;
            }

            if !allowed.is_empty() {
                has_failure = true;
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Fail,
                    reason: "not covered by any allowed boundary".into(),
                });
                evidence.push(Evidence::PatternMatch {
                    pattern: "<allowed-boundaries>".into(),
                    matched: false,
                    locations: vec![change],
                });
            } else {
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Pass,
                    reason: "no allow-list declared; change accepted because it is not forbidden"
                        .into(),
                });
            }
        }

        Ok(vec![ScenarioResult {
            scenario_name: "[boundaries] explicit change set respects declared paths".into(),
            verdict: if has_failure {
                Verdict::Fail
            } else {
                Verdict::Pass
            },
            step_results,
            evidence,
            duration_ms: 0,
            provenance: None,
        }])
    }
}

fn normalize_change_paths(paths: &[PathBuf], workspace_root: Option<&Path>) -> Vec<String> {
    let mut changes = Vec::new();

    for path in paths {
        let normalized = normalize_change_path(path, workspace_root);
        if !normalized.is_empty() && !changes.iter().any(|item| item == &normalized) {
            changes.push(normalized);
        }
    }

    changes
}

fn find_workspace_root(paths: &[PathBuf]) -> Option<PathBuf> {
    for path in paths {
        let mut current = if path.is_file() {
            path.parent()?.to_path_buf()
        } else {
            path.clone()
        };

        loop {
            if current.join("Cargo.toml").is_file() {
                return Some(current.canonicalize().unwrap_or(current));
            }
            if !current.pop() {
                break;
            }
        }
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use crate::spec_core::{ResolvedSpec, Scenario, SpecLevel, SpecMeta, SpecResult, Verdict};

    use super::BoundariesVerifier;
    use crate::spec_verify::{AiMode, VerificationContext, Verifier};

    fn make_resolved_spec(input: &str) -> SpecResult<ResolvedSpec> {
        let doc = crate::spec_parser::parse_spec_from_str(input)?;
        Ok(ResolvedSpec {
            task: doc,
            inherited_constraints: Vec::new(),
            inherited_decisions: Vec::new(),
            all_scenarios: Vec::<Scenario>::new(),
        })
    }

    fn run(spec: &str, changes: &[&str]) -> Vec<crate::spec_core::ScenarioResult> {
        let resolved = make_resolved_spec(spec).unwrap();
        BoundariesVerifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: changes.iter().map(PathBuf::from).collect(),
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap()
    }

    #[test]
    fn test_boundary_allow_accepts_bare_root_files_and_any_extension() {
        let results = run(
            r#"spec: task
name: "Root files"
---

## Boundaries

### Allowed Changes
- Cargo.toml
- `CLAUDE.md`
- LICENSE
- Makefile
- tools/x/gate.json
"#,
            &[
                "Cargo.toml",
                "CLAUDE.md",
                "LICENSE",
                "Makefile",
                "tools/x/gate.json",
            ],
        );
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].verdict,
            Verdict::Pass,
            "{:?}",
            results[0].step_results
        );
        assert_eq!(results[0].step_results.len(), 5);
        assert!(
            results[0]
                .step_results
                .iter()
                .all(|s| s.verdict == Verdict::Pass)
        );
    }

    #[test]
    fn test_boundary_allow_still_rejects_undeclared_root_file() {
        let results = run(
            r#"spec: task
name: "Root files"
---

## Boundaries

### Allowed Changes
- Cargo.toml
"#,
            &["Cargo.lock"],
        );
        assert_eq!(results[0].verdict, Verdict::Fail);
        let step = &results[0].step_results[0];
        assert_eq!(step.step_text, "Cargo.lock");
        assert_eq!(step.verdict, Verdict::Fail);
        assert_eq!(step.reason, "not covered by any allowed boundary");
    }

    #[test]
    fn test_boundary_deny_collects_only_path_tokens() {
        let doc = crate::spec_parser::parse_spec_from_str(
            r#"spec: task
name: "Deny"
---

## Boundaries

### Allowed Changes
- src/**

### Forbidden
- Do not modify `src/sliding_sync.rs`
- 不新增依赖
- src/sliding_sync.rs
"#,
        )
        .unwrap();
        let (allowed, forbidden) = crate::spec_core::collect_boundary_patterns(&doc.sections);
        assert_eq!(allowed, vec!["src/**".to_string()]);
        assert_eq!(forbidden, vec!["src/sliding_sync.rs".to_string()]);
    }

    #[test]
    fn test_boundary_deny_prose_mentioning_path_does_not_fail_change() {
        let results = run(
            r#"spec: task
name: "Deny prose"
---

## Boundaries

### Forbidden
- Do not break the JSON shape in `src/spec_report/mod.rs`
"#,
            &["src/spec_report/mod.rs"],
        );
        // No path pattern at all → verifier has nothing to enforce.
        assert!(
            results.is_empty()
                || results[0]
                    .step_results
                    .iter()
                    .all(|s| s.verdict == Verdict::Pass),
            "{results:?}"
        );
    }

    #[test]
    fn test_boundary_allow_annotated_entry_matches_after_note_strip() {
        let results = run(
            r#"spec: task
name: "Notes"
---

## Boundaries

### Allowed Changes
- `Cargo.toml` — dev-dep only
- docs/foo (copy).md
"#,
            &["Cargo.toml", "docs/foo (copy).md"],
        );
        assert_eq!(
            results[0].verdict,
            Verdict::Pass,
            "{:?}",
            results[0].step_results
        );
    }

    #[test]
    fn test_boundaries_verifier_accepts_changes_within_allowed_paths() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "边界"
---

## 边界

### 允许修改
- crates/spec-parser/**

### 禁止做
- crates/spec-gateway/**
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("crates/spec-parser/src/parser.rs")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Pass);
    }

    #[test]
    fn test_boundaries_verifier_accepts_root_markdown_boundaries() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "Docs"
---

## Boundaries

### Allowed Changes
- README.md
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("README.md")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Pass);
    }

    #[test]
    fn test_boundaries_verifier_rejects_change_outside_allowed_paths() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "边界"
---

## Boundaries

### Allowed Changes
- crates/spec-parser/**
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("crates/spec-gateway/src/lifecycle.rs")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Fail);
        assert!(
            results[0].step_results[0]
                .reason
                .contains("not covered by any allowed boundary")
        );
    }

    #[test]
    fn test_boundaries_verifier_rejects_change_matching_forbidden_boundary() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "边界"
---

## 边界

### 允许修改
- crates/spec-gateway/**

### 禁止做
- crates/spec-gateway/src/lib.rs
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("crates/spec-gateway/src/lib.rs")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Fail);
        assert!(
            results[0].step_results[0]
                .reason
                .contains("matches forbidden boundary")
        );
    }

    #[test]
    fn verifier_skips_when_no_explicit_change_paths_are_provided() {
        let resolved = ResolvedSpec {
            task: crate::spec_core::SpecDocument {
                meta: SpecMeta {
                    level: SpecLevel::Task,
                    name: "边界".into(),
                    inherits: None,
                    lang: vec![],
                    tags: vec![],
                    depends: vec![],
                    estimate: None,
                    capability: None,
                    satisfies: vec![],
                    risk: None,
                },
                sections: vec![],
                lint_acks: vec![],
                source_path: PathBuf::new(),
            },
            inherited_constraints: Vec::new(),
            inherited_decisions: Vec::new(),
            all_scenarios: Vec::new(),
        };

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: Vec::new(),
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert!(results.is_empty());
    }
}
