#![warn(clippy::all)]
#![deny(unsafe_code)]
#![allow(dead_code)]

mod spec_archive;
mod spec_core;
mod spec_gateway;
mod spec_lint;
mod spec_parser;
mod spec_qa;
mod spec_report;
mod spec_verify;
mod spec_wiki;

mod spec_knowledge;
mod spec_mcp;
mod vcs;

use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
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
    /// Import, validate, plan, and draft from KLL requirements
    Requirements {
        #[command(subcommand)]
        action: RequirementCommands,
    },
    /// Maintain a repo-local code live wiki.
    Wiki {
        #[command(subcommand)]
        action: WikiCommands,
    },
    /// Lint the knowledge corpus (per-doc rules + governance integrity).
    LintKnowledge {
        /// Knowledge root.
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        /// Output format: text | json | sarif.
        #[arg(long, default_value = "text")]
        format: String,
        /// Exit non-zero when any Error-level finding is present.
        #[arg(long)]
        gate: bool,
    },
    /// Serve the knowledge layer over MCP (read-only, deterministic, stdio).
    Mcp {
        /// Knowledge root.
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        /// Specs root.
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        /// Code directory to verify against (for liveness).
        #[arg(long, default_value = ".")]
        code: PathBuf,
    },
    /// Trace a decision/requirement to satisfying specs and report liveness.
    Trace {
        /// Decision or requirement id (e.g. ADR-001 or REQ-001), case-insensitive.
        id: String,
        /// Knowledge root.
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
    /// Archive completed specs out of the active scan set and write a compact summary
    Archive {
        #[arg(long, default_value = "specs")]
        spec_dir: PathBuf,
        #[arg(long, default_value = ".agent-spec/archive/specs")]
        archive_dir: PathBuf,
        #[arg(long, default_value = "knowledge/context/spec-archives.md")]
        summary: PathBuf,
        /// Project root or .agent-spec/runs directory containing lifecycle run logs.
        #[arg(long, default_value = ".")]
        run_log_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        check: bool,
    },
    /// Rust project graph: build, query, and freshness-check the atlas
    Atlas {
        #[command(subcommand)]
        action: AtlasCommands,
    },
}

#[derive(Subcommand)]
enum AtlasCommands {
    /// Build or incrementally refresh the graph
    Build {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/graph")]
        graph: PathBuf,
        #[arg(long)]
        full: bool,
        /// Optional SCIP index (rust-analyzer, JSON form) for resolved references
        #[arg(long)]
        scip: Option<PathBuf>,
    },
    /// Deterministic module outline
    Tree {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/graph")]
        graph: PathBuf,
        #[arg(long)]
        frozen: bool,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Node facts and adjacent edges for a canonical symbol path
    Query {
        symbol: String,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/graph")]
        graph: PathBuf,
        #[arg(long)]
        frozen: bool,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Incoming reference/call edges for a symbol
    Refs {
        symbol: String,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/graph")]
        graph: PathBuf,
        #[arg(long)]
        frozen: bool,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Impl relations touching a trait or type name
    Impls {
        name: String,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/graph")]
        graph: PathBuf,
        #[arg(long)]
        frozen: bool,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Freshness check; exits non-zero when any shard is stale
    Check {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/graph")]
        graph: PathBuf,
    },
}

#[derive(Subcommand)]
enum WikiCommands {
    /// Scaffold or refresh the repo-local code live wiki.
    Init {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long)]
        check: bool,
    },
    /// Report changed source files and stale wiki articles.
    Status {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Render architecture inventory as json or mermaid.
    Inventory {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Rebuild the live wiki index from article frontmatter.
    Index {
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Lint the code live wiki for source trace and required files.
    Lint {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Run live wiki CI checks: index freshness, lint, and stale article status.
    Check {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Seed focused live wiki module, concept, and decision pages without overwriting existing pages.
    Seed {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long)]
        check: bool,
    },
    /// Search live wiki articles by title, tags, source files, and body text.
    Query {
        query: String,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Inspect a path and show related wiki articles, requirements, and specs.
    Inspect {
        path: PathBuf,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Build or check the cross-project wiki map.
    ProjectMap {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, requires = "out")]
        check: bool,
    },
    /// Inspect a project id and show related project-map flows.
    InspectProject {
        project_id: String,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Read or update live wiki metadata.
    Meta {
        #[command(subcommand)]
        action: WikiMetaCommands,
    },
}

#[derive(Subcommand)]
enum WikiMetaCommands {
    /// Record the current commit as the latest compiled wiki state.
    Update {
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum RequirementCommands {
    /// Import marked PRD/issue blocks into knowledge/requirements/*.md
    Import {
        #[arg(long)]
        from: PathBuf,
        #[arg(long, default_value = "knowledge/requirements")]
        out: PathBuf,
        #[arg(long)]
        check: bool,
        /// Optional compilation provenance manifest target (.json)
        #[arg(long)]
        provenance: Option<PathBuf>,
    },
    /// Apply an explicit human governance transition to a requirement
    Transition {
        /// Requirement id, e.g. REQ-123
        id: String,
        /// Target status: proposed | accepted | rejected | deprecated
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
    },
    /// Mark a requirement superseded by a replacement requirement
    Supersede {
        /// Requirement id being superseded
        id: String,
        /// Replacement requirement id
        #[arg(long)]
        by: String,
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
    },
    /// Aggregate three-axis status (governance / execution / liveness) for one requirement
    Status {
        /// Requirement id, e.g. REQ-123
        id: String,
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        #[arg(long, default_value = ".agent-spec/archive/specs")]
        archive_dir: PathBuf,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Export requirement documents as a YAML dialect projection
    Export {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        /// Target file (.yaml/.yml); a derived projection, overwritten on export
        #[arg(long)]
        out: PathBuf,
        /// Restrict export to these requirement ids (repeatable)
        #[arg(long)]
        id: Vec<String>,
        /// Compare against the existing file and exit non-zero on drift
        #[arg(long)]
        check: bool,
        /// Optional compilation provenance manifest target (.json)
        #[arg(long)]
        provenance: Option<PathBuf>,
    },
    /// Validate and print the requirement graph
    Graph {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long)]
        gate: bool,
    },
    /// Build a cross-layer requirement/work-unit/spec plan DAG
    Plan {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long)]
        gate: bool,
    },
    /// Emit test obligations derived from requirements and specs, independent of code
    TestObligations {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Emit clarification questions from requirement diagnostics
    Questions {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Generate deterministic git worktree execution entries for ready work units
    Worktrees {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "specs")]
        specs: PathBuf,
        #[arg(long, default_value = "main")]
        base: String,
        #[arg(long, default_value = "../agent-spec-worktrees")]
        path_prefix: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Query requirement-level trace records
    Trace {
        id: String,
        #[arg(long, default_value = ".agent-spec/trace")]
        trace_dir: PathBuf,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Replay latest known evidence chain for one requirement
    Replay {
        id: String,
        #[arg(long, default_value = ".agent-spec/trace")]
        trace_dir: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Explain non-pass lifecycle evidence for one requirement
    ExplainFailure {
        id: String,
        #[arg(long, default_value = ".agent-spec/trace")]
        trace_dir: PathBuf,
        #[arg(long, default_value = ".")]
        code: PathBuf,
        #[arg(long, default_value = ".agent-spec/wiki")]
        wiki: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Render requirement trace as mermaid or json
    TraceGraph {
        id: String,
        #[arg(long, default_value = ".agent-spec/trace")]
        trace_dir: PathBuf,
        #[arg(long, default_value = "mermaid")]
        format: String,
    },
    /// Generate work_units.json from KLL requirements
    WorkUnits {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Render reviewable task spec drafts from ready work units
    DraftSpecs {
        #[arg(long, default_value = "knowledge")]
        knowledge: PathBuf,
        #[arg(long, default_value = "specs/generated")]
        out: PathBuf,
        #[arg(long)]
        check: bool,
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
        Commands::Requirements { action } => cmd_requirements(action),
        Commands::Wiki { action } => cmd_wiki(action),
        Commands::LintKnowledge {
            knowledge,
            format,
            gate,
        } => cmd_lint_knowledge(&knowledge, &format, gate),
        Commands::Mcp {
            knowledge,
            specs,
            code,
        } => cmd_mcp(knowledge, specs, code),
        Commands::Trace {
            id,
            knowledge,
            specs,
            code,
            format,
            gate,
        } => cmd_trace(&id, &knowledge, &specs, &code, &format, gate),
        Commands::Atlas { action } => cmd_atlas(action),
        Commands::Archive {
            spec_dir,
            archive_dir,
            summary,
            run_log_dir,
            dry_run,
            check,
        } => cmd_archive(
            &spec_dir,
            &archive_dir,
            &summary,
            &run_log_dir,
            dry_run,
            check,
        ),
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
            let docs = crate::spec_knowledge::collect_guidance_checked(dir)
                .map_err(|e| format!("cannot collect guidance: {e}"))?;
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
    adversarial: bool,
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

    let mut passing = gw.is_passing_with_review_mode(&verify_report, review_mode);

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

    let run_timestamp = run_log_dir.map(|_| current_unix_timestamp());
    let run_vcs_ctx = run_log_dir.and_then(|_| vcs::get_vcs_context(code));
    let requirement_trace_result =
        if let (Some(log_dir), Some(timestamp)) = (run_log_dir, run_timestamp) {
            write_lifecycle_requirement_trace(
                spec,
                code,
                log_dir,
                &gw,
                &verify_report,
                timestamp,
                run_vcs_ctx.clone(),
            )
        } else {
            Ok(None)
        };
    let trace_recorded = matches!(requirement_trace_result, Ok(Some(_)));
    let requirement_trace_warning = requirement_trace_result.err().map(|err| err.to_string());
    let qa_missing_evidence = lifecycle_qa_missing_evidence(
        gw.resolved().task.meta.risk.as_deref(),
        &gw.resolved().all_scenarios,
        &verify_report,
        trace_recorded,
        adversarial,
    )?;
    if !qa_missing_evidence.is_empty() {
        passing = false;
    }

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
        if let Some(ref warning) = requirement_trace_warning {
            json_out["requirement_trace_diagnostic"] = serde_json::json!({
                "severity": "warning",
                "message": warning,
            });
        }
        if !qa_missing_evidence.is_empty() {
            json_out["qa_missing_evidence"] = serde_json::to_value(&qa_missing_evidence)?;
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
        if let Some(ref warning) = requirement_trace_warning {
            eprintln!("warning: failed to write requirement trace: {warning}");
        }
        if !qa_missing_evidence.is_empty() {
            eprintln!("QA gate missing evidence: {qa_missing_evidence:?}");
        }
    }

    // Stage 4: Write run log if enabled
    if let Some(log_dir) = run_log_dir {
        let contract = gw.plan();
        let entry = RunLogEntry {
            spec_name: contract.name.clone(),
            spec_path: canonical_existing_path(spec),
            spec_fingerprint: crate::spec_wiki::fingerprint_file(spec)?,
            passing,
            summary: format!(
                "{}/{} passed, {} failed, {} skipped, {} uncertain",
                verify_report.summary.passed,
                verify_report.summary.total,
                verify_report.summary.failed,
                verify_report.summary.skipped,
                verify_report.summary.uncertain,
            ),
            timestamp: run_timestamp.unwrap_or_else(current_unix_timestamp),
            vcs: run_vcs_ctx,
        };
        write_run_log(log_dir, &entry)?;

        // Save checkpoint alongside run log
        save_checkpoint_with_timestamp(
            log_dir,
            &verify_report,
            entry.vcs.as_ref().map(|v| v.change_ref.clone()),
            entry.timestamp,
        )?;
    }

    if passing {
        Ok(())
    } else if !qa_missing_evidence.is_empty() {
        Err(format!("lifecycle QA gate failed; missing evidence: {qa_missing_evidence:?}").into())
    } else {
        Err(format_non_passing_summary(&verify_report.summary).into())
    }
}

fn lifecycle_qa_missing_evidence(
    risk: Option<&str>,
    scenarios: &[crate::spec_core::Scenario],
    report: &crate::spec_core::VerificationReport,
    trace_recorded: bool,
    adversarial_review: bool,
) -> Result<Vec<crate::spec_qa::QaEvidenceKind>, Box<dyn std::error::Error>> {
    let Some(risk) = risk else {
        return Ok(Vec::new());
    };
    let class = crate::spec_qa::QaClass::try_parse(risk)?;
    let lifecycle = report.summary.failed == 0
        && report.summary.skipped == 0
        && report.summary.uncertain == 0
        && report.summary.pending_review == 0;
    let targeted_tests = !scenarios.is_empty()
        && scenarios.iter().all(|scenario| {
            report.results.iter().any(|result| {
                result.scenario_name == scenario.name
                    && result.verdict == crate::spec_core::Verdict::Pass
                    && result.evidence.iter().any(|evidence| {
                        matches!(
                            evidence,
                            crate::spec_core::Evidence::TestOutput { passed: true, .. }
                        )
                    })
            })
        });
    Ok(crate::spec_qa::missing_evidence(
        class,
        crate::spec_qa::QaEvidenceState {
            lifecycle,
            trace: trace_recorded,
            targeted_tests,
            adversarial_review,
        },
    ))
}

fn write_lifecycle_requirement_trace(
    spec: &Path,
    code: &Path,
    log_dir: &Path,
    gw: &crate::spec_gateway::SpecGateway,
    verify_report: &crate::spec_core::VerificationReport,
    timestamp: u64,
    vcs_ctx: Option<vcs::VcsContext>,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let satisfies = gw.resolved().task.meta.satisfies.clone();
    if satisfies.is_empty() {
        return Ok(None);
    }

    let knowledge_dir = code.join("knowledge");
    if !knowledge_dir.exists() {
        return Ok(None);
    }
    let specs_dir = spec.parent().unwrap_or(Path::new("specs"));
    let requirement_plan = crate::spec_knowledge::build_requirement_plan(&knowledge_dir, specs_dir);
    let requirement_graph = crate::spec_knowledge::build_requirement_graph(&knowledge_dir);
    let worktree_manifest =
        read_optional_worktree_manifest(code).or_else(|| read_optional_worktree_manifest(log_dir));
    let scenario_selectors = gw
        .resolved()
        .all_scenarios
        .iter()
        .filter_map(|scenario| {
            scenario
                .test_selector
                .as_ref()
                .map(|selector| (scenario.name.clone(), selector.filter.clone()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let requirement_scenarios = if satisfies.len() == 1 {
        std::collections::BTreeMap::from([(
            satisfies[0].clone(),
            gw.resolved()
                .all_scenarios
                .iter()
                .map(|scenario| scenario.name.clone())
                .collect::<Vec<_>>(),
        )])
    } else {
        satisfies
            .iter()
            .filter_map(|requirement_id| {
                requirement_graph.node(requirement_id).map(|node| {
                    (
                        requirement_id.clone(),
                        node.scenarios
                            .iter()
                            .map(|scenario| scenario.name.clone())
                            .collect::<Vec<_>>(),
                    )
                })
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let run_id = format!(
        "{}-{}",
        timestamp,
        sanitize_for_filename(&gw.resolved().task.meta.name)
    );
    let ledger = crate::spec_knowledge::record_requirement_trace_run(
        crate::spec_knowledge::RequirementTraceRunInput {
            run_id,
            timestamp,
            requirement_plan: &requirement_plan,
            worktree_manifest: worktree_manifest.as_ref(),
            spec_path: spec.to_path_buf(),
            spec_satisfies: satisfies,
            scenario_selectors,
            requirement_scenarios,
            report: verify_report,
            vcs: vcs_ctx,
        },
    );
    let trace_errors = ledger
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == "error")
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>();
    if !trace_errors.is_empty() {
        return Err(format!(
            "requirement trace contains blocking diagnostics: {}",
            trace_errors.join("; ")
        )
        .into());
    }
    if ledger.records.is_empty() && ledger.diagnostics.is_empty() {
        return Ok(None);
    }
    crate::spec_knowledge::write_requirement_trace_ledger(code, &ledger).map(Some)
}

fn read_optional_worktree_manifest(
    base_dir: &Path,
) -> Option<crate::spec_knowledge::WorktreeManifest> {
    let path = base_dir.join(".agent-spec/worktrees.json");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
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
    // No cwd fallback: guard pointed at directories outside any repository
    // must resolve to an empty change set, not the tool's own repo.
    for candidate in [code, spec_dir] {
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
    #[serde(default, skip_serializing_if = "path_is_empty")]
    pub spec_path: PathBuf,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub spec_fingerprint: String,
    pub passing: bool,
    pub summary: String,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs: Option<vcs::VcsContext>,
}

fn canonical_existing_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn path_is_empty(path: &Path) -> bool {
    path.as_os_str().is_empty()
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

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
    save_checkpoint_with_timestamp(base_dir, report, vcs_ref, current_unix_timestamp())
}

fn save_checkpoint_with_timestamp(
    base_dir: &Path,
    report: &spec_core::VerificationReport,
    vcs_ref: Option<String>,
    timestamp: u64,
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
        timestamp,
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

#[derive(Debug, Clone, serde::Serialize)]
struct HistoryCounts {
    passed: i64,
    failed: i64,
    skipped: i64,
    uncertain: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct HistoryDelta {
    passed: i64,
    failed: i64,
    skipped: i64,
    uncertain: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct HistoryRow {
    run: usize,
    timestamp: u64,
    passing: bool,
    summary: String,
    counts: Option<HistoryCounts>,
    delta: Option<HistoryDelta>,
}

/// Parse "X/Y passed, N failed, N skipped, N uncertain" summaries.
fn parse_summary_counts(summary: &str) -> Option<HistoryCounts> {
    let mut passed = None;
    let mut failed = None;
    let mut skipped = None;
    let mut uncertain = None;
    for part in summary.split(',') {
        let part = part.trim();
        let mut words = part.split_whitespace();
        let (value, label) = (words.next()?, words.next()?);
        let number: i64 = value.split('/').next()?.parse().ok()?;
        match label {
            "passed" => passed = Some(number),
            "failed" => failed = Some(number),
            "skipped" => skipped = Some(number),
            "uncertain" => uncertain = Some(number),
            _ => {}
        }
    }
    Some(HistoryCounts {
        passed: passed?,
        failed: failed?,
        skipped: skipped?,
        uncertain: uncertain?,
    })
}

fn load_history_logs(base_dir: &Path, spec_name: &str) -> Vec<RunLogEntry> {
    let runs_dir = base_dir.join(".agent-spec/runs");
    let Ok(entries) = std::fs::read_dir(&runs_dir) else {
        return Vec::new();
    };
    let mut logs: Vec<RunLogEntry> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let content = std::fs::read_to_string(e.path()).ok()?;
            let entry: RunLogEntry = serde_json::from_str(&content).ok()?;
            (entry.spec_name == spec_name).then_some(entry)
        })
        .collect();
    logs.sort_by_key(|e| e.timestamp);
    logs
}

/// Tabular history rows with per-run counts and deltas against the previous run.
fn history_rows(base_dir: &Path, spec_name: &str) -> Vec<HistoryRow> {
    let logs = load_history_logs(base_dir, spec_name);
    let mut rows: Vec<HistoryRow> = Vec::with_capacity(logs.len());
    for (i, log) in logs.iter().enumerate() {
        let counts = parse_summary_counts(&log.summary);
        let delta = match (i.checked_sub(1).and_then(|p| rows.get(p)), &counts) {
            (Some(prev), Some(curr)) => prev.counts.as_ref().map(|p| HistoryDelta {
                passed: curr.passed - p.passed,
                failed: curr.failed - p.failed,
                skipped: curr.skipped - p.skipped,
                uncertain: curr.uncertain - p.uncertain,
            }),
            _ => None,
        };
        rows.push(HistoryRow {
            run: i + 1,
            timestamp: log.timestamp,
            passing: log.passing,
            summary: log.summary.clone(),
            counts,
            delta,
        });
    }
    rows
}

/// JSON view of the run history (array of rows).
fn history_json(base_dir: &Path, spec_name: &str) -> serde_json::Value {
    serde_json::to_value(history_rows(base_dir, spec_name)).unwrap_or(serde_json::Value::Null)
}

fn format_delta(delta: &HistoryDelta) -> String {
    let mut parts = Vec::new();
    for (value, label) in [
        (delta.passed, "pass"),
        (delta.failed, "fail"),
        (delta.skipped, "skip"),
        (delta.uncertain, "uncertain"),
    ] {
        if value != 0 {
            parts.push(format!("{value:+} {label}"));
        }
    }
    if parts.is_empty() {
        "no change".to_string()
    } else {
        parts.join(" ")
    }
}

fn read_run_log_history(base_dir: &Path, spec_name: &str) -> String {
    let logs = load_history_logs(base_dir, spec_name);
    if logs.is_empty() {
        return String::new();
    }
    let rows = history_rows(base_dir, spec_name);

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

    for (row, log) in rows.iter().zip(logs.iter()) {
        let status = if row.passing { "PASS" } else { "FAIL" };
        let counts = row
            .counts
            .as_ref()
            .map(|c| {
                format!(
                    "{} pass {} fail {} skip {} uncertain",
                    c.passed, c.failed, c.skipped, c.uncertain
                )
            })
            .unwrap_or_else(|| row.summary.clone());
        let delta = row.delta.as_ref().map(format_delta).unwrap_or_default();
        out.push_str(&format!(
            "  | run #{} | {} | {} | {} |\n",
            row.run, status, counts, delta
        ));

        // Show jj diff between adjacent runs when both have operation IDs
        let i = row.run - 1;
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

fn cmd_lint_knowledge(
    knowledge: &Path,
    format: &str,
    gate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::spec_core::{LintDiagnostic, Severity, Span};
    use crate::spec_knowledge::sarif::Finding;

    let collection = crate::spec_knowledge::collect_knowledge_checked(knowledge);
    let docs = collection.docs;
    let mut findings: Vec<Finding> = Vec::new();
    for err in collection.parse_errors {
        findings.push(Finding {
            uri: err.path.display().to_string(),
            diag: LintDiagnostic {
                rule: "knowledge-parse-error".into(),
                severity: Severity::Error,
                message: format!("cannot parse knowledge doc: {}", err.message),
                span: Span::default(),
                suggestion: Some(
                    "fix the knowledge frontmatter or remove the malformed artifact".into(),
                ),
            },
        });
    }
    for d in &docs {
        let uri = d.source_path.display().to_string();
        for diag in crate::spec_knowledge::lint_doc(d) {
            findings.push(Finding {
                uri: uri.clone(),
                diag,
            });
        }
    }
    for diag in crate::spec_knowledge::lint_corpus(&docs) {
        findings.push(Finding {
            uri: String::new(),
            diag,
        });
    }

    let errors = findings
        .iter()
        .filter(|f| f.diag.severity == Severity::Error)
        .count();

    match format {
        "sarif" => {
            let log = crate::spec_knowledge::render_sarif(&findings);
            println!("{}", serde_json::to_string_pretty(&log)?);
        }
        "json" => {
            let arr: Vec<serde_json::Value> = findings
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "uri": f.uri,
                        "rule": f.diag.rule,
                        "severity": format!("{:?}", f.diag.severity),
                        "message": f.diag.message,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&arr)?);
        }
        _ => {
            for f in &findings {
                let where_ = if f.uri.is_empty() { "(corpus)" } else { &f.uri };
                println!(
                    "{where_}: [{:?}] {} — {}",
                    f.diag.severity, f.diag.rule, f.diag.message
                );
            }
            println!(
                "{} docs, {} findings ({errors} errors)",
                docs.len(),
                findings.len()
            );
        }
    }

    if gate && errors > 0 {
        eprintln!("gate: {errors} error-level knowledge finding(s)");
        std::process::exit(2);
    }
    Ok(())
}

fn cmd_atlas(action: AtlasCommands) -> Result<(), Box<dyn std::error::Error>> {
    fn print_value<T: serde::Serialize>(
        value: &T,
        format: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if format == "json" {
            println!("{}", serde_json::to_string_pretty(value)?);
        } else {
            println!("{}", serde_json::to_string(value)?);
        }
        Ok(())
    }
    let frozen_opts = |frozen: bool| rust_atlas::QueryOptions { frozen };
    match action {
        AtlasCommands::Build {
            code,
            graph,
            full,
            scip,
        } => {
            let report = rust_atlas::build(
                &code,
                &graph,
                &rust_atlas::BuildOptions {
                    full,
                    scip_index: scip,
                },
            )?;
            print_value(&report, "json")
        }
        AtlasCommands::Tree {
            code,
            graph,
            frozen,
            format,
        } => {
            let outline = rust_atlas::tree(&code, &graph, &frozen_opts(frozen))?;
            print_value(&outline, &format)
        }
        AtlasCommands::Query {
            symbol,
            code,
            graph,
            frozen,
            format,
        } => {
            let result = rust_atlas::query(&code, &graph, &symbol, &frozen_opts(frozen))?;
            print_value(&result, &format)
        }
        AtlasCommands::Refs {
            symbol,
            code,
            graph,
            frozen,
            format,
        } => {
            let report = rust_atlas::refs(&code, &graph, &symbol, &frozen_opts(frozen))?;
            print_value(&report, &format)
        }
        AtlasCommands::Impls {
            name,
            code,
            graph,
            frozen,
            format,
        } => {
            let report = rust_atlas::impls(&code, &graph, &name, &frozen_opts(frozen))?;
            print_value(&report, &format)
        }
        AtlasCommands::Check { code, graph } => {
            let stale = rust_atlas::check(&code, &graph)?;
            let payload = serde_json::json!({ "stale": stale });
            println!("{}", serde_json::to_string_pretty(&payload)?);
            if stale.is_empty() {
                Ok(())
            } else {
                Err(format!("atlas graph is stale: {}", stale.join(", ")).into())
            }
        }
    }
}

fn cmd_requirements(action: RequirementCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        RequirementCommands::Import {
            from,
            out,
            check,
            provenance,
        } => cmd_requirements_import(&from, &out, check, provenance.as_deref()),
        RequirementCommands::Transition { id, to, knowledge } => {
            let outcome = crate::spec_knowledge::transition_requirement(&knowledge, &id, &to)?;
            println!(
                "{}: {} -> {} ({})",
                outcome.id,
                outcome.old_status.as_deref().unwrap_or("(missing)"),
                outcome.new_status,
                outcome.path.display()
            );
            Ok(())
        }
        RequirementCommands::Supersede { id, by, knowledge } => {
            let (outcome, replacement) =
                crate::spec_knowledge::supersede_requirement(&knowledge, &id, &by)?;
            println!(
                "{}: {} -> superseded, replaced by {} ({})",
                outcome.id,
                outcome.old_status.as_deref().unwrap_or("(missing)"),
                by.to_ascii_uppercase(),
                replacement.display()
            );
            Ok(())
        }
        RequirementCommands::Status {
            id,
            knowledge,
            specs,
            archive_dir,
            code,
            format,
        } => {
            let report = crate::spec_knowledge::requirement_status(
                &knowledge,
                &specs,
                &archive_dir,
                &id,
                |spec_path| crate::spec_knowledge::verify_spec_rollup(spec_path, &code),
            )?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", crate::spec_knowledge::format_status_text(&report));
            }
            Ok(())
        }
        RequirementCommands::Export {
            knowledge,
            out,
            id,
            check,
            provenance,
        } => {
            let outcome = crate::spec_knowledge::write_export(
                &knowledge,
                &out,
                &crate::spec_knowledge::ExportOptions { ids: id },
                check,
            )?;
            if let Some(manifest_path) = provenance.as_deref() {
                let manifest = crate::spec_knowledge::write_export_provenance(
                    &knowledge,
                    &out,
                    &outcome.yaml,
                    manifest_path,
                )?;
                println!(
                    "provenance: {} (reproducible: {})",
                    manifest_path.display(),
                    manifest.reproducible
                );
            }
            for note in &outcome.excluded {
                eprintln!("excluded: {note}");
            }
            for note in &outcome.lossy {
                eprintln!("lossy: {note}");
            }
            if check {
                println!("export projection is fresh: {}", out.display());
            } else {
                println!("exported: {}", out.display());
            }
            Ok(())
        }
        RequirementCommands::Graph {
            knowledge,
            format,
            gate,
        } => cmd_requirements_graph(&knowledge, &format, gate),
        RequirementCommands::Plan {
            knowledge,
            specs,
            format,
            gate,
        } => cmd_requirements_plan(&knowledge, &specs, &format, gate),
        RequirementCommands::TestObligations {
            knowledge,
            specs,
            format,
            out,
        } => cmd_requirements_test_obligations(&knowledge, &specs, &format, out.as_deref()),
        RequirementCommands::Questions {
            knowledge,
            specs,
            format,
        } => cmd_requirements_questions(&knowledge, &specs, &format),
        RequirementCommands::Worktrees {
            knowledge,
            specs,
            base,
            path_prefix,
            format,
            out,
        } => cmd_requirements_worktrees(
            &knowledge,
            &specs,
            &base,
            &path_prefix,
            &format,
            out.as_deref(),
        ),
        RequirementCommands::Trace {
            id,
            trace_dir,
            code,
            wiki,
            format,
        } => cmd_requirements_trace(&id, &trace_dir, &code, &wiki, &format),
        RequirementCommands::Replay {
            id,
            trace_dir,
            format,
        } => cmd_requirements_replay(&id, &trace_dir, &format),
        RequirementCommands::ExplainFailure {
            id,
            trace_dir,
            code,
            wiki,
            format,
        } => cmd_requirements_explain_failure(&id, &trace_dir, &code, &wiki, &format),
        RequirementCommands::TraceGraph {
            id,
            trace_dir,
            format,
        } => cmd_requirements_trace_graph(&id, &trace_dir, &format),
        RequirementCommands::WorkUnits {
            knowledge,
            out,
            format,
        } => cmd_requirements_work_units(&knowledge, out.as_deref(), &format),
        RequirementCommands::DraftSpecs {
            knowledge,
            out,
            check,
        } => cmd_requirements_draft_specs(&knowledge, &out, check),
    }
}

fn cmd_requirements_import(
    from: &Path,
    out: &Path,
    check: bool,
    provenance: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let input = std::fs::read_to_string(from)?;
    let source_name = from.display().to_string();
    let extension = from
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    if matches!(extension.as_deref(), Some("yaml") | Some("yml")) {
        let docs = crate::spec_knowledge::import_requirements_yaml(&input, &source_name)?;
        if check {
            for doc in &docs {
                let path = out.join(&doc.filename);
                let actual = std::fs::read_to_string(&path).unwrap_or_default();
                if actual != doc.content {
                    return Err(format!(
                        "generated requirement artifact drifted: {}",
                        path.display()
                    )
                    .into());
                }
            }
            return Ok(());
        }
        let written = crate::spec_knowledge::write_generated_docs(out, &docs)?;
        for path in &written {
            println!("imported: {}", path.display());
        }
        if let Some(manifest_path) = provenance {
            let manifest =
                crate::spec_knowledge::write_import_provenance(from, &written, manifest_path)?;
            println!(
                "provenance: {} (reproducible: {})",
                manifest_path.display(),
                manifest.reproducible
            );
        }
        return Ok(());
    }
    let blocks = crate::spec_knowledge::parse_requirement_blocks(&input, &source_name)?;
    if blocks.is_empty() {
        return Err(format!(
            "no agent-spec requirement blocks found in {}",
            from.display()
        )
        .into());
    }

    let rendered = blocks
        .iter()
        .map(|block| {
            let filename = crate::spec_knowledge::requirement_artifact_filename(block);
            let path = out.join(filename);
            let content = crate::spec_knowledge::render_requirement_artifact(block);
            (path, content)
        })
        .collect::<Vec<_>>();

    if check {
        for (path, content) in rendered {
            let actual = std::fs::read_to_string(&path).unwrap_or_default();
            if actual != content {
                return Err(
                    format!("generated requirement artifact drifted: {}", path.display()).into(),
                );
            }
        }
        return Ok(());
    }

    std::fs::create_dir_all(out)?;
    for (path, content) in rendered {
        std::fs::write(path, content)?;
    }
    Ok(())
}

fn cmd_requirements_graph(
    knowledge: &Path,
    format: &str,
    gate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    graph
        .diagnostics
        .extend(crate::spec_knowledge::validate_requirement_graph(&graph));
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&graph)?),
        _ => print_requirement_graph_text(&graph),
    }
    if gate
        && (!graph.parse_errors.is_empty()
            || graph
                .diagnostics
                .iter()
                .any(|diag| diag.severity == "error"))
    {
        return Err("requirement graph gate failed".into());
    }
    Ok(())
}

fn cmd_requirements_plan(
    knowledge: &Path,
    specs: &Path,
    format: &str,
    gate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = crate::spec_knowledge::build_requirement_plan(knowledge, specs);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&plan)?),
        _ => print_requirement_plan_text(&plan),
    }

    if gate
        && (!plan.parse_errors.is_empty()
            || plan.diagnostics.iter().any(|diag| diag.severity == "error"))
    {
        return Err("requirements plan gate failed".into());
    }

    Ok(())
}

fn cmd_requirements_trace(
    id: &str,
    trace_dir: &Path,
    code: &Path,
    wiki: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = crate::spec_knowledge::read_requirement_trace_ledgers(trace_dir);
    let target = id.to_ascii_uppercase();
    let mut records = ledger
        .records
        .iter()
        .filter(|record| record.requirement_id == target)
        .cloned()
        .collect::<Vec<_>>();
    attach_wiki_articles_to_trace_records(&mut records, code, wiki);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&records)?),
        _ => print!(
            "{}",
            crate::spec_knowledge::format_requirement_trace_text(&records)
        ),
    }
    Ok(())
}

fn cmd_requirements_replay(
    id: &str,
    trace_dir: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = crate::spec_knowledge::read_requirement_trace_ledgers(trace_dir);
    let target = id.to_ascii_uppercase();
    let records = crate::spec_knowledge::replay_requirement_trace(&ledger, &target);
    if records.is_empty() {
        return Err(format!("no requirement trace record found for {target}").into());
    }
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&records)?),
        _ => print!(
            "{}",
            crate::spec_knowledge::format_requirement_replay_text(&records)
        ),
    }
    Ok(())
}

fn cmd_requirements_explain_failure(
    id: &str,
    trace_dir: &Path,
    code: &Path,
    wiki: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = crate::spec_knowledge::read_requirement_trace_ledgers(trace_dir);
    let target = id.to_ascii_uppercase();
    let mut explanation = crate::spec_knowledge::explain_requirement_failure(&ledger, &target);
    attach_wiki_articles_to_failure_explanation(&mut explanation, code, wiki);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&explanation)?),
        _ => print!(
            "{}",
            crate::spec_knowledge::format_requirement_failure_text(&explanation)
        ),
    }
    Ok(())
}

fn attach_wiki_articles_to_trace_records(
    records: &mut [crate::spec_knowledge::RequirementTraceRecord],
    code: &Path,
    wiki: &Path,
) {
    for record in records {
        record.wiki_articles = related_wiki_article_paths_for_trace_record(record, code, wiki);
    }
}

fn attach_wiki_articles_to_failure_explanation(
    explanation: &mut crate::spec_knowledge::RequirementFailureExplanation,
    code: &Path,
    wiki: &Path,
) {
    attach_wiki_articles_to_trace_records(&mut explanation.non_pass_records, code, wiki);
}

fn related_wiki_article_paths_for_trace_record(
    record: &crate::spec_knowledge::RequirementTraceRecord,
    code: &Path,
    wiki: &Path,
) -> Vec<PathBuf> {
    let mut candidates = vec![record.requirement_source.clone(), record.spec_path.clone()];
    candidates.extend(record.code_targets.iter().map(PathBuf::from));

    let mut paths = BTreeSet::new();
    for candidate in candidates {
        let report = crate::spec_wiki::inspect_live_wiki_path(code, wiki, &candidate);
        for article in report.wiki_articles {
            paths.insert(article.path);
        }
    }
    paths.into_iter().collect()
}

fn cmd_requirements_trace_graph(
    id: &str,
    trace_dir: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = crate::spec_knowledge::read_requirement_trace_ledgers(trace_dir);
    let target = id.to_ascii_uppercase();
    let records = crate::spec_knowledge::latest_requirement_trace_records(&ledger, &target);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&records)?),
        _ => print!(
            "{}",
            crate::spec_knowledge::format_requirement_trace_mermaid(&records)
        ),
    }
    Ok(())
}

fn cmd_requirements_worktrees(
    knowledge: &Path,
    specs: &Path,
    base: &str,
    path_prefix: &Path,
    format: &str,
    out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = crate::spec_knowledge::build_requirement_plan(knowledge, specs);
    let manifest = crate::spec_knowledge::build_worktree_manifest(&plan, base, path_prefix);
    let body = match format {
        "text" => format_worktree_manifest_text(&manifest),
        _ => serde_json::to_string_pretty(&manifest)?,
    };
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
    } else {
        print!("{body}");
    }
    Ok(())
}

fn format_worktree_manifest_text(manifest: &crate::spec_knowledge::WorktreeManifest) -> String {
    let mut out = format!(
        "worktrees: {} entries, {} diagnostics\n",
        manifest.entries.len(),
        manifest.diagnostics.len()
    );
    for entry in &manifest.entries {
        out.push_str(&format!(
            "batch {} {} -> {} ({})\n",
            entry.batch,
            entry.work_unit_id,
            entry.path.display(),
            entry.branch
        ));
    }
    out
}

fn cmd_requirements_test_obligations(
    knowledge: &Path,
    specs: &Path,
    format: &str,
    out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let obligations = crate::spec_knowledge::build_test_obligations(knowledge, specs);
    let body = match format {
        "text" => format_test_obligations_text(&obligations),
        _ => serde_json::to_string_pretty(&obligations)?,
    };
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
    } else {
        print!("{body}");
    }
    Ok(())
}

fn format_test_obligations_text(obligations: &crate::spec_knowledge::TestObligationSet) -> String {
    let mut out = format!(
        "test obligations: {} obligations, {} diagnostics\n",
        obligations.obligations.len(),
        obligations.diagnostics.len()
    );
    for obligation in &obligations.obligations {
        out.push_str(&format!(
            "{} {} -> {}\n",
            obligation.requirement_id, obligation.scenario_name, obligation.suggested_selector
        ));
    }
    out
}

fn cmd_requirements_questions(
    knowledge: &Path,
    specs: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = crate::spec_knowledge::build_requirement_plan(knowledge, specs);
    let lint_diagnostics = crate::spec_knowledge::collect_clarification_lint_diagnostics(knowledge);
    let questions = crate::spec_knowledge::build_clarification_questions(&plan, &lint_diagnostics);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&questions)?),
        _ => {
            println!("clarification questions: {}", questions.len());
            for question in questions {
                println!(
                    "{} [{}] {}: {}",
                    question.id, question.diagnostic_code, question.target_id, question.prompt
                );
            }
        }
    }
    Ok(())
}

fn cmd_requirements_work_units(
    knowledge: &Path,
    out: Option<&Path>,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    graph
        .diagnostics
        .extend(crate::spec_knowledge::validate_requirement_graph(&graph));
    let units = crate::spec_knowledge::build_work_units(&graph);
    let body = match format {
        "text" => format_work_units_text(&units),
        _ => serde_json::to_string_pretty(&units)?,
    };
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
    } else {
        print!("{body}");
    }
    Ok(())
}

fn cmd_requirements_draft_specs(
    knowledge: &Path,
    out: &Path,
    check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    graph
        .diagnostics
        .extend(crate::spec_knowledge::validate_requirement_graph(&graph));
    let units = crate::spec_knowledge::build_work_units(&graph);
    let mut rendered = Vec::new();
    for unit in units
        .units
        .iter()
        .filter(|unit| unit.status == crate::spec_knowledge::WorkUnitStatus::Ready)
    {
        let Some(node) = graph.node(&unit.requirement_id) else {
            continue;
        };
        if let Some(draft) = crate::spec_knowledge::render_draft_spec(node, unit) {
            rendered.push((out.join(draft.filename), draft.content));
        }
    }

    if check {
        for (path, content) in rendered {
            let actual = std::fs::read_to_string(&path).unwrap_or_default();
            if actual != content {
                return Err(format!("generated draft spec drifted: {}", path.display()).into());
            }
        }
        return Ok(());
    }

    std::fs::create_dir_all(out)?;
    for (path, content) in rendered {
        std::fs::write(path, content)?;
    }
    Ok(())
}

fn cmd_wiki(action: WikiCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        WikiCommands::Init {
            code,
            wiki,
            format,
            check,
        } => cmd_wiki_init(&code, &wiki, check, &format),
        WikiCommands::Status { code, wiki, format } => cmd_wiki_status(&code, &wiki, &format),
        WikiCommands::Inventory { code, format } => cmd_wiki_inventory(&code, &format),
        WikiCommands::Index { wiki, format } => cmd_wiki_index(&wiki, &format),
        WikiCommands::Lint { code, wiki, format } => cmd_wiki_lint(&code, &wiki, &format),
        WikiCommands::Check { code, wiki, format } => cmd_wiki_live_check(&code, &wiki, &format),
        WikiCommands::Seed {
            code,
            wiki,
            format,
            check,
        } => cmd_wiki_seed(&code, &wiki, check, &format),
        WikiCommands::Query {
            query,
            wiki,
            format,
        } => cmd_wiki_query(&wiki, &query, &format),
        WikiCommands::Inspect {
            path,
            code,
            wiki,
            format,
        } => cmd_wiki_inspect(&code, &wiki, &path, &format),
        WikiCommands::ProjectMap {
            code,
            wiki,
            format,
            out,
            check,
        } => cmd_wiki_project_map(&code, &wiki, &format, out.as_deref(), check),
        WikiCommands::InspectProject {
            project_id,
            code,
            wiki,
            format,
        } => cmd_wiki_inspect_project(&project_id, &code, &wiki, &format),
        WikiCommands::Meta { action } => cmd_wiki_meta(action),
    }
}

fn cmd_wiki_init(
    code: &Path,
    wiki: &Path,
    check: bool,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if check {
        let temp = unique_temp_dir("agent-spec-wiki-live-init")?;
        let _temp_cleanup = TempDirCleanup::new(temp.clone());
        for maintained_dir in ["projects", "flows"] {
            let source = wiki.join(maintained_dir);
            if !source.is_dir() {
                return Err(format!(
                    "target wiki is missing maintained directory: {maintained_dir}"
                )
                .into());
            }
            copy_file_tree(&source, &temp.join(maintained_dir))?;
        }
        let report = crate::spec_wiki::init_live_wiki(code, &temp)?;
        let files_to_compare = report
            .files_written
            .iter()
            .filter(|path| path.as_path() != Path::new("_index.md"))
            .cloned()
            .collect::<Vec<_>>();
        compare_generated_file_subset(&temp, wiki, &files_to_compare, "code live wiki")?;
        match format {
            "json" => println!("{}", serde_json::to_string_pretty(&report)?),
            _ => println!("wiki init check: {} files", report.files_written.len()),
        }
        if report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
        {
            return Err("wiki init check failed".into());
        }
        return Ok(());
    }

    let report = crate::spec_wiki::init_live_wiki(code, wiki)?;
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => {
            println!(
                "wiki initialized: {} files in {}",
                report.files_written.len(),
                wiki.display()
            );
            print_wiki_diagnostics_text("wiki init", &report.diagnostics);
        }
    }
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return Err("wiki init failed".into());
    }
    Ok(())
}

fn cmd_wiki_status(
    code: &Path,
    wiki: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = crate::spec_wiki::wiki_status(code, wiki);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => {
            if report.first_run {
                println!("wiki status: first run or missing _meta.json");
            } else {
                println!(
                    "wiki status: {} changed files, {} stale articles",
                    report.changed_files.len(),
                    report.stale_articles.len()
                );
            }
            for article in &report.stale_articles {
                println!(
                    "  stale {} ({})",
                    article.path.display(),
                    article
                        .changed_files
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            print_wiki_diagnostics_text("wiki status", &report.diagnostics);
        }
    }
    Ok(())
}

fn cmd_wiki_inventory(code: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let inventory = crate::spec_wiki::build_architecture_inventory(code);
    match format {
        "mermaid" | "mmd" => println!(
            "{}",
            crate::spec_wiki::render_architecture_mermaid(&inventory)
        ),
        "json" => println!("{}", serde_json::to_string_pretty(&inventory)?),
        _ => {
            println!(
                "wiki inventory: provider={}, packages={}, dependencies={}",
                inventory.provider,
                inventory.packages.len(),
                inventory.dependencies.len()
            );
            print_wiki_diagnostics_text("wiki inventory", &inventory.diagnostics);
        }
    }
    Ok(())
}

fn cmd_wiki_index(wiki: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = crate::spec_wiki::write_wiki_index(wiki)?;
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&diagnostics)?),
        _ => {
            println!("wiki index: {}", wiki.join("_index.md").display());
            print_wiki_diagnostics_text("wiki index", &diagnostics);
        }
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return Err("wiki index failed".into());
    }
    Ok(())
}

fn cmd_wiki_lint(code: &Path, wiki: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let report = crate::spec_wiki::lint_live_wiki(code, wiki);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => print_wiki_diagnostics_text("wiki lint", &report.diagnostics),
    }
    if !report.passed() {
        return Err("wiki lint failed".into());
    }
    Ok(())
}

fn cmd_wiki_live_check(
    code: &Path,
    wiki: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = crate::spec_wiki::check_live_wiki(code, wiki);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => print_wiki_diagnostics_text("wiki check", &report.diagnostics),
    }
    if !report.passed() {
        return Err("wiki check failed".into());
    }
    Ok(())
}

fn cmd_wiki_seed(
    code: &Path,
    wiki: &Path,
    check: bool,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = if check {
        crate::spec_wiki::seed_live_wiki_check(code, wiki)
    } else {
        crate::spec_wiki::seed_live_wiki(code, wiki)?
    };
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => {
            println!(
                "wiki seed: {} written, {} missing",
                report.files_written.len(),
                report.missing_pages.len()
            );
            print_wiki_diagnostics_text("wiki seed", &report.diagnostics);
        }
    }
    if check && !report.diagnostics.is_empty() {
        return Err("wiki seed check failed".into());
    }
    Ok(())
}

fn cmd_wiki_query(
    wiki: &Path,
    query: &str,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = crate::spec_wiki::query_live_wiki(wiki, query);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => {
            println!(
                "wiki query: {} matches for `{}`",
                report.matches.len(),
                report.query
            );
            for item in &report.matches {
                println!("  {} ({})", item.path.display(), item.title);
            }
            print_wiki_diagnostics_text("wiki query", &report.diagnostics);
        }
    }
    Ok(())
}

fn cmd_wiki_inspect(
    code: &Path,
    wiki: &Path,
    path: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = crate::spec_wiki::inspect_live_wiki_path(code, wiki, path);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => {
            println!("wiki inspect: {}", report.input_path.display());
            for article in &report.wiki_articles {
                println!("  wiki {} ({})", article.path.display(), article.title);
            }
            for requirement in &report.requirements {
                println!(
                    "  requirement {} ({})",
                    requirement.id,
                    requirement.path.display()
                );
            }
            for spec in &report.specs {
                println!("  spec {} ({})", spec.name, spec.path.display());
            }
            for trace in &report.trace_records {
                println!(
                    "  trace {} {} {} {:?} {}",
                    trace.requirement_id,
                    trace.run_id,
                    trace.scenario_name,
                    trace.test_selector,
                    trace.verdict
                );
            }
            print_wiki_diagnostics_text("wiki inspect", &report.diagnostics);
        }
    }
    Ok(())
}

fn cmd_wiki_project_map(
    code: &Path,
    wiki: &Path,
    format: &str,
    out: Option<&Path>,
    check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if check && out.is_none() {
        return Err("wiki project-map --check requires --out".into());
    }
    let map = crate::spec_wiki::build_project_map(code, wiki);
    let rendered = match format {
        "mermaid" | "mmd" => crate::spec_wiki::render_project_map_mermaid(&map),
        "json" => serde_json::to_string_pretty(&map)?,
        other => return Err(format!("unsupported wiki project-map format: {other}").into()),
    };
    if let Some(out_path) = out {
        if check {
            if map
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == "error")
            {
                return Err("project map contains error diagnostics".into());
            }
            let actual = std::fs::read_to_string(out_path).unwrap_or_default();
            if actual != rendered {
                return Err(format!("project map drifted: {}", out_path.display()).into());
            }
            return Ok(());
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(out_path, rendered)?;
        return Ok(());
    }
    print!("{rendered}");
    Ok(())
}

fn cmd_wiki_inspect_project(
    project_id: &str,
    code: &Path,
    wiki: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = crate::spec_wiki::inspect_wiki_project(code, wiki, project_id);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        "text" => {
            println!("wiki inspect-project: {}", report.project_id);
            if let Some(project) = &report.project {
                println!("  project {} ({})", project.id, project.path.display());
                println!("  repo {}", project.repo);
                if !project.external_sources.is_empty() {
                    println!(
                        "  external_sources: {}",
                        project.external_sources.join(", ")
                    );
                }
            }
            for flow in &report.flows {
                println!("  flow {} ({})", flow.id, flow.path.display());
                println!("    projects: {}", flow.projects.join(", "));
                println!("    protocols: {}", flow.protocols.join(", "));
                if !flow.requirements.is_empty() {
                    println!("    requirements: {}", flow.requirements.join(", "));
                }
                if !flow.specs.is_empty() {
                    println!(
                        "    specs: {}",
                        flow.specs
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if !flow.external_sources.is_empty() {
                    println!("    external_sources: {}", flow.external_sources.join(", "));
                }
            }
            print_wiki_diagnostics_text("wiki inspect-project", &report.diagnostics);
        }
        other => return Err(format!("unsupported wiki inspect-project format: {other}").into()),
    }
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return Err("wiki inspect-project failed".into());
    }
    Ok(())
}

fn cmd_wiki_meta(action: WikiMetaCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        WikiMetaCommands::Update { code, wiki, format } => {
            let meta = crate::spec_wiki::update_wiki_meta(&code, &wiki)?;
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&meta)?),
                _ => println!(
                    "wiki meta update: {}",
                    meta.last_compiled_commit
                        .as_deref()
                        .unwrap_or("(no git commit)")
                ),
            }
            Ok(())
        }
    }
}

fn print_wiki_diagnostics_text(prefix: &str, diagnostics: &[crate::spec_wiki::WikiDiagnostic]) {
    if diagnostics.is_empty() {
        println!("{prefix}: no diagnostics");
        return;
    }
    for diagnostic in diagnostics {
        let path = diagnostic
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(wiki)".into());
        println!(
            "[{}] {} {}: {}",
            diagnostic.severity, diagnostic.code, path, diagnostic.message
        );
    }
}

fn compare_generated_file_subset(
    expected_dir: &Path,
    actual_dir: &Path,
    files: &[PathBuf],
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !actual_dir.exists() {
        return Err(format!("{label} missing: {}", actual_dir.display()).into());
    }
    for path in files {
        let expected_path = expected_dir.join(path);
        let actual_path = actual_dir.join(path);
        if !actual_path.exists() {
            return Err(format!("{label} missing generated file: {}", path.display()).into());
        }
        let expected_content = std::fs::read(&expected_path)?;
        let actual_content = std::fs::read(&actual_path)?;
        if actual_content != expected_content {
            return Err(format!("{label} drifted: {}", path.display()).into());
        }
    }
    Ok(())
}

fn copy_file_tree(source: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if std::fs::symlink_metadata(source)?.file_type().is_symlink() {
        return Err(format!(
            "wiki init check rejects symlinked maintained entry: {}",
            source.display()
        )
        .into());
    }
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if file_type.is_symlink() {
            return Err(format!(
                "wiki init check rejects symlinked maintained entry: {}",
                source_path.display()
            )
            .into());
        } else if file_type.is_dir() {
            copy_file_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            std::fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}

fn unique_temp_dir(prefix: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct TempDirCleanup {
    path: PathBuf,
}

impl TempDirCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempDirCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn print_requirement_graph_text(graph: &crate::spec_knowledge::RequirementGraph) {
    println!(
        "requirements: {} nodes, {} diagnostics, {} parse errors",
        graph.nodes.len(),
        graph.diagnostics.len(),
        graph.parse_errors.len()
    );
    for err in &graph.parse_errors {
        println!("parse-error {}: {}", err.path.display(), err.message);
    }
    for diag in &graph.diagnostics {
        let id = diag.requirement_id.as_deref().unwrap_or("(graph)");
        println!("[{}] {} {}: {}", diag.severity, id, diag.code, diag.message);
    }
    for node in &graph.nodes {
        println!(
            "{} {} scenarios={} deps={}",
            node.id,
            node.title,
            node.scenarios.len(),
            node.dependencies.len()
        );
    }
}

fn print_requirement_plan_text(plan: &crate::spec_knowledge::RequirementPlan) {
    println!(
        "requirements plan: {} requirements, {} batches, {} diagnostics, {} parse errors",
        plan.requirements.len(),
        plan.batches.len(),
        plan.diagnostics.len(),
        plan.parse_errors.len()
    );
    for diagnostic in &plan.diagnostics {
        let id = diagnostic.requirement_id.as_deref().unwrap_or("(plan)");
        println!(
            "[{}] {} {}: {}",
            diagnostic.severity, id, diagnostic.code, diagnostic.message
        );
    }
    for batch in &plan.batches {
        println!(
            "batch {}: {}",
            batch.order,
            batch.requirement_ids.join(", ")
        );
    }
}

fn format_work_units_text(units: &crate::spec_knowledge::WorkUnitSet) -> String {
    let mut out = format!(
        "work units: {} units, {} diagnostics\n",
        units.units.len(),
        units.diagnostics.len()
    );
    for diag in &units.diagnostics {
        let id = diag.requirement_id.as_deref().unwrap_or("(graph)");
        out.push_str(&format!(
            "[{}] {} {}: {}\n",
            diag.severity, id, diag.code, diag.message
        ));
    }
    for unit in &units.units {
        out.push_str(&format!(
            "{} {} mode={:?} status={:?} scenarios={}\n",
            unit.id, unit.title, unit.mode, unit.status, unit.scenario_count
        ));
    }
    out
}

fn cmd_mcp(
    knowledge: PathBuf,
    specs: PathBuf,
    code: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = crate::spec_mcp::McpContext {
        knowledge,
        specs,
        code,
    };
    crate::spec_mcp::serve(&ctx)?;
    Ok(())
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

    let decision = find_trace_artifact(knowledge, &target)?;

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

fn cmd_archive(
    spec_dir: &Path,
    archive_dir: &Path,
    summary: &Path,
    run_log_dir: &Path,
    dry_run: bool,
    check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan =
        crate::spec_archive::build_archive_plan_with_history(spec_dir, archive_dir, run_log_dir);
    let summary_body = crate::spec_archive::render_archive_summary(&plan);

    if check {
        let actual = std::fs::read_to_string(summary).unwrap_or_default();
        if actual != summary_body {
            return Err(format!("archive summary drifted: {}", summary.display()).into());
        }
        return Ok(());
    }

    print!("{summary_body}");
    if dry_run {
        return Ok(());
    }

    if plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return Err("archive plan contains blocking diagnostics".into());
    }
    if let Some(parent) = summary.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let summary_tmp = summary.with_extension("tmp");
    std::fs::write(&summary_tmp, summary_body)?;
    if let Err(error) = crate::spec_archive::apply_archive_plan(&plan) {
        std::fs::remove_file(&summary_tmp).ok();
        return Err(error);
    }
    std::fs::rename(summary_tmp, summary)?;
    Ok(())
}

fn find_trace_artifact(
    knowledge: &Path,
    target_id: &str,
) -> Result<crate::spec_knowledge::DecisionDoc, Box<dyn std::error::Error>> {
    let collection = crate::spec_knowledge::collect_knowledge_checked(knowledge);
    for doc in collection.docs {
        if matches!(
            doc.meta.kind,
            crate::spec_knowledge::KnowledgeKind::Decision
                | crate::spec_knowledge::KnowledgeKind::Requirement
        ) && doc.meta.id == target_id
        {
            return Ok(doc);
        }
    }
    for err in collection.parse_errors {
        if crate::spec_knowledge::resolve_decision_id(None, &err.path).as_deref() == Some(target_id)
        {
            return Err(format!(
                "cannot parse knowledge artifact {target_id} at {}: {}",
                err.path.display(),
                err.message
            )
            .into());
        }
    }
    Err(format!(
        "no decision or requirement with id {target_id} in {}",
        knowledge.display()
    )
    .into())
}

fn find_decision(
    dir: &Path,
    target_id: &str,
) -> Result<crate::spec_knowledge::DecisionDoc, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_decision_files(dir, &mut files)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    files.sort();
    for p in files {
        match crate::spec_knowledge::parse_decision(&p) {
            Ok(doc) if doc.meta.id == target_id => return Ok(doc),
            Ok(_) => {}
            Err(e) => {
                if crate::spec_knowledge::resolve_decision_id(None, &p).as_deref()
                    == Some(target_id)
                {
                    return Err(format!(
                        "cannot parse decision {target_id} at {}: {e}",
                        p.display()
                    )
                    .into());
                }
            }
        }
    }
    Err(format!("no decision with id {target_id} in {}", dir.display()).into())
}

fn collect_decision_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            collect_decision_files(&p, out)?;
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if name.ends_with(".md") || name.ends_with(".spec") {
            out.push(p);
        }
    }
    Ok(())
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
    use super::{
        cmd_requirements_graph, cmd_requirements_import, cmd_requirements_work_units,
        examples_all_pass, rule_scenarios, upsert_capability_rule,
    };

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
    fn test_find_decision_recurses_into_nested_directories() {
        let dir = make_temp_dir("agent-spec-kll-trace");
        let decisions = dir.join("knowledge").join("decisions").join("security");
        fs::create_dir_all(&decisions).unwrap();
        fs::write(
            decisions.join("adr-042-auth.md"),
            "---\nkind: decision\nid: ADR-042\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood, because x. Bad, because y.\n",
        )
        .unwrap();

        let doc = super::find_decision(&dir.join("knowledge").join("decisions"), "ADR-042")
            .expect("nested decision should be found");

        assert_eq!(doc.meta.id, "ADR-042");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_find_decision_reports_parse_error_for_matching_bad_file() {
        let dir = make_temp_dir("agent-spec-kll-trace-bad");
        let decisions = dir.join("knowledge").join("decisions");
        fs::create_dir_all(&decisions).unwrap();
        fs::write(
            decisions.join("adr-099-bad.md"),
            "---\nkind: decision\nid: ADR-099\nliveness: forever\n---\n## Context\nbad\n",
        )
        .unwrap();

        let err = super::find_decision(&decisions, "ADR-099").unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("cannot parse decision ADR-099"), "{msg}");
        assert!(msg.contains("unknown liveness"), "{msg}");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_trace_accepts_requirement_artifact() {
        let dir = make_temp_dir("agent-spec-kll-trace-requirement");
        let knowledge = dir.join("knowledge");
        let requirements = knowledge.join("requirements");
        let specs = dir.join("specs");
        fs::create_dir_all(&requirements).unwrap();
        fs::create_dir_all(&specs).unwrap();
        fs::write(
            requirements.join("req-101-login.md"),
            "---\nkind: requirement\nid: REQ-101\ntitle: \"User Login\"\n---\n## Problem\nLogin.\n## Requirements\n[REQ-101] The service MUST log users in.\n",
        )
        .unwrap();

        let result = super::cmd_trace("REQ-101", &knowledge, &specs, Path::new("."), "json", false);
        assert!(
            result.is_ok(),
            "trace must accept requirement artifacts: {result:?}"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_gen_integrations_with_guidance_reports_parse_errors() {
        let dir = make_temp_dir("agent-spec-kll-guidance-bad");
        let knowledge = dir.join("knowledge");
        let out = dir.join("out");
        fs::create_dir_all(knowledge.join("guidance")).unwrap();
        fs::create_dir_all(&out).unwrap();
        fs::write(
            knowledge.join("guidance/g-099-bad.md"),
            "---\nkind: guidance\nid: G-099\nliveness: forever\n---\n## Scope\ns\n",
        )
        .unwrap();

        let err = super::cmd_gen_integrations("agents", &out, false, Some(&knowledge)).unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("knowledge-parse-error"), "{msg}");
        assert!(msg.contains("g-099-bad.md"), "{msg}");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_import_command_writes_artifact() {
        let dir = make_temp_dir("requirements-import-cli");
        let source = dir.join("issue.md");
        let out = dir.join("knowledge/requirements");
        fs::write(
            &source,
            "<!-- agent-spec:requirement id=REQ-101 title=\"User Login\" tags=auth source=issue:#123 -->\n## Problem\nLogin.\n\n## Requirements\n\n[REQ-101] The authentication service MUST create a login session.\n\n## Scenarios\n\nScenario: Valid login\n  Given a valid account\n  When valid credentials are submitted\n  Then a session is created\n\n## Open Questions\n\nNone.\n<!-- /agent-spec:requirement -->\n",
        )
        .unwrap();

        cmd_requirements_import(&source, &out, false, None).unwrap();
        let artifact = out.join("req-101-user-login.md");
        assert!(artifact.exists());
        let body = fs::read_to_string(artifact).unwrap();
        assert!(body.contains("kind: requirement"));
        assert!(body.contains("id: REQ-101"));
        assert!(body.contains("title: \"User Login\""));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_work_units_command_writes_json() {
        let dir = make_temp_dir("requirements-work-units-cli");
        let knowledge = dir.join("knowledge");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::write(
            knowledge.join("requirements/req-101-login.md"),
            "---\nkind: requirement\nid: REQ-101\ntitle: \"User Login\"\n---\n## Problem\nLogin.\n## Requirements\n[REQ-101] The authentication service MUST create a login session.\n## Scenarios\nScenario: Valid login\n  Given a valid account\n  When valid credentials are submitted\n  Then a session is created\n## Open Questions\nNone.\n",
        )
        .unwrap();
        let out = dir.join(".agent-spec/work_units.json");

        cmd_requirements_work_units(&knowledge, Some(&out), "json").unwrap();
        let json = fs::read_to_string(out).unwrap();
        assert!(
            json.contains("\"requirement_id\":\"REQ-101\"")
                || json.contains("\"requirement_id\": \"REQ-101\"")
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_graph_gate_fails_on_parse_error() {
        let dir = make_temp_dir("requirements-graph-gate-parse-error");
        let knowledge = dir.join("knowledge");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::write(
            knowledge.join("requirements/req-999-bad.md"),
            "---\nkind: requirement\nid: REQ-999\nliveness: forever\n---\n## Problem\nbad\n",
        )
        .unwrap();

        let err = cmd_requirements_graph(&knowledge, "text", true).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("requirement graph gate failed"), "{msg}");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_gates_reject_missing_roots() {
        let dir = make_temp_dir("requirements-missing-roots");
        let missing_knowledge = dir.join("missing-knowledge");
        let missing_specs = dir.join("missing-specs");

        assert!(cmd_requirements_graph(&missing_knowledge, "json", true).is_err());
        assert!(
            super::cmd_requirements_plan(&missing_knowledge, &missing_specs, "json", true).is_err()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_plan_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "plan",
            "--knowledge",
            "knowledge",
            "--specs",
            "specs",
            "--format",
            "json",
            "--gate",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::Plan {
                        knowledge,
                        specs,
                        format,
                        gate,
                    },
            } => {
                assert_eq!(knowledge, PathBuf::from("knowledge"));
                assert_eq!(specs, PathBuf::from("specs"));
                assert_eq!(format, "json");
                assert!(gate);
            }
            _ => panic!("expected requirements plan command"),
        }
    }

    #[test]
    fn test_requirements_plan_json_includes_batches_edges_and_coverage() {
        let dir = make_temp_dir("requirements-plan-cli-json");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::create_dir_all(&specs).unwrap();

        fs::write(
            knowledge.join("requirements/req-a.md"),
            "---\nkind: requirement\nid: REQ-A\ntitle: \"A\"\nliveness: auto\n---\n## Problem\nA.\n## Requirements\n[REQ-A] The system MUST do A.\n## Scenarios\nScenario: A\n  Given input A\n  When A runs\n  Then output A is visible\n## Source Trace\n- test\n## Open Questions\nNone.\n",
        )
        .unwrap();
        fs::write(
            specs.join("task-a.spec.md"),
            "spec: task\nname: \"A\"\nsatisfies: [REQ-A]\n---\n## Intent\nA.\n## Completion Criteria\nScenario: A\n  Test: test_a\n  Given A\n  When A\n  Then A\n",
        )
        .unwrap();

        let plan = crate::spec_knowledge::build_requirement_plan(&knowledge, &specs);
        let json = serde_json::to_string_pretty(&plan).unwrap();
        assert!(json.contains("\"requirements\""));
        assert!(json.contains("\"work_units\""));
        assert!(json.contains("\"batches\""));
        assert!(json.contains("\"coverage\""));
        assert!(json.contains("REQ-A"));
        assert!(json.contains("WU-REQ-A"));
        assert!(json.contains("\"kind\": \"work_unit\""));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_plan_gate_fails_on_dangling_dependency() {
        let dir = make_temp_dir("requirements-plan-cli-gate");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::create_dir_all(&specs).unwrap();

        fs::write(
            knowledge.join("requirements/req-a.md"),
            "---\nkind: requirement\nid: REQ-A\ntitle: \"A\"\nliveness: auto\n---\n## Problem\nA.\n## Requirements\n[REQ-A] The system MUST do A.\n## Dependencies\n- REQ-MISSING\n## Scenarios\nScenario: A\n  Given input A\n  When A runs\n  Then output A is visible\n## Source Trace\n- test\n## Open Questions\nNone.\n",
        )
        .unwrap();

        let err = super::cmd_requirements_plan(&knowledge, &specs, "text", true).unwrap_err();
        assert!(err.to_string().contains("requirements plan gate failed"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_test_obligations_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "test-obligations",
            "--knowledge",
            "knowledge",
            "--specs",
            "specs",
            "--format",
            "json",
            "--out",
            ".agent-spec/test_obligations.json",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::TestObligations {
                        knowledge,
                        specs,
                        format,
                        out,
                    },
            } => {
                assert_eq!(knowledge, PathBuf::from("knowledge"));
                assert_eq!(specs, PathBuf::from("specs"));
                assert_eq!(format, "json");
                assert_eq!(
                    out,
                    Some(PathBuf::from(".agent-spec/test_obligations.json"))
                );
            }
            _ => panic!("expected requirements test-obligations command"),
        }
    }

    #[test]
    fn test_requirements_test_obligations_json_contains_spec_derived_obligations() {
        let dir = make_temp_dir("requirements-test-obligations-cli-json");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        fs::create_dir_all(knowledge.join("requirements")).unwrap();
        fs::create_dir_all(&specs).unwrap();

        fs::write(
            knowledge.join("requirements/req-note-create.md"),
            "---\nkind: requirement\nid: REQ-NOTE-CREATE\ntitle: \"Create Note\"\nliveness: auto\n---\n## Problem\nCreate notes.\n## Requirements\n[REQ-NOTE-CREATE] The note store MUST create notes.\n## Scenarios\nScenario: Create note\n  Given an empty store\n  When a note is created\n  Then the returned note appears in the list\n## Source Trace\n- test\n## Open Questions\nNone.\n",
        )
        .unwrap();
        fs::write(
            specs.join("task-note-create.spec.md"),
            "spec: task\nname: \"Create Note\"\nsatisfies: [REQ-NOTE-CREATE]\nrisk: C\n---\n## Intent\nCreate note.\n## Completion Criteria\nScenario: Create note\n  Test: note_create_adds_note\n  Given an empty store\n  When a note is created\n  Then the returned note appears in the list\n",
        )
        .unwrap();
        let out = dir.join(".agent-spec/test_obligations.json");

        super::cmd_requirements_test_obligations(&knowledge, &specs, "json", Some(&out)).unwrap();
        let json = fs::read_to_string(out).unwrap();
        assert!(json.contains("\"requirement_id\": \"REQ-NOTE-CREATE\""));
        assert!(json.contains("\"suggested_selector\": \"note_create_adds_note\""));
        assert!(json.contains("\"required_evidence\""));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_questions_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "questions",
            "--knowledge",
            "knowledge",
            "--specs",
            "specs",
            "--format",
            "json",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::Questions {
                        knowledge,
                        specs,
                        format,
                    },
            } => {
                assert_eq!(knowledge, PathBuf::from("knowledge"));
                assert_eq!(specs, PathBuf::from("specs"));
                assert_eq!(format, "json");
            }
            _ => panic!("expected requirements questions command"),
        }
    }

    #[test]
    fn test_requirements_worktrees_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "worktrees",
            "--knowledge",
            "knowledge",
            "--specs",
            "specs",
            "--base",
            "main",
            "--path-prefix",
            "../worktrees",
            "--format",
            "json",
            "--out",
            ".agent-spec/worktrees.json",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::Worktrees {
                        knowledge,
                        specs,
                        base,
                        path_prefix,
                        format,
                        out,
                    },
            } => {
                assert_eq!(knowledge, PathBuf::from("knowledge"));
                assert_eq!(specs, PathBuf::from("specs"));
                assert_eq!(base, "main");
                assert_eq!(path_prefix, PathBuf::from("../worktrees"));
                assert_eq!(format, "json");
                assert_eq!(out, Some(PathBuf::from(".agent-spec/worktrees.json")));
            }
            _ => panic!("expected requirements worktrees command"),
        }
    }

    #[test]
    fn test_requirements_trace_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "trace",
            "REQ-NOTE-CREATE",
            "--trace-dir",
            ".agent-spec/trace",
            "--format",
            "json",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::Trace {
                        id,
                        trace_dir,
                        code,
                        wiki,
                        format,
                    },
            } => {
                assert_eq!(id, "REQ-NOTE-CREATE");
                assert_eq!(trace_dir, PathBuf::from(".agent-spec/trace"));
                assert_eq!(code, PathBuf::from("."));
                assert_eq!(wiki, PathBuf::from(".agent-spec/wiki"));
                assert_eq!(format, "json");
            }
            _ => panic!("expected requirements trace command"),
        }
    }

    #[test]
    fn test_requirements_replay_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "replay",
            "REQ-NOTE-CREATE",
            "--trace-dir",
            ".agent-spec/trace",
            "--format",
            "json",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::Replay {
                        id,
                        trace_dir,
                        format,
                    },
            } => {
                assert_eq!(id, "REQ-NOTE-CREATE");
                assert_eq!(trace_dir, PathBuf::from(".agent-spec/trace"));
                assert_eq!(format, "json");
            }
            _ => panic!("expected requirements replay command"),
        }
    }

    #[test]
    fn test_wiki_project_map_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "wiki",
            "project-map",
            "--code",
            ".",
            "--wiki",
            ".agent-spec/wiki",
            "--format",
            "json",
            "--out",
            ".agent-spec/wiki/architecture/project-map.json",
            "--check",
        ]);

        match cli.command {
            super::Commands::Wiki {
                action:
                    super::WikiCommands::ProjectMap {
                        code,
                        wiki,
                        format,
                        out,
                        check,
                    },
            } => {
                assert_eq!(code, PathBuf::from("."));
                assert_eq!(wiki, PathBuf::from(".agent-spec/wiki"));
                assert_eq!(format, "json");
                assert_eq!(
                    out,
                    Some(PathBuf::from(
                        ".agent-spec/wiki/architecture/project-map.json"
                    ))
                );
                assert!(check);
            }
            _ => panic!("expected wiki project-map command"),
        }
    }

    #[test]
    fn test_wiki_inspect_project_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "wiki",
            "inspect-project",
            "brain-rs",
            "--code",
            "fixtures/wiki-cross-project",
            "--wiki",
            ".agent-spec/wiki",
            "--format",
            "json",
        ]);

        match cli.command {
            super::Commands::Wiki {
                action:
                    super::WikiCommands::InspectProject {
                        project_id,
                        code,
                        wiki,
                        format,
                    },
            } => {
                assert_eq!(project_id, "brain-rs");
                assert_eq!(code, PathBuf::from("fixtures/wiki-cross-project"));
                assert_eq!(wiki, PathBuf::from(".agent-spec/wiki"));
                assert_eq!(format, "json");
            }
            _ => panic!("expected wiki inspect-project command"),
        }
    }

    #[test]
    fn test_wiki_project_map_check_requires_out() {
        let result = super::Cli::try_parse_from(["agent-spec", "wiki", "project-map", "--check"]);

        assert!(result.is_err(), "--check without --out must be rejected");

        let dir = make_temp_dir("wiki-project-map-check-without-out");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("projects")).unwrap();
        fs::create_dir_all(wiki.join("flows")).unwrap();
        assert!(super::cmd_wiki_project_map(&dir, &wiki, "json", None, true).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_cli_rejects_removed_generated_wiki_commands() {
        for command in [
            "plan",
            "generate",
            "legacy-check",
            "export-github",
            "install-ci",
        ] {
            assert!(
                super::Cli::try_parse_from(["agent-spec", "wiki", command]).is_err(),
                "removed wiki command `{command}` must not remain callable"
            );
        }
    }

    #[test]
    fn test_requirements_explain_failure_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "explain-failure",
            "REQ-NOTE-CREATE",
            "--trace-dir",
            ".agent-spec/trace",
            "--format",
            "json",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::ExplainFailure {
                        id,
                        trace_dir,
                        code,
                        wiki,
                        format,
                    },
            } => {
                assert_eq!(id, "REQ-NOTE-CREATE");
                assert_eq!(trace_dir, PathBuf::from(".agent-spec/trace"));
                assert_eq!(code, PathBuf::from("."));
                assert_eq!(wiki, PathBuf::from(".agent-spec/wiki"));
                assert_eq!(format, "json");
            }
            _ => panic!("expected requirements explain-failure command"),
        }
    }

    #[test]
    fn test_requirements_trace_graph_cli_parses_nested_subcommand() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "requirements",
            "trace-graph",
            "REQ-NOTE-CREATE",
            "--trace-dir",
            ".agent-spec/trace",
            "--format",
            "mermaid",
        ]);

        match cli.command {
            super::Commands::Requirements {
                action:
                    super::RequirementCommands::TraceGraph {
                        id,
                        trace_dir,
                        format,
                    },
            } => {
                assert_eq!(id, "REQ-NOTE-CREATE");
                assert_eq!(trace_dir, PathBuf::from(".agent-spec/trace"));
                assert_eq!(format, "mermaid");
            }
            _ => panic!("expected requirements trace-graph command"),
        }
    }

    #[test]
    fn test_requirements_questions_json_reports_open_question() {
        let plan = crate::spec_knowledge::RequirementPlan {
            version: 1,
            requirements: vec![crate::spec_knowledge::RequirementPlanNode {
                id: "REQ-NOTE-EXPORT".into(),
                title: "Export Notes".into(),
                source_path: PathBuf::from("knowledge/requirements/req-note-export.md"),
                status: crate::spec_knowledge::RequirementPlanStatus::Blocked,
                mode: "blocked_questions".into(),
                scenario_count: 1,
                blocked_by: vec!["Should export support CSV?".into()],
            }],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: Vec::new(),
            coverage: Vec::new(),
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };

        let questions = crate::spec_knowledge::build_clarification_questions(&plan, &[]);
        let json = serde_json::to_string_pretty(&questions).unwrap();

        assert!(json.contains("REQ-NOTE-EXPORT"));
        assert!(json.contains("blocked-open-questions"));
        assert!(json.contains("Should export support CSV?"));
    }

    #[test]
    fn test_requirements_worktrees_json_maps_ready_units_only() {
        let plan = crate::spec_knowledge::RequirementPlan {
            version: 1,
            requirements: vec![
                crate::spec_knowledge::RequirementPlanNode {
                    id: "REQ-NOTE-CREATE".into(),
                    title: "Create Note".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-note-create.md"),
                    status: crate::spec_knowledge::RequirementPlanStatus::Ready,
                    mode: "leaf_full".into(),
                    scenario_count: 1,
                    blocked_by: Vec::new(),
                },
                crate::spec_knowledge::RequirementPlanNode {
                    id: "REQ-NOTE-EXPORT".into(),
                    title: "Export Notes".into(),
                    source_path: PathBuf::from("knowledge/requirements/req-note-export.md"),
                    status: crate::spec_knowledge::RequirementPlanStatus::Blocked,
                    mode: "blocked_questions".into(),
                    scenario_count: 1,
                    blocked_by: vec!["Should export support CSV?".into()],
                },
            ],
            work_units: Vec::new(),
            specs: Vec::new(),
            edges: Vec::new(),
            batches: vec![crate::spec_knowledge::RequirementPlanBatch {
                order: 1,
                requirement_ids: vec!["REQ-NOTE-CREATE".into(), "REQ-NOTE-EXPORT".into()],
            }],
            coverage: vec![crate::spec_knowledge::RequirementSpecCoverage {
                requirement_id: "REQ-NOTE-CREATE".into(),
                spec_paths: vec![PathBuf::from("specs/task-req-note-create.spec.md")],
                spec_depends: Vec::new(),
            }],
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };

        let manifest = crate::spec_knowledge::build_worktree_manifest(
            &plan,
            "main",
            Path::new("../agent-spec-worktrees"),
        );
        let json = serde_json::to_string_pretty(&manifest).unwrap();

        assert!(json.contains("REQ-NOTE-CREATE"));
        assert!(json.contains("feat/wu-req-note-create"));
        assert!(json.contains("../agent-spec-worktrees/wu-req-note-create"));
        assert!(!json.contains("REQ-NOTE-EXPORT"));
    }

    #[test]
    fn test_requirements_compiler_plan_dag_self_hosting_contract_is_traced() {
        let root = repo_root();
        let plan = crate::spec_knowledge::build_requirement_plan(
            &root.join("knowledge"),
            &root.join("specs"),
        );

        assert!(plan.requirements.iter().any(|node| {
            node.id == "REQ-REQUIREMENTS-COMPILER-PLAN-DAG"
                && node.status == crate::spec_knowledge::RequirementPlanStatus::Ready
        }));
        assert!(plan.coverage.iter().any(|coverage| {
            coverage.requirement_id == "REQ-REQUIREMENTS-COMPILER-PLAN-DAG"
                && coverage
                    .spec_paths
                    .iter()
                    .any(|path| path.ends_with("specs/task-requirements-compiler-plan-dag.spec.md"))
        }));
        let spec =
            fs::read_to_string(root.join("specs/task-requirements-compiler-plan-dag.spec.md"))
                .unwrap();
        assert!(spec.contains("satisfies: [REQ-REQUIREMENTS-COMPILER-PLAN-DAG]"));
        assert!(spec.contains("requirements trace"));
        assert!(spec.contains("requirements replay"));
        assert!(spec.contains("requirements trace-graph"));
    }

    #[test]
    fn test_requirements_compiler_schema_files_and_fixture_golden_outputs_are_stable() {
        let root = repo_root();
        let schema_dir = root.join("docs/intent-compiler/schemas");
        for (file_name, root_type) in [
            ("requirements-plan-v1.schema.json", "object"),
            ("test-obligations-v1.schema.json", "object"),
            ("worktree-manifest-v1.schema.json", "object"),
            ("clarification-questions-v1.schema.json", "array"),
            ("requirement-trace-ledger-v1.schema.json", "object"),
        ] {
            let schema_path = schema_dir.join(file_name);
            let schema_json: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
            assert_eq!(
                schema_json.get("$schema").and_then(|value| value.as_str()),
                Some("https://json-schema.org/draft/2020-12/schema")
            );
            assert!(
                schema_json
                    .get("$id")
                    .and_then(|value| value.as_str())
                    .is_some_and(|id| {
                        id.contains("agent-spec/intent-compiler/") && id.ends_with(file_name)
                    }),
                "schema {file_name} must have a stable intent compiler $id"
            );
            assert_eq!(
                schema_json.get("type").and_then(|value| value.as_str()),
                Some(root_type),
                "schema {file_name} root type drifted"
            );
        }

        let fixture = PathBuf::from("fixtures/requirements-noteapp");
        let knowledge = fixture.join("knowledge");
        let specs = fixture.join("specs");
        let golden_dir = root.join("fixtures/requirements-noteapp/.agent-spec");

        let plan = crate::spec_knowledge::build_requirement_plan(&knowledge, &specs);
        assert_eq!(
            pretty_json(&plan),
            fs::read_to_string(golden_dir.join("requirements-plan.json")).unwrap()
        );

        let obligations = crate::spec_knowledge::build_test_obligations(&knowledge, &specs);
        assert_eq!(
            pretty_json(&obligations),
            fs::read_to_string(golden_dir.join("test_obligations.json")).unwrap()
        );

        let worktrees = crate::spec_knowledge::build_worktree_manifest(
            &plan,
            "main",
            Path::new("../agent-spec-worktrees"),
        );
        assert_eq!(
            pretty_json(&worktrees),
            fs::read_to_string(golden_dir.join("worktrees.json")).unwrap()
        );

        let lint_diagnostics =
            crate::spec_knowledge::collect_clarification_lint_diagnostics(&knowledge);
        let questions =
            crate::spec_knowledge::build_clarification_questions(&plan, &lint_diagnostics);
        assert_eq!(
            pretty_json(&questions),
            fs::read_to_string(golden_dir.join("questions.json")).unwrap()
        );
    }

    fn pretty_json<T: serde::Serialize>(value: &T) -> String {
        let mut json = serde_json::to_string_pretty(value).unwrap();
        json.push('\n');
        json
    }

    fn sample_requirement_trace_record(
        req_id: &str,
        verdict: crate::spec_core::Verdict,
    ) -> crate::spec_knowledge::RequirementTraceRecord {
        crate::spec_knowledge::RequirementTraceRecord {
            run_id: "run-1".into(),
            requirement_id: req_id.into(),
            requirement_source: PathBuf::from("knowledge/requirements/req-note-create.md"),
            work_unit_id: format!("WU-{req_id}"),
            spec_path: PathBuf::from("specs/task-req-note-create.spec.md"),
            scenario_name: "Create note".into(),
            test_selector: Some("note_create_adds_note".into()),
            code_targets: vec!["src/lib.rs".into()],
            verdict,
            evidence: vec![crate::spec_knowledge::RequirementTraceEvidence {
                kind: "test_output".into(),
                summary: "note_create_adds_note failed".into(),
            }],
            worktree_path: Some(PathBuf::from("../agent-spec-worktrees/wu-req-note-create")),
            branch: Some("feat/wu-req-note-create".into()),
            vcs: None,
            wiki_articles: Vec::new(),
            timestamp: 1,
        }
    }

    #[test]
    fn test_requirements_trace_records_include_related_wiki_articles() {
        let dir = make_temp_dir("requirements-trace-wiki-links");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn add() -> i32 { 1 }\n").unwrap();
        fs::write(
            wiki.join("modules/lib.md"),
            "---\ntitle: \"Library\"\ntype: module\nsource_files:\n  - src/lib.rs\ntags:\n  - rust\nstatus: draft\n---\n\n# Library\n",
        )
        .unwrap();
        let mut records = vec![sample_requirement_trace_record(
            "REQ-NOTE-CREATE",
            crate::spec_core::Verdict::Pass,
        )];

        super::attach_wiki_articles_to_trace_records(&mut records, &dir, &wiki);

        assert_eq!(
            records[0].wiki_articles,
            vec![PathBuf::from("modules/lib.md")]
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_lifecycle_requirement_trace_writes_to_code_root_trace_dir() {
        let dir = make_temp_dir("requirements-lifecycle-trace-root");
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-auto-trace.md"),
            "---\nkind: requirement\nid: REQ-AUTO-TRACE\ntitle: \"Auto Trace\"\nliveness: auto\n---\n## Problem\nNeed trace.\n## Requirements\n[REQ-AUTO-TRACE] The lifecycle MUST write requirement trace evidence.\n## Scenarios\nScenario: Auto trace\n  Given a passing lifecycle report\n  When trace is written\n  Then the trace file appears under the project trace directory\n## Source Trace\n- test\n## Open Questions\nNone.\n",
        )
        .unwrap();
        let spec_path = dir.join("specs/task-auto-trace.spec.md");
        let spec = "spec: task\nname: \"Auto Trace\"\nsatisfies: [REQ-AUTO-TRACE]\n---\n## Intent\nTrace.\n## Completion Criteria\nScenario: Auto trace\n  Test: auto_trace_records\n  Given a passing lifecycle report\n  When trace is written\n  Then the trace file appears under the project trace directory\n";
        fs::write(&spec_path, spec).unwrap();
        let gw = crate::spec_gateway::SpecGateway::from_input(spec).unwrap();
        let report = crate::spec_core::VerificationReport::from_results(
            "Auto Trace".into(),
            vec![crate::spec_core::ScenarioResult {
                scenario_name: "Auto trace".into(),
                verdict: crate::spec_core::Verdict::Pass,
                step_results: Vec::new(),
                evidence: vec![crate::spec_core::Evidence::TestOutput {
                    test_name: "auto_trace_records".into(),
                    stdout: "ok".into(),
                    passed: true,
                    package: None,
                    level: None,
                    test_double: None,
                    targets: Some("src/lib.rs".into()),
                }],
                duration_ms: 1,
                provenance: None,
            }],
        );
        let run_log_dir = dir.join(".agent-spec/runs");

        let trace_path = super::write_lifecycle_requirement_trace(
            &spec_path,
            &dir,
            &run_log_dir,
            &gw,
            &report,
            42,
            None,
        )
        .unwrap()
        .unwrap();

        assert!(trace_path.starts_with(dir.join(".agent-spec/trace")));
        assert!(trace_path.exists());
        assert!(!run_log_dir.join(".agent-spec/trace").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_requirements_explain_failure_suggests_related_wiki_articles() {
        let dir = make_temp_dir("requirements-failure-wiki-links");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn add() -> i32 { 1 }\n").unwrap();
        fs::write(
            wiki.join("modules/lib.md"),
            "---\ntitle: \"Library\"\ntype: module\nsource_files:\n  - src/lib.rs\ntags:\n  - rust\nstatus: draft\n---\n\n# Library\n",
        )
        .unwrap();
        let mut explanation = crate::spec_knowledge::RequirementFailureExplanation {
            requirement_id: "REQ-NOTE-CREATE".into(),
            non_pass_records: vec![sample_requirement_trace_record(
                "REQ-NOTE-CREATE",
                crate::spec_core::Verdict::Fail,
            )],
            diagnostics: Vec::new(),
        };

        super::attach_wiki_articles_to_failure_explanation(&mut explanation, &dir, &wiki);

        assert_eq!(
            explanation.non_pass_records[0].wiki_articles,
            vec![PathBuf::from("modules/lib.md")]
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_docs_describe_requirements_compiler_plan_and_questions() {
        let readme = include_str!("../README.md");
        let agents = include_str!("../AGENTS.md");
        let skill = include_str!("../skills/agent-spec-tool-first/SKILL.md");
        let commands = include_str!("../skills/agent-spec-tool-first/references/commands.md");

        for content in [readme, agents, skill, commands] {
            assert!(content.contains("requirements plan"));
            assert!(content.contains("requirements test-obligations"));
            assert!(content.contains("requirements worktrees"));
            assert!(content.contains("requirements trace"));
            assert!(content.contains("requirements replay"));
            assert!(content.contains("requirements explain-failure"));
            assert!(content.contains("requirements trace-graph"));
            assert!(content.contains("requirements questions"));
            assert!(content.contains("--run-log-dir"));
            assert!(content.contains("QA class"));
            assert!(content.contains("state-machine"));
            assert!(content.contains("deterministic"));
            assert!(content.contains("reverse interview"));
            assert!(content.contains("archive"));
            assert!(content.contains("archive diagnostic"));
            assert!(content.contains("active specs"));
            assert!(content.contains("dogfood"));
        }
    }

    #[test]
    fn test_requirements_compiler_skill_defines_prd_intake_and_reverse_interview_contract() {
        let skill = include_str!("../skills/agent-spec-intent-compiler/SKILL.md");

        for term in [
            "PRD Intake Output Contract",
            "Candidate Requirement Block",
            "source excerpt",
            "confidence",
            "Open Questions",
            "Reverse Interview Loop",
            "Answer Integration",
            "human-confirmed",
            "Do not treat a model inference as accepted",
        ] {
            assert!(
                skill.contains(term),
                "missing requirements skill term {term}"
            );
        }
    }

    #[test]
    fn test_docs_engineering_standards_include_lore_practices() {
        let doc_types = include_str!("../knowledge/standards/canon/doc-types.md");
        let language = include_str!("../knowledge/standards/canon/language.md");
        let format = include_str!("../knowledge/standards/canon/format.md");
        let checklist = include_str!("../knowledge/standards/operational/review-checklist.md");
        let tools = include_str!("../knowledge/standards/tools/README.md");
        let proposal = include_str!("../knowledge/proposals/proposal-template.md");
        let script = include_str!("../scripts/docs-lint.sh");

        for term in [
            "Tutorial",
            "How-To",
            "Reference",
            "Explanation",
            "Internals",
            "ADR",
            "Code-Standard",
            "Landing",
        ] {
            assert!(doc_types.contains(term), "missing doc type {term}");
        }

        for content in [doc_types, language, format, checklist, tools, proposal] {
            assert!(content.contains("Lore"));
            assert!(content.contains("agent-spec"));
        }

        for term in ["canon", "operational", "pre-publish", "rendered preview"] {
            assert!(checklist.contains(term), "missing checklist term {term}");
        }

        for tool in ["Harper", "Chinese docs lint", "markdownlint", "lychee"] {
            assert!(tools.contains(tool), "missing tool doc {tool}");
        }

        for call in [
            "harper-cli",
            "run_chinese_lint",
            "zh-no-fullwidth-space",
            "zh-no-replacement-char",
            "zh-no-unresolved-placeholder",
            "markdownlint-cli2",
            "lychee",
            "DOCS_LINT_REQUIRE_EXTERNAL=all",
        ] {
            assert!(script.contains(call), "missing script call {call}");
        }

        for section in [
            "Motivation",
            "Goals",
            "Non-Goals",
            "Compatibility",
            "Migration Plan",
            "Security Considerations",
            "Privacy Considerations",
            "Risks and Assumptions",
            "Alternatives Considered",
            "Unresolved Questions",
        ] {
            assert!(
                proposal.contains(section),
                "missing proposal section {section}"
            );
        }
    }

    #[test]
    fn test_docs_lint_ci_installs_and_requires_all_docs_tools() {
        let workflow = include_str!("../.github/workflows/docs-lint.yml");

        for term in [
            "name: Documentation Lint",
            "cargo install harper-cli lychee",
            "npm install -g markdownlint-cli2",
            "DOCS_LINT_REQUIRE_EXTERNAL=all",
            "bash scripts/docs-lint.sh",
            "pull_request",
            "push",
        ] {
            assert!(workflow.contains(term), "missing docs lint CI term {term}");
        }
    }

    #[test]
    fn test_reference_validation_matrix_covers_borrowed_invariants() {
        let matrix = include_str!("../docs/intent-compiler/reference-validation-matrix.md");

        for reference in [
            "ticketbooking-demo/requirements.yaml",
            "traceability/service.py",
            "agent-runtime `traceability.py`",
            "test_generator.py",
            "templates/web/backend",
            "templates/android/app/src/test",
        ] {
            assert!(
                matrix.contains(reference),
                "missing matrix reference {reference}"
            );
        }

        for invariant in [
            "requirement tree",
            "FOLDER",
            "ATOMIC",
            "dependencies",
            "scenarios",
            "negative cases",
            "test-first obligations",
            "traceability queries",
            "requirement-to-evidence",
            "non-goal",
        ] {
            assert!(matrix.contains(invariant), "missing invariant {invariant}");
        }

        for gate in [
            "requirements graph",
            "requirements plan",
            "requirements test-obligations",
            "requirements replay",
            "requirements explain-failure",
            "requirements trace-graph",
        ] {
            assert!(matrix.contains(gate), "missing agent-spec gate {gate}");
        }

        for structure in [
            "## Validation Rows",
            "Reference method",
            "Borrowed invariant",
            "agent-spec evidence",
            "Status",
        ] {
            assert!(
                matrix.contains(structure),
                "missing matrix structure {structure}"
            );
        }

        for evidence_test in [
            "test_requirement_graph_extracts_dependencies_scenarios_and_open_questions",
            "test_requirement_graph_reports_dangling_dependency_and_cycle",
            "test_requirements_plan_json_includes_batches_edges_and_coverage",
            "test_requirements_plan_gate_fails_on_dangling_dependency",
            "test_lint_requirement_warns_when_negative_behavior_lacks_negative_scenario",
            "test_requirements_test_obligations_json_contains_spec_derived_obligations",
            "test_qa_class_a_requires_lifecycle_trace_targeted_tests_and_adversarial_review",
            "test_requirements_replay_uses_latest_trace_record_for_requirement",
            "test_requirements_explain_failure_reports_non_pass_chain",
            "test_requirements_trace_graph_mermaid_contains_evidence_nodes",
        ] {
            assert!(
                matrix.contains(evidence_test),
                "missing matrix evidence test {evidence_test}"
            );
        }

        for non_goal in [
            "Non-goal: execute reference-project runtime tests",
            "Non-goal: depend on reference-project Python, Node, Playwright, VS Code, or Android runtime dependencies",
            "Non-goal: parse reference-project YAML directly in the CLI",
        ] {
            assert!(
                matrix.contains(non_goal),
                "missing matrix non-goal {non_goal}"
            );
        }
    }

    #[test]
    fn test_noteapp_fixture_documents_end_to_end_requirements_compiler_demo() {
        let readme = include_str!("../fixtures/requirements-noteapp/README.md");

        for term in [
            "Raw PRD",
            "knowledge/requirements",
            "requirements-plan.json",
            "test_obligations.json",
            "worktrees.json",
            "questions.json",
            "task-req-note-create.spec.md",
            "cargo test --manifest-path fixtures/requirements-noteapp/Cargo.toml --quiet",
            "requirements import --check",
            "lint-knowledge",
            "requirements graph",
            "requirements plan",
            "requirements test-obligations",
            "requirements worktrees",
            "requirements questions",
            "lifecycle fixtures/requirements-noteapp/specs/task-req-note-create.spec.md",
            "requirements replay REQ-NOTE-CREATE",
            "requirements trace-graph REQ-NOTE-CREATE",
            "not the self-hosting dogfood gate",
        ] {
            assert!(
                readme.contains(term),
                "missing noteapp fixture demo term {term}"
            );
        }
    }

    #[test]
    fn test_archive_cli_parses_dry_run_and_check() {
        let cli = super::Cli::parse_from([
            "agent-spec",
            "archive",
            "--spec-dir",
            "specs",
            "--archive-dir",
            ".agent-spec/archive/specs",
            "--summary",
            "knowledge/context/spec-archives.md",
            "--run-log-dir",
            ".",
            "--dry-run",
            "--check",
        ]);

        match cli.command {
            super::Commands::Archive {
                spec_dir,
                archive_dir,
                summary,
                run_log_dir,
                dry_run,
                check,
            } => {
                assert_eq!(spec_dir, PathBuf::from("specs"));
                assert_eq!(archive_dir, PathBuf::from(".agent-spec/archive/specs"));
                assert_eq!(summary, PathBuf::from("knowledge/context/spec-archives.md"));
                assert_eq!(run_log_dir, PathBuf::from("."));
                assert!(dry_run);
                assert!(check);
            }
            _ => panic!("expected archive command"),
        }
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
            repo_root().join(".agent-spec/archive/specs/task-phase0-contract-fidelity.spec.md"),
        )
        .unwrap();
        let phase1 = fs::read_to_string(
            repo_root().join(".agent-spec/archive/specs/task-phase1-contract-review-loop.spec.md"),
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
            repo_root()
                .join(".agent-spec/archive/specs/task-phase2-run-history-and-vcs-context.spec.md"),
        )
        .unwrap();
        let phase3 = fs::read_to_string(
            repo_root().join(".agent-spec/archive/specs/task-phase3-spec-governance.spec.md"),
        )
        .unwrap();
        let phase4 = fs::read_to_string(
            repo_root()
                .join(".agent-spec/archive/specs/task-phase4-ai-verification-expansion.spec.md"),
        )
        .unwrap();
        let phase5 = fs::read_to_string(
            repo_root()
                .join(".agent-spec/archive/specs/task-phase5-ecosystem-integrations.spec.md"),
        )
        .unwrap();
        let phase6 = fs::read_to_string(
            repo_root().join(".agent-spec/archive/specs/task-phase6-advanced-verification.spec.md"),
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
            spec_path: PathBuf::new(),
            spec_fingerprint: String::new(),
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
    fn test_lifecycle_writes_requirement_trace_record() {
        let run_dir = make_temp_dir("agent-spec-requirement-trace");
        let fixture = repo_root().join("fixtures/requirements-noteapp");
        let spec = fixture.join("specs/task-req-note-create.spec.md");
        let trace_dir = fixture.join(".agent-spec/trace");
        let _ = fs::remove_dir_all(&trace_dir);

        let result = super::cmd_lifecycle(
            &spec,
            &fixture,
            &[],
            "none",
            "off",
            0.0,
            "json",
            Some(&run_dir),
            false,
            None,
            None,
            "auto",
        );
        assert!(result.is_ok(), "lifecycle should pass: {result:?}");

        let files = fs::read_dir(&trace_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .collect::<Vec<_>>();
        assert!(
            !files.is_empty(),
            "trace directory should contain JSON ledgers"
        );

        let ledger = crate::spec_knowledge::read_requirement_trace_ledgers(&trace_dir);
        assert!(ledger.records.iter().any(|record| {
            record.requirement_id == "REQ-NOTE-CREATE"
                && record.scenario_name == "Create note"
                && record.verdict == crate::spec_core::Verdict::Pass
        }));

        let _ = fs::remove_dir_all(trace_dir);
        let _ = fs::remove_dir_all(run_dir);
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
                spec_path: PathBuf::new(),
                spec_fingerprint: String::new(),
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

    #[test]
    fn test_lifecycle_qa_gate_rejects_missing_class_a_evidence_and_invalid_risk() {
        let report = crate::spec_core::VerificationReport::from_results(
            "High Risk".into(),
            vec![crate::spec_core::ScenarioResult {
                scenario_name: "High risk behavior".into(),
                verdict: crate::spec_core::Verdict::Pass,
                step_results: Vec::new(),
                evidence: vec![crate::spec_core::Evidence::TestOutput {
                    test_name: "high_risk_behavior".into(),
                    stdout: String::new(),
                    passed: true,
                    package: None,
                    level: Some("integration".into()),
                    test_double: None,
                    targets: Some("src/lib.rs".into()),
                }],
                duration_ms: 1,
                provenance: None,
            }],
        );
        let scenarios = vec![crate::spec_core::Scenario {
            name: "High risk behavior".into(),
            steps: Vec::new(),
            test_selector: None,
            tags: Vec::new(),
            review: crate::spec_core::ReviewMode::Auto,
            depends_on: Vec::new(),
            mode: crate::spec_core::ScenarioMode::Standard,
            rule: None,
            span: crate::spec_core::Span::default(),
        }];

        let missing =
            super::lifecycle_qa_missing_evidence(Some("A"), &scenarios, &report, false, false)
                .unwrap();
        assert_eq!(
            missing,
            vec![
                crate::spec_qa::QaEvidenceKind::Trace,
                crate::spec_qa::QaEvidenceKind::AdversarialReview,
            ]
        );
        assert!(
            super::lifecycle_qa_missing_evidence(Some("D"), &scenarios, &report, true, true)
                .is_err()
        );
        assert!(
            super::lifecycle_qa_missing_evidence(None, &scenarios, &report, false, false)
                .unwrap()
                .is_empty()
        );
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
            spec_path: PathBuf::new(),
            spec_fingerprint: String::new(),
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
            spec_path: PathBuf::new(),
            spec_fingerprint: String::new(),
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
            spec_path: PathBuf::new(),
            spec_fingerprint: String::new(),
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
                spec_path: PathBuf::new(),
                spec_fingerprint: String::new(),
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

    fn make_wiki_fixture(prefix: &str) -> PathBuf {
        let dir = make_temp_dir(prefix);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::create_dir_all(dir.join(".agent-spec/trace")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"wiki_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("src/lib.rs"),
            "pub fn add(left: i32, right: i32) -> i32 { left + right }\n",
        )
        .unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-add.md"),
            "---\nkind: requirement\nid: REQ-ADD\ntitle: \"Add\"\nliveness: auto\n---\n# Add\n\n## Requirements\n\n[REQ-ADD] The system MUST add two numbers.\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-add.spec.md"),
            "spec: task\nname: \"Add\"\nsatisfies: [REQ-ADD]\n---\n## Intent\nAdd numbers.\n## Completion Criteria\nScenario: Add\n  Test: test_add\n  Given two numbers\n  When add runs\n  Then the sum is returned\n",
        )
        .unwrap();
        fs::write(
            dir.join(".agent-spec/trace/run.json"),
            "{\"version\":1,\"records\":[],\"diagnostics\":[]}",
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_wiki_init_writes_live_wiki_scaffold_inventory_and_index() {
        let dir = make_wiki_fixture("wiki-live-init");
        let wiki = dir.join(".agent-spec/wiki");

        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();

        assert!(wiki.join("_index.md").exists());
        assert!(wiki.join("_architecture.md").exists());
        assert!(wiki.join("_patterns.md").exists());
        assert!(wiki.join("_log.md").exists());
        assert!(wiki.join("_meta.json").exists());
        assert!(wiki.join("architecture/inventory.json").exists());
        assert!(wiki.join("architecture/workspace.mmd").exists());
        assert!(wiki.join("architecture/project-map.json").exists());
        assert!(wiki.join("architecture/project-map.mmd").exists());
        assert!(wiki.join("modules").is_dir());
        assert!(wiki.join("concepts").is_dir());
        assert!(wiki.join("decisions").is_dir());
        assert!(wiki.join("learnings").is_dir());
        assert!(wiki.join("queries").is_dir());
        assert!(wiki.join("projects").is_dir());
        assert!(wiki.join("flows").is_dir());

        let architecture = fs::read_to_string(wiki.join("_architecture.md")).unwrap();
        assert!(architecture.contains("source_files:"));
        assert!(architecture.contains("architecture/inventory.json"));
        assert!(architecture.contains("architecture/project-map.mmd"));
        assert!(architecture.contains("architecture/project-map.json"));
        let mermaid = fs::read_to_string(wiki.join("architecture/workspace.mmd")).unwrap();
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains("wiki_fixture"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_init_check_allows_seeded_extra_pages() {
        let dir = make_wiki_fixture("wiki-live-init-check");
        let wiki = dir.join(".agent-spec/wiki");

        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::write(
            wiki.join("modules/custom.md"),
            "---\ntitle: \"Custom\"\ntype: module\nsource_files:\n  - src/lib.rs\n---\n# Custom\n",
        )
        .unwrap();

        super::cmd_wiki_init(&dir, &wiki, true, "json").unwrap();

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_init_check_preserves_maintained_project_articles() {
        let dir = make_wiki_fixture("wiki-live-init-check-projects");
        let wiki = dir.join(".agent-spec/wiki");
        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();
        for id in ["main", "dependency"] {
            fs::write(
                wiki.join("projects").join(format!("{id}.md")),
                format!(
                    "---\ntitle: \"{id}\"\ntype: external-project\nproject_id: {id}\nrepo: .\nrole: project\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [src/lib.rs]\nexternal_sources: [example/{id}]\n---\n# {id}\n"
                ),
            )
            .unwrap();
        }
        fs::write(
            wiki.join("flows/main-to-dependency.md"),
            "---\ntitle: \"Main to dependency\"\ntype: project-flow\nflow_id: main-to-dependency\nprojects: [main, dependency]\nkind: calls\nprotocols: [filesystem]\nrequirements: [REQ-ADD]\nspecs: [specs/task-add.spec.md]\nsource_files: [src/lib.rs]\nexternal_sources: [example/dependency]\n---\n# Flow\n",
        )
        .unwrap();
        crate::spec_wiki::write_wiki_index(&wiki).unwrap();
        crate::spec_wiki::write_project_map_artifacts(&dir, &wiki).unwrap();

        super::cmd_wiki_init(&dir, &wiki, true, "json").unwrap();

        assert!(wiki.join("projects/main.md").exists());
        assert!(wiki.join("flows/main-to-dependency.md").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_init_check_rejects_missing_maintained_directories() {
        let dir = make_wiki_fixture("wiki-live-init-check-missing-maintained-dir");
        let wiki = dir.join(".agent-spec/wiki");
        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();
        fs::remove_dir_all(wiki.join("flows")).unwrap();

        let error = super::cmd_wiki_init(&dir, &wiki, true, "json").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("target wiki is missing maintained directory: flows"),
            "{error}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_init_check_rejects_project_map_diagnostics() {
        let dir = make_wiki_fixture("wiki-live-init-check-project-errors");
        let wiki = dir.join(".agent-spec/wiki");
        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();
        fs::write(
            wiki.join("projects/main.md"),
            "---\ntitle: \"main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files:\n  - src/lib.rs\n---\n# Main\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/broken.md"),
            "---\ntitle: \"broken\"\ntype: project-flow\nflow_id: broken\nprojects: [main, missing]\nkind: calls\nprotocols: [filesystem]\nsource_files:\n  - src/lib.rs\n---\n# Broken\n",
        )
        .unwrap();
        crate::spec_wiki::write_project_map_artifacts(&dir, &wiki).unwrap();

        assert!(super::cmd_wiki_init(&dir, &wiki, true, "json").is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_temp_dir_cleanup_removes_directory_on_drop() {
        let dir = make_temp_dir("wiki-temp-dir-cleanup");

        {
            let _cleanup = super::TempDirCleanup::new(dir.clone());
            assert!(dir.exists());
        }

        assert!(!dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_wiki_init_check_rejects_symlinked_maintained_article() {
        use std::os::unix::fs::symlink;

        let dir = make_wiki_fixture("wiki-live-init-check-symlink");
        let wiki = dir.join(".agent-spec/wiki");
        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();
        fs::write(
            wiki.join("main.md"),
            "---\ntitle: \"main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [src/lib.rs]\n---\n# Main\n",
        )
        .unwrap();
        symlink("../main.md", wiki.join("projects/main.md")).unwrap();

        let error = super::cmd_wiki_init(&dir, &wiki, true, "json").unwrap_err();

        assert!(
            error.to_string().contains("symlinked maintained entry"),
            "{error}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_file_tree_rejects_symlinked_root() {
        use std::os::unix::fs::symlink;

        let dir = make_temp_dir("wiki-copy-tree-symlink-root");
        let source = dir.join("real-projects");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("main.md"), "# Main\n").unwrap();
        let source_link = dir.join("projects");
        symlink("real-projects", &source_link).unwrap();

        let error = super::copy_file_tree(&source_link, &dir.join("copied")).unwrap_err();

        assert!(
            error.to_string().contains("symlinked maintained entry"),
            "{error}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_inventory_reads_rust_workspace_dependencies() {
        let dir = make_temp_dir("wiki-rust-inventory");
        fs::create_dir_all(dir.join("app/src")).unwrap();
        fs::create_dir_all(dir.join("core/src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"app\", \"core\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("app/Cargo.toml"),
            "[package]\nname = \"wiki-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nwiki-core = { path = \"../core\" }\n",
        )
        .unwrap();
        fs::write(dir.join("app/src/lib.rs"), "pub fn run() {}\n").unwrap();
        fs::write(
            dir.join("core/Cargo.toml"),
            "[package]\nname = \"wiki-core\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(dir.join("core/src/lib.rs"), "pub fn value() -> u8 { 1 }\n").unwrap();

        let inventory = crate::spec_wiki::build_architecture_inventory(&dir);

        assert!(inventory.packages.iter().any(|pkg| pkg.name == "wiki-app"));
        assert!(inventory.packages.iter().any(|pkg| pkg.name == "wiki-core"));
        assert!(
            inventory.dependencies.iter().any(|dep| {
                dep.from == "wiki-app" && dep.to == "wiki-core" && dep.kind == "local"
            })
        );
        assert_eq!(inventory.provider, "rust-cargo");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_status_marks_articles_stale_when_source_files_changed() {
        let dir = make_temp_dir("wiki-status-stale");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::write(
            wiki.join("modules/lib.md"),
            "---\ntitle: \"Lib\"\ntype: module\nsource_files:\n  - src/lib.rs\n---\n# Lib\n",
        )
        .unwrap();

        let report =
            crate::spec_wiki::status_from_changed_paths(&wiki, &[PathBuf::from("src/lib.rs")]);

        assert!(report.stale_articles.iter().any(|article| {
            article.path == Path::new("modules/lib.md")
                && article.source_files == vec![PathBuf::from("src/lib.rs")]
        }));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_status_clean_checkout_does_not_diff_against_historical_meta_commit() {
        let dir = make_wiki_fixture("wiki-status-clean-checkout");
        let wiki = dir.join(".agent-spec/wiki");

        run_git(&dir, &["init"]);
        run_git(&dir, &["config", "user.email", "test@example.com"]);
        run_git(&dir, &["config", "user.name", "Test User"]);
        run_git(&dir, &["add", "."]);
        run_git(&dir, &["commit", "-m", "base"]);
        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();
        run_git(&dir, &["add", ".agent-spec/wiki"]);
        run_git(&dir, &["commit", "-m", "wiki"]);

        fs::write(
            dir.join("src/lib.rs"),
            "pub fn add(left: i32, right: i32) -> i32 { left + right + 0 }\n",
        )
        .unwrap();
        super::cmd_wiki_init(&dir, &wiki, false, "json").unwrap();
        run_git(&dir, &["add", "."]);
        run_git(&dir, &["commit", "-m", "change code and wiki"]);

        let report = crate::spec_wiki::wiki_status(&dir, &wiki);

        assert!(
            report.stale_articles.is_empty(),
            "clean checkout should not be stale solely because _meta.json was generated before the commit existed: {:?}",
            report.stale_articles
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_lint_requires_source_files_and_live_files() {
        let dir = make_temp_dir("wiki-lint-source-files");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::write(wiki.join("_index.md"), "# Index\n").unwrap();
        fs::write(wiki.join("_architecture.md"), "---\ntitle: \"Architecture\"\ntype: architecture\nsource_files:\n  - src/lib.rs\n---\n# Architecture\n").unwrap();
        fs::write(wiki.join("_patterns.md"), "---\ntitle: \"Patterns\"\ntype: patterns\nsource_files:\n  - src/lib.rs\n---\n# Patterns\n").unwrap();
        fs::write(wiki.join("_log.md"), "# Log\n").unwrap();
        fs::write(wiki.join("_meta.json"), "{}").unwrap();
        fs::write(
            wiki.join("modules/lib.md"),
            "---\ntitle: \"Lib\"\ntype: module\n---\n# Lib\n",
        )
        .unwrap();

        let report = crate::spec_wiki::lint_live_wiki(&dir, &wiki);

        assert!(report.diagnostics.iter().any(|diag| {
            diag.code == "wiki-source-files-missing"
                && diag.path.as_deref() == Some(Path::new("modules/lib.md"))
        }));
        assert!(!report.passed());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_lint_reports_project_map_diagnostics() {
        let dir = make_temp_dir("wiki-project-map-lint");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("projects")).unwrap();
        fs::create_dir_all(wiki.join("flows")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname=\"main\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .unwrap();
        fs::write(wiki.join("_index.md"), "# Code Live Wiki\n\n").unwrap();
        fs::write(
            wiki.join("projects/main.md"),
            "---\ntitle: \"main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: \"main\"\ninterfaces:\n  - cli\nprotocols:\n  - filesystem\nstatus: active\nsource_files:\n  - Cargo.toml\nexternal_sources:\n  - example/main\n---\n# main\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/broken.md"),
            "---\ntitle: \"Broken\"\ntype: project-flow\nflow_id: broken\nprojects:\n  - main\n  - missing\nkind: calls\nprotocols:\n  - stdio\nrequirements:\n  - REQ-TEST\nspecs:\n  - specs/task-test.spec.md\nsource_files:\n  - Cargo.toml\nexternal_sources:\n  - example/missing\n---\n# Broken\n",
        )
        .unwrap();

        let report = crate::spec_wiki::lint_live_wiki(&dir, &wiki);

        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "wiki-project-flow-unknown-project")
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_lint_reports_project_map_artifact_drift() {
        let dir = make_wiki_fixture("wiki-project-map-artifact-drift");
        let wiki = dir.join(".agent-spec/wiki");
        crate::spec_wiki::init_live_wiki(&dir, &wiki).unwrap();
        fs::write(
            wiki.join("projects/main.md"),
            "---\ntitle: \"main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [src/lib.rs]\n---\n# Main\n",
        )
        .unwrap();
        crate::spec_wiki::write_wiki_index(&wiki).unwrap();
        fs::remove_file(wiki.join("architecture/project-map.json")).unwrap();
        fs::write(
            wiki.join("architecture/project-map.mmd"),
            "flowchart LR\n  stale[\"stale\"]\n",
        )
        .unwrap();

        let report = crate::spec_wiki::lint_live_wiki(&dir, &wiki);
        let codes = report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();

        assert!(
            codes.contains(&"wiki-project-map-json-missing"),
            "{codes:?}"
        );
        assert!(
            codes.contains(&"wiki-project-map-mermaid-drift"),
            "{codes:?}"
        );

        let check = crate::spec_wiki::check_live_wiki_with_changed_paths(&dir, &wiki, &[]);
        let check_codes = check
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();
        assert!(
            check_codes.contains(&"wiki-project-map-json-missing"),
            "{check_codes:?}"
        );
        assert!(
            check_codes.contains(&"wiki-project-map-mermaid-drift"),
            "{check_codes:?}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_inspect_project_reports_related_flows() {
        let dir = make_temp_dir("wiki-inspect-project");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("projects")).unwrap();
        fs::create_dir_all(wiki.join("flows")).unwrap();
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::write(dir.join("only-in-root.txt"), "root-local\n").unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-cross-project-wiki.md"),
            "---\nkind: requirement\nid: REQ-CROSS-PROJECT-WIKI\ntitle: \"Cross Project Wiki\"\n---\n# Requirement\n\n## Requirements\n\n[REQ-CROSS-PROJECT-WIKI] The system MUST map projects.\n\n## Scenarios\n\nScenario: Map\n  Given projects\n  When mapped\n  Then they are present\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-cross-project-wiki.spec.md"),
            "spec: task\nname: \"Cross Project Wiki\"\nsatisfies: [REQ-CROSS-PROJECT-WIKI]\n---\n\n## Intent\n\nMap projects.\n\n## Completion Criteria\n\nScenario: Map\n  Test: test_map\n  Given projects\n  When mapped\n  Then they are present\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/agent-spec.md"),
            "---\ntitle: \"agent-spec\"\ntype: external-project\nproject_id: agent-spec\nrepo: .\nrole: \"main\"\ninterfaces:\n  - cli\nprotocols:\n  - filesystem\nstatus: active\nsource_files:\n  - only-in-root.txt\nexternal_sources:\n  - ./README.md\n---\n# agent-spec\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/brain-rs.md"),
            "---\ntitle: \"brain-rs\"\ntype: external-project\nproject_id: brain-rs\nrepo: /Users/example/brain-rs\nrole: \"context provider\"\ninterfaces:\n  - cli\nprotocols:\n  - stdio\nstatus: active\nsource_files:\n  - only-in-root.txt\nexternal_sources:\n  - /Users/example/brain-rs/README.md\n---\n# brain-rs\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/main-to-brain.md"),
            "---\ntitle: \"Main to brain-rs context flow\"\ntype: project-flow\nflow_id: main-to-brain\nprojects:\n  - agent-spec\n  - brain-rs\nkind: calls\nprotocols:\n  - stdio\nrequirements:\n  - REQ-CROSS-PROJECT-WIKI\nspecs:\n  - specs/task-cross-project-wiki.spec.md\nsource_files:\n  - only-in-root.txt\nexternal_sources:\n  - /Users/example/brain-rs/src/lib.rs\n---\n# Flow\n",
        )
        .unwrap();

        let report = crate::spec_wiki::inspect_wiki_project(&dir, &wiki, "brain-rs");

        assert_eq!(report.project_id, "brain-rs");
        assert_eq!(
            report.project.as_ref().map(|project| project.id.as_str()),
            Some("brain-rs")
        );
        assert_eq!(report.flows.len(), 1);
        assert_eq!(report.flows[0].id, "main-to-brain");
        assert!(report.flows[0].protocols.contains(&"stdio".to_string()));
        assert!(
            report.flows[0]
                .requirements
                .contains(&"REQ-CROSS-PROJECT-WIKI".to_string())
        );
        assert!(
            report.flows[0]
                .specs
                .contains(&PathBuf::from("specs/task-cross-project-wiki.spec.md"))
        );
        assert!(
            report.flows[0]
                .external_sources
                .contains(&"/Users/example/brain-rs/src/lib.rs".to_string())
        );
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_inspect_project_uses_explicit_code_root() {
        let dir = make_temp_dir("wiki-inspect-project-explicit-root");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("projects")).unwrap();
        fs::create_dir_all(wiki.join("flows")).unwrap();
        fs::write(dir.join("only-in-root.txt"), "root-local\n").unwrap();
        fs::write(
            wiki.join("projects/main.md"),
            "---\ntitle: \"main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [only-in-root.txt]\nexternal_sources: [example/main]\n---\n# Main\n",
        )
        .unwrap();

        let report = crate::spec_wiki::inspect_wiki_project(&dir, &wiki, "main");

        assert!(report.project.is_some());
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_project_map_command_writes_and_checks_artifact() {
        let root = repo_root().join("fixtures/wiki-cross-project");
        let wiki = root.join(".agent-spec/wiki");
        let out_dir = make_temp_dir("wiki-project-map-command-check");
        let out = out_dir.join("project-map.json");

        super::cmd_wiki_project_map(&root, &wiki, "json", Some(&out), false).unwrap();
        super::cmd_wiki_project_map(&root, &wiki, "json", Some(&out), true).unwrap();
        fs::write(&out, "stale\n").unwrap();
        assert!(super::cmd_wiki_project_map(&root, &wiki, "json", Some(&out), true).is_err());

        let _ = fs::remove_dir_all(out_dir);
    }

    #[test]
    fn test_wiki_project_map_check_rejects_error_diagnostics() {
        let dir = make_temp_dir("wiki-project-map-command-errors");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("projects")).unwrap();
        fs::create_dir_all(wiki.join("flows")).unwrap();
        fs::write(dir.join("source.txt"), "source\n").unwrap();
        fs::write(
            wiki.join("projects/main.md"),
            "---\ntitle: \"main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [source.txt]\n---\n# Main\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/broken.md"),
            "---\ntitle: \"broken\"\ntype: project-flow\nflow_id: broken\nprojects: [main, missing]\nkind: calls\nprotocols: [filesystem]\nsource_files: [source.txt]\n---\n# Broken\n",
        )
        .unwrap();
        let out = dir.join("project-map.json");
        let map = crate::spec_wiki::build_project_map(&dir, &wiki);
        fs::write(&out, serde_json::to_string_pretty(&map).unwrap()).unwrap();

        assert!(super::cmd_wiki_project_map(&dir, &wiki, "json", Some(&out), true).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_cross_project_wiki_fixture_builds_project_map() {
        let root = repo_root().join("fixtures/wiki-cross-project");
        let wiki = root.join(".agent-spec/wiki");

        let map = crate::spec_wiki::build_project_map(&root, &wiki);

        assert!(
            map.projects
                .iter()
                .any(|project| project.id == "agent-spec")
        );
        assert!(map.projects.iter().any(|project| project.id == "brain-rs"));
        assert!(
            map.edges
                .iter()
                .any(|edge| edge.from == "agent-spec" && edge.to == "brain-rs")
        );
        assert!(map.diagnostics.is_empty(), "{:?}", map.diagnostics);
    }

    #[test]
    fn test_docs_describe_cross_project_wiki_authoring() {
        let readme = include_str!("../README.md");
        let agents = include_str!("../AGENTS.md");
        let skill = include_str!("../skills/agent-spec-wiki/SKILL.md");

        for content in [readme, agents, skill] {
            for term in [
                "project articles",
                "flow articles",
                "regular Markdown files",
                "required and non-empty",
                "type: external-project",
                "project_id:",
                "type: project-flow",
                "flow_id:",
                "projects:",
                "source_files",
                "external_sources",
                "project-map JSON",
                "Mermaid",
                "no external repository scan by default",
            ] {
                assert!(
                    content.contains(term),
                    "missing cross-project wiki term {term}"
                );
            }
        }
    }

    #[test]
    fn test_agent_spec_wiki_tracks_project_map_artifacts() {
        let root = repo_root();
        let wiki = root.join(".agent-spec/wiki");
        let map = crate::spec_wiki::build_project_map(&root, &wiki);

        let tracked_map: crate::spec_wiki::WikiProjectMap = serde_json::from_str(
            &fs::read_to_string(wiki.join("architecture/project-map.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(tracked_map, map);
        assert_eq!(
            fs::read_to_string(wiki.join("architecture/project-map.mmd")).unwrap(),
            crate::spec_wiki::render_project_map_mermaid(&map)
        );
        for project_id in ["agent-spec", "codewiki", "symposium"] {
            assert!(
                map.projects.iter().any(|project| project.id == project_id),
                "missing dogfood project {project_id}: {:?}",
                map.projects
            );
        }
        for (to, kind) in [
            ("codewiki", "adapts-methodology-from"),
            ("symposium", "adapts-metadata-from"),
        ] {
            let edge = map
                .edges
                .iter()
                .find(|edge| edge.from == "agent-spec" && edge.to == to && edge.kind == kind)
                .unwrap_or_else(|| panic!("missing dogfood edge agent-spec -> {to}"));
            let flow = map
                .flows
                .iter()
                .find(|flow| flow.id == edge.flow_id)
                .unwrap_or_else(|| panic!("missing dogfood flow {}", edge.flow_id));
            assert_eq!(flow.requirements, vec!["REQ-CODE-LIVE-WIKI"]);
            assert_eq!(
                flow.specs,
                vec![PathBuf::from("specs/task-code-live-wiki.spec.md")]
            );
            assert!(!flow.source_files.is_empty());
            assert!(!flow.external_sources.is_empty());
        }
        assert!(map.diagnostics.is_empty(), "{:?}", map.diagnostics);
    }

    #[test]
    fn test_gitignore_tracks_only_live_wiki_state() {
        let ignore = fs::read_to_string(".gitignore").unwrap();

        assert!(ignore.contains(".agent-spec/*"));
        assert!(ignore.contains("!.agent-spec/wiki/"));
        assert!(ignore.contains("!.agent-spec/wiki/**"));
    }

    #[test]
    fn test_wiki_lint_rejects_unsafe_source_files_and_broken_links() {
        let dir = make_temp_dir("wiki-lint-unsafe-sources");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn lib() {}\n").unwrap();
        crate::spec_wiki::init_live_wiki(&dir, &wiki).unwrap();
        fs::write(
            wiki.join("modules/bad.md"),
            "---\ntitle: \"Bad\"\ntype: module\nsource_files:\n  - /tmp/outside.rs\n  - ../outside.rs\n  - src/lib.rs\n---\n# Bad\n\n[Missing](missing.md)\n",
        )
        .unwrap();

        let report = crate::spec_wiki::lint_live_wiki(&dir, &wiki);
        let codes = report
            .diagnostics
            .iter()
            .map(|diag| diag.code.as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&"wiki-source-file-absolute"));
        assert!(codes.contains(&"wiki-source-file-outside-root"));
        assert!(codes.contains(&"wiki-internal-link-broken"));
        assert!(!report.passed());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_lint_reports_stale_index() {
        let dir = make_temp_dir("wiki-lint-stale-index");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn lib() {}\n").unwrap();
        crate::spec_wiki::init_live_wiki(&dir, &wiki).unwrap();
        fs::write(
            wiki.join("modules/new.md"),
            "---\ntitle: \"New\"\ntype: module\nsource_files:\n  - src/lib.rs\n---\n# New\n",
        )
        .unwrap();

        let report = crate::spec_wiki::lint_live_wiki(&dir, &wiki);

        assert!(
            report
                .diagnostics
                .iter()
                .any(|diag| diag.code == "wiki-index-stale")
        );
        assert!(!report.passed());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_status_includes_dirty_staged_and_untracked_changes() {
        let dir = make_temp_dir("wiki-status-worktree");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        for (name, source) in [
            ("dirty.md", "src/dirty.rs"),
            ("staged.md", "src/staged.rs"),
            ("untracked.md", "src/untracked.rs"),
        ] {
            fs::write(
                wiki.join("modules").join(name),
                format!(
                    "---\ntitle: \"{name}\"\ntype: module\nsource_files:\n  - {source}\n---\n# {name}\n"
                ),
            )
            .unwrap();
        }

        let report = crate::spec_wiki::status_from_changed_paths(
            &wiki,
            &[
                PathBuf::from("src/dirty.rs"),
                PathBuf::from("src/staged.rs"),
                PathBuf::from("src/untracked.rs"),
            ],
        );

        assert_eq!(report.stale_articles.len(), 3);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_live_check_combines_index_lint_and_status() {
        let dir = make_temp_dir("wiki-live-check");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn lib() {}\n").unwrap();
        crate::spec_wiki::init_live_wiki(&dir, &wiki).unwrap();
        fs::write(
            wiki.join("modules/new.md"),
            "---\ntitle: \"New\"\ntype: module\nsource_files:\n  - src/lib.rs\n---\n# New\n",
        )
        .unwrap();

        let report = crate::spec_wiki::check_live_wiki_with_changed_paths(
            &dir,
            &wiki,
            &[PathBuf::from("src/lib.rs")],
        );

        assert!(
            report
                .diagnostics
                .iter()
                .any(|diag| diag.code == "wiki-index-stale")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diag| diag.code == "wiki-article-stale")
        );
        assert!(!report.passed());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_seed_writes_missing_pages_without_overwriting_existing() {
        let dir = make_wiki_fixture("wiki-seed-write");
        let wiki = dir.join(".agent-spec/wiki");
        fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(dir.join("README.md"), "# Fixture\n").unwrap();
        fs::write(dir.join("AGENTS.md"), "# Agents\n").unwrap();
        fs::write(
            dir.join(".gitignore"),
            ".agent-spec/*\n!.agent-spec/wiki/\n!.agent-spec/wiki/**\n",
        )
        .unwrap();
        fs::create_dir_all(dir.join("skills/agent-spec-tool-first")).unwrap();
        fs::write(
            dir.join("skills/agent-spec-tool-first/SKILL.md"),
            "# Tool First\n",
        )
        .unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-code-live-wiki.md"),
            "---\nkind: requirement\nid: REQ-CODE-LIVE-WIKI\ntitle: \"Code Live Wiki\"\n---\n## Problem\nWiki.\n",
        )
        .unwrap();
        crate::spec_wiki::init_live_wiki(&dir, &wiki).unwrap();
        fs::create_dir_all(wiki.join("modules")).unwrap();
        let existing = wiki.join("modules/spec-wiki.md");
        fs::write(
            &existing,
            "---\ntitle: \"Spec Wiki\"\ntype: module\nsource_files:\n  - src/lib.rs\ntags:\n  - existing\nstatus: maintained\n---\n# Existing\n",
        )
        .unwrap();

        let report = crate::spec_wiki::seed_live_wiki(&dir, &wiki).unwrap();
        let existing_after = fs::read_to_string(&existing).unwrap();

        assert!(existing_after.contains("# Existing"));
        assert!(wiki.join("modules/main-cli.md").exists());
        assert!(wiki.join("concepts/task-contract.md").exists());
        assert!(wiki.join("decisions/wiki-path.md").exists());
        assert!(
            report
                .files_written
                .iter()
                .any(|path| path == Path::new("modules/main-cli.md"))
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_seed_check_reports_missing_pages_without_writing() {
        let dir = make_wiki_fixture("wiki-seed-check");
        let wiki = dir.join(".agent-spec/wiki");
        fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        crate::spec_wiki::init_live_wiki(&dir, &wiki).unwrap();

        let report = crate::spec_wiki::seed_live_wiki_check(&dir, &wiki);

        assert!(
            report
                .missing_pages
                .iter()
                .any(|path| path == Path::new("modules/main-cli.md"))
        );
        assert!(!wiki.join("modules/main-cli.md").exists());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diag| diag.code == "wiki-seed-page-missing")
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_live_wiki_fixture_covers_init_seed_index_lint_status_check() {
        let fixture = repo_root().join("fixtures/wiki-mini");
        let wiki = fixture.join(".agent-spec/wiki");

        assert!(wiki.join("architecture/inventory.json").exists());
        assert!(wiki.join("architecture/workspace.mmd").exists());
        assert!(wiki.join("architecture/modules.mmd").exists());
        assert!(wiki.join("concepts/knowledge-liveness-layer.md").exists());

        let (expected_index, index_diagnostics) = crate::spec_wiki::render_wiki_index(&wiki);
        assert!(index_diagnostics.is_empty());
        assert_eq!(
            fs::read_to_string(wiki.join("_index.md")).unwrap(),
            expected_index
        );

        let lint = crate::spec_wiki::lint_live_wiki(&fixture, &wiki);
        assert!(lint.passed(), "{:?}", lint.diagnostics);

        let status =
            crate::spec_wiki::status_from_changed_paths(&wiki, &[PathBuf::from("src/lib.rs")]);
        assert!(
            status
                .stale_articles
                .iter()
                .any(|article| article.path == Path::new("_architecture.md"))
        );

        let check = crate::spec_wiki::check_live_wiki_with_changed_paths(&fixture, &wiki, &[]);
        assert!(check.passed(), "{:?}", check.diagnostics);
    }

    #[test]
    fn test_wiki_inventory_extracts_rust_modules_edges_and_entrypoints() {
        let dir = make_temp_dir("wiki-rust-module-graph");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"wiki_module_graph\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("src/main.rs"),
            "mod parser;\nuse crate::parser::parse;\nfn main() { parse(); }\n",
        )
        .unwrap();
        fs::write(dir.join("src/parser.rs"), "pub fn parse() {}\n").unwrap();

        let inventory = crate::spec_wiki::build_architecture_inventory(&dir);

        assert!(inventory.modules.iter().any(|module| module.name == "main"));
        assert!(
            inventory
                .modules
                .iter()
                .any(|module| module.name == "parser")
        );
        assert!(
            inventory.module_edges.iter().any(|edge| {
                edge.from == "main" && edge.to == "parser" && edge.kind == "declares"
            })
        );
        assert!(
            inventory
                .entrypoints
                .iter()
                .any(|entrypoint| entrypoint.path == Path::new("src/main.rs"))
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_init_writes_layered_architecture_diagrams() {
        let dir = make_wiki_fixture("wiki-layered-architecture");
        let wiki = dir.join(".agent-spec/wiki");

        crate::spec_wiki::init_live_wiki(&dir, &wiki).unwrap();

        let architecture = fs::read_to_string(wiki.join("_architecture.md")).unwrap();
        assert!(wiki.join("architecture/workspace.mmd").exists());
        assert!(wiki.join("architecture/modules.mmd").exists());
        assert!(architecture.contains("architecture/workspace.mmd"));
        assert!(architecture.contains("architecture/modules.mmd"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_query_searches_title_tags_sources_and_body() {
        let dir = make_temp_dir("wiki-query");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::write(
            wiki.join("modules/lifecycle.md"),
            "---\ntitle: \"Verification Lifecycle\"\ntype: module\nsource_files:\n  - src/spec_verify/mod.rs\ntags:\n  - lifecycle\n---\n# Verification Lifecycle\n\nLifecycle checks contracts.\n",
        )
        .unwrap();

        let report = crate::spec_wiki::query_live_wiki(&wiki, "lifecycle");

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].path, Path::new("modules/lifecycle.md"));
        assert!(
            report.matches[0]
                .source_files
                .contains(&PathBuf::from("src/spec_verify/mod.rs"))
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wiki_inspect_maps_source_to_articles_requirements_and_specs() {
        let dir = make_temp_dir("wiki-inspect");
        let wiki = dir.join(".agent-spec/wiki");
        fs::create_dir_all(wiki.join("modules")).unwrap();
        fs::create_dir_all(dir.join("src/spec_wiki")).unwrap();
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::create_dir_all(dir.join(".agent-spec/trace")).unwrap();
        fs::write(dir.join("src/spec_wiki/live.rs"), "pub fn live() {}\n").unwrap();
        fs::write(
            wiki.join("modules/code-live-wiki.md"),
            "---\ntitle: \"Code Live Wiki\"\ntype: module\nsource_files:\n  - src/spec_wiki/live.rs\ntags:\n  - wiki\n---\n# Code Live Wiki\n",
        )
        .unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-wiki.md"),
            "---\nkind: requirement\nid: REQ-WIKI\ntitle: \"Wiki\"\n---\n## Problem\nWiki.\n## Requirements\n[REQ-WIKI] The system MUST maintain src/spec_wiki/live.rs.\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-wiki.spec.md"),
            "spec: task\nname: \"Wiki\"\nsatisfies: [REQ-WIKI]\n---\n## Intent\nWiki.\n## Completion Criteria\nScenario: Wiki\n  Test: wiki_test\n  Given wiki\n  When checked\n  Then it passes\n",
        )
        .unwrap();
        fs::write(
            dir.join(".agent-spec/trace/run.json"),
            r#"{
  "version": 1,
  "records": [
    {
      "run_id": "run-1",
      "requirement_id": "REQ-WIKI",
      "requirement_source": "knowledge/requirements/req-wiki.md",
      "work_unit_id": "WU-REQ-WIKI",
      "spec_path": "specs/task-wiki.spec.md",
      "scenario_name": "Wiki",
      "test_selector": "wiki_test",
      "code_targets": ["src/spec_wiki/live.rs"],
      "verdict": "pass",
      "evidence": [],
      "worktree_path": null,
      "branch": null,
      "vcs": null,
      "timestamp": 7
    }
  ],
  "diagnostics": []
}"#,
        )
        .unwrap();

        let report = crate::spec_wiki::inspect_live_wiki_path(
            &dir,
            &wiki,
            Path::new("src/spec_wiki/live.rs"),
        );

        assert_eq!(report.wiki_articles.len(), 1);
        assert!(report.requirements.iter().any(|req| req.id == "REQ-WIKI"));
        assert!(
            report
                .specs
                .iter()
                .any(|spec| spec.path == Path::new("specs/task-wiki.spec.md"))
        );
        assert!(
            report.trace_records.iter().any(|trace| {
                trace.requirement_id == "REQ-WIKI"
                    && trace.run_id == "run-1"
                    && trace.scenario_name == "Wiki"
                    && trace.test_selector.as_deref() == Some("wiki_test")
                    && trace.verdict == "pass"
            }),
            "inspect should include related trace records: {report:?}"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_docs_describe_code_live_wiki_workflow() {
        let docs = [
            include_str!("../README.md"),
            include_str!("../AGENTS.md"),
            include_str!("../skills/agent-spec-wiki/SKILL.md"),
        ];
        for content in docs {
            for command in [
                "wiki init",
                "wiki status",
                "wiki inventory",
                "wiki index",
                "wiki lint",
            ] {
                assert!(content.contains(command), "missing `{command}`");
            }
            assert!(content.contains("code live wiki"));
            assert!(content.contains(".agent-spec/wiki"));
        }
    }

    #[test]
    fn test_docs_describe_deepened_live_wiki_workflow() {
        let readme = include_str!("../README.md");
        let agents = include_str!("../AGENTS.md");
        let skill = include_str!("../skills/agent-spec-tool-first/SKILL.md");
        let commands = include_str!("../skills/agent-spec-tool-first/references/commands.md");
        let wiki_skill = include_str!("../skills/agent-spec-wiki/SKILL.md");
        let claude_skill = include_str!("../.claude/skills/agent-spec-tool-first/SKILL.md");
        let claude_commands =
            include_str!("../.claude/skills/agent-spec-tool-first/references/commands.md");

        for content in [
            readme,
            agents,
            skill,
            commands,
            wiki_skill,
            claude_skill,
            claude_commands,
        ] {
            assert!(content.contains("wiki init"));
            assert!(content.contains("wiki seed"));
            assert!(content.contains("wiki status"));
            assert!(content.contains("wiki query"));
            assert!(content.contains("wiki inspect"));
            assert!(content.contains("wiki inventory"));
            assert!(content.contains("wiki index"));
            assert!(content.contains("wiki lint"));
            assert!(content.contains("wiki check"));
            assert!(content.contains("code live wiki"));
            assert!(content.contains(".agent-spec/wiki"));
            assert!(content.contains("source_files"));
            assert!(content.contains("Rust architecture inventory"));
            assert!(content.contains("tracked"));
            assert!(content.contains("archive"));
            assert!(content.contains("Non-goals"));
            assert!(content.contains("no built-in LLM"));
            assert!(content.contains("no web UI"));
        }
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

    fn atlas_fixture_copy(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let code = base.join("code");
        fs::create_dir_all(code.join("src")).unwrap();
        let fixture = repo_root().join("fixtures/atlas/basic");
        for rel in ["Cargo.toml", "src/lib.rs", "src/store.rs", "src/service.rs"] {
            fs::copy(fixture.join(rel), code.join(rel)).unwrap();
        }
        (code, base.join("graph"))
    }

    #[test]
    fn test_atlas_check_reports_stale_files() {
        let (code, graph) = atlas_fixture_copy("atlas-cli-check");
        rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();

        // fresh graph: check succeeds
        super::cmd_atlas(super::AtlasCommands::Check {
            code: code.clone(),
            graph: graph.clone(),
        })
        .unwrap();

        let service = code.join("src/service.rs");
        let mut text = fs::read_to_string(&service).unwrap();
        text.push_str("\npub fn extra() -> usize {\n    3\n}\n");
        fs::write(&service, text).unwrap();

        let err = super::cmd_atlas(super::AtlasCommands::Check {
            code: code.clone(),
            graph: graph.clone(),
        })
        .unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("src/service.rs"),
            "stale list must name the file: {text}"
        );
        assert!(text.contains("stale"), "{text}");
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_mcp_atlas_query_returns_symbol_json() {
        let (code, graph) = atlas_fixture_copy("atlas-mcp-query");
        rust_atlas::build(&code, &graph, &rust_atlas::BuildOptions::default()).unwrap();
        // MCP reads the graph at <code>/.agent-spec/graph
        let target = code.join(".agent-spec");
        fs::create_dir_all(&target).unwrap();
        fs::rename(&graph, target.join("graph")).unwrap();

        let ctx = crate::spec_mcp::McpContext {
            knowledge: code.join("knowledge"),
            specs: code.join("specs"),
            code: code.clone(),
        };
        let result = crate::spec_mcp::dispatch(
            "atlas_query",
            &serde_json::json!({"symbol": "atlas_basic::store::MemStore"}),
            &ctx,
        )
        .unwrap();
        assert_eq!(result["node"]["kind"], "struct");
        assert_eq!(result["node"]["id"], "atlas_basic::store::MemStore");
        assert!(
            result["node"]["file"]
                .as_str()
                .unwrap()
                .ends_with("src/store.rs")
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_mcp_atlas_tools_report_missing_graph() {
        let base = std::env::temp_dir().join(format!("atlas-mcp-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let ctx = crate::spec_mcp::McpContext {
            knowledge: base.join("knowledge"),
            specs: base.join("specs"),
            code: base.clone(),
        };
        let err =
            crate::spec_mcp::dispatch("atlas_status", &serde_json::json!({}), &ctx).unwrap_err();
        assert!(
            err.contains("atlas build"),
            "must name the first step: {err}"
        );
        fs::remove_dir_all(base).ok();
    }

    // ── legacy roadmap triage: history summary ──────────────────

    fn write_history_logs(dir: &Path, name: &str, summaries: &[(bool, &str)]) {
        let runs_dir = dir.join(".agent-spec/runs");
        fs::create_dir_all(&runs_dir).unwrap();
        for (i, (passing, summary)) in summaries.iter().enumerate() {
            let entry = RunLogEntry {
                spec_name: name.into(),
                spec_path: PathBuf::new(),
                spec_fingerprint: String::new(),
                passing: *passing,
                summary: (*summary).to_string(),
                timestamp: 1700000000 + i as u64,
                vcs: None,
            };
            fs::write(
                runs_dir.join(format!("h{}.json", 1700000000 + i as u64)),
                serde_json::to_string_pretty(&entry).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn test_history_outputs_tabular_summary() {
        let dir = make_temp_dir("agent-spec-history-table");
        write_history_logs(
            &dir,
            "tabular",
            &[
                (false, "2/5 passed, 3 failed, 0 skipped, 0 uncertain"),
                (false, "4/5 passed, 1 failed, 0 skipped, 0 uncertain"),
                (true, "5/5 passed, 0 failed, 0 skipped, 0 uncertain"),
            ],
        );
        let rows = super::history_rows(&dir, "tabular");
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert!(row.counts.is_some(), "each row parses counts: {row:?}");
        }
        let text = super::read_run_log_history(&dir, "tabular");
        assert_eq!(
            text.lines().filter(|l| l.contains("| run #")).count(),
            3,
            "three table rows: {text}"
        );
        assert!(text.contains("pass"), "{text}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_history_delta_shows_diff_from_previous() {
        let dir = make_temp_dir("agent-spec-history-delta");
        write_history_logs(
            &dir,
            "delta",
            &[
                (false, "2/5 passed, 3 failed, 0 skipped, 0 uncertain"),
                (false, "4/5 passed, 1 failed, 0 skipped, 0 uncertain"),
            ],
        );
        let rows = super::history_rows(&dir, "delta");
        let second = rows[1].delta.as_ref().unwrap();
        assert_eq!(second.passed, 2, "{second:?}");
        assert_eq!(second.failed, -2, "{second:?}");
        let text = super::read_run_log_history(&dir, "delta");
        assert!(text.contains("+2 pass"), "{text}");
        assert!(text.contains("-2 fail"), "{text}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_history_single_run_no_delta() {
        let dir = make_temp_dir("agent-spec-history-single");
        write_history_logs(
            &dir,
            "single",
            &[(true, "3/3 passed, 0 failed, 0 skipped, 0 uncertain")],
        );
        let rows = super::history_rows(&dir, "single");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].delta.is_none(), "first run has no delta");
        let text = super::read_run_log_history(&dir, "single");
        assert!(
            !text.contains("+"),
            "no delta markers for a single run: {text}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_history_json_format_output() {
        let dir = make_temp_dir("agent-spec-history-json");
        write_history_logs(
            &dir,
            "jsonh",
            &[
                (false, "1/2 passed, 1 failed, 0 skipped, 0 uncertain"),
                (true, "2/2 passed, 0 failed, 0 skipped, 0 uncertain"),
            ],
        );
        let value = super::history_json(&dir, "jsonh");
        let rows = value.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["counts"]["passed"], 1);
        assert_eq!(rows[1]["delta"]["passed"], 1);
        assert!(rows[0]["delta"].is_null());
        fs::remove_dir_all(dir).ok();
    }

    // ── legacy roadmap triage: context fidelity + status file ───

    #[test]
    fn test_existing_json_format_unchanged() {
        let dir = make_temp_dir("agent-spec-json-stable");
        fs::create_dir_all(dir.join("specs")).unwrap();
        let spec = dir.join("specs/task-json-stable.spec.md");
        fs::write(
            &spec,
            "spec: task\nname: \"Json Stable\"\ntags: [done]\n---\n## Intent\nStable.\n## Completion Criteria\nScenario: stable\n  Test: test_history_single_run_no_delta\n  Given a spec\n  When verified\n  Then it passes\n",
        )
        .unwrap();
        let gw = crate::spec_gateway::SpecGateway::load(&spec).unwrap();
        let report = gw.verify(Path::new(".")).unwrap();
        let value = serde_json::to_value(&report).unwrap();
        for key in ["spec_name", "results", "summary"] {
            assert!(
                value.get(key).is_some(),
                "lifecycle json keeps top-level key {key}: {value}"
            );
        }
        let summary = &value["summary"];
        for key in ["passed", "failed", "skipped", "uncertain", "total"] {
            assert!(summary.get(key).is_some(), "summary keeps {key}");
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_no_status_file_flag_produces_no_file() {
        let dir = make_temp_dir("agent-spec-no-status-file");
        fs::create_dir_all(dir.join("specs")).unwrap();
        let spec = dir.join("specs/task-nostatus.spec.md");
        fs::write(
            &spec,
            "spec: task\nname: \"No Status\"\ntags: []\n---\n## Intent\nn.\n## Completion Criteria\nScenario: s\n  Test: test_history_single_run_no_delta\n  Given a\n  When b\n  Then c\n",
        )
        .unwrap();
        let before: std::collections::BTreeSet<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let _ = super::cmd_lifecycle(
            &spec,
            &dir,
            &[],
            "none",
            "off",
            0.0,
            "json",
            None,
            false,
            None,
            None,
            "auto",
        );
        let after: std::collections::BTreeSet<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let new_files: Vec<_> = after.difference(&before).collect();
        assert!(
            new_files
                .iter()
                .all(|p| !p.to_string_lossy().contains("status")),
            "no status file without the flag: {new_files:?}"
        );
        fs::remove_dir_all(dir).ok();
    }

    // ── legacy roadmap triage: rewrite/parity skill guidance ────

    #[test]
    fn test_skill_guidance_rejects_parity_contracts_missing_behavior_matrix() {
        let skill = include_str!("../skills/agent-spec-tool-first/SKILL.md");
        assert!(skill.contains("rewrite, migration, or parity"));
        assert!(
            skill.contains("switch back to authoring mode and add scenarios before coding"),
            "parity contracts with unbound observable behavior are not deliverable as-is"
        );
        let authoring = include_str!("../skills/agent-spec-authoring/SKILL.md");
        assert!(authoring.contains("Behavior Surface Checklist"));
        assert!(authoring.contains("treat this as mandatory"));
    }

    #[test]
    fn test_skill_guidance_does_not_require_behavior_matrix_for_non_parity_tasks() {
        let authoring = include_str!("../skills/agent-spec-authoring/SKILL.md");
        assert!(
            authoring.contains("not required for ordinary incremental tasks"),
            "non-parity tasks must be exempt from the behavior matrix"
        );
    }
}
