use std::path::{Path, PathBuf};

use crate::spec_core::{BoundaryCategory, Evidence, ScenarioResult, SpecResult, StepVerdict, Verdict};

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
        let (allowed, forbidden) = collect_path_boundaries(&ctx.resolved_spec.task.sections);
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
        }])
    }
}

fn collect_path_boundaries(sections: &[crate::spec_core::Section]) -> (Vec<String>, Vec<String>) {
    let mut allowed = Vec::new();
    let mut forbidden = Vec::new();

    for section in sections {
        if let crate::spec_core::Section::Boundaries { items, .. } = section {
            for item in items {
                if !looks_like_path_boundary(&item.text) {
                    continue;
                }

                match item.category {
                    BoundaryCategory::Allow => allowed.push(normalize_pattern(&item.text)),
                    BoundaryCategory::Deny | BoundaryCategory::General => {
                        forbidden.push(normalize_pattern(&item.text))
                    }
                }
            }
        }
    }

    (allowed, forbidden)
}

fn looks_like_path_boundary(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('*')
        || trimmed.ends_with(".rs")
        || trimmed.ends_with(".ts")
        || trimmed.ends_with(".js")
        || trimmed.ends_with(".py")
        || trimmed.ends_with(".spec")
}

fn normalize_change_paths(paths: &[PathBuf], workspace_root: Option<&Path>) -> Vec<String> {
    let mut changes = Vec::new();

    for path in paths {
        let normalized = normalize_path(path, workspace_root);
        if !normalized.is_empty() && !changes.iter().any(|item| item == &normalized) {
            changes.push(normalized);
        }
    }

    changes
}

fn normalize_path(path: &Path, workspace_root: Option<&Path>) -> String {
    let candidate = workspace_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);

    candidate
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

fn normalize_pattern(pattern: &str) -> String {
    pattern
        .trim()
        .trim_matches('`')
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let path_segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    match_segments(&pattern_segments, &path_segments)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }

    if pattern[0] == "**" {
        return (0..=path.len()).any(|index| match_segments(&pattern[1..], &path[index..]));
    }

    if path.is_empty() {
        return false;
    }

    segment_matches(pattern[0], path[0]) && match_segments(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return pattern == segment;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let mut cursor = 0usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if index == 0 && anchored_start {
            if !segment[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            continue;
        }

        if let Some(found) = segment[cursor..].find(part) {
            cursor += found + part.len();
        } else {
            return false;
        }
    }

    if anchored_end && let Some(last_part) = parts.iter().rev().find(|part| !part.is_empty()) {
        return segment.ends_with(last_part);
    }

    true
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
                return Some(current);
            }
            if !current.pop() {
                break;
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::spec_core::{ResolvedSpec, Scenario, SpecLevel, SpecMeta, SpecResult, Verdict};

    use super::{BoundariesVerifier, path_matches_pattern};
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

    #[test]
    fn matches_double_star_path_patterns() {
        assert!(path_matches_pattern(
            "crates/spec-parser/**",
            "crates/spec-parser/src/parser.rs"
        ));
        assert!(path_matches_pattern("specs/**", "specs/task.spec"));
        assert!(!path_matches_pattern(
            "crates/spec-parser/**",
            "crates/spec-gateway/src/lib.rs"
        ));
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
                },
                sections: vec![],
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
