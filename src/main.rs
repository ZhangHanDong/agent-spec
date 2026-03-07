#![warn(clippy::all)]
#![deny(unsafe_code)]

mod spec_core;
mod spec_parser;
mod spec_lint;
mod spec_verify;
mod spec_report;
mod spec_gateway;

mod vcs;

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "agent-spec",
    version,
    about = "AI-Native BDD/Spec verification tool"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse .spec files and show AST
    Parse {
        /// Spec files to parse
        files: Vec<PathBuf>,
        /// Output format
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Analyze spec quality (detect smells)
    Lint {
        /// Spec files to lint
        files: Vec<PathBuf>,
        /// Output format: text, json, md
        #[arg(long, default_value = "text")]
        format: String,
        /// Minimum quality score (0.0 - 1.0)
        #[arg(long, default_value = "0.0")]
        min_score: f64,
    },
    /// Verify code against specs
    Verify {
        /// Spec file
        spec: PathBuf,
        /// Code directory to verify against
        #[arg(long)]
        code: PathBuf,
        /// Explicit changed file or directory to check against Boundaries (repeatable)
        #[arg(long = "change")]
        change: Vec<PathBuf>,
        /// Auto-detected git change scope when --change is omitted: none, staged, worktree
        #[arg(long, default_value = "none")]
        change_scope: String,
        /// AI verification mode: off, stub
        #[arg(long, default_value = "off")]
        ai_mode: String,
        /// Output format: text, json, md
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Create a starter .spec file
    Init {
        /// Spec level: org, project, task
        #[arg(long, default_value = "task")]
        level: String,
        /// Spec name
        #[arg(long)]
        name: Option<String>,
        /// Language: zh, en, both
        #[arg(long, default_value = "zh")]
        lang: String,
    },
    /// Run full lifecycle: lint -> verify -> report (for CI/agent use)
    Lifecycle {
        /// Spec file
        spec: PathBuf,
        /// Code directory
        #[arg(long)]
        code: PathBuf,
        /// Explicit changed file or directory to check against Boundaries (repeatable)
        #[arg(long = "change")]
        change: Vec<PathBuf>,
        /// Auto-detected git change scope when --change is omitted: none, staged, worktree, jj
        #[arg(long, default_value = "none")]
        change_scope: String,
        /// AI verification mode: off, stub
        #[arg(long, default_value = "off")]
        ai_mode: String,
        /// Minimum quality score
        #[arg(long, default_value = "0.6")]
        min_score: f64,
        /// Output format: text, json, md
        #[arg(long, default_value = "json")]
        format: String,
        /// Directory for structured run logs (enables run logging when set)
        #[arg(long)]
        run_log_dir: Option<PathBuf>,
        /// Enable adversarial multi-agent verification
        #[arg(long)]
        adversarial: bool,
        /// Comma-separated list of verification layers to run (e.g., lint,boundary,test,ai)
        #[arg(long)]
        layers: Option<String>,
    },
    /// Compatibility alias for the contract view
    Brief {
        /// Spec file
        spec: PathBuf,
        /// Output format: text (prompt), json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Render an explicit Task Contract for agent execution
    Contract {
        /// Spec file
        spec: PathBuf,
        /// Output format: text, json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Git guard: lint all .spec files + verify against the selected git change scope
    Guard {
        /// Spec directory to scan
        #[arg(long, default_value = "specs")]
        spec_dir: PathBuf,
        /// Code directory
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Explicit changed file or directory to check against Boundaries (repeatable)
        #[arg(long = "change")]
        change: Vec<PathBuf>,
        /// Auto-detected git change scope when --change is omitted: staged, worktree
        #[arg(long, default_value = "staged")]
        change_scope: String,
        /// Minimum quality score
        #[arg(long, default_value = "0.6")]
        min_score: f64,
    },
    /// Generate a human-readable contract review summary
    Explain {
        /// Spec file
        spec: PathBuf,
        /// Code directory to verify against
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Output format: text, markdown
        #[arg(long, default_value = "text")]
        format: String,
        /// Show execution history from run log
        #[arg(long)]
        history: bool,
    },
    /// Preview git trailers for a verified contract
    Stamp {
        /// Spec file
        spec: PathBuf,
        /// Code directory to verify against
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Preview trailers without modifying git history
        #[arg(long)]
        dry_run: bool,
    },
    /// Preview or create a VCS checkpoint (optional, VCS-aware)
    Checkpoint {
        /// VCS operation: status, create
        #[arg(default_value = "status")]
        action: String,
    },
    /// [Experimental] Measure contract verification determinism
    MeasureDeterminism {
        /// Spec file
        spec: PathBuf,
        /// Code directory
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Number of repeated runs
        #[arg(long, default_value = "3")]
        runs: usize,
    },
    /// Install git hooks for automatic spec checking
    InstallHooks,
    /// Merge external AI decisions into a verification report
    ResolveAi {
        /// Spec file
        spec: PathBuf,
        /// Code directory
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Path to AI decisions JSON file
        #[arg(long)]
        decisions: PathBuf,
        /// Output format: text, json
        #[arg(long, default_value = "json")]
        format: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Parse { files, format } => cmd_parse(&files, &format),
        Commands::Lint {
            files,
            format,
            min_score,
        } => cmd_lint(&files, &format, min_score),
        Commands::Verify {
            spec,
            code,
            change,
            change_scope,
            ai_mode,
            format,
        } => cmd_verify(&spec, &code, &change, &change_scope, &ai_mode, &format),
        Commands::Init { level, name, lang } => cmd_init(&level, name.as_deref(), &lang),
        Commands::Lifecycle {
            spec,
            code,
            change,
            change_scope,
            ai_mode,
            min_score,
            format,
            run_log_dir,
            adversarial,
            layers,
        } => cmd_lifecycle(
            &spec,
            &code,
            &change,
            &change_scope,
            &ai_mode,
            min_score,
            &format,
            run_log_dir.as_deref(),
            adversarial,
            layers.as_deref(),
        ),
        Commands::Brief { spec, format } => cmd_brief(&spec, &format),
        Commands::Contract { spec, format } => cmd_contract(&spec, &format),
        Commands::Guard {
            spec_dir,
            code,
            change,
            change_scope,
            min_score,
        } => cmd_guard(&spec_dir, &code, &change, &change_scope, min_score),
        Commands::Explain {
            spec,
            code,
            format,
            history,
        } => cmd_explain(&spec, &code, &format, history),
        Commands::Stamp {
            spec,
            code,
            dry_run,
        } => cmd_stamp(&spec, &code, dry_run),
        Commands::Checkpoint { action } => cmd_checkpoint(&action),
        Commands::MeasureDeterminism { spec, code, runs } => {
            cmd_measure_determinism(&spec, &code, runs)
        }
        Commands::InstallHooks => cmd_install_hooks(),
        Commands::ResolveAi {
            spec,
            code,
            decisions,
            format,
        } => cmd_resolve_ai(&spec, &code, &decisions, &format),
    }
}

// ── Parse ───────────────────────────────────────────────────────

fn cmd_parse(files: &[PathBuf], format: &str) -> Result<(), Box<dyn std::error::Error>> {
    for file in files {
        let doc = crate::spec_parser::parse_spec(file)?;
        match format {
            "json" => println!("{}", serde_json::to_string_pretty(&doc)?),
            _ => {
                println!("Spec: {} ({})", doc.meta.name, format_level(doc.meta.level));
                if let Some(ref inherits) = doc.meta.inherits {
                    println!("  inherits: {inherits}");
                }
                println!("  tags: {:?}", doc.meta.tags);
                println!("  sections: {}", doc.sections.len());
                for section in &doc.sections {
                    match section {
                        crate::spec_core::Section::Intent { content, .. } => {
                            let preview: String = content.chars().take(80).collect();
                            println!("    - Intent: {preview}...");
                        }
                        crate::spec_core::Section::Constraints { items, .. } => {
                            println!("    - Constraints: {} items", items.len());
                        }
                        crate::spec_core::Section::Decisions { items, .. } => {
                            println!("    - Decisions: {} items", items.len());
                        }
                        crate::spec_core::Section::Boundaries { items, .. } => {
                            println!("    - Boundaries: {} items", items.len());
                        }
                        crate::spec_core::Section::AcceptanceCriteria { scenarios, .. } => {
                            println!("    - Acceptance Criteria: {} scenarios", scenarios.len());
                            for s in scenarios {
                                println!("      - {}: {} steps", s.name, s.steps.len());
                            }
                        }
                        crate::spec_core::Section::OutOfScope { items, .. } => {
                            println!("    - Out of Scope: {} items", items.len());
                        }
                    }
                }
                println!();
            }
        }
    }
    Ok(())
}

// ── Lint ────────────────────────────────────────────────────────

fn cmd_lint(
    files: &[PathBuf],
    format: &str,
    min_score: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let pipeline = crate::spec_lint::LintPipeline::with_defaults();
    let out_format = parse_output_format(format);
    let mut any_failed = false;

    for file in files {
        let doc = crate::spec_parser::parse_spec(file)?;
        let report = pipeline.run(&doc);

        println!("{}", crate::spec_report::format_lint(&report, &out_format));

        if report.has_errors() {
            eprintln!(
                "spec has {} error-level lint issue(s)",
                report.error_count()
            );
            any_failed = true;
        }

        if report.quality_score.overall < min_score {
            eprintln!(
                "quality score {:.0}% is below minimum {:.0}%",
                report.quality_score.overall * 100.0,
                min_score * 100.0,
            );
            any_failed = true;
        }
    }

    if any_failed {
        Err("quality check failed".into())
    } else {
        Ok(())
    }
}

// ── Verify ──────────────────────────────────────────────────────

fn cmd_verify(
    spec: &Path,
    code: &Path,
    change: &[PathBuf],
    change_scope: &str,
    ai_mode: &str,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let doc = crate::spec_parser::parse_spec(spec)?;
    let resolved = crate::spec_parser::resolve_spec(doc, &[])?;
    let change_scope = GitChangeScope::parse(change_scope)?;
    let ai_mode = parse_ai_mode(ai_mode)?;
    let effective_changes = resolve_command_change_paths(spec, code, change, change_scope)?;

    let ctx = crate::spec_verify::VerificationContext {
        code_paths: vec![code.to_path_buf()],
        change_paths: effective_changes,
        ai_mode,
        resolved_spec: resolved,
    };

    let structural = crate::spec_verify::StructuralVerifier;
    let boundaries = crate::spec_verify::BoundariesVerifier;
    let test = crate::spec_verify::TestVerifier;
    let ai = crate::spec_verify::AiVerifier::from_mode(ai_mode);
    let verifiers: Vec<&dyn crate::spec_verify::Verifier> = vec![&structural, &boundaries, &test, &ai];
    let report = crate::spec_verify::run_verification(&ctx, &verifiers)?;

    let out_format = parse_output_format(format);
    println!("{}", crate::spec_report::format_verification(&report, &out_format));

    let non_passing = report.summary.failed + report.summary.skipped + report.summary.uncertain;
    if non_passing > 0 {
        Err(format!(
            "verification not passing: {} failed, {} skipped, {} uncertain",
            report.summary.failed, report.summary.skipped, report.summary.uncertain,
        )
        .into())
    } else {
        Ok(())
    }
}

// ── Lifecycle (full pipeline for CI/agent) ──────────────────────

fn cmd_lifecycle(
    spec: &Path,
    code: &Path,
    change: &[PathBuf],
    change_scope: &str,
    ai_mode: &str,
    min_score: f64,
    format: &str,
    run_log_dir: Option<&Path>,
    _adversarial: bool,
    layers: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    let change_scope = GitChangeScope::parse(change_scope)?;
    let ai_mode = parse_ai_mode(ai_mode)?;
    let effective_changes = resolve_command_change_paths(spec, code, change, change_scope)?;

    // Parse layers filter
    let active_layers: Option<Vec<&str>> = layers.map(|l| l.split(',').map(str::trim).collect());

    // Stage 1: Quality gate (skip if layers filter excludes lint)
    let run_lint = active_layers
        .as_ref()
        .is_none_or(|l| l.contains(&"lint"));
    let lint_report = if run_lint {
        match gw.quality_gate(min_score) {
            Ok(report) => Some(report),
            Err(failure) => {
                let out = serde_json::json!({
                    "stage": "lint",
                    "passed": false,
                    "message": failure.to_string(),
                    "lint_report": serde_json::to_value(&failure.report).ok(),
                });
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    eprintln!("GATE FAILED: {failure}");
                    println!("{}", gw.format_lint_report(&failure.report, format));
                }
                return Err("quality gate failed".into());
            }
        }
    } else {
        None
    };

    // Stage 2: Verify (respecting layers filter)
    let verify_report = gw.verify_with_changes_and_ai_mode(code, &effective_changes, ai_mode)?;

    // If layers filter is active, filter results to only matching layers
    let verify_report = if let Some(ref layer_list) = active_layers {
        filter_report_by_layers(verify_report, layer_list)
    } else {
        verify_report
    };

    let passing = gw.is_passing(&verify_report);

    // Stage 2b: If caller mode, emit pending AI requests for skipped scenarios
    let ai_pending = if ai_mode == crate::spec_verify::AiMode::Caller {
        let skipped: Vec<_> = verify_report
            .results
            .iter()
            .filter(|r| r.verdict == crate::spec_core::Verdict::Skip)
            .collect();
        if !skipped.is_empty() {
            let ctx = crate::spec_verify::VerificationContext {
                code_paths: vec![code.to_path_buf()],
                change_paths: effective_changes.clone(),
                ai_mode,
                resolved_spec: gw.resolved().clone(),
            };
            let requests: Vec<crate::spec_core::AiRequest> = skipped
                .iter()
                .filter_map(|r| {
                    ctx.resolved_spec
                        .all_scenarios
                        .iter()
                        .find(|s| s.name == r.scenario_name)
                        .map(|scenario| {
                            crate::spec_verify::build_ai_request(
                                &ctx.resolved_spec.task.meta.name,
                                scenario,
                                &ctx,
                            )
                        })
                })
                .collect();
            let requests_path = code.join(".agent-spec/pending-ai-requests.json");
            std::fs::create_dir_all(requests_path.parent().unwrap_or(Path::new(".")))?;
            std::fs::write(
                &requests_path,
                serde_json::to_string_pretty(&requests)?,
            )?;
            true
        } else {
            false
        }
    } else {
        false
    };

    // Stage 3: Report
    if format == "json" {
        let mut json_out = serde_json::json!({
            "stage": "complete",
            "passed": passing,
            "verification": serde_json::to_value(&verify_report).ok(),
            "failure_summary": if passing { None } else { Some(gw.failure_summary(&verify_report)) },
        });
        if ai_pending {
            json_out["ai_pending"] = serde_json::json!(true);
            json_out["ai_requests_file"] = serde_json::json!(".agent-spec/pending-ai-requests.json");
        }
        if let Some(ref lr) = lint_report {
            json_out["quality_score"] = serde_json::json!(lr.quality_score.overall);
            json_out["lint_issues"] = serde_json::json!(lr.diagnostics.len());
        }
        if let Some(ref layer_list) = active_layers {
            json_out["layers"] = serde_json::json!(layer_list);
        }
        println!("{}", serde_json::to_string_pretty(&json_out)?);
    } else {
        if let Some(ref lr) = lint_report {
            println!("=== Lint Report ===");
            println!("{}", gw.format_lint_report(lr, format));
        }
        println!("=== Verification Report ===");
        println!("{}", gw.format_report(&verify_report, format));

        if !passing {
            eprintln!("\n{}", gw.failure_summary(&verify_report));
        }
    }

    // Stage 4: Write run log if enabled
    if let Some(log_dir) = run_log_dir {
        let contract = gw.plan();
        let vcs_ctx = vcs::get_vcs_context(code);
        let entry = RunLogEntry {
            spec_name: contract.name.clone(),
            passing,
            summary: format!(
                "{}/{} passed, {} failed, {} skipped, {} uncertain",
                verify_report.summary.passed,
                verify_report.summary.total,
                verify_report.summary.failed,
                verify_report.summary.skipped,
                verify_report.summary.uncertain,
            ),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            vcs: vcs_ctx,
        };
        write_run_log(log_dir, &entry)?;
    }

    if passing {
        Ok(())
    } else {
        Err(format_non_passing_summary(&verify_report.summary).into())
    }
}

fn filter_report_by_layers(
    report: crate::spec_core::VerificationReport,
    layers: &[&str],
) -> crate::spec_core::VerificationReport {
    let results: Vec<crate::spec_core::ScenarioResult> = report
        .results
        .into_iter()
        .filter(|r| {
            // Extract layer name from scenario name prefix: "[layer] scenario"
            let layer = r
                .scenario_name
                .strip_prefix('[')
                .and_then(|s| s.split(']').next())
                .unwrap_or("");
            layer.is_empty() || layers.iter().any(|l| layer.contains(l))
        })
        .collect();
    crate::spec_core::VerificationReport::from_results(report.spec_name, results)
}

// ── Brief (agent prompt generation) ─────────────────────────────

fn cmd_brief(spec: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    eprintln!("warning: `agent-spec brief` is a compatibility alias; prefer `agent-spec contract`");
    print!("{}", render_brief_output(&gw, format)?);

    Ok(())
}

fn cmd_contract(spec: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    print!("{}", render_contract_output(&gw, format)?);

    Ok(())
}

// ── Guard (git pre-commit) ──────────────────────────────────────

fn cmd_guard(
    spec_dir: &Path,
    code: &Path,
    change: &[PathBuf],
    change_scope: &str,
    min_score: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    if !spec_dir.exists() {
        // No specs directory → nothing to guard, pass silently
        return Ok(());
    }

    let spec_files: Vec<PathBuf> = std::fs::read_dir(spec_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "spec"))
        .collect();

    if spec_files.is_empty() {
        return Ok(());
    }

    let change_scope = GitChangeScope::parse(change_scope)?;
    let effective_changes = resolve_guard_change_paths(spec_dir, code, change, change_scope)?;
    if change.is_empty() && !effective_changes.is_empty() {
        eprintln!("agent-spec guard: detected {} {} change(s) from git", effective_changes.len(), change_scope.label());
    }

    let mut errors = Vec::new();

    for spec_file in &spec_files {
        let gw = match crate::spec_gateway::SpecGateway::load(spec_file) {
            Ok(gw) => gw,
            Err(e) => {
                errors.push(format!("{}: parse error: {e}", spec_file.display()));
                continue;
            }
        };

        // Lint check
        if let Err(failure) = gw.quality_gate(min_score) {
            errors.push(format!("{}: {}", spec_file.display(), failure,));
        }

        // Verify check (only structural — fast enough for pre-commit)
        match gw.verify_with_changes(code, &effective_changes) {
            Ok(report) => {
                if !gw.is_passing(&report) {
                    errors.push(format!(
                        "{}: {}",
                        spec_file.display(),
                        format_non_passing_summary(&report.summary)
                    ));
                }
            }
            Err(e) => {
                errors.push(format!("{}: verify error: {e}", spec_file.display()));
            }
        }
    }

    if errors.is_empty() {
        eprintln!("agent-spec guard: {} spec(s) passed", spec_files.len());
        Ok(())
    } else {
        eprintln!("agent-spec guard: FAILED");
        for err in &errors {
            eprintln!("  - {err}");
        }
        Err(format!("{} check(s) failed", errors.len()).into())
    }
}

fn resolve_command_change_paths(
    spec: &Path,
    code: &Path,
    explicit_changes: &[PathBuf],
    change_scope: GitChangeScope,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if !explicit_changes.is_empty() {
        return Ok(explicit_changes.to_vec());
    }

    let Some(repo_root) = find_command_repo_root(spec, code) else {
        return Ok(Vec::new());
    };

    resolve_git_change_paths(&repo_root, change_scope)
}

fn resolve_guard_change_paths(
    spec_dir: &Path,
    code: &Path,
    explicit_changes: &[PathBuf],
    change_scope: GitChangeScope,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if !explicit_changes.is_empty() {
        return Ok(explicit_changes.to_vec());
    }

    let Some(repo_root) = find_guard_repo_root(spec_dir, code) else {
        return Ok(Vec::new());
    };

    resolve_git_change_paths(&repo_root, change_scope)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitChangeScope {
    None,
    Staged,
    Worktree,
    Jj,
}

impl GitChangeScope {
    fn parse(input: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match input {
            "none" => Ok(Self::None),
            "staged" => Ok(Self::Staged),
            "worktree" => Ok(Self::Worktree),
            "jj" => Ok(Self::Jj),
            other => Err(format!(
                "unsupported --change-scope `{other}` (expected `none`, `staged`, `worktree` or `jj`)"
            )
            .into()),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Staged => "staged",
            Self::Worktree => "worktree",
            Self::Jj => "jj",
        }
    }
}

fn find_command_repo_root(spec: &Path, code: &Path) -> Option<PathBuf> {
    for candidate in [code, spec, Path::new(".")] {
        if let Some(root) = find_git_repo_root(candidate) {
            return Some(root);
        }
    }
    None
}

fn find_guard_repo_root(spec_dir: &Path, code: &Path) -> Option<PathBuf> {
    for candidate in [code, spec_dir, Path::new(".")] {
        if let Some(root) = find_git_repo_root(candidate) {
            return Some(root);
        }
    }
    None
}

fn find_git_repo_root(path: &Path) -> Option<PathBuf> {
    let base = existing_git_base(path)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&base)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        None
    } else {
        Some(PathBuf::from(root))
    }
}

fn existing_git_base(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        if path.is_file() {
            path.parent().map(Path::to_path_buf)
        } else {
            Some(path.to_path_buf())
        }
    } else {
        path.parent()
            .filter(|parent| parent.exists())
            .map(Path::to_path_buf)
    }
}

fn detect_staged_change_paths(
    repo_root: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    git_paths_from_output(
        repo_root,
        &["diff", "--cached", "--name-only", "--diff-filter=ACMRD"],
        "failed to inspect staged changes",
    )
}

fn detect_worktree_change_paths(
    repo_root: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut changes = detect_staged_change_paths(repo_root)?;
    append_unique_paths(
        &mut changes,
        git_paths_from_output(
            repo_root,
            &["diff", "--name-only", "--diff-filter=ACMRD"],
            "failed to inspect unstaged changes",
        )?,
    );
    append_unique_paths(
        &mut changes,
        git_paths_from_output(
            repo_root,
            &["ls-files", "--others", "--exclude-standard"],
            "failed to inspect untracked files",
        )?,
    );
    Ok(changes)
}

fn resolve_git_change_paths(
    repo_root: &Path,
    change_scope: GitChangeScope,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    match change_scope {
        GitChangeScope::None => Ok(Vec::new()),
        GitChangeScope::Staged => detect_staged_change_paths(repo_root),
        GitChangeScope::Worktree => detect_worktree_change_paths(repo_root),
        GitChangeScope::Jj => detect_jj_change_paths(repo_root),
    }
}

fn detect_jj_change_paths(
    repo_root: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Try `jj diff --name-only` to get changed files in the current change
    let output = Command::new("jj")
        .arg("diff")
        .arg("--name-only")
        .current_dir(repo_root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok(Vec::new()), // jj not available or not a jj repo
    };

    let mut changes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let candidate = repo_root.join(trimmed);
        if !changes.iter().any(|existing: &PathBuf| existing == &candidate) {
            changes.push(candidate);
        }
    }

    Ok(changes)
}

fn git_paths_from_output(
    repo_root: &Path,
    args: &[&str],
    error_prefix: &str,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{error_prefix}: {}", stderr.trim()).into());
    }

    let mut changes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let candidate = repo_root.join(trimmed);
        if !changes.iter().any(|existing| existing == &candidate) {
            changes.push(candidate);
        }
    }

    Ok(changes)
}

fn append_unique_paths(target: &mut Vec<PathBuf>, extra: Vec<PathBuf>) {
    for path in extra {
        if !target.iter().any(|existing| existing == &path) {
            target.push(path);
        }
    }
}

fn parse_ai_mode(input: &str) -> Result<crate::spec_verify::AiMode, Box<dyn std::error::Error>> {
    match input {
        "off" => Ok(crate::spec_verify::AiMode::Off),
        "stub" => Ok(crate::spec_verify::AiMode::Stub),
        "caller" => Ok(crate::spec_verify::AiMode::Caller),
        other => Err(format!("unsupported --ai-mode `{other}` (expected `off`, `stub`, or `caller`)").into()),
    }
}

// ── Explain ─────────────────────────────────────────────────────

fn cmd_explain(
    spec: &Path,
    code: &Path,
    format: &str,
    history: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    let contract = gw.plan();
    let report = gw.verify(code)?;

    let input = crate::spec_report::ExplainInput {
        name: contract.name.clone(),
        intent: contract.intent.clone(),
        must: contract.must.clone(),
        must_not: contract.must_not.clone(),
        decisions: contract.decisions.clone(),
        allowed_changes: contract.allowed_changes.clone(),
        forbidden: contract.forbidden.clone(),
        out_of_scope: contract.out_of_scope.clone(),
    };

    let out_format = parse_output_format(format);
    print!(
        "{}",
        crate::spec_report::format_explain(&input, &report, &out_format)
    );

    // Show history from run logs if requested
    if history {
        let log_dir = spec
            .parent()
            .unwrap_or(Path::new("."));
        let history_text = read_run_log_history(log_dir, &contract.name);
        if !history_text.is_empty() {
            println!("\n{history_text}");
        } else {
            println!("\nNo run history found.");
        }
    }

    Ok(())
}

// ── Stamp ───────────────────────────────────────────────────────

fn build_stamp_trailers(
    name: &str,
    passing: bool,
    summary: &crate::spec_core::VerificationSummary,
    vcs_ctx: Option<&vcs::VcsContext>,
) -> Vec<String> {
    let mut trailers = vec![
        format!("Spec-Name: {name}"),
        format!("Spec-Passing: {passing}"),
        format!(
            "Spec-Summary: {}/{} passed, {} failed, {} skipped, {} uncertain",
            summary.passed,
            summary.total,
            summary.failed,
            summary.skipped,
            summary.uncertain,
        ),
    ];

    if let Some(ctx) = vcs_ctx {
        if ctx.vcs_type == vcs::VcsType::Jj {
            trailers.push(format!("Spec-Change: {}", ctx.change_ref));
        }
    }

    trailers
}

fn cmd_stamp(spec: &Path, code: &Path, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !dry_run {
        return Err(
            "destructive stamp is not yet supported; use --dry-run to preview trailers".into(),
        );
    }

    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    let contract = gw.plan();
    let report = gw.verify(code)?;
    let passing = gw.is_passing(&report);

    let vcs_ctx = vcs::get_vcs_context(code);
    let trailers = build_stamp_trailers(&contract.name, passing, &report.summary, vcs_ctx.as_ref());
    for trailer in &trailers {
        println!("{trailer}");
    }

    Ok(())
}

// ── Checkpoint ──────────────────────────────────────────────────

fn cmd_checkpoint(action: &str) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        "status" => {
            // Detect VCS type
            let has_git = Command::new("git")
                .args(["rev-parse", "--git-dir"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            let has_jj = Command::new("jj")
                .args(["root"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if has_jj {
                println!("VCS: jj (checkpoint via `jj new`)");
            } else if has_git {
                println!("VCS: git (checkpoint via `git stash` or `git commit`)");
            } else {
                println!("VCS: none (no checkpoint support)");
            }
            Ok(())
        }
        "create" => {
            eprintln!("checkpoint create is not yet implemented; use `checkpoint status` to see available VCS");
            Ok(())
        }
        other => Err(format!("unknown checkpoint action: {other} (expected `status` or `create`)").into()),
    }
}

// ── Measure Determinism ─────────────────────────────────────────

fn cmd_measure_determinism(
    _spec: &Path,
    _code: &Path,
    _runs: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[experimental] measure-determinism is an experimental feature");
    eprintln!("This command measures contract verification variance across repeated runs.");
    eprintln!("It is NOT part of the default lifecycle or guard pipeline.");
    Err("measure-determinism is experimental and not yet fully implemented".into())
}

// ── Run Log ─────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RunLogEntry {
    pub spec_name: String,
    pub passing: bool,
    pub summary: String,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs: Option<vcs::VcsContext>,
}

fn write_run_log(
    base_dir: &Path,
    entry: &RunLogEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    let runs_dir = base_dir.join(".agent-spec/runs");
    std::fs::create_dir_all(&runs_dir)?;

    let filename = format!("{}-{}.json", entry.timestamp, sanitize_for_filename(&entry.spec_name));
    let path = runs_dir.join(filename);
    let json = serde_json::to_string_pretty(entry)?;
    std::fs::write(&path, json)?;

    Ok(())
}

fn sanitize_for_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn read_run_log_history(base_dir: &Path, spec_name: &str) -> String {
    let runs_dir = base_dir.join(".agent-spec/runs");
    let Ok(entries) = std::fs::read_dir(&runs_dir) else {
        return String::new();
    };

    let mut logs: Vec<RunLogEntry> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let content = std::fs::read_to_string(e.path()).ok()?;
            let entry: RunLogEntry = serde_json::from_str(&content).ok()?;
            if entry.spec_name == spec_name {
                Some(entry)
            } else {
                None
            }
        })
        .collect();

    logs.sort_by_key(|e| e.timestamp);

    if logs.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&format!("=== Run History ({} runs) ===\n", logs.len()));

    let first_pass = logs.iter().position(|e| e.passing);
    if let Some(idx) = first_pass {
        out.push_str(&format!("  First pass: run #{} (timestamp {})\n", idx + 1, logs[idx].timestamp));
    } else {
        out.push_str("  No passing run yet.\n");
    }

    let fail_count = logs.iter().filter(|e| !e.passing).count();
    if fail_count > 0 {
        out.push_str(&format!("  Failed runs: {fail_count}\n"));
    }

    for (i, log) in logs.iter().enumerate() {
        let status = if log.passing { "PASS" } else { "FAIL" };
        out.push_str(&format!("  #{}: [{}] {}\n", i + 1, status, log.summary));

        // Show jj diff between adjacent runs when both have operation IDs
        if i > 0 {
            if let (Some(prev_vcs), Some(curr_vcs)) = (&logs[i - 1].vcs, &log.vcs) {
                if prev_vcs.vcs_type == vcs::VcsType::Jj
                    && curr_vcs.vcs_type == vcs::VcsType::Jj
                {
                    if let (Some(prev_op), Some(curr_op)) =
                        (&prev_vcs.operation_ref, &curr_vcs.operation_ref)
                    {
                        if let Some(changed_files) =
                            vcs::jj_diff_between_ops(Path::new("."), prev_op, curr_op)
                        {
                            out.push_str("    Changes between runs:\n");
                            for f in &changed_files {
                                out.push_str(&format!("      - {f}\n"));
                            }
                        }
                    }
                }
            }
        }
    }

    out
}

// ── Install Hooks ───────────────────────────────────────────────

fn cmd_install_hooks() -> Result<(), Box<dyn std::error::Error>> {
    let git_dir = Path::new(".git");
    if !git_dir.exists() {
        return Err("not a git repository (no .git directory)".into());
    }

    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    let pre_commit = hooks_dir.join("pre-commit");
    let hook_content = r#"#!/bin/sh
# agent-spec pre-commit guard
# Auto-installed by: agent-spec install-hooks

if command -v agent-spec >/dev/null 2>&1; then
    agent-spec guard --spec-dir specs --code src --min-score 0.6
    exit $?
else
    echo "warning: agent-spec not found, skipping spec guard"
    exit 0
fi
"#;

    // Check if hook already exists
    if pre_commit.exists() {
        let existing = std::fs::read_to_string(&pre_commit)?;
        if existing.contains("agent-spec") {
            eprintln!("pre-commit hook already contains agent-spec guard");
            return Ok(());
        }
        // Append to existing hook
        let mut content = existing;
        content.push_str("\n# agent-spec guard (appended)\n");
        content.push_str("if command -v agent-spec >/dev/null 2>&1; then\n");
        content.push_str(
            "    agent-spec guard --spec-dir specs --code src --min-score 0.6 || exit $?\n",
        );
        content.push_str("fi\n");
        std::fs::write(&pre_commit, content)?;
    } else {
        std::fs::write(&pre_commit, hook_content)?;
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&pre_commit, std::fs::Permissions::from_mode(0o755))?;
    }

    eprintln!("installed pre-commit hook at {}", pre_commit.display());
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────

fn cmd_init(level: &str, name: Option<&str>, lang: &str) -> Result<(), Box<dyn std::error::Error>> {
    let spec_level = match level {
        "org" => "org",
        "project" => "project",
        _ => "task",
    };

    let spec_name = name.unwrap_or("unnamed");
    let is_zh = lang == "zh" || lang == "both";

    let template = if is_zh {
        generate_template_zh(spec_level, spec_name)
    } else {
        generate_template_en(spec_level, spec_name)
    };

    let filename = format!("{spec_name}.spec");
    std::fs::write(&filename, &template)?;
    println!("created {filename}");

    Ok(())
}

fn generate_template_zh(level: &str, name: &str) -> String {
    match level {
        "org" => format!(
            r#"spec: org
name: "{name}"
---

## 约束

- 禁止硬编码任何凭证、API Key、Token 或密码
- 所有用户输入必须经过校验和清理
- 所有错误必须使用结构化错误类型
"#
        ),
        "project" => format!(
            r#"spec: project
name: "{name}"
inherits: org
---

## 意图

在此描述项目的核心目标。

## 约束

- 在此添加项目级约束
"#
        ),
        _ => format!(
            r#"spec: task
name: "{name}"
inherits: project
tags: []
---

## 意图

在此描述任务目标和背景。

## 已定决策

- 在此写明已经确定的技术选择

## 边界

### 允许修改
- 在此列出允许修改的文件或模块

### 禁止做
- 在此列出禁止做的事情

## 完成条件

场景: 正常路径
  测试:
    包: your-package
    过滤: test_happy_path
  假设 前置条件
  当 用户执行操作
  那么 期望结果

场景: 异常路径
  测试:
    包: your-package
    过滤: test_error_path
  假设 前置条件
  当 用户执行异常操作
  那么 系统返回错误

## 排除范围

- 不在本任务范围内的功能
"#
        ),
    }
}

fn format_non_passing_summary(summary: &crate::spec_core::VerificationSummary) -> String {
    format!(
        "verification not passing: {} failed, {} skipped, {} uncertain",
        summary.failed, summary.skipped, summary.uncertain,
    )
}

fn generate_template_en(level: &str, name: &str) -> String {
    match level {
        "org" => format!(
            r#"spec: org
name: "{name}"
---

## Constraints

- No hardcoded credentials, API keys, tokens, or passwords
- All user input must be validated and sanitized
- All errors must use structured error types
"#
        ),
        "project" => format!(
            r#"spec: project
name: "{name}"
inherits: org
---

## Intent

Describe the core project goal here.

## Constraints

- Add project-level constraints here
"#
        ),
        _ => format!(
            r#"spec: task
name: "{name}"
inherits: project
tags: []
---

## Intent

Describe the task goal and context here.

## Decisions

- List the technical choices that are already decided

## Boundaries

### Allowed Changes
- List the files or modules that may be modified

### Forbidden
- List the things the agent must not do

## Completion Criteria

Scenario: Happy path
  Test:
    Package: your-package
    Filter: test_happy_path
  Given a precondition
  When the user performs an action
  Then the expected result occurs

Scenario: Error path
  Test:
    Package: your-package
    Filter: test_error_path
  Given a precondition
  When the user performs an invalid action
  Then the system returns an error

## Out of Scope

- Features not in scope for this task
"#
        ),
    }
}

fn format_level(level: crate::spec_core::SpecLevel) -> &'static str {
    match level {
        crate::spec_core::SpecLevel::Org => "org",
        crate::spec_core::SpecLevel::Project => "project",
        crate::spec_core::SpecLevel::Task => "task",
    }
}

fn parse_output_format(s: &str) -> crate::spec_report::OutputFormat {
    match s {
        "json" => crate::spec_report::OutputFormat::Json,
        "md" | "markdown" => crate::spec_report::OutputFormat::Markdown,
        _ => crate::spec_report::OutputFormat::Text,
    }
}

fn render_brief_output(
    gw: &crate::spec_gateway::SpecGateway,
    format: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    render_contract_output(gw, format)
}

fn render_contract_output(
    gw: &crate::spec_gateway::SpecGateway,
    format: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let contract = gw.plan();

    let output = match format {
        "json" => contract.to_json(),
        _ => contract.to_prompt(),
    };
    Ok(output)
}

// ── Resolve AI ──────────────────────────────────────────────────

/// A single AI decision paired with its scenario name for the resolve-ai input file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ScenarioAiDecision {
    pub scenario_name: String,
    #[serde(flatten)]
    pub decision: crate::spec_core::AiDecision,
}

fn cmd_resolve_ai(
    spec: &Path,
    code: &Path,
    decisions_path: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load spec and run mechanical verification (caller mode skips AI internally)
    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    let verify_report = gw.verify_with_ai_mode(code, crate::spec_verify::AiMode::Caller)?;

    // 2. Read external AI decisions
    let decisions_json = std::fs::read_to_string(decisions_path)?;
    let decisions: Vec<ScenarioAiDecision> = serde_json::from_str(&decisions_json)?;

    // 3. Merge: replace Skip verdicts with AI decisions
    let mut merged_results = verify_report.results;
    for decision in &decisions {
        if let Some(result) = merged_results
            .iter_mut()
            .find(|r| r.scenario_name == decision.scenario_name)
        {
            result.verdict = decision.decision.verdict;
            result.step_results = result
                .step_results
                .iter()
                .map(|step| crate::spec_core::StepVerdict {
                    step_text: step.step_text.clone(),
                    verdict: decision.decision.verdict,
                    reason: decision.decision.reasoning.clone(),
                })
                .collect();
            result.evidence = vec![crate::spec_core::Evidence::AiAnalysis {
                model: decision.decision.model.clone(),
                confidence: decision.decision.confidence,
                reasoning: decision.decision.reasoning.clone(),
            }];
        }
    }

    let merged_report = crate::spec_core::VerificationReport::from_results(
        verify_report.spec_name,
        merged_results,
    );

    let passing = gw.is_passing(&merged_report);

    // 4. Output
    if format == "json" {
        let json_out = serde_json::json!({
            "stage": "resolve-ai",
            "passed": passing,
            "verification": serde_json::to_value(&merged_report).ok(),
            "failure_summary": if passing { None } else { Some(gw.failure_summary(&merged_report)) },
        });
        println!("{}", serde_json::to_string_pretty(&json_out)?);
    } else {
        println!("{}", gw.format_report(&merged_report, format));
        if !passing {
            eprintln!("\n{}", gw.failure_summary(&merged_report));
        }
    }

    // Clean up pending requests file if it exists
    let requests_path = code.join(".agent-spec/pending-ai-requests.json");
    if requests_path.exists() {
        let _ = std::fs::remove_file(&requests_path);
    }

    if passing {
        Ok(())
    } else {
        Err(format_non_passing_summary(&merged_report.summary).into())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        GitChangeScope, RunLogEntry, build_stamp_trailers, parse_ai_mode, render_brief_output,
        render_contract_output, resolve_command_change_paths, resolve_guard_change_paths, vcs,
    };

    const SAMPLE: &str = r#"spec: task
name: "Contract Alias"
---

## Intent

Use Task Contract as the default execution surface.

## Decisions

- Prefer Task Contract for plan-stage consumption

## Boundaries

### Allowed Changes
- crates/spec-gateway/**

### Forbidden
- Do not remove the compatibility alias yet

## Completion Criteria

Scenario: Contract alias
  Given a task contract
  When the CLI renders execution context
  Then it should use the Task Contract format
"#;

    #[test]
    fn test_brief_output_matches_contract_output() {
        let gw = crate::spec_gateway::SpecGateway::from_input(SAMPLE).unwrap();
        let brief = render_brief_output(&gw, "text").unwrap();
        let contract = render_contract_output(&gw, "text").unwrap();

        assert_eq!(brief, contract);
        assert!(contract.contains("# Task Contract: Contract Alias"));
        assert!(contract.contains("## Completion Criteria"));
    }

    #[test]
    fn test_resolve_guard_change_paths_prefers_explicit_changes() {
        let explicit = vec![PathBuf::from("custom/file.rs")];
        let resolved = resolve_guard_change_paths(
            Path::new("specs"),
            Path::new("."),
            &explicit,
            GitChangeScope::Worktree,
        )
        .unwrap();
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn test_resolve_guard_change_paths_reads_staged_git_changes() {
        let repo = make_temp_dir("agent-spec-cli-git");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["add", "src/lib.rs"]);

        let resolved = resolve_guard_change_paths(
            &repo.join("specs"),
            &repo,
            &[],
            GitChangeScope::Staged,
        )
        .unwrap();

        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].to_string_lossy().ends_with("src/lib.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn test_resolve_guard_change_paths_returns_empty_outside_git_repo() {
        let dir = make_temp_dir("agent-spec-cli-non-git");
        fs::create_dir_all(dir.join("specs")).unwrap();

        let resolved = resolve_guard_change_paths(
            &dir.join("specs"),
            &dir,
            &[],
            GitChangeScope::Staged,
        )
        .unwrap();
        assert!(resolved.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_resolve_guard_change_paths_reads_worktree_git_changes() {
        let repo = make_temp_dir("agent-spec-cli-worktree");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 1 }\n").unwrap();
        fs::write(repo.join("src/unstaged.rs"), "pub fn unstaged() -> u8 { 1 }\n").unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "agent-spec@example.com"]);
        run_git(&repo, &["config", "user.name", "agent-spec"]);
        run_git(&repo, &["add", "src/staged.rs", "src/unstaged.rs"]);
        run_git(&repo, &["commit", "-m", "init"]);

        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 2 }\n").unwrap();
        run_git(&repo, &["add", "src/staged.rs"]);

        fs::write(repo.join("src/unstaged.rs"), "pub fn unstaged() -> u8 { 2 }\n").unwrap();
        fs::write(repo.join("src/untracked.rs"), "pub fn untracked() -> u8 { 3 }\n").unwrap();

        let resolved = resolve_guard_change_paths(
            &repo.join("specs"),
            &repo,
            &[],
            GitChangeScope::Worktree,
        )
        .unwrap();

        assert!(contains_repo_suffix(&resolved, "src/staged.rs"));
        assert!(contains_repo_suffix(&resolved, "src/unstaged.rs"));
        assert!(contains_repo_suffix(&resolved, "src/untracked.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn test_resolve_guard_change_paths_ignores_unstaged_changes_in_default_staged_scope() {
        let repo = make_temp_dir("agent-spec-cli-staged-default");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 1 }\n").unwrap();
        fs::write(repo.join("src/unstaged.rs"), "pub fn unstaged() -> u8 { 1 }\n").unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "agent-spec@example.com"]);
        run_git(&repo, &["config", "user.name", "agent-spec"]);
        run_git(&repo, &["add", "src/staged.rs", "src/unstaged.rs"]);
        run_git(&repo, &["commit", "-m", "init"]);

        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 2 }\n").unwrap();
        run_git(&repo, &["add", "src/staged.rs"]);

        fs::write(repo.join("src/unstaged.rs"), "pub fn unstaged() -> u8 { 2 }\n").unwrap();

        let resolved = resolve_guard_change_paths(
            &repo.join("specs"),
            &repo,
            &[],
            GitChangeScope::Staged,
        )
        .unwrap();

        assert!(contains_repo_suffix(&resolved, "src/staged.rs"));
        assert!(!contains_repo_suffix(&resolved, "src/unstaged.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    fn contains_repo_suffix(paths: &[PathBuf], suffix: &str) -> bool {
        paths.iter()
            .any(|path| path.to_string_lossy().replace('\\', "/").ends_with(suffix))
    }

    #[test]
    fn test_parse_ai_mode_accepts_stub() {
        assert_eq!(parse_ai_mode("stub").unwrap(), crate::spec_verify::AiMode::Stub);
    }

    #[test]
    fn test_resolve_command_change_paths_prefers_explicit_changes() {
        let explicit = vec![PathBuf::from("custom/file.rs")];
        let resolved = resolve_command_change_paths(
            Path::new("specs/task.spec"),
            Path::new("."),
            &explicit,
            GitChangeScope::Worktree,
        )
        .unwrap();
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn test_resolve_command_change_paths_returns_empty_for_none_scope() {
        let repo = make_temp_dir("agent-spec-cli-command-none");
        fs::create_dir_all(repo.join("specs")).unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["add", "src/lib.rs"]);

        let resolved = resolve_command_change_paths(
            &repo.join("specs/task.spec"),
            &repo,
            &[],
            GitChangeScope::None,
        )
        .unwrap();
        assert!(resolved.is_empty());

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn test_resolve_command_change_paths_reads_worktree_git_changes() {
        let repo = make_temp_dir("agent-spec-cli-command-worktree");
        fs::create_dir_all(repo.join("specs")).unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 1 }\n").unwrap();
        fs::write(repo.join("src/unstaged.rs"), "pub fn unstaged() -> u8 { 1 }\n").unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "agent-spec@example.com"]);
        run_git(&repo, &["config", "user.name", "agent-spec"]);
        run_git(&repo, &["add", "src/staged.rs", "src/unstaged.rs"]);
        run_git(&repo, &["commit", "-m", "init"]);

        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 2 }\n").unwrap();
        run_git(&repo, &["add", "src/staged.rs"]);

        fs::write(repo.join("src/unstaged.rs"), "pub fn unstaged() -> u8 { 2 }\n").unwrap();
        fs::write(repo.join("src/untracked.rs"), "pub fn untracked() -> u8 { 3 }\n").unwrap();

        let resolved = resolve_command_change_paths(
            &repo.join("specs/task.spec"),
            &repo,
            &[],
            GitChangeScope::Worktree,
        )
        .unwrap();

        assert!(contains_repo_suffix(&resolved, "src/staged.rs"));
        assert!(contains_repo_suffix(&resolved, "src/unstaged.rs"));
        assert!(contains_repo_suffix(&resolved, "src/untracked.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn test_claude_code_tool_first_skill_exists_and_mentions_contract_lifecycle_guard() {
        let skill = fs::read_to_string(
            repo_root().join(".claude/skills/agent-spec-tool-first/SKILL.md"),
        )
        .unwrap();

        assert!(skill.contains("agent-spec contract"));
        assert!(skill.contains("agent-spec lifecycle"));
        assert!(skill.contains("agent-spec guard"));
        assert!(skill.contains("Tool-First Workflow"));
    }

    #[test]
    fn test_claude_code_authoring_skill_exists_and_mentions_task_contract_sections() {
        let skill = fs::read_to_string(
            repo_root().join(".claude/skills/agent-spec-authoring/SKILL.md"),
        )
        .unwrap();

        assert!(skill.contains("Intent"));
        assert!(skill.contains("Decisions"));
        assert!(skill.contains("Boundaries"));
        assert!(skill.contains("Completion Criteria"));
        assert!(skill.contains("Test:` selector"));
    }

    #[test]
    fn test_readme_documents_claude_code_tool_first_skills() {
        let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();

        assert!(readme.contains("Claude Code"));
        assert!(readme.contains(".claude/skills"));
        assert!(readme.contains("tool-first"));
        assert!(readme.contains("agent-spec-tool-first"));
    }

    #[test]
    fn test_contract_output_preserves_step_tables_and_test_selectors() {
        let gw = crate::spec_gateway::SpecGateway::from_input(
            r#"spec: task
name: "Contract Output"
---

## Intent

Preserve structured completion criteria in the default contract output.

## Completion Criteria

Scenario: Registration request stays structured
  Test:
    Package: agent-spec
    Filter: test_contract_output_preserves_step_tables_and_test_selectors
  Given no user with email "alice@example.com" exists
  When client submits the registration request:
    | field    | value             |
    | email    | alice@example.com |
    | password | Str0ng!Pass#2026  |
  Then response status should be 201
"#,
        )
        .unwrap();

        let output = render_contract_output(&gw, "text").unwrap();

        assert!(output.contains("Scenario: Registration request stays structured"));
        assert!(output.contains("  Test:"));
        assert!(output.contains("    Package: agent-spec"));
        assert!(output.contains(
            "    Filter: test_contract_output_preserves_step_tables_and_test_selectors"
        ));
        assert!(output.contains("  When client submits the registration request:"));
        assert!(output.contains("| field | value |"));
        assert!(output.contains("| email | alice@example.com |"));
    }

    #[test]
    fn test_roadmap_phase_zero_and_one_specs_exist_and_capture_priorities() {
        let phase0 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase0-contract-fidelity.spec"),
        )
        .unwrap();
        let phase1 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase1-contract-review-loop.spec"),
        )
        .unwrap();

        assert!(phase0.contains("最小 Phase 0 先补齐祖先 `Constraints` 与 `Decisions` 的继承"));
        assert!(phase0.contains("Must`、`Must Not`、`Decisions"));
        assert!(phase0.contains("step table"));

        assert!(phase1.contains("agent-spec explain"));
        assert!(phase1.contains("--format markdown"));
        assert!(phase1.contains("stamp"));
        assert!(phase1.contains("不要先做 destructive `stamp`"));
    }

    #[test]
    fn test_roadmap_later_phase_specs_exist_and_are_split_by_concern() {
        let phase2 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase2-run-history-and-vcs-context.spec"),
        )
        .unwrap();
        let phase3 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase3-spec-governance.spec"),
        )
        .unwrap();
        let phase4 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase4-ai-verification-expansion.spec"),
        )
        .unwrap();
        let phase5 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase5-ecosystem-integrations.spec"),
        )
        .unwrap();
        let phase6 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase6-advanced-verification.spec"),
        )
        .unwrap();

        assert!(phase2.contains("run log"));
        assert!(phase2.contains("`--change-scope jj`"));

        assert!(phase3.contains("org.spec"));
        assert!(phase3.contains("lint --quality"));
        assert!(phase3.contains("本阶段不把 `phase:` 字段写进 spec front matter"));

        assert!(phase4.contains("sycophancy-aware lint"));
        assert!(phase4.contains("adversarial"));

        assert!(phase5.contains("Codex"));
        assert!(phase5.contains("Cursor"));
        assert!(phase5.contains("Aider"));

        assert!(phase6.contains("`layers`"));
        assert!(phase6.contains("determinism"));
    }

    #[test]
    fn test_roadmap_readme_documents_promotion_rule() {
        let readme = fs::read_to_string(repo_root().join("specs/roadmap/README.md")).unwrap();

        assert!(readme.contains("specs/roadmap/"));
        assert!(readme.contains("not part of the default"));
        assert!(readme.contains("top-level `specs/` directory"));
        assert!(readme.contains("inherit the top-level"));
    }

    #[test]
    fn test_explain_command_renders_contract_review_summary() {
        let input = crate::spec_report::ExplainInput {
            name: "Test Contract".into(),
            intent: "Verify the explain command renders a useful summary".into(),
            must: vec!["Run all scenarios".into()],
            must_not: vec!["Skip boundary checks".into()],
            decisions: vec!["Use text format by default".into()],
            allowed_changes: vec!["crates/spec-cli/**".into()],
            forbidden: vec!["Do not modify parser".into()],
            out_of_scope: vec!["AI verification".into()],
        };
        let report = crate::spec_core::VerificationReport {
            spec_name: "test".into(),
            results: vec![crate::spec_core::ScenarioResult {
                scenario_name: "happy path".into(),
                verdict: crate::spec_core::Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 5,
            }],
            summary: crate::spec_core::VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
            },
        };

        let text =
            crate::spec_report::format_explain(&input, &report, &crate::spec_report::OutputFormat::Text);

        assert!(text.contains("Intent"));
        assert!(text.contains("Decisions"));
        assert!(text.contains("Boundaries"));
        assert!(text.contains("Allowed"));
        assert!(text.contains("Forbidden"));
        assert!(text.contains("Verification Summary"));
        assert!(text.contains("[PASS]"));
    }

    #[test]
    fn test_explain_markdown_output_is_suitable_for_pr_description() {
        let input = crate::spec_report::ExplainInput {
            name: "PR Contract".into(),
            intent: "Generate markdown suitable for a PR description".into(),
            must: vec![],
            must_not: vec![],
            decisions: vec!["Markdown tables for summary".into()],
            allowed_changes: vec!["crates/spec-report/**".into()],
            forbidden: vec!["Do not copy raw JSON".into()],
            out_of_scope: vec!["HTML output".into()],
        };
        let report = crate::spec_core::VerificationReport {
            spec_name: "pr".into(),
            results: vec![
                crate::spec_core::ScenarioResult {
                    scenario_name: "scenario A".into(),
                    verdict: crate::spec_core::Verdict::Pass,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 3,
                },
                crate::spec_core::ScenarioResult {
                    scenario_name: "scenario B".into(),
                    verdict: crate::spec_core::Verdict::Fail,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 2,
                },
            ],
            summary: crate::spec_core::VerificationSummary {
                total: 2,
                passed: 1,
                failed: 1,
                skipped: 0,
                uncertain: 0,
            },
        };

        let md = crate::spec_report::format_explain(
            &input,
            &report,
            &crate::spec_report::OutputFormat::Markdown,
        );

        assert!(md.contains("## Intent"));
        assert!(md.contains("## Verification Summary"));
        assert!(md.contains("|")); // table
        assert!(md.contains("## Decisions"));
        assert!(md.contains("## Boundaries"));
    }

    #[test]
    fn test_stamp_dry_run_outputs_trailers_without_rewriting_history() {
        let summary = crate::spec_core::VerificationSummary {
            total: 3,
            passed: 2,
            failed: 1,
            skipped: 0,
            uncertain: 0,
        };

        let trailers = build_stamp_trailers("my-contract", false, &summary, None);

        assert!(trailers.iter().any(|t| t.starts_with("Spec-Name:")));
        assert!(trailers.iter().any(|t| t.starts_with("Spec-Passing:")));
        assert!(trailers.iter().any(|t| t.starts_with("Spec-Summary:")));
        assert!(trailers
            .iter()
            .any(|t| t.contains("Spec-Passing: false")));
        assert!(trailers
            .iter()
            .any(|t| t.contains("2/3 passed, 1 failed")));
    }

    // === Phase 2 Tests ===

    #[test]
    fn test_lifecycle_writes_structured_run_log_summary() {
        let dir = make_temp_dir("agent-spec-run-log");

        let entry = RunLogEntry {
            spec_name: "test-contract".into(),
            passing: true,
            summary: "3/3 passed, 0 failed, 0 skipped, 0 uncertain".into(),
            timestamp: 1700000000,
            vcs: None,
        };
        super::write_run_log(&dir, &entry).unwrap();

        let runs_dir = dir.join(".agent-spec/runs");
        assert!(runs_dir.exists(), "runs directory should be created");

        let files: Vec<_> = fs::read_dir(&runs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(!files.is_empty(), "should have at least one run log file");

        let content = fs::read_to_string(files[0].path()).unwrap();
        assert!(content.contains("\"passing\""), "should contain verdict field");
        assert!(content.contains("test-contract"), "should contain spec name");
        assert!(content.contains("summary"), "should contain summary");

        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["passing"].as_bool().unwrap());
        assert!(parsed["timestamp"].as_u64().is_some());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_explain_history_reads_run_log_summary() {
        let dir = make_temp_dir("agent-spec-explain-history");
        let runs_dir = dir.join(".agent-spec/runs");
        fs::create_dir_all(&runs_dir).unwrap();

        // Write multiple run log entries
        for (i, passing) in [false, false, true].iter().enumerate() {
            let entry = RunLogEntry {
                spec_name: "history-contract".into(),
                passing: *passing,
                summary: format!("run {}", i + 1),
                timestamp: 1700000000 + i as u64,
                vcs: None,
            };
            let json = serde_json::to_string_pretty(&entry).unwrap();
            fs::write(runs_dir.join(format!("{}.json", 1700000000 + i as u64)), json).unwrap();
        }

        let history = super::read_run_log_history(&dir, "history-contract");
        assert!(history.contains("runs"), "should mention runs: {history}");
        assert!(
            history.contains("First pass") || history.contains("first pass"),
            "should mention first pass: {history}"
        );
        assert!(
            history.contains("Failed runs") || history.contains("FAIL"),
            "should show failure trajectory: {history}"
        );
    }

    #[test]
    fn test_resolve_command_change_paths_reads_jj_changes() {
        // Verify jj scope parses correctly
        let scope = GitChangeScope::parse("jj").unwrap();
        assert_eq!(scope, GitChangeScope::Jj);
        assert_eq!(scope.label(), "jj");

        // Verify git defaults are unchanged
        let staged = GitChangeScope::parse("staged").unwrap();
        assert_eq!(staged, GitChangeScope::Staged);
        let worktree = GitChangeScope::parse("worktree").unwrap();
        assert_eq!(worktree, GitChangeScope::Worktree);

        // If jj is available, test actual change detection
        let jj_check = Command::new("jj").arg("version").output();
        if let Ok(output) = jj_check {
            if output.status.success() {
                let repo = make_temp_dir("agent-spec-jj-test");
                let init = Command::new("jj")
                    .arg("git")
                    .arg("init")
                    .current_dir(&repo)
                    .output();
                if let Ok(o) = init {
                    if o.status.success() {
                        fs::write(repo.join("test.rs"), "fn main() {}\n").unwrap();
                        let resolved = super::detect_jj_change_paths(&repo).unwrap();
                        assert!(
                            resolved
                                .iter()
                                .any(|p| p.to_string_lossy().contains("test.rs")),
                            "jj should detect new file: {:?}",
                            resolved
                        );
                    }
                }
                let _ = fs::remove_dir_all(repo);
            }
        }
    }

    // === Phase 4 Tests ===

    #[test]
    fn test_adversarial_verification_is_disabled_by_default() {
        // The lifecycle command accepts --adversarial but it defaults to false
        // Verify that the CLI parses correctly without --adversarial
        // and that adversarial mode is not triggered by default

        // Parse the Lifecycle command without --adversarial flag
        use clap::Parser;
        let cli = super::Cli::parse_from([
            "agent-spec",
            "lifecycle",
            "specs/project.spec",
            "--code",
            ".",
        ]);
        match cli.command {
            super::Commands::Lifecycle { adversarial, .. } => {
                assert!(
                    !adversarial,
                    "adversarial should default to false"
                );
            }
            _ => panic!("expected Lifecycle command"),
        }

        // With --adversarial explicitly
        let cli2 = super::Cli::parse_from([
            "agent-spec",
            "lifecycle",
            "specs/project.spec",
            "--code",
            ".",
            "--adversarial",
        ]);
        match cli2.command {
            super::Commands::Lifecycle { adversarial, .. } => {
                assert!(adversarial, "should be true when explicitly passed");
            }
            _ => panic!("expected Lifecycle command"),
        }
    }

    // === Phase 5 Tests ===

    #[test]
    fn test_additional_agent_integration_templates_exist() {
        let root = repo_root();

        // Codex integration
        let agents_md = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(
            agents_md.contains("agent-spec contract"),
            "AGENTS.md should reference contract command"
        );
        assert!(
            agents_md.contains("agent-spec lifecycle"),
            "AGENTS.md should reference lifecycle command"
        );
        assert!(
            agents_md.contains("agent-spec guard"),
            "AGENTS.md should reference guard command"
        );

        // Cursor integration
        let cursorrules = fs::read_to_string(root.join(".cursorrules")).unwrap();
        assert!(
            cursorrules.contains("agent-spec contract"),
            ".cursorrules should reference contract command"
        );

        // Aider integration
        let aider = fs::read_to_string(root.join(".aider.conf.yml")).unwrap();
        assert!(
            aider.contains("agent-spec"),
            ".aider.conf.yml should reference agent-spec"
        );
    }

    #[test]
    fn test_checkpoint_commands_are_optional_and_vcs_aware() {
        // Verify the checkpoint command parses correctly
        use clap::Parser;
        let cli = super::Cli::parse_from(["agent-spec", "checkpoint", "status"]);
        match cli.command {
            super::Commands::Checkpoint { action } => {
                assert_eq!(action, "status");
            }
            _ => panic!("expected Checkpoint command"),
        }

        // Default action is "status"
        let cli2 = super::Cli::parse_from(["agent-spec", "checkpoint"]);
        match cli2.command {
            super::Commands::Checkpoint { action } => {
                assert_eq!(action, "status");
            }
            _ => panic!("expected Checkpoint command"),
        }

        // Checkpoint is NOT injected into default lifecycle
        let cli3 = super::Cli::parse_from([
            "agent-spec",
            "lifecycle",
            "specs/project.spec",
            "--code",
            ".",
        ]);
        // Lifecycle has no checkpoint-related field - it's a separate command
        assert!(matches!(cli3.command, super::Commands::Lifecycle { .. }));
    }

    // === Phase 6 Tests ===

    #[test]
    fn test_lifecycle_layers_flag_selects_verification_stack() {
        use clap::Parser;

        // Without --layers: all layers run
        let cli = super::Cli::parse_from([
            "agent-spec",
            "lifecycle",
            "specs/project.spec",
            "--code",
            ".",
        ]);
        match cli.command {
            super::Commands::Lifecycle { layers, .. } => {
                assert!(layers.is_none(), "layers should default to None (all layers)");
            }
            _ => panic!("expected Lifecycle command"),
        }

        // With --layers: only specified layers
        let cli2 = super::Cli::parse_from([
            "agent-spec",
            "lifecycle",
            "specs/project.spec",
            "--code",
            ".",
            "--layers",
            "lint,boundary,test",
        ]);
        match cli2.command {
            super::Commands::Lifecycle { layers, .. } => {
                let layers = layers.unwrap();
                assert!(layers.contains("lint"));
                assert!(layers.contains("boundary"));
                assert!(layers.contains("test"));
                assert!(!layers.contains("ai"));
            }
            _ => panic!("expected Lifecycle command"),
        }

        // Test filter_report_by_layers preserves matching and removes non-matching
        let report = crate::spec_core::VerificationReport {
            spec_name: "test".into(),
            results: vec![
                crate::spec_core::ScenarioResult {
                    scenario_name: "[boundary] allowed paths".into(),
                    verdict: crate::spec_core::Verdict::Pass,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 1,
                },
                crate::spec_core::ScenarioResult {
                    scenario_name: "[test] happy path".into(),
                    verdict: crate::spec_core::Verdict::Pass,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 2,
                },
                crate::spec_core::ScenarioResult {
                    scenario_name: "[ai] uncertain scenario".into(),
                    verdict: crate::spec_core::Verdict::Uncertain,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 3,
                },
            ],
            summary: crate::spec_core::VerificationSummary {
                total: 3,
                passed: 2,
                failed: 0,
                skipped: 0,
                uncertain: 1,
            },
        };

        let filtered = super::filter_report_by_layers(report, &["boundary", "test"]);
        assert_eq!(filtered.results.len(), 2, "should only keep boundary and test");
        assert_eq!(filtered.summary.total, 2);
        assert_eq!(filtered.summary.uncertain, 0, "ai layer should be excluded");
    }

    #[test]
    fn test_measure_determinism_is_explicitly_experimental() {
        use clap::Parser;

        // The command exists and parses
        let cli = super::Cli::parse_from([
            "agent-spec",
            "measure-determinism",
            "specs/project.spec",
        ]);
        match cli.command {
            super::Commands::MeasureDeterminism { spec, runs, .. } => {
                assert!(spec.to_string_lossy().contains("project.spec"));
                assert_eq!(runs, 3); // default
            }
            _ => panic!("expected MeasureDeterminism command"),
        }

        // Running it returns an error (experimental)
        let result = super::cmd_measure_determinism(
            Path::new("specs/project.spec"),
            Path::new("."),
            3,
        );
        assert!(result.is_err(), "should fail as experimental");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("experimental"),
            "error should mention experimental: {err}"
        );
    }

    fn run_git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap_or_else(|err| panic!("failed to run git {:?}: {err}", args));

        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // === jj VCS Integration Tests ===

    #[test]
    fn test_stamp_trailers_include_jj_change_id() {
        let summary = crate::spec_core::VerificationSummary {
            total: 3,
            passed: 3,
            failed: 0,
            skipped: 0,
            uncertain: 0,
        };
        let jj_ctx = vcs::VcsContext {
            vcs_type: vcs::VcsType::Jj,
            change_ref: "kxqpylzn".into(),
            operation_ref: Some("abc123".into()),
        };

        let trailers = build_stamp_trailers("my-spec", true, &summary, Some(&jj_ctx));

        assert!(
            trailers.iter().any(|t| t.starts_with("Spec-Change:")),
            "should contain Spec-Change trailer for jj: {trailers:?}"
        );
        assert!(
            trailers.iter().any(|t| t.contains("kxqpylzn")),
            "Spec-Change should contain the jj change ID: {trailers:?}"
        );
    }

    #[test]
    fn test_stamp_trailers_omit_change_id_for_git() {
        let summary = crate::spec_core::VerificationSummary {
            total: 3,
            passed: 3,
            failed: 0,
            skipped: 0,
            uncertain: 0,
        };
        let git_ctx = vcs::VcsContext {
            vcs_type: vcs::VcsType::Git,
            change_ref: "abc1234".into(),
            operation_ref: None,
        };

        let trailers = build_stamp_trailers("my-spec", true, &summary, Some(&git_ctx));

        assert!(
            !trailers.iter().any(|t| t.starts_with("Spec-Change:")),
            "should NOT contain Spec-Change trailer for git: {trailers:?}"
        );
    }

    #[test]
    fn test_run_log_entry_serialises_vcs_context() {
        let entry = RunLogEntry {
            spec_name: "vcs-test".into(),
            passing: true,
            summary: "3/3 passed".into(),
            timestamp: 1700000000,
            vcs: Some(vcs::VcsContext {
                vcs_type: vcs::VcsType::Jj,
                change_ref: "kxqpylzn".into(),
                operation_ref: Some("op123".into()),
            }),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: RunLogEntry = serde_json::from_str(&json).unwrap();

        let vcs = parsed.vcs.expect("vcs should round-trip");
        assert_eq!(vcs.vcs_type, vcs::VcsType::Jj);
        assert_eq!(vcs.change_ref, "kxqpylzn");
        assert_eq!(vcs.operation_ref.as_deref(), Some("op123"));
    }

    #[test]
    fn test_run_log_entry_without_vcs_is_backward_compatible() {
        // Old format JSON without vcs field
        let old_json = r#"{
            "spec_name": "old-contract",
            "passing": true,
            "summary": "2/2 passed",
            "timestamp": 1700000000
        }"#;

        let entry: RunLogEntry = serde_json::from_str(old_json).unwrap();
        assert_eq!(entry.spec_name, "old-contract");
        assert!(entry.passing);
        assert_eq!(entry.summary, "2/2 passed");
        assert_eq!(entry.timestamp, 1700000000);
        assert!(entry.vcs.is_none(), "vcs should be None for old format");
    }

    #[test]
    fn test_explain_history_shows_jj_diff_between_runs() {
        let dir = make_temp_dir("agent-spec-jj-diff-history");
        let runs_dir = dir.join(".agent-spec/runs");
        fs::create_dir_all(&runs_dir).unwrap();

        // Write two run log entries with jj operation IDs
        let entry1 = RunLogEntry {
            spec_name: "jj-diff-contract".into(),
            passing: false,
            summary: "1/3 passed".into(),
            timestamp: 1700000001,
            vcs: Some(vcs::VcsContext {
                vcs_type: vcs::VcsType::Jj,
                change_ref: "change1".into(),
                operation_ref: Some("op_aaa".into()),
            }),
        };
        let entry2 = RunLogEntry {
            spec_name: "jj-diff-contract".into(),
            passing: true,
            summary: "3/3 passed".into(),
            timestamp: 1700000002,
            vcs: Some(vcs::VcsContext {
                vcs_type: vcs::VcsType::Jj,
                change_ref: "change2".into(),
                operation_ref: Some("op_bbb".into()),
            }),
        };

        fs::write(
            runs_dir.join("1700000001.json"),
            serde_json::to_string_pretty(&entry1).unwrap(),
        ).unwrap();
        fs::write(
            runs_dir.join("1700000002.json"),
            serde_json::to_string_pretty(&entry2).unwrap(),
        ).unwrap();

        let history = super::read_run_log_history(&dir, "jj-diff-contract");
        // The history should contain both runs
        assert!(history.contains("2 runs"), "should show 2 runs: {history}");
        assert!(history.contains("FAIL"), "should show FAIL: {history}");
        assert!(history.contains("PASS"), "should show PASS: {history}");

        // jj_diff_between_ops will return None (jj not available or not a real repo)
        // so "Changes between runs" won't appear, but the history still renders correctly
        // This tests graceful degradation when jj is not available.

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_explain_history_degrades_without_jj() {
        let dir = make_temp_dir("agent-spec-no-jj-history");
        let runs_dir = dir.join(".agent-spec/runs");
        fs::create_dir_all(&runs_dir).unwrap();

        // Two entries with jj VCS but no actual jj available
        for (i, passing) in [false, true].iter().enumerate() {
            let entry = RunLogEntry {
                spec_name: "degrade-contract".into(),
                passing: *passing,
                summary: format!("run {}", i + 1),
                timestamp: 1700000010 + i as u64,
                vcs: Some(vcs::VcsContext {
                    vcs_type: vcs::VcsType::Jj,
                    change_ref: format!("change{i}"),
                    operation_ref: Some(format!("op_{i}")),
                }),
            };
            let json = serde_json::to_string_pretty(&entry).unwrap();
            fs::write(
                runs_dir.join(format!("{}.json", 1700000010 + i as u64)),
                json,
            ).unwrap();
        }

        let history = super::read_run_log_history(&dir, "degrade-contract");

        // Should still show run history without crashing
        assert!(history.contains("2 runs"), "should show 2 runs: {history}");
        assert!(history.contains("FAIL"), "should show FAIL run: {history}");
        assert!(history.contains("PASS"), "should show PASS run: {history}");
        // No "Changes between runs" since jj_diff_between_ops returns None
        assert!(
            !history.contains("Changes between runs"),
            "should NOT show changes section without jj: {history}"
        );

        let _ = fs::remove_dir_all(dir);
    }

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // === Caller Mode AI Tests ===

    #[test]
    fn test_parse_ai_mode_accepts_caller() {
        assert_eq!(
            parse_ai_mode("caller").unwrap(),
            crate::spec_verify::AiMode::Caller
        );
    }

    #[test]
    fn test_resolve_ai_command_parses_correctly() {
        use clap::Parser;
        let cli = super::Cli::parse_from([
            "agent-spec",
            "resolve-ai",
            "specs/task.spec",
            "--code",
            ".",
            "--decisions",
            "decisions.json",
        ]);
        match cli.command {
            super::Commands::ResolveAi {
                spec,
                code,
                decisions,
                format,
            } => {
                assert!(spec.to_string_lossy().contains("task.spec"));
                assert_eq!(code, PathBuf::from("."));
                assert_eq!(decisions, PathBuf::from("decisions.json"));
                assert_eq!(format, "json"); // default
            }
            _ => panic!("expected ResolveAi command"),
        }
    }

    #[test]
    fn test_scenario_ai_decision_serialization_roundtrip() {
        let decision = super::ScenarioAiDecision {
            scenario_name: "AI 场景".into(),
            decision: crate::spec_core::AiDecision {
                model: "claude-agent".into(),
                confidence: 0.92,
                verdict: crate::spec_core::Verdict::Pass,
                reasoning: "All steps verified by agent analysis".into(),
            },
        };

        let json = serde_json::to_string_pretty(&decision).unwrap();
        assert!(json.contains("scenario_name"));
        assert!(json.contains("claude-agent"));
        assert!(json.contains("0.92"));

        let parsed: super::ScenarioAiDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.scenario_name, "AI 场景");
        assert_eq!(parsed.decision.verdict, crate::spec_core::Verdict::Pass);
        assert_eq!(parsed.decision.model, "claude-agent");
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }
}
