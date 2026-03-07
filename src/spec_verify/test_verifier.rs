use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::spec_core::{
    Evidence, Scenario, ScenarioResult, SpecError, SpecResult, StepVerdict, TestSelector, Verdict,
};

use super::{VerificationContext, Verifier};

pub struct TestVerifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingSource {
    ExplicitScenarioSelector,
    LegacyComment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestBinding {
    selector: TestSelector,
    source: BindingSource,
}

impl Verifier for TestVerifier {
    fn name(&self) -> &str {
        "test"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        let Some(workspace_root) = find_workspace_root(&ctx.code_paths) else {
            return Ok(Vec::new());
        };

        let legacy_bindings = collect_legacy_comment_bindings(&ctx.code_paths)?;
        let mut results = Vec::new();

        for scenario in &ctx.resolved_spec.all_scenarios {
            let Some(binding) = resolve_test_binding(scenario, &legacy_bindings) else {
                continue;
            };

            let started = Instant::now();
            let command_args = build_cargo_test_args(&binding.selector);
            let output = Command::new("cargo")
                .args(&command_args)
                .current_dir(&workspace_root)
                .output()
                .map_err(|err| {
                    SpecError::Verification(format!("failed to run cargo test: {err}"))
                })?;
            let duration_ms = started.elapsed().as_millis() as u64;

            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let combined = if stderr.trim().is_empty() {
                stdout.clone()
            } else if stdout.trim().is_empty() {
                stderr.clone()
            } else {
                format!("{stdout}\n{stderr}")
            };

            let verdict = if output.status.success() {
                Verdict::Pass
            } else {
                Verdict::Fail
            };
            let selector_label = binding.selector.label();
            let reason = if output.status.success() {
                match binding.source {
                    BindingSource::ExplicitScenarioSelector => {
                        format!("covered by explicit test `{selector_label}`")
                    }
                    BindingSource::LegacyComment => {
                        format!("covered by legacy @spec test `{selector_label}`")
                    }
                }
            } else {
                match binding.source {
                    BindingSource::ExplicitScenarioSelector => {
                        format!("explicit test `{selector_label}` failed")
                    }
                    BindingSource::LegacyComment => {
                        format!("legacy @spec test `{selector_label}` failed")
                    }
                }
            };

            let step_results = scenario
                .steps
                .iter()
                .map(|step| StepVerdict {
                    step_text: step.text.clone(),
                    verdict,
                    reason: reason.clone(),
                })
                .collect();

            results.push(ScenarioResult {
                scenario_name: scenario.name.clone(),
                verdict,
                step_results,
                evidence: vec![Evidence::TestOutput {
                    test_name: selector_label,
                    stdout: combined,
                    passed: output.status.success(),
                }],
                duration_ms,
            });
        }

        Ok(results)
    }
}

fn find_workspace_root(code_paths: &[PathBuf]) -> Option<PathBuf> {
    for path in code_paths {
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

fn collect_legacy_comment_bindings(code_paths: &[PathBuf]) -> SpecResult<HashMap<String, String>> {
    let mut bindings = HashMap::new();
    let mut files = Vec::new();

    for path in code_paths {
        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            collect_rust_files(path, &mut files);
        }
    }

    for file in files {
        let content = fs::read_to_string(&file)?;
        for (scenario, test_name) in extract_bindings(&content) {
            bindings.entry(scenario).or_insert(test_name);
        }
    }

    Ok(bindings)
}

fn resolve_test_binding(
    scenario: &Scenario,
    legacy_bindings: &HashMap<String, String>,
) -> Option<TestBinding> {
    if let Some(selector) = scenario.test_selector.as_ref() {
        return Some(TestBinding {
            selector: selector.clone(),
            source: BindingSource::ExplicitScenarioSelector,
        });
    }

    legacy_bindings
        .get(&scenario.name)
        .map(|selector| TestBinding {
            selector: TestSelector::filter_only(selector.clone()),
            source: BindingSource::LegacyComment,
        })
}

fn build_cargo_test_args(selector: &TestSelector) -> Vec<String> {
    let mut args = vec!["test".to_string(), "-q".to_string()];
    if let Some(package) = &selector.package {
        args.push("-p".to_string());
        args.push(package.clone());
    }
    args.push(selector.filter.clone());
    args
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str())
                && (name.starts_with('.') || name == "target")
            {
                continue;
            }
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn extract_bindings(source: &str) -> Vec<(String, String)> {
    let mut bindings = Vec::new();
    let mut pending_specs = Vec::new();
    let mut saw_test_attr = false;

    for line in source.lines() {
        let trimmed = line.trim();

        if let Some(spec_name) = trimmed
            .strip_prefix("// @spec:")
            .or_else(|| trimmed.strip_prefix("/// @spec:"))
        {
            pending_specs.push(spec_name.trim().to_string());
            continue;
        }

        if trimmed.starts_with("#[test]") || trimmed.starts_with("#[tokio::test") {
            saw_test_attr = true;
            continue;
        }

        if saw_test_attr && trimmed.starts_with("fn ") {
            if let Some(test_name) = extract_fn_name(trimmed) {
                for spec_name in pending_specs.drain(..) {
                    bindings.push((spec_name, test_name.clone()));
                }
            }
            saw_test_attr = false;
            continue;
        }

        if !trimmed.starts_with("#[") && !trimmed.is_empty() {
            pending_specs.clear();
            saw_test_attr = false;
        }
    }

    bindings
}

fn extract_fn_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("fn ")?;
    let name = rest.split('(').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::spec_core::{Scenario, Span, TestSelector};

    use super::{BindingSource, build_cargo_test_args, extract_bindings, resolve_test_binding};

    #[test]
    fn extracts_spec_bindings_from_test_comments() {
        let source = r#"
// @spec: 场景一
// @spec: 场景二
#[test]
fn test_example() {}
"#;

        let bindings = extract_bindings(source);
        assert_eq!(bindings.len(), 2);
        assert_eq!(
            bindings[0],
            ("场景一".to_string(), "test_example".to_string())
        );
        assert_eq!(
            bindings[1],
            ("场景二".to_string(), "test_example".to_string())
        );
    }

    #[test]
    fn ignores_comments_not_followed_by_a_test() {
        let source = r#"
// @spec: 场景一
fn helper() {}
"#;

        assert!(extract_bindings(source).is_empty());
    }

    #[test]
    fn test_explicit_scenario_selector_takes_precedence_over_legacy_comment_binding() {
        let scenario = Scenario {
            name: "场景一".into(),
            steps: Vec::new(),
            test_selector: Some(TestSelector::filter_only(
                "test_explicit_scenario_selector_takes_precedence_over_legacy_comment_binding",
            )),
            tags: Vec::new(),
            span: Span::default(),
        };
        let legacy = HashMap::from([("场景一".to_string(), "legacy_test_name".to_string())]);

        let binding = resolve_test_binding(&scenario, &legacy).unwrap();
        assert_eq!(
            binding.selector,
            TestSelector::filter_only(
                "test_explicit_scenario_selector_takes_precedence_over_legacy_comment_binding"
            )
        );
        assert_eq!(binding.source, BindingSource::ExplicitScenarioSelector);
    }

    #[test]
    fn test_legacy_comment_binding_is_used_when_no_explicit_selector_exists() {
        let scenario = Scenario {
            name: "场景一".into(),
            steps: Vec::new(),
            test_selector: None,
            tags: Vec::new(),
            span: Span::default(),
        };
        let legacy = HashMap::from([(
            "场景一".to_string(),
            "test_legacy_comment_binding_is_used_when_no_explicit_selector_exists".to_string(),
        )]);

        let binding = resolve_test_binding(&scenario, &legacy).unwrap();
        assert_eq!(
            binding.selector,
            TestSelector::filter_only(
                "test_legacy_comment_binding_is_used_when_no_explicit_selector_exists"
            )
        );
        assert_eq!(binding.source, BindingSource::LegacyComment);
    }

    #[test]
    fn test_build_cargo_test_command_with_package_selector() {
        let selector = TestSelector {
            package: Some("spec-parser".into()),
            filter: "test_parse_structured_test_selector_block".into(),
        };

        let args = build_cargo_test_args(&selector);
        assert_eq!(
            args,
            vec![
                "test".to_string(),
                "-q".to_string(),
                "-p".to_string(),
                "spec-parser".to_string(),
                "test_parse_structured_test_selector_block".to_string(),
            ]
        );
    }
}
