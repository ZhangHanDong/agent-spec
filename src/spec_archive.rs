use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArchivePlan {
    pub version: u32,
    pub entries: Vec<ArchiveEntry>,
    pub diagnostics: Vec<ArchiveDiagnostic>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub spec_name: String,
    pub source_path: PathBuf,
    pub archive_path: PathBuf,
    pub satisfies: Vec<String>,
    pub depends: Vec<String>,
    pub scenarios: Vec<String>,
    pub test_selectors: Vec<String>,
    pub tags: Vec<String>,
    pub last_verification: Option<ArchiveVerificationStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArchiveVerificationStatus {
    pub passing: bool,
    pub summary: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArchiveDiagnostic {
    pub code: String,
    pub severity: String,
    pub path: PathBuf,
    pub message: String,
}

pub fn build_archive_plan(spec_dir: &Path, archive_dir: &Path) -> ArchivePlan {
    build_archive_plan_internal(spec_dir, archive_dir, None)
}

pub fn build_archive_plan_with_history(
    spec_dir: &Path,
    archive_dir: &Path,
    run_log_root: &Path,
) -> ArchivePlan {
    build_archive_plan_internal(spec_dir, archive_dir, Some(run_log_root))
}

fn build_archive_plan_internal(
    spec_dir: &Path,
    archive_dir: &Path,
    run_log_root: Option<&Path>,
) -> ArchivePlan {
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    let (spec_paths, mut collection_diagnostics) = spec_files(spec_dir);
    diagnostics.append(&mut collection_diagnostics);
    for path in spec_paths {
        let Ok(doc) = crate::spec_parser::parse_spec(&path) else {
            diagnostics.push(ArchiveDiagnostic {
                code: "archive-parse-error".into(),
                severity: "warning".into(),
                path,
                message: "spec could not be parsed and was not selected for archive".into(),
            });
            continue;
        };
        let is_completed = doc
            .meta
            .tags
            .iter()
            .any(|tag| tag == "done" || tag == "completed");
        if !is_completed {
            continue;
        }
        let verification_lookup = run_log_root.map(|root| {
            latest_verification(
                root,
                &doc.meta.name,
                &path,
                &crate::spec_wiki::fingerprint_file(&path).unwrap_or_default(),
            )
        });
        let last_verification = verification_lookup
            .as_ref()
            .and_then(|lookup| match lookup {
                VerificationLookup::Matched(status) => Some(status.clone()),
                VerificationLookup::Missing | VerificationLookup::Stale => None,
            });
        if run_log_root.is_some() {
            match verification_lookup.as_ref() {
                Some(VerificationLookup::Matched(status)) if status.passing => {}
                Some(VerificationLookup::Matched(status)) => {
                    diagnostics.push(ArchiveDiagnostic {
                        code: "archive-lifecycle-not-passing".into(),
                        severity: "error".into(),
                        path,
                        message: format!(
                            "latest lifecycle evidence is not passing: {}",
                            status.summary
                        ),
                    });
                    continue;
                }
                Some(VerificationLookup::Stale) => {
                    diagnostics.push(ArchiveDiagnostic {
                        code: "archive-lifecycle-stale".into(),
                        severity: "error".into(),
                        path,
                        message: "lifecycle evidence exists for the spec name but does not match the current path and content fingerprint".into(),
                    });
                    continue;
                }
                Some(VerificationLookup::Missing) | None => {
                    diagnostics.push(ArchiveDiagnostic {
                        code: "archive-lifecycle-missing".into(),
                        severity: "error".into(),
                        path,
                        message:
                            "completed spec has no lifecycle run log; archive requires passing evidence"
                                .into(),
                    });
                    continue;
                }
            }
        }
        let relative = path.strip_prefix(spec_dir).unwrap_or(path.as_path());
        entries.push(ArchiveEntry {
            spec_name: doc.meta.name.clone(),
            source_path: path.clone(),
            archive_path: archive_dir.join(relative),
            satisfies: doc.meta.satisfies.clone(),
            depends: doc.meta.depends.clone(),
            scenarios: collect_scenario_names(&doc),
            test_selectors: collect_test_selectors(&doc),
            tags: doc.meta.tags.clone(),
            last_verification,
        });
    }
    entries.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    diagnostics.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.code.cmp(&b.code)));
    ArchivePlan {
        version: 1,
        entries,
        diagnostics,
    }
}

pub fn render_archive_summary(plan: &ArchivePlan) -> String {
    let mut out = String::new();
    out.push_str("# Spec Archive Summary\n\n");
    out.push_str("## Archived Specs\n\n");
    for entry in &plan.entries {
        out.push_str(&format!("### {}\n\n", entry.spec_name));
        out.push_str(&format!("- Source: `{}`\n", entry.source_path.display()));
        out.push_str(&format!("- Archive: `{}`\n", entry.archive_path.display()));
        out.push_str(&format!(
            "- Satisfies: `{}`\n",
            entry.satisfies.join("`, `")
        ));
        out.push_str(&format!("- Depends: `{}`\n", entry.depends.join("`, `")));
        out.push_str(&format!("- Tags: `{}`\n", entry.tags.join("`, `")));
        match &entry.last_verification {
            Some(status) => out.push_str(&format!(
                "- Last verification: {} at {} ({})\n",
                if status.passing { "pass" } else { "non-pass" },
                status.timestamp,
                status.summary
            )),
            None => out.push_str("- Last verification: unknown\n"),
        }
        out.push_str("- Scenarios:\n");
        for scenario in &entry.scenarios {
            out.push_str(&format!("  - {scenario}\n"));
        }
        out.push_str("- Test selectors:\n");
        for selector in &entry.test_selectors {
            out.push_str(&format!("  - {selector}\n"));
        }
        out.push('\n');
    }
    if plan.entries.is_empty() {
        out.push_str("_No specs selected for archive._\n\n");
    }
    if !plan.diagnostics.is_empty() {
        out.push_str("## Archive Diagnostics\n\n");
        for diagnostic in &plan.diagnostics {
            out.push_str(&format!(
                "- {} `{}`: {} ({})\n",
                diagnostic.severity,
                diagnostic.path.display(),
                diagnostic.message,
                diagnostic.code
            ));
        }
        out.push('\n');
    }
    out
}

pub fn apply_archive_plan(plan: &ArchivePlan) -> Result<(), Box<dyn std::error::Error>> {
    if plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return Err("archive plan contains blocking diagnostics".into());
    }
    let mut targets = std::collections::BTreeSet::new();
    for entry in &plan.entries {
        if !entry.source_path.is_file() {
            return Err(format!(
                "archive source is missing or not a file: {}",
                entry.source_path.display()
            )
            .into());
        }
        if entry.archive_path.exists() {
            return Err(format!(
                "archive target already exists: {}",
                entry.archive_path.display()
            )
            .into());
        }
        if entry.source_path == entry.archive_path {
            return Err(format!(
                "archive source and target are identical: {}",
                entry.source_path.display()
            )
            .into());
        }
        if !targets.insert(entry.archive_path.clone()) {
            return Err(format!(
                "archive target is duplicated in plan: {}",
                entry.archive_path.display()
            )
            .into());
        }
    }

    let mut moved = Vec::<(&Path, &Path)>::new();
    for entry in &plan.entries {
        let result = match entry.archive_path.parent() {
            Some(parent) => std::fs::create_dir_all(parent)
                .and_then(|_| std::fs::rename(&entry.source_path, &entry.archive_path)),
            None => std::fs::rename(&entry.source_path, &entry.archive_path),
        };
        if let Err(error) = result {
            for (source, target) in moved.into_iter().rev() {
                std::fs::rename(target, source).ok();
            }
            return Err(error.into());
        }
        moved.push((&entry.source_path, &entry.archive_path));
    }
    Ok(())
}

fn collect_scenario_names(doc: &crate::spec_core::SpecDocument) -> Vec<String> {
    let mut out = Vec::new();
    for section in &doc.sections {
        if let crate::spec_core::Section::AcceptanceCriteria { scenarios, .. } = section {
            out.extend(scenarios.iter().map(|scenario| scenario.name.clone()));
        }
    }
    out.sort();
    out
}

fn collect_test_selectors(doc: &crate::spec_core::SpecDocument) -> Vec<String> {
    let mut out = Vec::new();
    for section in &doc.sections {
        if let crate::spec_core::Section::AcceptanceCriteria { scenarios, .. } = section {
            for scenario in scenarios {
                if let Some(test) = &scenario.test_selector {
                    out.push(test.filter.clone());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn spec_files(dir: &Path) -> (Vec<PathBuf>, Vec<ArchiveDiagnostic>) {
    let mut out = Vec::new();
    let mut diagnostics = Vec::new();
    collect_spec_files(dir, &mut out, &mut diagnostics);
    out.sort();
    (out, diagnostics)
}

fn collect_spec_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<ArchiveDiagnostic>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            diagnostics.push(ArchiveDiagnostic {
                code: "archive-entry-type-unreadable".into(),
                severity: "error".into(),
                path,
                message: "archive source entry type could not be read".into(),
            });
            continue;
        };
        if file_type.is_symlink() {
            diagnostics.push(ArchiveDiagnostic {
                code: "archive-symlink-rejected".into(),
                severity: "error".into(),
                path,
                message: "archive source traversal rejects symbolic links".into(),
            });
        } else if file_type.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, "archive" | "_archive" | ".agent-spec"))
            {
                continue;
            }
            collect_spec_files(&path, out, diagnostics);
        } else if file_type.is_file() && is_spec_file(&path) {
            out.push(path);
        }
    }
}

#[derive(Debug, Deserialize)]
struct ArchiveRunLogEntry {
    spec_name: String,
    #[serde(default)]
    spec_path: PathBuf,
    #[serde(default)]
    spec_fingerprint: String,
    passing: bool,
    summary: String,
    timestamp: u64,
}

enum VerificationLookup {
    Missing,
    Stale,
    Matched(ArchiveVerificationStatus),
}

fn latest_verification(
    run_log_root: &Path,
    spec_name: &str,
    spec_path: &Path,
    spec_fingerprint: &str,
) -> VerificationLookup {
    let runs_dir = runs_dir(run_log_root);
    let Ok(entries) = std::fs::read_dir(&runs_dir) else {
        return VerificationLookup::Missing;
    };
    let logs = entries
        .flatten()
        .filter_map(|entry| {
            let content = std::fs::read_to_string(entry.path()).ok()?;
            let log: ArchiveRunLogEntry = serde_json::from_str(&content).ok()?;
            (log.spec_name == spec_name).then_some(log)
        })
        .collect::<Vec<_>>();
    if logs.is_empty() {
        return VerificationLookup::Missing;
    }
    let canonical_spec_path = spec_path
        .canonicalize()
        .unwrap_or_else(|_| spec_path.to_path_buf());
    let Some(log) = logs
        .into_iter()
        .filter(|log| {
            let log_path = log
                .spec_path
                .canonicalize()
                .unwrap_or_else(|_| log.spec_path.clone());
            !log.spec_path.as_os_str().is_empty()
                && log_path == canonical_spec_path
                && !log.spec_fingerprint.is_empty()
                && log.spec_fingerprint == spec_fingerprint
        })
        .max_by_key(|log| log.timestamp)
    else {
        return VerificationLookup::Stale;
    };
    VerificationLookup::Matched(ArchiveVerificationStatus {
        passing: log.passing,
        summary: log.summary,
        timestamp: log.timestamp,
    })
}

fn runs_dir(run_log_root: &Path) -> PathBuf {
    if run_log_root.file_name().and_then(|name| name.to_str()) == Some("runs")
        && run_log_root
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some(".agent-spec")
    {
        return run_log_root.to_path_buf();
    }
    run_log_root.join(".agent-spec/runs")
}

fn is_spec_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".spec") || name.ends_with(".spec.md"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_archive_run_log(
        runs: &Path,
        file_name: &str,
        spec: &Path,
        passing: bool,
        summary: &str,
        timestamp: u64,
    ) {
        let body = serde_json::json!({
            "spec_name": "Done",
            "spec_path": spec.canonicalize().unwrap(),
            "spec_fingerprint": crate::spec_wiki::fingerprint_file(spec).unwrap(),
            "passing": passing,
            "summary": summary,
            "timestamp": timestamp,
        });
        fs::write(runs.join(file_name), serde_json::to_string(&body).unwrap()).unwrap();
    }

    #[test]
    fn test_archive_plan_selects_completed_specs_only() {
        let dir = make_temp_dir("archive-plan-selects-completed");
        let specs = dir.join("specs");
        let archive = dir.join(".agent-spec/archive/specs");
        fs::create_dir_all(&specs).unwrap();
        fs::write(
            specs.join("task-done.spec.md"),
            "spec: task\nname: \"Done\"\ntags: [done]\nsatisfies: [REQ-DONE]\n---\n## Intent\nDone.\n## Completion Criteria\nScenario: Done\n  Test: test_done\n  Given done\n  When checked\n  Then stdout contains \"done\"\n",
        )
        .unwrap();
        fs::write(
            specs.join("task-active.spec.md"),
            "spec: task\nname: \"Active\"\ntags: [active]\nsatisfies: [REQ-ACTIVE]\n---\n## Intent\nActive.\n## Completion Criteria\nScenario: Active\n  Test: test_active\n  Given active\n  When checked\n  Then stdout contains \"active\"\n",
        )
        .unwrap();

        let plan = build_archive_plan(&specs, &archive);

        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].spec_name, "Done");
        assert_eq!(plan.entries[0].satisfies, vec!["REQ-DONE"]);
        assert!(plan.entries[0].archive_path.ends_with("task-done.spec.md"));

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_archive_summary_preserves_contract_evidence() {
        let plan = ArchivePlan {
            version: 1,
            entries: vec![ArchiveEntry {
                spec_name: "Done".into(),
                source_path: PathBuf::from("specs/task-done.spec.md"),
                archive_path: PathBuf::from(".agent-spec/archive/specs/task-done.spec.md"),
                satisfies: vec!["REQ-DONE".into()],
                depends: Vec::new(),
                scenarios: vec!["Done".into()],
                test_selectors: vec!["test_done".into()],
                tags: vec!["done".into()],
                last_verification: Some(ArchiveVerificationStatus {
                    passing: true,
                    summary: "1/1 passed, 0 failed, 0 skipped, 0 uncertain".into(),
                    timestamp: 42,
                }),
            }],
            diagnostics: Vec::new(),
        };

        let summary = render_archive_summary(&plan);

        assert!(summary.contains("## Archived Specs"));
        assert!(summary.contains("REQ-DONE"));
        assert!(summary.contains("test_done"));
        assert!(summary.contains("specs/task-done.spec.md"));
        assert!(summary.contains("Last verification: pass"));
        assert!(summary.contains("1/1 passed"));
    }

    #[test]
    fn test_archive_plan_uses_latest_passing_run_log_as_archive_evidence() {
        let dir = make_temp_dir("archive-plan-run-log");
        let specs = dir.join("specs");
        let archive = dir.join(".agent-spec/archive/specs");
        let runs = dir.join(".agent-spec/runs");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&runs).unwrap();
        fs::write(
            specs.join("task-done.spec.md"),
            "spec: task\nname: \"Done\"\ntags: [done]\nsatisfies: [REQ-DONE]\n---\n## Intent\nDone.\n## Completion Criteria\nScenario: Done\n  Test: test_done\n  Given done\n  When checked\n  Then stdout contains \"done\"\n",
        )
        .unwrap();
        let spec_path = specs.join("task-done.spec.md");
        write_archive_run_log(
            &runs,
            "1-Done.json",
            &spec_path,
            false,
            "0/1 passed, 1 failed, 0 skipped, 0 uncertain",
            1,
        );
        write_archive_run_log(
            &runs,
            "2-Done.json",
            &spec_path,
            true,
            "1/1 passed, 0 failed, 0 skipped, 0 uncertain",
            2,
        );

        let plan = build_archive_plan_with_history(&specs, &archive, &dir);

        assert_eq!(plan.entries.len(), 1);
        let status = plan.entries[0].last_verification.as_ref().unwrap();
        assert!(status.passing);
        assert_eq!(status.timestamp, 2);
        assert_eq!(
            status.summary,
            "1/1 passed, 0 failed, 0 skipped, 0 uncertain"
        );
        assert!(plan.diagnostics.is_empty());

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_archive_plan_blocks_completed_specs_without_passing_lifecycle_evidence() {
        let dir = make_temp_dir("archive-plan-blocks-unverified");
        let specs = dir.join("specs");
        let archive = dir.join(".agent-spec/archive/specs");
        let runs = dir.join(".agent-spec/runs");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&runs).unwrap();
        fs::write(
            specs.join("task-done.spec.md"),
            "spec: task\nname: \"Done\"\ntags: [done]\nsatisfies: [REQ-DONE]\n---\n## Intent\nDone.\n## Completion Criteria\nScenario: Done\n  Test: test_done\n  Given done\n  When checked\n  Then stdout contains \"done\"\n",
        )
        .unwrap();
        write_archive_run_log(
            &runs,
            "3-Done.json",
            &specs.join("task-done.spec.md"),
            false,
            "0/1 passed, 1 failed, 0 skipped, 0 uncertain",
            3,
        );

        let plan = build_archive_plan_with_history(&specs, &archive, &dir);

        assert!(plan.entries.is_empty());
        assert_eq!(plan.diagnostics.len(), 1);
        assert_eq!(plan.diagnostics[0].code, "archive-lifecycle-not-passing");
        assert!(
            plan.diagnostics[0]
                .message
                .contains("latest lifecycle evidence is not passing")
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_archive_plan_rejects_name_only_or_stale_lifecycle_evidence() {
        let dir = make_temp_dir("archive-plan-stale-evidence");
        let specs = dir.join("specs");
        let archive = dir.join(".agent-spec/archive/specs");
        let runs = dir.join(".agent-spec/runs");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&runs).unwrap();
        fs::write(
            specs.join("task-done.spec.md"),
            "spec: task\nname: Done\ntags: [done]\n---\n## Intent\nDone.\n## Completion Criteria\nScenario: Done\n  Test: test_done\n  Given done\n  When checked\n  Then output is visible\n",
        )
        .unwrap();
        fs::write(
            runs.join("1-Done.json"),
            r#"{"spec_name":"Done","passing":true,"summary":"1/1 passed","timestamp":1}"#,
        )
        .unwrap();

        let plan = build_archive_plan_with_history(&specs, &archive, &dir);
        assert!(plan.entries.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diag| { diag.code == "archive-lifecycle-stale" && diag.severity == "error" })
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_apply_archive_plan_rolls_back_when_directory_creation_fails() {
        let dir = make_temp_dir("archive-plan-dir-rollback");
        let specs = dir.join("specs");
        let archive = dir.join("archive");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&archive).unwrap();
        let first = specs.join("first.spec.md");
        let second = specs.join("second.spec.md");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        // A file at `blocked` makes create_dir_all(blocked/sub) fail after
        // preflight passes: the target path itself does not exist yet.
        fs::write(dir.join("blocked"), "not a directory").unwrap();
        let entry = |name: &str, source_path: PathBuf, archive_path: PathBuf| ArchiveEntry {
            spec_name: name.into(),
            source_path,
            archive_path,
            satisfies: Vec::new(),
            depends: Vec::new(),
            scenarios: Vec::new(),
            test_selectors: Vec::new(),
            tags: vec!["done".into()],
            last_verification: None,
        };
        let plan = ArchivePlan {
            version: 1,
            entries: vec![
                entry("First", first.clone(), archive.join("first.spec.md")),
                entry(
                    "Second",
                    second.clone(),
                    dir.join("blocked/sub/second.spec.md"),
                ),
            ],
            diagnostics: Vec::new(),
        };

        assert!(apply_archive_plan(&plan).is_err());
        assert!(first.exists());
        assert!(second.exists());
        assert!(!archive.join("first.spec.md").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_apply_archive_plan_preflights_all_targets_before_moving_sources() {
        let dir = make_temp_dir("archive-plan-preflight");
        let specs = dir.join("specs");
        let archive = dir.join("archive");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&archive).unwrap();
        let first = specs.join("first.spec.md");
        let second = specs.join("second.spec.md");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        fs::write(archive.join("second.spec.md"), "existing").unwrap();
        let entry = |name: &str, source_path: PathBuf, archive_path: PathBuf| ArchiveEntry {
            spec_name: name.into(),
            source_path,
            archive_path,
            satisfies: Vec::new(),
            depends: Vec::new(),
            scenarios: Vec::new(),
            test_selectors: Vec::new(),
            tags: vec!["done".into()],
            last_verification: None,
        };
        let plan = ArchivePlan {
            version: 1,
            entries: vec![
                entry("First", first.clone(), archive.join("first.spec.md")),
                entry("Second", second.clone(), archive.join("second.spec.md")),
            ],
            diagnostics: Vec::new(),
        };

        assert!(apply_archive_plan(&plan).is_err());
        assert!(first.exists());
        assert!(second.exists());
        assert_eq!(
            fs::read_to_string(archive.join("second.spec.md")).unwrap(),
            "existing"
        );
        fs::remove_dir_all(dir).ok();
    }
}
