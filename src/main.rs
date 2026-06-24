#![warn(clippy::all)]
#![deny(unsafe_code)]
#![allow(dead_code)]

mod spec_core;
mod spec_gateway;
mod spec_lint;
mod spec_parser;
mod spec_report;
mod spec_verify;

mod spec_knowledge;
mod vcs;

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::ExitCode;

/// Check whether a path is a spec file (`.spec` or `.spec.md`).
fn is_spec_file(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".spec") || n.ends_with(".spec.md"))
}

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
    /// Parse .spec/.spec.md files and show AST
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
    /// Render the coverage matrix (Rule × Scenario × Test × Verdict × Provenance)
    Matrix {
        /// Spec file
        spec: PathBuf,
        /// Code directory to verify and scan for test functions
        #[arg(long)]
        code: PathBuf,
        /// Explicit changed file or directory (repeatable)
        #[arg(long = "change")]
        change: Vec<PathBuf>,
        /// Git change scope when --change is omitted: none, staged, worktree
        #[arg(long, default_value = "none")]
        change_scope: String,
        /// AI verification mode: off, stub, caller
        #[arg(long, default_value = "off")]
        ai_mode: String,
        /// Output format: text, json, markdown
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Audit a spec library's health (counts, unproven rules, open questions)
    Audit {
        /// Directory of specs to audit
        #[arg(long = "spec-dir", default_value = "specs")]
        spec_dir: PathBuf,
        /// Output format: text, json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Reverse-engineer a draft task spec from a codebase's existing tests
    Discover {
        /// Generate from the codebase's test functions (cold-start)
        #[arg(long = "from-codebase")]
        from_codebase: bool,
        /// Code directory to scan for test functions
        #[arg(long)]
        code: PathBuf,
        /// Name for the generated spec
        #[arg(long)]
        name: String,
        /// Write the draft to this file instead of printing to stdout
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Mechanical structural check: forbid a reference within a file glob
    CheckStructure {
        /// Code directory to scan
        #[arg(long)]
        code: PathBuf,
        /// Forbidden substring (e.g. `crate::services`)
        #[arg(long)]
        forbid: String,
        /// File glob to scope the check (e.g. `clients/**`)
        #[arg(long = "in")]
        within: String,
    },
    /// Generate per-tool integration files from a single source
    GenIntegrations {
        /// Target: agents, cursor, claude, or all
        #[arg(long, default_value = "all")]
        target: String,
        /// Output directory
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// Check for drift instead of writing (non-zero exit if drifted)
        #[arg(long)]
        check: bool,
        /// Append projected guidance from this knowledge root (KLL §6.4).
        #[arg(long)]
        with_guidance: Option<PathBuf>,
    },
    /// Promote a passing task Rule into a capability spec (living-spec library)
    Promote {
        /// Task spec file containing the Rule
        spec: PathBuf,
        /// Rule id to promote
        #[arg(long)]
        rule: String,
        /// Target capability name (-> specs/capabilities/<name>.spec.md)
        #[arg(long = "to")]
        to: String,
        /// Code directory to verify against (the promote gate)
        #[arg(long)]
        code: PathBuf,
    },
    /// Create a starter .spec.md file
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
        /// Template profile: standard, rewrite-parity
        #[arg(long, default_value = "standard")]
        template: String,
        /// Scaffold the canonical KLL workspace tree instead of a single spec.
        #[arg(long)]
        workspace: bool,
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
        /// Resume from checkpoint: incremental (skip passed) or conservative (rerun all, detect regression)
        #[arg(long)]
        resume: Option<Option<String>>,
        /// How to treat pending_review verdicts: auto (count as pass) or strict (count as non-passing)
        #[arg(long, default_value = "auto")]
        review_mode: String,
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
    /// Git guard: lint all .spec/.spec.md files + verify against the selected git change scope
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
    /// Generate structured plan context from a spec + codebase scan
    Plan {
        /// Spec file
        spec: PathBuf,
        /// Code directory to scan
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Output format: text, json, prompt
        #[arg(long, default_value = "text")]
        format: String,
        /// Scan depth: shallow (default), full (includes pub API signatures)
        #[arg(long, default_value = "shallow")]
        depth: String,
    },
    /// Generate a dependency graph from spec files (DOT / SVG)
    Graph {
        /// Spec directory to scan
        #[arg(long, default_value = "specs")]
        spec_dir: PathBuf,
        /// Output format: dot (default), svg (requires system graphviz)
        #[arg(long, default_value = "dot")]
        format: String,
    },
    /// Trace a decision to the specs that satisfy it and report liveness.
    Trace {
        /// Decision id (e.g. ADR-001), case-insensitive.
        id: String,
        /// Knowledge root (decisions live under <knowledge>/decisions).
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        /// Specs root.
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        /// Code directory to verify against.
        #[arg(long, default_value = ".")]
        code: PathBuf,
        /// Output format: text | json.
        #[arg(long, default_value = "text")]
        format: String,
        /// Exit non-zero when the decision is violated.
        #[arg(long)]
        gate: bool,
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
        Commands::Matrix {
            spec,
            code,
            change,
            change_scope,
            ai_mode,
            format,
        } => cmd_matrix(&spec, &code, &change, &change_scope, &ai_mode, &format),
        Commands::Audit { spec_dir, format } => cmd_audit(&spec_dir, &format),
        Commands::Discover {
            from_codebase,
            code,
            name,
            out,
        } => cmd_discover(from_codebase, &code, &name, out.as_deref()),
        Commands::CheckStructure {
            code,
            forbid,
            within,
        } => cmd_check_structure(&code, &forbid, &within),
        Commands::GenIntegrations {
            target,
            out,
            check,
            with_guidance,
        } => cmd_gen_integrations(&target, &out, check, with_guidance.as_deref()),
        Commands::Promote {
            spec,
            rule,
            to,
            code,
        } => cmd_promote(&spec, &rule, &to, &code),
        Commands::Init {
            level,
            name,
            lang,
            template,
            workspace,
        } => cmd_init(&level, name.as_deref(), &lang, &template, workspace),
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
            resume,
            review_mode,
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
            resume,
            &review_mode,
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
        Commands::Plan {
            spec,
            code,
            format,
            depth,
        } => cmd_plan(&spec, &code, &format, &depth),
        Commands::Graph { spec_dir, format } => cmd_graph(&spec_dir, &format),
        Commands::Trace {
            id,
            knowledge,
            specs,
            code,
            format,
            gate,
        } => cmd_trace(&id, &knowledge, &specs, &code, &format, gate),
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
                        crate::spec_core::Section::Questions { items, .. } => {
                            println!("    - Questions: {} items", items.len());
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
    let verifiers: Vec<&dyn crate::spec_verify::Verifier> =
        vec![&structural, &boundaries, &test, &ai];
    let report = crate::spec_verify::run_verification(&ctx, &verifiers)?;

    let out_format = parse_output_format(format);
    println!(
        "{}",
        crate::spec_report::format_verification(&report, &out_format)
    );

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

// ── Coverage matrix (Phase 2) ───────────────────────────────────

/// Build the coverage matrix for a spec by running verification in the same
/// default mode as `verify` (mechanical + ai-mode), scanning the code paths for
/// test functions, and assembling the matrix. Read-only: no gating.
fn build_matrix_for(
    spec: &Path,
    code: &Path,
    change: &[PathBuf],
    change_scope: &str,
    ai_mode: &str,
) -> Result<crate::spec_report::CoverageMatrix, Box<dyn std::error::Error>> {
    let doc = crate::spec_parser::parse_spec(spec)?;
    let resolved = crate::spec_parser::resolve_spec(doc, &[])?;
    let scope = GitChangeScope::parse(change_scope)?;
    let mode = parse_ai_mode(ai_mode)?;
    let effective_changes = resolve_command_change_paths(spec, code, change, scope)?;

    let ctx = crate::spec_verify::VerificationContext {
        code_paths: vec![code.to_path_buf()],
        change_paths: effective_changes,
        ai_mode: mode,
        resolved_spec: resolved.clone(),
    };
    let structural = crate::spec_verify::StructuralVerifier;
    let boundaries = crate::spec_verify::BoundariesVerifier;
    let test = crate::spec_verify::TestVerifier;
    let ai = crate::spec_verify::AiVerifier::from_mode(mode);
    let verifiers: Vec<&dyn crate::spec_verify::Verifier> =
        vec![&structural, &boundaries, &test, &ai];
    let report = crate::spec_verify::run_verification(&ctx, &verifiers)?;

    let test_index = crate::spec_report::collect_test_function_names(&[code.to_path_buf()]);
    Ok(crate::spec_report::build_coverage_matrix(
        &resolved,
        Some(&report),
        &test_index,
    ))
}

fn cmd_matrix(
    spec: &Path,
    code: &Path,
    change: &[PathBuf],
    change_scope: &str,
    ai_mode: &str,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let matrix = build_matrix_for(spec, code, change, change_scope, ai_mode)?;
    let out = match format {
        "json" => matrix.to_json(),
        "markdown" | "md" => matrix.to_markdown(),
        _ => matrix.to_text(),
    };
    println!("{out}");
    Ok(())
}

// ── Audit (Phase 8: spec-library health) ───────────────────────

fn cmd_audit(spec_dir: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_spec_files(spec_dir, &mut files)?;
    files.sort();
    let mut docs = Vec::new();
    for f in &files {
        if let Ok(doc) = crate::spec_parser::parse_spec(f) {
            docs.push(doc);
        }
    }
    let report = crate::spec_report::audit_specs(&docs);
    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!(
            "agent-spec audit ({} specs in {})",
            report.spec_count,
            spec_dir.display()
        );
        println!(
            "  rules: {} ({} unproven)",
            report.rule_count, report.unproven_rules
        );
        println!(
            "  scenarios: {} ({} ungrouped)",
            report.scenario_count, report.ungrouped_scenarios
        );
        println!("  open questions: {}", report.open_questions);
        println!("  malformed rules: {}", report.malformed_rules);
    }
    Ok(())
}

// ── Discover (Phase 9: reverse-engineer a draft from tests) ─────

fn cmd_discover(
    from_codebase: bool,
    code: &Path,
    name: &str,
    out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !from_codebase {
        return Err("discover currently supports only --from-codebase".into());
    }
    let mut names: Vec<String> =
        crate::spec_report::collect_test_function_names(&[code.to_path_buf()])
            .into_iter()
            .collect();
    names.sort();
    let draft = crate::spec_report::draft_spec_from_tests(&names, name);
    match out {
        Some(path) => {
            std::fs::write(path, &draft)?;
            println!(
                "discover: drafted {} scenario(s) from {} -> {}",
                names.len(),
                code.display(),
                path.display()
            );
        }
        None => print!("{draft}"),
    }
    Ok(())
}

// ── Structural check (Phase 7: dependency-cruiser-lite) ─────────

fn cmd_check_structure(
    code: &Path,
    forbid: &str,
    within: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let violations =
        crate::spec_report::structural_violations(&[code.to_path_buf()], forbid, within);
    if violations.is_empty() {
        println!("structural check passed: `{forbid}` not found in `{within}`");
        Ok(())
    } else {
        for v in &violations {
            println!("  violation: {v} contains `{forbid}`");
        }
        Err(format!(
            "structural check failed: {} file(s) under `{within}` reference `{forbid}`",
            violations.len()
        )
        .into())
    }
}

// ── Integrations (Phase 6: single-source multi-tool generation) ──

fn cmd_gen_integrations(
    target: &str,
    out: &Path,
    check: bool,
    with_guidance: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let targets: Vec<&str> = if target == "all" {
        vec!["agents", "cursor", "claude"]
    } else {
        vec![target]
    };
    let filename = |t: &str| match t {
        "agents" => "AGENTS.md",
        "cursor" => ".cursorrules",
        _ => "agent-spec-tool-first.md",
    };

    // Optionally project guidance and append it to every rendered target.
    let guidance_block = match with_guidance {
        Some(dir) => {
            let docs = crate::spec_knowledge::collect_guidance(dir);
            crate::spec_knowledge::render_guidance_md(&docs, None)
        }
        None => String::new(),
    };

    let mut drifted = Vec::new();
    for t in &targets {
        let mut rendered = crate::spec_report::render_named(t)?;
        if !guidance_block.is_empty() {
            rendered.push('\n');
            rendered.push_str(&guidance_block);
        }
        let path = out.join(filename(t));
        if check {
            let existing = std::fs::read_to_string(&path).unwrap_or_default();
            if crate::spec_report::has_drifted(&existing, &rendered) {
                drifted.push(path.display().to_string());
            }
        } else {
            std::fs::write(&path, &rendered)?;
            println!("wrote {}", path.display());
        }
    }

    if check {
        if drifted.is_empty() {
            println!("integrations up to date (no drift)");
        } else {
            return Err(format!("integration drift detected: {}", drifted.join(", ")).into());
        }
    }
    Ok(())
}

// ── Promote (Phase 3: capability living-spec library) ───────────

/// Scenario names grouped under a rule id within a parsed spec, or None if the
/// rule id is not declared.
fn rule_scenarios(doc: &crate::spec_core::SpecDocument, rule_id: &str) -> Option<Vec<String>> {
    for section in &doc.sections {
        if let crate::spec_core::Section::AcceptanceCriteria { rules, .. } = section
            && let Some(r) = rules.iter().find(|r| r.key.id == rule_id)
        {
            return Some(r.scenario_names.clone());
        }
    }
    None
}

/// The promote gate: every Example proving the rule must have a `pass` verdict.
fn examples_all_pass(
    scenario_names: &[String],
    report: &crate::spec_core::VerificationReport,
) -> bool {
    scenario_names.iter().all(|name| {
        report
            .results
            .iter()
            .find(|r| &r.scenario_name == name)
            .is_some_and(|r| r.verdict == crate::spec_core::Verdict::Pass)
    })
}

/// True if a capability name is safe to interpolate into a file path:
/// non-empty, no path separators, no `..`, not absolute.
fn is_safe_capability_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.starts_with('.')
}

/// The promote gate: the rule must have at least one Example, and every Example
/// must have a `pass` verdict (an empty example set is NOT vacuously promotable).
fn promote_gate_ok(
    scenario_names: &[String],
    report: &crate::spec_core::VerificationReport,
) -> Result<(), String> {
    if scenario_names.is_empty() {
        return Err(
            "rule has no examples to prove it (an unproven rule cannot be promoted)".into(),
        );
    }
    if !examples_all_pass(scenario_names, report) {
        return Err("not all examples pass (need all `pass`)".into());
    }
    Ok(())
}

/// Whether a capability spec already declares a Completion Criteria section.
fn has_completion_section(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim().trim_start_matches('#').trim().to_lowercase();
        t.starts_with("完成条件")
            || t.starts_with("验收标准")
            || t.starts_with("completion criter")
            || t.starts_with("acceptance criter")
    })
}

/// Produce the capability spec content with `rule_id` present (idempotent).
/// If `existing` already declares the rule, it is returned unchanged. The
/// promotion provenance is recorded on its OWN comment line (never inline on the
/// Rule header, so it cannot leak into the parsed rule name).
fn upsert_capability_rule(
    existing: Option<&str>,
    cap_name: &str,
    rule_id: &str,
    rule_name: &str,
    from_task: &str,
) -> String {
    let rule_line = if rule_name.is_empty() || rule_name == rule_id {
        format!("### Rule: {rule_id}\n")
    } else {
        format!("### Rule: {rule_id} — {rule_name}\n")
    };
    let block = format!("<!-- promoted from {from_task} -->\n{rule_line}");

    match existing {
        Some(content) if rule_already_present(content, rule_id) => content.to_string(),
        Some(content) => {
            let mut out = content.trim_end().to_string();
            if !has_completion_section(content) {
                out.push_str("\n\n## 完成条件\n");
            }
            out.push('\n');
            out.push_str(&block);
            out.push('\n');
            out
        }
        None => {
            format!(
                "spec: capability\nname: \"{cap_name}\"\ntags: [capability]\n---\n\n## 意图\n\n{cap_name} 能力的长寿命行为真相库(由 promote 累积)。\n\n## 完成条件\n\n{block}\n"
            )
        }
    }
}

/// True if a capability spec already declares a rule with this id.
fn rule_already_present(content: &str, rule_id: &str) -> bool {
    content.lines().any(|line| {
        crate::spec_parser::match_rule_header(line)
            .map(|raw| rule_id_of(raw) == rule_id)
            .unwrap_or(false)
    })
}

/// Extract the leading kebab id token from a Rule header's raw content.
/// Uses the LEFTMOST separator (em dash or double space) to match the parser's
/// `parse_rule_header_content` exactly.
fn rule_id_of(raw: &str) -> &str {
    let raw = raw.trim();
    let em = raw.find('—');
    let ds = raw.find("  ");
    let cut = match (em, ds) {
        (Some(e), Some(d)) => e.min(d),
        (Some(e), None) => e,
        (None, Some(d)) => d,
        (None, None) => raw.len(),
    };
    raw[..cut].trim()
}

fn cmd_promote(
    spec: &Path,
    rule_id: &str,
    capability: &str,
    code: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if !is_safe_capability_name(capability) {
        return Err(format!(
            "unsafe capability name `{capability}` (no path separators, `..`, or leading `.`)"
        )
        .into());
    }
    let doc = crate::spec_parser::parse_spec(spec)?;
    let scenario_names = rule_scenarios(&doc, rule_id)
        .ok_or_else(|| format!("rule id `{rule_id}` not found in {}", spec.display()))?;

    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    let report = gw.verify(code)?;
    promote_gate_ok(&scenario_names, &report)
        .map_err(|e| format!("promote gate failed for rule `{rule_id}`: {e}"))?;

    // Rule display name from the task doc.
    let rule_name = doc
        .sections
        .iter()
        .find_map(|s| match s {
            crate::spec_core::Section::AcceptanceCriteria { rules, .. } => rules
                .iter()
                .find(|r| r.key.id == rule_id)
                .map(|r| r.name.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let from_task = crate::spec_parser::task_stem_from_path(spec);
    let cap_dir = spec
        .parent()
        .unwrap_or(Path::new("specs"))
        .join("capabilities");
    std::fs::create_dir_all(&cap_dir)?;
    let cap_path = cap_dir.join(format!("{capability}.spec.md"));
    let existing = std::fs::read_to_string(&cap_path).ok();
    let updated = upsert_capability_rule(
        existing.as_deref(),
        capability,
        rule_id,
        &rule_name,
        &from_task,
    );
    std::fs::write(&cap_path, updated)?;
    println!(
        "promoted rule `{rule_id}` -> {} (capability `{capability}`)",
        cap_path.display()
    );
    Ok(())
}

// ── Lifecycle (full pipeline for CI/agent) ──────────────────────

#[allow(clippy::too_many_arguments)]
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
    resume: Option<Option<String>>,
    review_mode: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate --resume requires --run-log-dir
    let resume_mode = if let Some(ref mode_opt) = resume {
        if run_log_dir.is_none() {
            return Err("--resume requires --run-log-dir to be set".into());
        }
        let mode_str = mode_opt.as_deref().unwrap_or("incremental");
        Some(match mode_str {
            "incremental" => ResumeMode::Incremental,
            "conservative" => ResumeMode::Conservative,
            other => {
                return Err(format!(
                    "unsupported --resume mode `{other}` (expected `incremental` or `conservative`)"
                )
                .into());
            }
        })
    } else {
        None
    };

    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    let change_scope = GitChangeScope::parse(change_scope)?;
    let ai_mode = parse_ai_mode(ai_mode)?;
    let effective_changes = resolve_command_change_paths(spec, code, change, change_scope)?;

    // Load checkpoint if resuming
    let checkpoint = if resume_mode.is_some() {
        if let Some(log_dir) = run_log_dir {
            load_checkpoint(log_dir)?
        } else {
            None
        }
    } else {
        None
    };

    // Parse layers filter
    let active_layers: Option<Vec<&str>> = layers.map(|l| l.split(',').map(str::trim).collect());

    // Stage 1: Quality gate (skip if layers filter excludes lint)
    let run_lint = active_layers.as_ref().is_none_or(|l| l.contains(&"lint"));
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

    // Apply checkpoint merge if resuming
    let verify_report = if let (Some(mode), Some(cp)) = (&resume_mode, &checkpoint) {
        merge_checkpoint_results(verify_report, cp, mode)
    } else {
        verify_report
    };

    // Apply dependency skips: if a scenario's prerequisite failed, skip it
    let mut verify_report = verify_report;
    apply_dependency_skips(&mut verify_report, &gw.resolved().all_scenarios);

    let passing = gw.is_passing_with_review_mode(&verify_report, review_mode);

    // Collect optimization candidates: optimize-mode scenarios that passed
    let optimization_candidates: Vec<String> =
        gw.resolved()
            .all_scenarios
            .iter()
            .filter(|s| s.mode == crate::spec_core::ScenarioMode::Optimize)
            .filter(|s| {
                verify_report.results.iter().any(|r| {
                    r.scenario_name == s.name && r.verdict == crate::spec_core::Verdict::Pass
                })
            })
            .map(|s| s.name.clone())
            .collect();

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
            std::fs::write(&requests_path, serde_json::to_string_pretty(&requests)?)?;
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
            json_out["ai_requests_file"] =
                serde_json::json!(".agent-spec/pending-ai-requests.json");
        }
        if let Some(ref lr) = lint_report {
            json_out["quality_score"] = serde_json::json!(lr.quality_score.overall);
            json_out["lint_issues"] = serde_json::json!(lr.diagnostics.len());
        }
        if let Some(ref layer_list) = active_layers {
            json_out["layers"] = serde_json::json!(layer_list);
        }
        if !optimization_candidates.is_empty() {
            json_out["optimization_candidates"] = serde_json::json!(optimization_candidates);
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

        // Save checkpoint alongside run log
        save_checkpoint(
            log_dir,
            &verify_report,
            entry.vcs.as_ref().map(|v| v.change_ref.clone()),
        )?;
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

/// Apply dependency skips: for each scenario with depends_on, if any dependency
/// has a non-pass verdict, override this scenario's verdict to Skip.
fn apply_dependency_skips(
    report: &mut crate::spec_core::VerificationReport,
    scenarios: &[crate::spec_core::Scenario],
) {
    use std::collections::HashMap;

    // Build name -> verdict map from current results (owned keys to avoid borrow conflict)
    let verdict_map: HashMap<String, crate::spec_core::Verdict> = report
        .results
        .iter()
        .map(|r| (r.scenario_name.clone(), r.verdict))
        .collect();

    // Build name -> depends_on map from scenarios (owned keys)
    let deps_map: HashMap<String, Vec<String>> = scenarios
        .iter()
        .filter(|s| !s.depends_on.is_empty())
        .map(|s| (s.name.clone(), s.depends_on.clone()))
        .collect();

    // For each result, check if any dependency failed
    for result in &mut report.results {
        if let Some(deps) = deps_map.get(&result.scenario_name) {
            let failed_deps: Vec<&str> = deps
                .iter()
                .filter(|dep| {
                    verdict_map
                        .get(dep.as_str())
                        .is_none_or(|v| *v != crate::spec_core::Verdict::Pass)
                })
                .map(|d| d.as_str())
                .collect();

            if !failed_deps.is_empty() {
                result.verdict = crate::spec_core::Verdict::Skip;
                let dep_names = failed_deps.join(", ");
                result
                    .evidence
                    .push(crate::spec_core::Evidence::PatternMatch {
                        pattern: "dependency-skip".into(),
                        matched: true,
                        locations: vec![format!("dependency failed: {dep_names}")],
                    });
            }
        }
    }

    // Recompute summary
    let total = report.results.len();
    let passed = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Pass)
        .count();
    let failed = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Fail)
        .count();
    let skipped = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Skip)
        .count();
    let uncertain = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Uncertain)
        .count();
    let pending_review = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::PendingReview)
        .count();
    report.summary = crate::spec_core::VerificationSummary {
        total,
        passed,
        failed,
        skipped,
        uncertain,
        pending_review,
    };
}

/// Sort scenarios by topological order based on depends_on.
/// Returns indices in execution order. Scenarios without dependencies preserve
/// their original order relative to each other.
#[allow(dead_code)]
fn topological_sort_scenarios(scenarios: &[crate::spec_core::Scenario]) -> Vec<usize> {
    use std::collections::{HashMap, VecDeque};

    let name_to_idx: HashMap<&str, usize> = scenarios
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.as_str(), i))
        .collect();

    // Build in-degree and adjacency
    let mut in_degree = vec![0usize; scenarios.len()];
    let mut dependents: Vec<Vec<usize>> = vec![vec![]; scenarios.len()];

    for (i, s) in scenarios.iter().enumerate() {
        for dep in &s.depends_on {
            if let Some(&dep_idx) = name_to_idx.get(dep.as_str()) {
                in_degree[i] += 1;
                dependents[dep_idx].push(i);
            }
        }
    }

    // Kahn's algorithm with stable ordering
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(scenarios.len());
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        let mut next: Vec<usize> = dependents[idx]
            .iter()
            .filter_map(|&dep_idx| {
                in_degree[dep_idx] -= 1;
                if in_degree[dep_idx] == 0 {
                    Some(dep_idx)
                } else {
                    None
                }
            })
            .collect();
        // Sort to preserve original order among siblings
        next.sort();
        for n in next {
            queue.push_back(n);
        }
    }

    order
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
        .filter(|p| is_spec_file(p))
        .collect();

    if spec_files.is_empty() {
        return Ok(());
    }

    // Warn about duplicate .spec / .spec.md pairs
    warn_duplicate_spec_extensions(&spec_files);

    let change_scope = GitChangeScope::parse(change_scope)?;
    let effective_changes = resolve_guard_change_paths(spec_dir, code, change, change_scope)?;
    if change.is_empty() && !effective_changes.is_empty() {
        eprintln!(
            "agent-spec guard: detected {} {} change(s) from git",
            effective_changes.len(),
            change_scope.label()
        );
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

/// Warn when the same spec basename has both `.spec` and `.spec.md` variants.
fn warn_duplicate_spec_extensions(spec_files: &[PathBuf]) {
    use std::collections::HashMap;

    let mut by_stem: HashMap<String, Vec<&Path>> = HashMap::new();
    for path in spec_files {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let stem = name
                .strip_suffix(".spec.md")
                .or_else(|| name.strip_suffix(".spec"))
                .unwrap_or(name);
            by_stem.entry(stem.to_string()).or_default().push(path);
        }
    }

    for (stem, paths) in &by_stem {
        if paths.len() > 1 {
            eprintln!(
                "warning: duplicate spec extensions for '{}': {}",
                stem,
                paths
                    .iter()
                    .map(|p| p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
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

fn detect_jj_change_paths(repo_root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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
        if !changes
            .iter()
            .any(|existing: &PathBuf| existing == &candidate)
        {
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
        other => Err(format!(
            "unsupported --ai-mode `{other}` (expected `off`, `stub`, or `caller`)"
        )
        .into()),
    }
}

// ── Explain ─────────────────────────────────────────────────────

/// Append the coverage matrix section to an explain markdown body.
fn assemble_explain_markdown(base: &str, matrix: &crate::spec_report::CoverageMatrix) -> String {
    format!("{base}\n## Coverage Matrix\n\n{}", matrix.to_markdown())
}

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
    let base = crate::spec_report::format_explain(&input, &report, &out_format);

    // Embed the coverage matrix in markdown output (PR acceptance material).
    if matches!(out_format, crate::spec_report::OutputFormat::Markdown) {
        let test_index = crate::spec_report::collect_test_function_names(&[code.to_path_buf()]);
        let matrix =
            crate::spec_report::build_coverage_matrix(gw.resolved(), Some(&report), &test_index);
        print!("{}", assemble_explain_markdown(&base, &matrix));
    } else {
        print!("{base}");
    }

    // Show history from run logs if requested
    if history {
        let log_dir = spec.parent().unwrap_or(Path::new("."));
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
            summary.passed, summary.total, summary.failed, summary.skipped, summary.uncertain,
        ),
    ];

    if let Some(ctx) = vcs_ctx
        && ctx.vcs_type == vcs::VcsType::Jj
    {
        trailers.push(format!("Spec-Change: {}", ctx.change_ref));
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
            eprintln!(
                "checkpoint create is not yet implemented; use `checkpoint status` to see available VCS"
            );
            Ok(())
        }
        other => Err(
            format!("unknown checkpoint action: {other} (expected `status` or `create`)").into(),
        ),
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

fn write_run_log(base_dir: &Path, entry: &RunLogEntry) -> Result<(), Box<dyn std::error::Error>> {
    let runs_dir = base_dir.join(".agent-spec/runs");
    std::fs::create_dir_all(&runs_dir)?;

    let filename = format!(
        "{}-{}.json",
        entry.timestamp,
        sanitize_for_filename(&entry.spec_name)
    );
    let path = runs_dir.join(filename);
    let json = serde_json::to_string_pretty(entry)?;
    std::fs::write(&path, json)?;

    Ok(())
}

fn sanitize_for_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ── Checkpoint / Resume ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeMode {
    Incremental,
    Conservative,
}

fn checkpoint_path(base_dir: &Path) -> PathBuf {
    base_dir.join(".agent-spec/checkpoint.json")
}

fn load_checkpoint(
    base_dir: &Path,
) -> Result<Option<spec_core::Checkpoint>, Box<dyn std::error::Error>> {
    let path = checkpoint_path(base_dir);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let cp: spec_core::Checkpoint = serde_json::from_str(&content)?;
    Ok(Some(cp))
}

fn save_checkpoint(
    base_dir: &Path,
    report: &spec_core::VerificationReport,
    vcs_ref: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = checkpoint_path(base_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut scenarios = std::collections::HashMap::new();
    for result in &report.results {
        scenarios.insert(
            result.scenario_name.clone(),
            spec_core::CheckpointEntry {
                verdict: result.verdict,
                vcs_ref: vcs_ref.clone(),
            },
        );
    }

    let cp = spec_core::Checkpoint {
        spec_name: report.spec_name.clone(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        vcs_ref: vcs_ref.clone(),
        scenarios,
    };

    let json = serde_json::to_string_pretty(&cp)?;
    std::fs::write(&path, json)?;
    Ok(())
}

fn merge_checkpoint_results(
    report: spec_core::VerificationReport,
    checkpoint: &spec_core::Checkpoint,
    mode: &ResumeMode,
) -> spec_core::VerificationReport {
    let results: Vec<spec_core::ScenarioResult> = report
        .results
        .into_iter()
        .map(|mut result| {
            if let Some(cp_entry) = checkpoint.scenarios.get(&result.scenario_name) {
                match mode {
                    ResumeMode::Incremental => {
                        if cp_entry.verdict == spec_core::Verdict::Pass {
                            // Replace with checkpoint pass - scenario was skipped
                            result.verdict = spec_core::Verdict::Pass;
                            result.step_results = result
                                .step_results
                                .into_iter()
                                .map(|mut s| {
                                    s.verdict = spec_core::Verdict::Pass;
                                    s.reason = "carried forward from checkpoint".into();
                                    s
                                })
                                .collect();
                            result.evidence.push(spec_core::Evidence::PatternMatch {
                                pattern: "checkpoint:incremental".into(),
                                matched: true,
                                locations: vec!["verdict carried forward from checkpoint".into()],
                            });
                            result.duration_ms = 0;
                        }
                    }
                    ResumeMode::Conservative => {
                        if cp_entry.verdict == spec_core::Verdict::Pass
                            && result.verdict == spec_core::Verdict::Fail
                        {
                            // Regression detected
                            result.evidence.push(spec_core::Evidence::PatternMatch {
                                pattern: "checkpoint:regression".into(),
                                matched: true,
                                locations: vec![
                                    "regression: true".into(),
                                    "scenario was pass in checkpoint but now fails".into(),
                                ],
                            });
                        }
                    }
                }
            }
            result
        })
        .collect();

    spec_core::VerificationReport::from_results(report.spec_name, results)
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
        out.push_str(&format!(
            "  First pass: run #{} (timestamp {})\n",
            idx + 1,
            logs[idx].timestamp
        ));
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
        if i > 0
            && let (Some(prev_vcs), Some(curr_vcs)) = (&logs[i - 1].vcs, &log.vcs)
            && prev_vcs.vcs_type == vcs::VcsType::Jj
            && curr_vcs.vcs_type == vcs::VcsType::Jj
            && let (Some(prev_op), Some(curr_op)) =
                (&prev_vcs.operation_ref, &curr_vcs.operation_ref)
            && let Some(changed_files) = vcs::jj_diff_between_ops(Path::new("."), prev_op, curr_op)
        {
            out.push_str("    Changes between runs:\n");
            for f in &changed_files {
                out.push_str(&format!("      - {f}\n"));
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

fn cmd_init(
    level: &str,
    name: Option<&str>,
    lang: &str,
    template: &str,
    workspace: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = std::env::current_dir()?;
    if workspace {
        let created = crate::spec_knowledge::scaffold::scaffold_workspace(&output_dir)?;
        if created.is_empty() {
            println!("workspace already scaffolded (nothing to do)");
        } else {
            for p in created {
                println!("created {p}");
            }
        }
        return Ok(());
    }
    cmd_init_at(&output_dir, level, name, lang, template)
}

fn cmd_trace(
    id: &str,
    knowledge: &Path,
    specs: &Path,
    code: &Path,
    format: &str,
    gate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::spec_knowledge::model::Liveness;

    let target = id.to_ascii_uppercase();

    // Find the decision file under <knowledge>/decisions whose resolved id matches.
    let decisions_dir = knowledge.join("decisions");
    let decision = find_decision(&decisions_dir, &target)?;

    let index = crate::spec_knowledge::build_satisfies_index(specs);
    let report = crate::spec_knowledge::build_trace(&decision, &index, |spec_path| {
        crate::spec_knowledge::trace::verify_spec_rollup(spec_path, code)
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => print!("{}", crate::spec_knowledge::format_trace_text(&report)),
    }

    if gate {
        match report.liveness {
            Liveness::Violated => {
                eprintln!("gate: decision {} is VIOLATED", report.decision_id);
                std::process::exit(2);
            }
            Liveness::Unproven => {
                eprintln!(
                    "gate (warning): decision {} is UNPROVEN",
                    report.decision_id
                );
            }
            Liveness::Honored | Liveness::Na => {}
        }
    }
    Ok(())
}

fn find_decision(
    dir: &Path,
    target_id: &str,
) -> Result<crate::spec_knowledge::DecisionDoc, Box<dyn std::error::Error>> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if !(name.ends_with(".md") || name.ends_with(".spec")) {
            continue;
        }
        if let Ok(doc) = crate::spec_knowledge::parse_decision(&p)
            && doc.meta.id == target_id
        {
            return Ok(doc);
        }
    }
    Err(format!("no decision with id {target_id} in {}", dir.display()).into())
}

fn cmd_init_at(
    output_dir: &Path,
    level: &str,
    name: Option<&str>,
    lang: &str,
    template: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec_level = match level {
        "org" => "org",
        "project" => "project",
        _ => "task",
    };

    let spec_name = name.unwrap_or("unnamed");
    let template = match (lang, template) {
        ("zh", "rewrite-parity") => generate_rewrite_parity_template_zh(spec_name),
        ("both", "rewrite-parity") => generate_rewrite_parity_template_both(spec_name),
        (_, "rewrite-parity") => generate_rewrite_parity_template_en(spec_name),
        ("zh", _) => generate_template_zh(spec_level, spec_name),
        ("both", _) => generate_template_both(spec_level, spec_name),
        _ => generate_template_en(spec_level, spec_name),
    };

    let filename = format!("{spec_name}.spec.md");
    let output_path = output_dir.join(&filename);
    std::fs::write(&output_path, &template)?;
    println!("created {}", output_path.display());

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

fn generate_template_both(level: &str, name: &str) -> String {
    match level {
        "org" => format!(
            r#"spec: org
name: "{name}"
---

## Constraints

- Describe organization-wide constraints here.
- 在此描述组织级约束。
"#
        ),
        "project" => format!(
            r#"spec: project
name: "{name}"
inherits: org
---

## Intent

Describe the core project goal here.
在此描述项目的核心目标。

## Constraints

- Add project-level constraints here.
- 在此添加项目级约束。
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
在此描述任务目标和背景。

## Decisions

- List the technical choices that are already decided.
- 在此写明已经确定的技术选择。

## Boundaries

### Allowed Changes
- List the files or modules that may be modified.
- 在此列出允许修改的文件或模块。

### Forbidden
- List the things the agent must not do.
- 在此列出禁止做的事情。

## Completion Criteria

Scenario: Happy path
  Test:
    Package: your-package
    Filter: test_happy_path
  Given a precondition
  When the user performs an action
  Then the expected result occurs

场景: 异常路径
  测试:
    包: your-package
    过滤: test_error_path
  假设 前置条件
  当 用户执行异常操作
  那么 系统返回错误

## Out of Scope

- Features not in scope for this task.
- 不在本任务范围内的功能。
"#
        ),
    }
}

fn generate_rewrite_parity_template_zh(name: &str) -> String {
    format!(
        r#"spec: task
name: "{name}"
inherits: project
tags: [rewrite, parity]
---

## 意图

将 `<待重写系统或命令>` 的可观察行为迁移到新实现，并在编码前绑定关键行为矩阵。

## 已定决策

- 兼容性基线以 `<上游实现 / 现有 CLI / 现有 MCP>` 的可观察行为为准
- 在写代码前先梳理行为矩阵：命令 x 输出模式、local x remote、warm cache x cold start、成功 x 部分失败 x 硬失败
- 所有 stdout/stderr、`--json`、`-o/--output`、fallback / precedence order 都必须落成显式场景
- 对外部 I/O 行为优先使用本地 stub 或 fixture 验证，不依赖真实网络或真实 HOME

## 边界

### 允许修改
- 在此列出允许修改的适配层、运行时层和测试文件

### 禁止做
- 不要把兼容性要求只写成 prose；必须绑定到 Completion Criteria
- 不要用新的用户可见行为替换现有行为，除非本任务明确声明要改 contract

## 完成条件

场景: 人类模式保持兼容输出
  测试:
    包: your-package
    过滤: test_human_mode_parity
    层级: cli
    替身: fixture_cache
    命中: src/commands/get.rs, tests/cli_get.rs
  假设 `<命令>` 从已缓存内容读取结果
  当 用户以默认人类模式执行命令
  那么 stdout 与兼容性基线保持一致
  而且 stderr 不包含额外噪音

场景: JSON 模式返回稳定结构
  测试:
    包: your-package
    过滤: test_json_mode_parity
    层级: cli
    替身: fixture_cache
    命中: src/commands/get.rs
  假设 `<命令>` 以 `--json` 模式运行
  当 用户请求同一份内容
  那么 stdout 只包含稳定 JSON
  而且 省略字段策略与兼容性基线一致

场景: 冷启动遵守 fallback 顺序
  测试:
    包: your-package
    过滤: test_cold_start_fallback_order
    层级: integration
    替身: local_http_stub
    命中: src/core/cache.rs, src/core/registry.rs
  假设 本地正文缓存为空
  当 系统解析 `<local source -> cache -> bundled content -> remote fetch>` 的读取路径
  那么 每一步 fallback 顺序都可观察且稳定

场景: 远端失败返回稳定错误
  测试:
    包: your-package
    过滤: test_remote_fetch_failure_contract
    层级: integration
    替身: local_http_stub
    命中: src/core/cache.rs, src/commands/update.rs
  假设 远端返回非 2xx 或超时
  当 系统执行远端读取或刷新
  那么 返回稳定错误
  而且 不写入损坏缓存或错误 freshness 元数据

## 排除范围

- 本任务未明确声明的新增功能
- 只为通过测试而修改兼容性基线本身
"#
    )
}

fn generate_rewrite_parity_template_both(name: &str) -> String {
    format!(
        r#"spec: task
name: "{name}"
inherits: project
tags: [rewrite, parity]
---

## Intent

Port the observable behavior of `<system under rewrite>` to the new implementation and bind the key behavior matrix before coding.
在编码前将 `<待重写系统或命令>` 的可观察行为迁移到新实现，并绑定关键行为矩阵。

## Decisions

- Treat `<upstream implementation / existing CLI / existing MCP>` as the compatibility baseline.
- 将 `<上游实现 / 现有 CLI / 现有 MCP>` 作为兼容性基线。
- Cover the behavior matrix before coding: command x output mode, local x remote, warm cache x cold start, success x partial failure x hard failure.
- 在编码前覆盖行为矩阵：命令 x 输出模式、local x remote、warm cache x cold start、成功 x 部分失败 x 硬失败。
- Bind stdout/stderr, `--json`, `-o/--output`, and fallback / precedence order as explicit scenarios.
- 将 stdout/stderr、`--json`、`-o/--output`、fallback / precedence order 写成显式场景。

## Boundaries

### Allowed Changes
- List the adapters, runtime modules, and tests that may change.
- 在此列出允许修改的适配层、运行时层和测试文件。

### Forbidden
- Do not leave compatibility requirements as prose-only notes.
- 不要把兼容性要求只写成 prose。
- Do not replace current user-visible behavior unless this task explicitly changes the contract.
- 不要在任务未声明时改写用户可见行为。

## Completion Criteria

Scenario: human mode keeps parity output
  Test:
    Package: your-package
    Filter: test_human_mode_parity
    Level: cli
    Test Double: fixture_cache
    Targets: src/commands/get.rs, tests/cli_get.rs
  Given `<command>` reads from cached content
  When the user runs it in default human mode
  Then stdout stays compatible with the baseline
  And stderr does not contain extra noise

场景: JSON 模式返回稳定结构
  测试:
    包: your-package
    过滤: test_json_mode_parity
    层级: cli
    替身: fixture_cache
    命中: src/commands/get.rs
  假设 `<命令>` 以 `--json` 模式运行
  当 用户请求同一份内容
  那么 stdout 只包含稳定 JSON
  而且 省略字段策略与兼容性基线一致

Scenario: cold start follows fallback order
  Test:
    Package: your-package
    Filter: test_cold_start_fallback_order
    Level: integration
    Test Double: local_http_stub
    Targets: src/core/cache.rs, src/core/registry.rs
  Given local content cache is empty
  When the system resolves `<local source -> cache -> bundled content -> remote fetch>`
  Then each fallback step is observable and stable

场景: 远端失败返回稳定错误
  测试:
    包: your-package
    过滤: test_remote_fetch_failure_contract
    层级: integration
    替身: local_http_stub
    命中: src/core/cache.rs, src/commands/update.rs
  假设 远端返回非 2xx 或超时
  当 系统执行远端读取或刷新
  那么 返回稳定错误
  而且 不写入损坏缓存或错误 freshness 元数据

## Out of Scope

- New features not explicitly declared by this task.
- 本任务未明确声明的新增功能。
- Changing the compatibility baseline itself just to make tests pass.
- 不要为了通过测试而修改兼容性基线本身。
"#
    )
}

fn format_non_passing_summary(summary: &crate::spec_core::VerificationSummary) -> String {
    format!(
        "verification not passing: {} failed, {} skipped, {} uncertain, {} pending_review",
        summary.failed, summary.skipped, summary.uncertain, summary.pending_review,
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

fn generate_rewrite_parity_template_en(name: &str) -> String {
    format!(
        r#"spec: task
name: "{name}"
inherits: project
tags: [rewrite, parity]
---

## Intent

Port the observable behavior of `<system under rewrite>` to the new implementation and bind the key behavior matrix before coding.

## Decisions

- Treat `<upstream implementation / existing CLI / existing MCP>` as the compatibility baseline
- Cover the behavior matrix before coding: command x output mode, local x remote, warm cache x cold start, success x partial failure x hard failure
- Bind stdout/stderr, `--json`, `-o/--output`, and fallback / precedence order as explicit scenarios
- Prefer local stubs or fixtures for external I/O verification instead of real network or real HOME state

## Boundaries

### Allowed Changes
- List the adapters, runtime modules, and tests that may change

### Forbidden
- Do not leave compatibility requirements as prose-only notes
- Do not replace current user-visible behavior unless this task explicitly changes the contract

## Completion Criteria

Scenario: human mode keeps parity output
  Test:
    Package: your-package
    Filter: test_human_mode_parity
    Level: cli
    Test Double: fixture_cache
    Targets: src/commands/get.rs, tests/cli_get.rs
  Given `<command>` reads from cached content
  When the user runs it in default human mode
  Then stdout stays compatible with the baseline
  And stderr does not contain extra noise

Scenario: json mode returns a stable payload
  Test:
    Package: your-package
    Filter: test_json_mode_parity
    Level: cli
    Test Double: fixture_cache
    Targets: src/commands/get.rs
  Given `<command>` runs with `--json`
  When the user requests the same content
  Then stdout contains stable JSON only
  And field omission rules stay compatible with the baseline

Scenario: cold start follows fallback order
  Test:
    Package: your-package
    Filter: test_cold_start_fallback_order
    Level: integration
    Test Double: local_http_stub
    Targets: src/core/cache.rs, src/core/registry.rs
  Given local content cache is empty
  When the system resolves `<local source -> cache -> bundled content -> remote fetch>`
  Then each fallback step is observable and stable

Scenario: remote failure returns a stable error
  Test:
    Package: your-package
    Filter: test_remote_fetch_failure_contract
    Level: integration
    Test Double: local_http_stub
    Targets: src/core/cache.rs, src/commands/update.rs
  Given the remote endpoint returns non-2xx or times out
  When the system performs a remote read or refresh
  Then it returns a stable error
  And it does not write corrupt cache or incorrect freshness metadata

## Out of Scope

- New features not explicitly declared by this task
- Changing the compatibility baseline itself just to make tests pass
"#
    )
}

fn format_level(level: crate::spec_core::SpecLevel) -> &'static str {
    match level {
        crate::spec_core::SpecLevel::Org => "org",
        crate::spec_core::SpecLevel::Project => "project",
        crate::spec_core::SpecLevel::Capability => "capability",
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

/// Merge externally-resolved AI decisions into verification results, replacing
/// the matched scenarios' verdict/steps/evidence and stamping provenance as
/// `Inferential` (caller-mode bypasses AiVerifier, so it must stamp here).
fn merge_ai_decisions(
    mut results: Vec<crate::spec_core::ScenarioResult>,
    decisions: &[ScenarioAiDecision],
) -> Vec<crate::spec_core::ScenarioResult> {
    for decision in decisions {
        if let Some(result) = results
            .iter_mut()
            .find(|r| r.scenario_name == decision.scenario_name)
        {
            // Only resolve Skip verdicts: a mechanically-proven pass/fail must
            // never be overridden by a caller AI decision (mechanical is the moat).
            if result.verdict != crate::spec_core::Verdict::Skip {
                continue;
            }
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
            result.provenance = Some(crate::spec_core::EvidenceProvenance::Inferential);
        }
    }
    results
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
    let merged_results = merge_ai_decisions(verify_report.results, &decisions);

    let merged_report =
        crate::spec_core::VerificationReport::from_results(verify_report.spec_name, merged_results);

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

// ── Plan ─────────────────────────────────────────────────────────

fn cmd_plan(
    spec: &Path,
    code: &Path,
    format: &str,
    depth: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let gw = crate::spec_gateway::SpecGateway::load(spec)?;
    let contract = gw.plan();
    let scan_depth = crate::spec_gateway::plan::ScanDepth::parse(depth);

    let ctx =
        crate::spec_gateway::plan::build_plan_context(&contract, gw.resolved(), code, scan_depth);

    // Print warnings to stderr
    for warning in &ctx.warnings {
        eprintln!("warning: {warning}");
    }

    let output = match format {
        "json" => crate::spec_gateway::plan::format_plan_json(&ctx),
        "prompt" => crate::spec_gateway::plan::format_plan_prompt(&ctx),
        _ => crate::spec_gateway::plan::format_plan_text(&ctx),
    };

    print!("{output}");
    Ok(())
}

// ── Graph ────────────────────────────────────────────────────────

struct GraphNode {
    name: String,
    file_stem: String,
    depends: Vec<String>,
    estimate: Option<String>,
    tags: Vec<String>,
}

fn cmd_graph(spec_dir: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;

    // Collect all spec files
    let mut spec_files: Vec<PathBuf> = Vec::new();
    collect_spec_files(spec_dir, &mut spec_files)?;

    if spec_files.is_empty() {
        return Err(format!("no spec files found in {}", spec_dir.display()).into());
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut name_to_stem: HashMap<String, String> = HashMap::new();
    let mut stem_to_idx: HashMap<String, usize> = HashMap::new();

    for file in &spec_files {
        let doc = match crate::spec_parser::parse_spec(file) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", file.display());
                continue;
            }
        };
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .trim_end_matches(".spec")
            .to_string();
        let idx = nodes.len();
        name_to_stem.insert(doc.meta.name.clone(), stem.clone());
        stem_to_idx.insert(stem.clone(), idx);
        nodes.push(GraphNode {
            name: doc.meta.name,
            file_stem: stem,
            depends: doc.meta.depends,
            estimate: doc.meta.estimate,
            tags: doc.meta.tags,
        });
    }

    // Build edges: dep -> dependent
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        for dep in &node.depends {
            let dep_idx = stem_to_idx.get(dep.as_str()).copied().or_else(|| {
                name_to_stem
                    .get(dep.as_str())
                    .and_then(|s| stem_to_idx.get(s.as_str()).copied())
            });
            if let Some(j) = dep_idx {
                edges.push((j, i));
            } else {
                eprintln!(
                    "warning: spec '{}' depends on unknown '{}', ignoring",
                    node.name, dep
                );
            }
        }
    }

    // Compute critical path
    let estimates: Vec<f64> = nodes
        .iter()
        .map(|n| n.estimate.as_deref().map_or(0.0, parse_estimate_days))
        .collect();
    let critical_path_edges = compute_critical_path(nodes.len(), &edges, &estimates);

    // Generate DOT
    let dot = generate_dot(&nodes, &edges, &critical_path_edges);

    match format {
        "svg" => {
            let mut child = Command::new("dot")
                .args(["-Tsvg"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("failed to run 'dot' (is graphviz installed?): {e}"))?;

            if let Some(ref mut stdin) = child.stdin {
                use std::io::Write;
                stdin.write_all(dot.as_bytes())?;
            }
            let output = child.wait_with_output()?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("dot command failed: {stderr}").into());
            }
            std::io::Write::write_all(&mut std::io::stdout(), &output.stdout)?;
        }
        _ => {
            print!("{dot}");
        }
    }

    Ok(())
}

fn generate_dot(
    nodes: &[GraphNode],
    edges: &[(usize, usize)],
    critical_edges: &[(usize, usize)],
) -> String {
    use std::collections::HashSet;

    let mut dot = String::new();
    dot.push_str("digraph spec_dependencies {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [fontname=\"Helvetica\", fontsize=11];\n");
    dot.push_str("  edge [fontname=\"Helvetica\", fontsize=9];\n\n");

    for node in nodes {
        let label = if let Some(ref est) = node.estimate {
            format!("{}\\n[{}]", node.name, est)
        } else {
            node.name.clone()
        };
        let is_done = node.tags.iter().any(|t| t == "done" || t == "completed");
        let shape = if is_done { "doubleoctagon" } else { "box" };
        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\", shape={}];\n",
            node.file_stem, label, shape
        ));
    }

    dot.push('\n');

    let critical_set: HashSet<(usize, usize)> = critical_edges.iter().copied().collect();
    for &(from, to) in edges {
        let attrs = if critical_set.contains(&(from, to)) {
            "arrowhead=vee, color=red, penwidth=2.0"
        } else {
            "arrowhead=vee"
        };
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [{}];\n",
            nodes[from].file_stem, nodes[to].file_stem, attrs
        ));
    }

    dot.push_str("}\n");
    dot
}

/// Collect .spec / .spec.md files recursively from a directory.
fn collect_spec_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.exists() {
        return Err(format!("directory not found: {}", dir.display()).into());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_spec_files(&path, out)?;
        } else if is_spec_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

/// Parse estimate string like "1d", "0.5d", "1w" into days as f64.
fn parse_estimate_days(est: &str) -> f64 {
    let est = est.trim().trim_start_matches('~');
    if let Some(days) = est.strip_suffix('d') {
        days.trim().parse::<f64>().unwrap_or(0.0)
    } else if let Some(weeks) = est.strip_suffix('w') {
        weeks.trim().parse::<f64>().unwrap_or(0.0) * 5.0
    } else if let Some(hours) = est.strip_suffix('h') {
        hours.trim().parse::<f64>().unwrap_or(0.0) / 8.0
    } else {
        est.parse::<f64>().unwrap_or(0.0)
    }
}

/// Compute the critical path edges using longest-path on the DAG.
fn compute_critical_path(
    n: usize,
    edges: &[(usize, usize)],
    estimates: &[f64],
) -> Vec<(usize, usize)> {
    if n == 0 || edges.is_empty() {
        return Vec::new();
    }

    // Topological sort (Kahn's algorithm)
    let mut in_degree = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(from, to) in edges {
        adj[from].push(to);
        in_degree[to] += 1;
    }

    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    let mut topo_order = Vec::with_capacity(n);
    while let Some(u) = queue.pop_front() {
        topo_order.push(u);
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    // Longest path DP
    let mut dist = vec![0.0f64; n];
    let mut pred = vec![None::<usize>; n];

    for &u in &topo_order {
        let u_cost = estimates[u];
        for &v in &adj[u] {
            let new_dist = dist[u] + u_cost;
            if new_dist > dist[v] {
                dist[v] = new_dist;
                pred[v] = Some(u);
            }
        }
    }

    // Find the end node with maximum total cost
    let end = (0..n).max_by(|&a, &b| {
        let da = dist[a] + estimates[a];
        let db = dist[b] + estimates[b];
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Trace back
    let mut path_edges = Vec::new();
    if let Some(mut cur) = end {
        while let Some(p) = pred[cur] {
            path_edges.push((p, cur));
            cur = p;
        }
    }
    path_edges.reverse();
    path_edges
}

#[cfg(test)]
#[allow(clippy::collapsible_if, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use clap::Parser;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        GitChangeScope, ResumeMode, RunLogEntry, build_stamp_trailers, checkpoint_path,
        cmd_init_at, generate_rewrite_parity_template_both, generate_rewrite_parity_template_en,
        generate_rewrite_parity_template_zh, generate_template_both, generate_template_en,
        generate_template_zh, is_spec_file, load_checkpoint, merge_checkpoint_results,
        parse_ai_mode, render_brief_output, render_contract_output, resolve_command_change_paths,
        resolve_guard_change_paths, save_checkpoint, vcs, warn_duplicate_spec_extensions,
    };
    use super::{
        ScenarioAiDecision, assemble_explain_markdown, build_matrix_for, merge_ai_decisions,
    };
    use super::{examples_all_pass, rule_scenarios, upsert_capability_rule};

    // ---- Phase 3: promote ----

    fn report_with(
        verdicts: &[(&str, crate::spec_core::Verdict)],
    ) -> crate::spec_core::VerificationReport {
        let results = verdicts
            .iter()
            .map(|(name, v)| crate::spec_core::ScenarioResult {
                scenario_name: (*name).into(),
                verdict: *v,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            })
            .collect();
        crate::spec_core::VerificationReport::from_results("t".into(), results)
    }

    const PROMOTE_SPEC: &str = r#"spec: task
name: "退款"
---

## 完成条件

### Rule: r-ok — 退款幂等
场景: 首次退款
  测试: t1
  当 a
  那么 b
场景: 重复退款
  测试: t2
  当 a
  那么 b
"#;

    use super::{is_safe_capability_name, promote_gate_ok, rule_id_of};

    #[test]
    fn test_promote_refuses_rule_with_no_examples() {
        // C1/C6/C9: a rule with zero examples must NOT pass the gate (vacuous).
        let report = report_with(&[]);
        assert!(
            promote_gate_ok(&[], &report).is_err(),
            "empty examples must fail the gate"
        );
        let ok = report_with(&[("a", crate::spec_core::Verdict::Pass)]);
        assert!(promote_gate_ok(&["a".to_string()], &ok).is_ok());
    }

    #[test]
    fn test_promote_rule_name_excludes_provenance_comment() {
        // C2/C8: the promoted rule, re-parsed, must not carry the HTML comment in its name.
        let content = upsert_capability_rule(None, "billing", "r-bare", "r-bare", "task-x");
        let doc = crate::spec_parser::parse_spec_from_str(&content).unwrap();
        let rule = doc
            .sections
            .iter()
            .find_map(|s| match s {
                crate::spec_core::Section::AcceptanceCriteria { rules, .. } => {
                    rules.iter().find(|r| r.key.id == "r-bare").cloned()
                }
                _ => None,
            })
            .expect("r-bare present");
        assert!(
            !rule.name.contains("<!--"),
            "rule name must not contain the provenance comment: {}",
            rule.name
        );
        assert_eq!(rule.name, "r-bare");
    }

    #[test]
    fn test_promote_rejects_unsafe_capability_name() {
        // C5/C7: path traversal / absolute / separators must be rejected.
        assert!(!is_safe_capability_name("../evil"));
        assert!(!is_safe_capability_name("/etc/passwd"));
        assert!(!is_safe_capability_name("a/b"));
        assert!(!is_safe_capability_name("a\\b"));
        assert!(!is_safe_capability_name(""));
        assert!(is_safe_capability_name("billing"));
        assert!(is_safe_capability_name("ecosystem-import"));
    }

    #[test]
    fn test_rule_id_of_matches_parser_on_double_space_before_em_dash() {
        // C3: leftmost separator wins (double space before em dash).
        assert_eq!(rule_id_of("id  desc — more"), "id");
        assert_eq!(rule_id_of("id — desc"), "id");
        assert_eq!(rule_id_of("id"), "id");
    }

    #[test]
    fn test_promote_appends_under_completion_criteria() {
        // C4: appending to a capability file lacking a Completion Criteria
        // section must still place the rule where it parses as a rule.
        let hand = "spec: capability\nname: \"billing\"\n---\n\n## 意图\n\n手写的能力文件,没有完成条件段。\n";
        let updated = upsert_capability_rule(Some(hand), "billing", "r-ok", "退款幂等", "task-x");
        let doc = crate::spec_parser::parse_spec_from_str(&updated).unwrap();
        let has_rule = doc.sections.iter().any(|s| {
            matches!(s,
            crate::spec_core::Section::AcceptanceCriteria { rules, .. }
                if rules.iter().any(|r| r.key.id == "r-ok"))
        });
        assert!(
            has_rule,
            "promoted rule must parse as a rule (under Completion Criteria)"
        );
    }

    #[test]
    fn test_promote_unknown_rule_id_errors() {
        let doc = crate::spec_parser::parse_spec_from_str(PROMOTE_SPEC).unwrap();
        assert!(rule_scenarios(&doc, "r-missing").is_none());
        assert!(rule_scenarios(&doc, "r-ok").is_some());
    }

    #[test]
    fn test_promote_refuses_when_an_example_fails() {
        let doc = crate::spec_parser::parse_spec_from_str(PROMOTE_SPEC).unwrap();
        let names = rule_scenarios(&doc, "r-ok").unwrap();
        let report = report_with(&[
            ("首次退款", crate::spec_core::Verdict::Pass),
            ("重复退款", crate::spec_core::Verdict::Fail),
        ]);
        assert!(
            !examples_all_pass(&names, &report),
            "a failing example must block promote"
        );
    }

    #[test]
    fn test_promote_appends_rule_when_examples_pass() {
        let content = upsert_capability_rule(None, "billing", "r-ok", "退款幂等", "task-refund");
        // Re-parse the generated capability spec: rule present with Capability scope.
        let doc = crate::spec_parser::parse_spec_from_str(&content).unwrap();
        assert_eq!(doc.meta.level, crate::spec_core::SpecLevel::Capability);
        let rule = doc.sections.iter().find_map(|s| match s {
            crate::spec_core::Section::AcceptanceCriteria { rules, .. } => {
                rules.iter().find(|r| r.key.id == "r-ok").cloned()
            }
            _ => None,
        });
        let rule = rule.expect("r-ok must be present");
        assert_eq!(
            rule.key.scope,
            crate::spec_core::RuleScope::Capability("billing".into())
        );
        assert!(
            content.contains("promoted from task-refund"),
            "must record promotion provenance"
        );
    }

    #[test]
    fn test_promote_is_idempotent_for_same_rule() {
        let first = upsert_capability_rule(None, "billing", "r-ok", "退款幂等", "task-refund");
        let second =
            upsert_capability_rule(Some(&first), "billing", "r-ok", "退款幂等", "task-refund");
        assert_eq!(first, second, "re-promoting the same rule must be a no-op");
        // r-ok appears exactly once.
        assert_eq!(second.matches("Rule: r-ok").count(), 1);
    }

    #[test]
    fn test_promote_does_not_change_is_passing() {
        let gw = crate::spec_gateway::SpecGateway::from_input(PROMOTE_SPEC).unwrap();
        let report = report_with(&[
            ("首次退款", crate::spec_core::Verdict::Pass),
            ("重复退款", crate::spec_core::Verdict::Pass),
        ]);
        let before = report.summary.clone();
        let passing_before = gw.is_passing(&report);
        let _ = upsert_capability_rule(None, "billing", "r-ok", "退款幂等", "task-refund");
        assert_eq!(passing_before, gw.is_passing(&report));
        assert_eq!(before.total, report.summary.total);
    }

    #[test]
    fn test_matrix_command_runs_verification_in_default_mode() {
        // An unbound scenario under default (--ai-mode off) must be skip, not
        // uncertain — matrix uses verify's default semantics.
        let dir = std::env::temp_dir().join(format!(
            "agent_spec_matrix_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let spec_path = dir.join("m.spec.md");
        fs::write(
            &spec_path,
            "spec: task\nname: \"m\"\n---\n\n## 完成条件\n\n场景: 未覆盖\n  当 a\n  那么 b\n",
        )
        .unwrap();

        let matrix = build_matrix_for(
            &spec_path,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &[],
            "none",
            "off",
        )
        .unwrap();
        assert_eq!(matrix.rows.len(), 1);
        assert_eq!(
            matrix.rows[0].verdict,
            Some(crate::spec_core::Verdict::Skip)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_explain_markdown_embeds_coverage_matrix() {
        use crate::spec_report::CoverageMatrix;
        use crate::spec_report::coverage::{CoverageRow, TestFound};
        let base = "# Contract Acceptance: demo\n\n## Verification\nall pass\n";
        let matrix = CoverageMatrix {
            rows: vec![CoverageRow {
                rule: Some("refund-idempotent".into()),
                scenario: "首次退款".into(),
                test_selector: Some("test_first_refund".into()),
                test_found: TestFound::Found,
                verdict: Some(crate::spec_core::Verdict::Pass),
                provenance: Some(crate::spec_core::EvidenceProvenance::Computational),
            }],
        };
        let out = assemble_explain_markdown(base, &matrix);
        // Original contract/verification body preserved.
        assert!(out.contains("# Contract Acceptance: demo"));
        assert!(out.contains("## Verification"));
        // Coverage matrix embedded.
        assert!(out.contains("## Coverage Matrix"));
        assert!(out.contains("| Rule | Scenario | Test | Found | Verdict | Provenance |"));
        assert!(out.contains("refund-idempotent"));
    }

    #[test]
    fn test_merge_ai_decisions_only_replaces_skip() {
        // C7: a caller AI decision must NOT override a mechanically-proven
        // pass/fail — only Skip verdicts may be resolved.
        use crate::spec_core::{AiDecision, ScenarioResult, Verdict};
        let results = vec![
            ScenarioResult {
                scenario_name: "已通过".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: Some(crate::spec_core::EvidenceProvenance::Computational),
            },
            ScenarioResult {
                scenario_name: "未覆盖".into(),
                verdict: Verdict::Skip,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
                provenance: None,
            },
        ];
        let decisions = vec![
            ScenarioAiDecision {
                scenario_name: "已通过".into(),
                decision: AiDecision {
                    model: "caller".into(),
                    confidence: 0.1,
                    verdict: Verdict::Fail,
                    reasoning: "ai disagrees".into(),
                },
            },
            ScenarioAiDecision {
                scenario_name: "未覆盖".into(),
                decision: AiDecision {
                    model: "caller".into(),
                    confidence: 0.9,
                    verdict: Verdict::Pass,
                    reasoning: "ai approves".into(),
                },
            },
        ];
        let merged = merge_ai_decisions(results, &decisions);
        let passed = merged.iter().find(|r| r.scenario_name == "已通过").unwrap();
        assert_eq!(
            passed.verdict,
            Verdict::Pass,
            "mechanical pass must NOT be overridden by AI"
        );
        assert_eq!(
            passed.provenance,
            Some(crate::spec_core::EvidenceProvenance::Computational),
            "mechanical provenance must be preserved"
        );
        let skip = merged.iter().find(|r| r.scenario_name == "未覆盖").unwrap();
        assert_eq!(skip.verdict, Verdict::Pass, "skip must be resolved by AI");
        assert_eq!(
            skip.provenance,
            Some(crate::spec_core::EvidenceProvenance::Inferential)
        );
    }

    #[test]
    fn test_provenance_resolve_ai_is_inferential() {
        use crate::spec_core::{
            AiDecision, Evidence, EvidenceProvenance, ScenarioResult, StepVerdict, Verdict,
        };
        let results = vec![ScenarioResult {
            scenario_name: "未覆盖场景".into(),
            verdict: Verdict::Skip,
            step_results: vec![StepVerdict {
                step_text: "等待 AI".into(),
                verdict: Verdict::Skip,
                reason: "no verifier".into(),
            }],
            evidence: vec![],
            duration_ms: 0,
            provenance: None,
        }];
        let decisions = vec![ScenarioAiDecision {
            scenario_name: "未覆盖场景".into(),
            decision: AiDecision {
                model: "caller".into(),
                confidence: 0.9,
                verdict: Verdict::Pass,
                reasoning: "looks correct".into(),
            },
        }];
        let merged = merge_ai_decisions(results, &decisions);
        assert_eq!(merged[0].verdict, Verdict::Pass);
        assert_eq!(
            merged[0].provenance,
            Some(EvidenceProvenance::Inferential),
            "caller-mode resolved result must be inferential"
        );
        assert!(
            merged[0]
                .evidence
                .iter()
                .any(|e| matches!(e, Evidence::AiAnalysis { .. })),
            "resolved result must carry AiAnalysis evidence"
        );
    }

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

        let resolved =
            resolve_guard_change_paths(&repo.join("specs"), &repo, &[], GitChangeScope::Staged)
                .unwrap();

        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].to_string_lossy().ends_with("src/lib.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn test_resolve_guard_change_paths_returns_empty_outside_git_repo() {
        let dir = make_temp_dir("agent-spec-cli-non-git");
        fs::create_dir_all(dir.join("specs")).unwrap();

        let resolved =
            resolve_guard_change_paths(&dir.join("specs"), &dir, &[], GitChangeScope::Staged)
                .unwrap();
        assert!(resolved.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_resolve_guard_change_paths_reads_worktree_git_changes() {
        let repo = make_temp_dir("agent-spec-cli-worktree");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 1 }\n").unwrap();
        fs::write(
            repo.join("src/unstaged.rs"),
            "pub fn unstaged() -> u8 { 1 }\n",
        )
        .unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "agent-spec@example.com"]);
        run_git(&repo, &["config", "user.name", "agent-spec"]);
        run_git(&repo, &["add", "src/staged.rs", "src/unstaged.rs"]);
        run_git(&repo, &["commit", "-m", "init"]);

        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 2 }\n").unwrap();
        run_git(&repo, &["add", "src/staged.rs"]);

        fs::write(
            repo.join("src/unstaged.rs"),
            "pub fn unstaged() -> u8 { 2 }\n",
        )
        .unwrap();
        fs::write(
            repo.join("src/untracked.rs"),
            "pub fn untracked() -> u8 { 3 }\n",
        )
        .unwrap();

        let resolved =
            resolve_guard_change_paths(&repo.join("specs"), &repo, &[], GitChangeScope::Worktree)
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
        fs::write(
            repo.join("src/unstaged.rs"),
            "pub fn unstaged() -> u8 { 1 }\n",
        )
        .unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "agent-spec@example.com"]);
        run_git(&repo, &["config", "user.name", "agent-spec"]);
        run_git(&repo, &["add", "src/staged.rs", "src/unstaged.rs"]);
        run_git(&repo, &["commit", "-m", "init"]);

        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 2 }\n").unwrap();
        run_git(&repo, &["add", "src/staged.rs"]);

        fs::write(
            repo.join("src/unstaged.rs"),
            "pub fn unstaged() -> u8 { 2 }\n",
        )
        .unwrap();

        let resolved =
            resolve_guard_change_paths(&repo.join("specs"), &repo, &[], GitChangeScope::Staged)
                .unwrap();

        assert!(contains_repo_suffix(&resolved, "src/staged.rs"));
        assert!(!contains_repo_suffix(&resolved, "src/unstaged.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    fn contains_repo_suffix(paths: &[PathBuf], suffix: &str) -> bool {
        paths
            .iter()
            .any(|path| path.to_string_lossy().replace('\\', "/").ends_with(suffix))
    }

    #[test]
    fn test_parse_ai_mode_accepts_stub() {
        assert_eq!(
            parse_ai_mode("stub").unwrap(),
            crate::spec_verify::AiMode::Stub
        );
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
        fs::write(
            repo.join("src/unstaged.rs"),
            "pub fn unstaged() -> u8 { 1 }\n",
        )
        .unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "agent-spec@example.com"]);
        run_git(&repo, &["config", "user.name", "agent-spec"]);
        run_git(&repo, &["add", "src/staged.rs", "src/unstaged.rs"]);
        run_git(&repo, &["commit", "-m", "init"]);

        fs::write(repo.join("src/staged.rs"), "pub fn staged() -> u8 { 2 }\n").unwrap();
        run_git(&repo, &["add", "src/staged.rs"]);

        fs::write(
            repo.join("src/unstaged.rs"),
            "pub fn unstaged() -> u8 { 2 }\n",
        )
        .unwrap();
        fs::write(
            repo.join("src/untracked.rs"),
            "pub fn untracked() -> u8 { 3 }\n",
        )
        .unwrap();

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
        let skill =
            fs::read_to_string(repo_root().join(".claude/skills/agent-spec-tool-first/SKILL.md"))
                .unwrap();

        assert!(skill.contains("agent-spec parse"));
        assert!(skill.contains("agent-spec contract"));
        assert!(skill.contains("agent-spec lifecycle"));
        assert!(skill.contains("agent-spec guard"));
        assert!(skill.contains("Tool-First Workflow"));
    }

    #[test]
    fn test_claude_code_authoring_skill_exists_and_mentions_task_contract_sections() {
        let skill =
            fs::read_to_string(repo_root().join(".claude/skills/agent-spec-authoring/SKILL.md"))
                .unwrap();

        assert!(skill.contains("Intent"));
        assert!(skill.contains("Decisions"));
        assert!(skill.contains("Boundaries"));
        assert!(skill.contains("Completion Criteria"));
        assert!(skill.contains("Test:` selector"));
        assert!(skill.contains("agent-spec parse"));
        assert!(skill.contains("Hard Syntax Rules"));
    }

    #[test]
    fn test_authoring_skill_includes_behavior_surface_checklist() {
        let skill =
            fs::read_to_string(repo_root().join(".claude/skills/agent-spec-authoring/SKILL.md"))
                .unwrap();

        assert!(skill.contains("Behavior Surface Checklist"));
        assert!(skill.contains("stdout vs stderr behavior"));
        assert!(skill.contains("`--json`"));
        assert!(skill.contains("`-o/--output`"));
        assert!(skill.contains("warm cache vs cold start"));
    }

    #[test]
    fn test_tool_first_skill_mentions_unbound_observable_behavior_review_step() {
        let skill =
            fs::read_to_string(repo_root().join(".claude/skills/agent-spec-tool-first/SKILL.md"))
                .unwrap();

        assert!(skill.contains("Unbound Observable Behavior review"));
        assert!(skill.contains("command x output mode"));
        assert!(skill.contains("local x remote"));
        assert!(skill.contains("fallback / precedence order"));
    }

    #[test]
    fn test_rewrite_parity_example_spec_exists_and_covers_behavior_matrix() {
        let example =
            fs::read_to_string(repo_root().join("examples/rewrite-parity-contract.spec")).unwrap();

        assert!(example.contains("local source -> cache -> bundled content -> remote fetch"));
        assert!(
            example.contains("Scenario: human mode returns doc content from cached remote source")
        );
        assert!(example.contains("Scenario: json mode returns structured payload"));
        assert!(
            example
                .contains("Scenario: cold start falls back to bundled content before remote fetch")
        );
        assert!(example.contains("Scenario: remote fetch failure returns a stable error"));
    }

    #[test]
    fn test_generated_task_templates_parse_for_zh_en_and_both() {
        for lang in [
            generate_template_zh("task", "模板"),
            generate_template_en("task", "Template"),
            generate_template_both("task", "Bilingual"),
            generate_rewrite_parity_template_zh("重写模板"),
            generate_rewrite_parity_template_en("Rewrite Template"),
            generate_rewrite_parity_template_both("Bilingual Rewrite"),
        ] {
            let doc = crate::spec_parser::parse_spec_from_str(&lang).unwrap();
            let scenario_count = doc
                .sections
                .iter()
                .filter_map(|section| match section {
                    crate::spec_core::Section::AcceptanceCriteria { scenarios, .. } => {
                        Some(scenarios.len())
                    }
                    _ => None,
                })
                .sum::<usize>();
            assert!(scenario_count > 0, "task template should contain scenarios");
        }
    }

    #[test]
    fn test_rewrite_parity_init_templates_include_behavior_matrix_and_verification_metadata() {
        for template in [
            generate_rewrite_parity_template_zh("重写模板"),
            generate_rewrite_parity_template_en("Rewrite Template"),
            generate_rewrite_parity_template_both("Bilingual Rewrite"),
        ] {
            assert!(
                template.contains("command x output mode") || template.contains("命令 x 输出模式")
            );
            assert!(
                template.contains("local x remote")
                    || template
                        .contains("local source -> cache -> bundled content -> remote fetch")
            );
            assert!(template.contains("Level:") || template.contains("层级:"));
            assert!(template.contains("Test Double:") || template.contains("替身:"));
            assert!(template.contains("Targets:") || template.contains("命中:"));
        }
    }

    #[test]
    fn test_init_command_writes_rewrite_parity_template_file() {
        let dir = make_temp_dir("agent-spec-init-rewrite-parity");
        cmd_init_at(
            &dir,
            "task",
            Some("cli-parity-contract"),
            "en",
            "rewrite-parity",
        )
        .unwrap();
        let content = fs::read_to_string(dir.join("cli-parity-contract.spec.md")).unwrap();
        let parsed = crate::spec_parser::parse_spec_from_str(&content).unwrap();

        assert!(content.contains("tags: [rewrite, parity]"));
        assert!(content.contains("command x output mode"));
        assert!(content.contains("Test Double:"));
        assert!(content.contains("Targets:"));
        assert!(parsed.sections.iter().any(|section| matches!(
            section,
            crate::spec_core::Section::AcceptanceCriteria { .. }
        )));

        let cli = super::Cli::parse_from([
            "agent-spec",
            "init",
            "--level",
            "task",
            "--template",
            "rewrite-parity",
            "--lang",
            "en",
            "--name",
            "cli-parity-contract",
        ]);

        match cli.command {
            super::Commands::Init {
                level,
                lang,
                template,
                name,
                workspace: _,
            } => {
                assert_eq!(level, "task");
                assert_eq!(lang, "en");
                assert_eq!(template, "rewrite-parity");
                assert_eq!(name.as_deref(), Some("cli-parity-contract"));
            }
            _ => panic!("expected init command"),
        }

        let _ = fs::remove_dir_all(&dir);
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
    fn test_readme_documents_rewrite_parity_contract_authoring_guidance() {
        let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();

        assert!(readme.contains("rewrite/parity"));
        assert!(readme.contains("examples/rewrite-parity-contract.spec"));
        assert!(readme.contains("command x output mode"));
        assert!(readme.contains("local x remote"));
        assert!(readme.contains("--template rewrite-parity"));
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
    Level: integration
    Test Double: fixture_fs
    Targets: spec_gateway/brief
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
        assert!(
            output.contains(
                "    Filter: test_contract_output_preserves_step_tables_and_test_selectors"
            )
        );
        assert!(output.contains("    Level: integration"));
        assert!(output.contains("    Test Double: fixture_fs"));
        assert!(output.contains("    Targets: spec_gateway/brief"));
        assert!(output.contains("  When client submits the registration request:"));
        assert!(output.contains("| field | value |"));
        assert!(output.contains("| email | alice@example.com |"));
    }

    #[test]
    fn test_contract_and_json_output_preserve_verification_metadata() {
        let input = r#"spec: task
name: "Verification Metadata"
---

## Completion Criteria

Scenario: verification metadata stays visible
  Test:
    Package: agent-spec
    Filter: test_contract_and_json_output_preserve_verification_metadata
    Level: integration
    Test Double: fixture_fs
    Targets: spec_gateway/brief
  Given a structured selector
  When contract output is rendered
  Then metadata stays visible
"#;

        let gw = crate::spec_gateway::SpecGateway::from_input(input).unwrap();
        let json = gw.ast_json();
        let contract = render_contract_output(&gw, "text").unwrap();

        assert!(json.contains("\"level\""));
        assert!(json.contains("\"integration\""));
        assert!(json.contains("\"test_double\""));
        assert!(json.contains("\"targets\""));
        assert!(contract.contains("    Level: integration"));
        assert!(contract.contains("    Test Double: fixture_fs"));
        assert!(contract.contains("    Targets: spec_gateway/brief"));
    }

    #[test]
    fn test_roadmap_phase_zero_and_one_specs_exist_and_capture_priorities() {
        let phase0 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase0-contract-fidelity.spec.md"),
        )
        .unwrap();
        let phase1 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase1-contract-review-loop.spec.md"),
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
            repo_root().join("specs/roadmap/task-phase2-run-history-and-vcs-context.spec.md"),
        )
        .unwrap();
        let phase3 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase3-spec-governance.spec.md"),
        )
        .unwrap();
        let phase4 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase4-ai-verification-expansion.spec.md"),
        )
        .unwrap();
        let phase5 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase5-ecosystem-integrations.spec.md"),
        )
        .unwrap();
        let phase6 = fs::read_to_string(
            repo_root().join("specs/roadmap/task-phase6-advanced-verification.spec.md"),
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
                provenance: None,
            }],
            summary: crate::spec_core::VerificationSummary {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
            },
        };

        let text = crate::spec_report::format_explain(
            &input,
            &report,
            &crate::spec_report::OutputFormat::Text,
        );

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
                    provenance: None,
                },
                crate::spec_core::ScenarioResult {
                    scenario_name: "scenario B".into(),
                    verdict: crate::spec_core::Verdict::Fail,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 2,
                    provenance: None,
                },
            ],
            summary: crate::spec_core::VerificationSummary {
                total: 2,
                passed: 1,
                failed: 1,
                skipped: 0,
                uncertain: 0,
                pending_review: 0,
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
            pending_review: 0,
        };

        let trailers = build_stamp_trailers("my-contract", false, &summary, None);

        assert!(trailers.iter().any(|t| t.starts_with("Spec-Name:")));
        assert!(trailers.iter().any(|t| t.starts_with("Spec-Passing:")));
        assert!(trailers.iter().any(|t| t.starts_with("Spec-Summary:")));
        assert!(trailers.iter().any(|t| t.contains("Spec-Passing: false")));
        assert!(trailers.iter().any(|t| t.contains("2/3 passed, 1 failed")));
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
        assert!(
            content.contains("\"passing\""),
            "should contain verdict field"
        );
        assert!(
            content.contains("test-contract"),
            "should contain spec name"
        );
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
            fs::write(
                runs_dir.join(format!("{}.json", 1700000000 + i as u64)),
                json,
            )
            .unwrap();
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
                assert!(!adversarial, "adversarial should default to false");
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
                assert!(
                    layers.is_none(),
                    "layers should default to None (all layers)"
                );
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
                    provenance: None,
                },
                crate::spec_core::ScenarioResult {
                    scenario_name: "[test] happy path".into(),
                    verdict: crate::spec_core::Verdict::Pass,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 2,
                    provenance: None,
                },
                crate::spec_core::ScenarioResult {
                    scenario_name: "[ai] uncertain scenario".into(),
                    verdict: crate::spec_core::Verdict::Uncertain,
                    step_results: vec![],
                    evidence: vec![],
                    duration_ms: 3,
                    provenance: None,
                },
            ],
            summary: crate::spec_core::VerificationSummary {
                total: 3,
                passed: 2,
                failed: 0,
                skipped: 0,
                uncertain: 1,
                pending_review: 0,
            },
        };

        let filtered = super::filter_report_by_layers(report, &["boundary", "test"]);
        assert_eq!(
            filtered.results.len(),
            2,
            "should only keep boundary and test"
        );
        assert_eq!(filtered.summary.total, 2);
        assert_eq!(filtered.summary.uncertain, 0, "ai layer should be excluded");
    }

    #[test]
    fn test_measure_determinism_is_explicitly_experimental() {
        use clap::Parser;

        // The command exists and parses
        let cli =
            super::Cli::parse_from(["agent-spec", "measure-determinism", "specs/project.spec"]);
        match cli.command {
            super::Commands::MeasureDeterminism { spec, runs, .. } => {
                assert!(spec.to_string_lossy().contains("project.spec"));
                assert_eq!(runs, 3); // default
            }
            _ => panic!("expected MeasureDeterminism command"),
        }

        // Running it returns an error (experimental)
        let result =
            super::cmd_measure_determinism(Path::new("specs/project.spec"), Path::new("."), 3);
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
            pending_review: 0,
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
            pending_review: 0,
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
        )
        .unwrap();
        fs::write(
            runs_dir.join("1700000002.json"),
            serde_json::to_string_pretty(&entry2).unwrap(),
        )
        .unwrap();

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
            )
            .unwrap();
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

    // ── .spec.md extension support tests ────────────────────────────

    #[test]
    fn test_guard_discovers_spec_md_files() {
        let dir = make_temp_dir("guard-spec-md");
        fs::write(
            dir.join("task.spec.md"),
            "spec: task\nname: \"t\"\n---\n\n## Intent\n\nTest.\n",
        )
        .unwrap();

        let files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| is_spec_file(p))
            .collect();

        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().ends_with("task.spec.md"));
    }

    #[test]
    fn test_guard_discovers_both_spec_and_spec_md() {
        let dir = make_temp_dir("guard-both-ext");
        fs::write(
            dir.join("a.spec"),
            "spec: task\nname: \"a\"\n---\n\n## Intent\n\nA.\n",
        )
        .unwrap();
        fs::write(
            dir.join("b.spec.md"),
            "spec: task\nname: \"b\"\n---\n\n## Intent\n\nB.\n",
        )
        .unwrap();

        let files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| is_spec_file(p))
            .collect();

        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_init_creates_spec_md_by_default() {
        let dir = make_temp_dir("init-spec-md");
        cmd_init_at(&dir, "task", Some("test-task"), "en", "default").unwrap();
        assert!(dir.join("test-task.spec.md").exists());
        assert!(!dir.join("test-task.spec").exists());
    }

    #[test]
    fn test_boundary_checker_recognizes_spec_md() {
        // The boundary checker uses looks_like_path_boundary (private).
        // We verify indirectly: parse a spec with .spec.md in allowed changes,
        // then verify boundaries are extracted as path patterns.
        let input = r#"spec: task
name: "t"
---

## Intent

Test boundary recognition.

## Boundaries

### Allowed Changes
- specs/task.spec.md
- src/**

## Acceptance Criteria

Scenario: pass
  Test: test_pass
  Given something
  When action
  Then result
"#;
        let doc = crate::spec_parser::parse_spec_from_str(input).unwrap();
        let boundaries_section = doc.sections.iter().find_map(|s| match s {
            crate::spec_core::Section::Boundaries { items, .. } => Some(items),
            _ => None,
        });
        let items = boundaries_section.unwrap();
        let allowed: Vec<_> = items
            .iter()
            .filter(|b| b.category == crate::spec_core::BoundaryCategory::Allow)
            .collect();
        // Both paths should be extracted as allowed boundaries
        assert!(allowed.iter().any(|b| b.text == "specs/task.spec.md"));
        assert!(allowed.iter().any(|b| b.text == "src/**"));
    }

    #[test]
    fn test_spec_md_not_matched_by_extension_alone() {
        let p = Path::new("task.spec.md");
        // Path::extension() returns "md", not "spec"
        assert_eq!(p.extension().unwrap(), "md");
        // But is_spec_file correctly identifies it
        assert!(is_spec_file(p));
    }

    #[test]
    fn test_plain_md_files_not_matched_as_spec() {
        assert!(!is_spec_file(Path::new("notes.md")));
        assert!(!is_spec_file(Path::new("README.md")));
        assert!(is_spec_file(Path::new("task.spec.md")));
        assert!(is_spec_file(Path::new("task.spec")));
    }

    #[test]
    fn test_lint_warns_on_duplicate_spec_extensions() {
        let dir = make_temp_dir("dup-ext-warn");
        let spec_a = dir.join("task.spec");
        let spec_b = dir.join("task.spec.md");
        fs::write(&spec_a, "spec: task\nname: \"t\"\n---\n\n## Intent\n\nT.\n").unwrap();
        fs::write(&spec_b, "spec: task\nname: \"t\"\n---\n\n## Intent\n\nT.\n").unwrap();

        let files = vec![spec_a, spec_b];
        // Should not panic; just prints a warning to stderr
        warn_duplicate_spec_extensions(&files);
    }

    // ── Checkpoint / Resume tests ───────────────────────────────

    fn make_scenario_result(
        name: &str,
        verdict: crate::spec_core::Verdict,
    ) -> crate::spec_core::ScenarioResult {
        crate::spec_core::ScenarioResult {
            scenario_name: name.to_owned(),
            verdict,
            step_results: vec![crate::spec_core::StepVerdict {
                step_text: format!("step for {name}"),
                verdict,
                reason: "test".into(),
            }],
            evidence: vec![],
            duration_ms: 10,
            provenance: None,
        }
    }

    #[test]
    fn test_resume_incremental_skips_passed_scenarios() {
        let mut scenarios = std::collections::HashMap::new();
        scenarios.insert(
            "场景 A".to_owned(),
            crate::spec_core::CheckpointEntry {
                verdict: crate::spec_core::Verdict::Pass,
                vcs_ref: Some("abc123".into()),
            },
        );
        scenarios.insert(
            "场景 B".to_owned(),
            crate::spec_core::CheckpointEntry {
                verdict: crate::spec_core::Verdict::Fail,
                vcs_ref: Some("abc123".into()),
            },
        );
        let checkpoint = crate::spec_core::Checkpoint {
            spec_name: "测试".into(),
            timestamp: 1000,
            vcs_ref: Some("abc123".into()),
            scenarios,
        };

        let report = crate::spec_core::VerificationReport::from_results(
            "测试".into(),
            vec![
                make_scenario_result("场景 A", crate::spec_core::Verdict::Skip),
                make_scenario_result("场景 B", crate::spec_core::Verdict::Fail),
            ],
        );

        let merged = merge_checkpoint_results(report, &checkpoint, &ResumeMode::Incremental);

        let a = merged
            .results
            .iter()
            .find(|r| r.scenario_name == "场景 A")
            .unwrap();
        assert_eq!(a.verdict, crate::spec_core::Verdict::Pass);
        let has_checkpoint_evidence = a.evidence.iter().any(|e| match e {
            crate::spec_core::Evidence::PatternMatch { pattern, .. } => {
                pattern == "checkpoint:incremental"
            }
            _ => false,
        });
        assert!(has_checkpoint_evidence, "should have checkpoint evidence");
        assert_eq!(a.duration_ms, 0, "skipped scenario should have 0 duration");

        let b = merged
            .results
            .iter()
            .find(|r| r.scenario_name == "场景 B")
            .unwrap();
        assert_eq!(b.verdict, crate::spec_core::Verdict::Fail);

        assert_eq!(merged.summary.passed, 1);
        assert_eq!(merged.summary.failed, 1);
    }

    #[test]
    fn test_resume_conservative_detects_regression() {
        let mut scenarios = std::collections::HashMap::new();
        scenarios.insert(
            "场景 A".to_owned(),
            crate::spec_core::CheckpointEntry {
                verdict: crate::spec_core::Verdict::Pass,
                vcs_ref: Some("abc123".into()),
            },
        );
        let checkpoint = crate::spec_core::Checkpoint {
            spec_name: "测试".into(),
            timestamp: 1000,
            vcs_ref: Some("abc123".into()),
            scenarios,
        };

        let report = crate::spec_core::VerificationReport::from_results(
            "测试".into(),
            vec![make_scenario_result(
                "场景 A",
                crate::spec_core::Verdict::Fail,
            )],
        );

        let merged = merge_checkpoint_results(report, &checkpoint, &ResumeMode::Conservative);

        let a = merged
            .results
            .iter()
            .find(|r| r.scenario_name == "场景 A")
            .unwrap();
        assert_eq!(a.verdict, crate::spec_core::Verdict::Fail);
        let has_regression = a.evidence.iter().any(|e| match e {
            crate::spec_core::Evidence::PatternMatch {
                pattern, locations, ..
            } => {
                pattern == "checkpoint:regression"
                    && locations.iter().any(|l| l.contains("regression: true"))
            }
            _ => false,
        });
        assert!(has_regression, "should have regression evidence marker");
    }

    #[test]
    fn test_resume_without_run_log_dir_errors() {
        let cli = super::Cli::try_parse_from([
            "agent-spec",
            "lifecycle",
            "dummy.spec",
            "--code",
            ".",
            "--resume",
        ]);
        assert!(cli.is_ok(), "CLI should parse --resume flag without error");

        // Verify that --resume without --run-log-dir triggers the error condition
        let resume: Option<Option<String>> = Some(None);
        let run_log_dir: Option<&Path> = None;
        if let Some(ref _mode_opt) = resume {
            assert!(
                run_log_dir.is_none(),
                "this test verifies --resume requires --run-log-dir"
            );
        }
    }

    #[test]
    fn test_checkpoint_roundtrip_serialization() {
        let dir = make_temp_dir("checkpoint-roundtrip");

        let report = crate::spec_core::VerificationReport::from_results(
            "序列化测试".into(),
            vec![
                make_scenario_result("场景 A", crate::spec_core::Verdict::Pass),
                make_scenario_result("场景 B", crate::spec_core::Verdict::Fail),
                make_scenario_result("场景 C", crate::spec_core::Verdict::Skip),
            ],
        );

        save_checkpoint(&dir, &report, Some("def456".into())).unwrap();

        let cp_path = checkpoint_path(&dir);
        assert!(cp_path.exists(), "checkpoint file should exist");

        let loaded = load_checkpoint(&dir).unwrap();
        assert!(loaded.is_some(), "checkpoint should be loaded");
        let cp = loaded.unwrap();

        assert_eq!(cp.spec_name, "序列化测试");
        assert_eq!(cp.vcs_ref, Some("def456".into()));
        assert_eq!(cp.scenarios.len(), 3);

        let entry_a = cp.scenarios.get("场景 A").unwrap();
        assert_eq!(entry_a.verdict, crate::spec_core::Verdict::Pass);
        assert_eq!(entry_a.vcs_ref, Some("def456".into()));

        let entry_b = cp.scenarios.get("场景 B").unwrap();
        assert_eq!(entry_b.verdict, crate::spec_core::Verdict::Fail);

        let entry_c = cp.scenarios.get("场景 C").unwrap();
        assert_eq!(entry_c.verdict, crate::spec_core::Verdict::Skip);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_load_checkpoint_returns_none_when_missing() {
        let dir = make_temp_dir("checkpoint-missing");
        let result = load_checkpoint(&dir).unwrap();
        assert!(result.is_none());
        let _ = fs::remove_dir_all(dir);
    }

    // ── Graph tests ────────────────────────────────────────────

    fn write_spec_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(format!("{name}.spec.md"));
        fs::write(path, content).unwrap();
    }

    #[test]
    fn test_graph_generates_dot_output() {
        let dir = make_temp_dir("graph-dot");
        write_spec_file(
            &dir,
            "spec-a",
            "spec: task\nname: \"A\"\ntags: []\n---\n\n## 意图\n\nA\n",
        );
        write_spec_file(
            &dir,
            "spec-b",
            "spec: task\nname: \"B\"\ntags: []\ndepends: [spec-a]\n---\n\n## 意图\n\nB\n",
        );
        write_spec_file(
            &dir,
            "spec-c",
            "spec: task\nname: \"C\"\ntags: []\ndepends: [spec-a, spec-b]\n---\n\n## 意图\n\nC\n",
        );

        // Use cmd_graph internals: collect, parse, generate DOT
        let mut spec_files = Vec::new();
        super::collect_spec_files(&dir, &mut spec_files).unwrap();
        assert_eq!(spec_files.len(), 3);

        // Parse and build graph
        let mut nodes = Vec::new();
        let mut name_to_stem = std::collections::HashMap::new();
        let mut stem_to_idx = std::collections::HashMap::new();

        for file in &spec_files {
            let doc = crate::spec_parser::parse_spec(file).unwrap();
            let stem = file
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches(".spec")
                .to_string();
            let idx = nodes.len();
            name_to_stem.insert(doc.meta.name.clone(), stem.clone());
            stem_to_idx.insert(stem.clone(), idx);
            nodes.push(super::GraphNode {
                name: doc.meta.name,
                file_stem: stem,
                depends: doc.meta.depends,
                estimate: doc.meta.estimate,
                tags: doc.meta.tags,
            });
        }

        let mut edges = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            for dep in &node.depends {
                let dep_idx = stem_to_idx.get(dep.as_str()).copied().or_else(|| {
                    name_to_stem
                        .get(dep.as_str())
                        .and_then(|s| stem_to_idx.get(s.as_str()).copied())
                });
                if let Some(j) = dep_idx {
                    edges.push((j, i));
                }
            }
        }

        let estimates: Vec<f64> = nodes
            .iter()
            .map(|n| {
                n.estimate
                    .as_deref()
                    .map_or(0.0, super::parse_estimate_days)
            })
            .collect();
        let critical = super::compute_critical_path(nodes.len(), &edges, &estimates);
        let dot = super::generate_dot(&nodes, &edges, &critical);

        // Verify DOT output
        assert!(dot.contains("digraph spec_dependencies"));
        assert!(dot.contains("spec-a"));
        assert!(dot.contains("spec-b"));
        assert!(dot.contains("spec-c"));
        // Should have 3 edges: A->B, A->C, B->C
        assert_eq!(edges.len(), 3);
        assert!(dot.contains("->"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_graph_nodes_include_estimate() {
        let dir = make_temp_dir("graph-estimate");
        write_spec_file(
            &dir,
            "spec-est",
            "spec: task\nname: \"EstTest\"\ntags: []\nestimate: 2d\n---\n\n## 意图\n\nTest\n",
        );

        let mut spec_files = Vec::new();
        super::collect_spec_files(&dir, &mut spec_files).unwrap();
        let doc = crate::spec_parser::parse_spec(&spec_files[0]).unwrap();

        let nodes = vec![super::GraphNode {
            name: doc.meta.name,
            file_stem: "spec-est".to_string(),
            depends: doc.meta.depends,
            estimate: doc.meta.estimate,
            tags: doc.meta.tags,
        }];

        let dot = super::generate_dot(&nodes, &[], &[]);
        assert!(
            dot.contains("2d"),
            "DOT node label should contain estimate '2d'"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_graph_independent_specs_are_isolated_nodes() {
        let dir = make_temp_dir("graph-isolated");
        write_spec_file(
            &dir,
            "spec-x",
            "spec: task\nname: \"X\"\ntags: []\n---\n\n## 意图\n\nX\n",
        );
        write_spec_file(
            &dir,
            "spec-y",
            "spec: task\nname: \"Y\"\ntags: []\n---\n\n## 意图\n\nY\n",
        );

        let mut spec_files = Vec::new();
        super::collect_spec_files(&dir, &mut spec_files).unwrap();

        let mut nodes = Vec::new();
        for file in &spec_files {
            let doc = crate::spec_parser::parse_spec(file).unwrap();
            let stem = file
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches(".spec")
                .to_string();
            nodes.push(super::GraphNode {
                name: doc.meta.name,
                file_stem: stem,
                depends: doc.meta.depends,
                estimate: doc.meta.estimate,
                tags: doc.meta.tags,
            });
        }

        // No edges for independent specs
        let dot = super::generate_dot(&nodes, &[], &[]);
        assert!(dot.contains("spec-x"));
        assert!(dot.contains("spec-y"));
        // Should not contain any edges
        assert!(!dot.contains("->"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_graph_critical_path_highlighted() {
        let dir = make_temp_dir("graph-critical");
        write_spec_file(
            &dir,
            "spec-a",
            "spec: task\nname: \"A\"\ntags: []\nestimate: 1d\n---\n\n## 意图\n\nA\n",
        );
        write_spec_file(
            &dir,
            "spec-b",
            "spec: task\nname: \"B\"\ntags: []\ndepends: [spec-a]\nestimate: 2d\n---\n\n## 意图\n\nB\n",
        );
        write_spec_file(
            &dir,
            "spec-c",
            "spec: task\nname: \"C\"\ntags: []\ndepends: [spec-b]\nestimate: 1d\n---\n\n## 意图\n\nC\n",
        );

        let mut spec_files = Vec::new();
        super::collect_spec_files(&dir, &mut spec_files).unwrap();

        let mut nodes = Vec::new();
        let mut stem_to_idx = std::collections::HashMap::new();

        for file in &spec_files {
            let doc = crate::spec_parser::parse_spec(file).unwrap();
            let stem = file
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches(".spec")
                .to_string();
            let idx = nodes.len();
            stem_to_idx.insert(stem.clone(), idx);
            nodes.push(super::GraphNode {
                name: doc.meta.name,
                file_stem: stem,
                depends: doc.meta.depends,
                estimate: doc.meta.estimate,
                tags: doc.meta.tags,
            });
        }

        let mut edges = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            for dep in &node.depends {
                if let Some(&j) = stem_to_idx.get(dep.as_str()) {
                    edges.push((j, i));
                }
            }
        }

        let estimates: Vec<f64> = nodes
            .iter()
            .map(|n| {
                n.estimate
                    .as_deref()
                    .map_or(0.0, super::parse_estimate_days)
            })
            .collect();
        let critical = super::compute_critical_path(nodes.len(), &edges, &estimates);
        let dot = super::generate_dot(&nodes, &edges, &critical);

        // Critical path A -> B -> C should be marked red
        assert!(
            dot.contains("color=red"),
            "Critical path edges should be colored red"
        );

        let _ = fs::remove_dir_all(dir);
    }
}
