use std::collections::VecDeque;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::TaskContract;
use crate::spec_core::{BoundaryCategory, Section};

/// Complete plan context: contract + codebase + task sketch.
#[derive(Debug, Clone, Serialize)]
pub struct PlanContext {
    pub contract: TaskContract,
    pub codebase_context: CodebaseContext,
    pub task_sketch: TaskSketch,
    pub warnings: Vec<String>,
}

/// Scanned codebase information.
#[derive(Debug, Clone, Serialize)]
pub struct CodebaseContext {
    pub files: Vec<FileEntry>,
    pub test_functions: Vec<TestEntry>,
}

/// A single file entry from the codebase scan.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub summary: String,
    pub pub_signatures: Vec<String>,
}

/// Test functions found in a file.
#[derive(Debug, Clone, Serialize)]
pub struct TestEntry {
    pub file: String,
    pub function_names: Vec<String>,
}

/// Scenario grouping by topological order.
#[derive(Debug, Clone, Serialize)]
pub struct TaskSketch {
    pub groups: Vec<TaskGroup>,
}

/// A group of scenarios that can be implemented together.
#[derive(Debug, Clone, Serialize)]
pub struct TaskGroup {
    pub order: usize,
    pub scenarios: Vec<String>,
    pub boundary_paths: Vec<String>,
    pub test_selectors: Vec<String>,
}

/// Depth of codebase scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanDepth {
    Shallow,
    Full,
}

impl ScanDepth {
    pub fn parse(s: &str) -> Self {
        match s {
            "full" => ScanDepth::Full,
            _ => ScanDepth::Shallow,
        }
    }
}

/// Build a complete PlanContext from a resolved spec and code directory.
pub fn build_plan_context(
    contract: &TaskContract,
    resolved: &crate::spec_core::ResolvedSpec,
    code_dir: &Path,
    depth: ScanDepth,
) -> PlanContext {
    let mut warnings = Vec::new();

    // Extract allowed path patterns from boundaries
    let allowed_patterns = collect_allowed_patterns(&resolved.task.sections);

    // Scan codebase
    let codebase_context = scan_codebase(code_dir, &allowed_patterns, depth, &mut warnings);

    // Build task sketch from scenario dependencies
    let task_sketch = build_task_sketch(&resolved.all_scenarios, &resolved.task.sections);

    PlanContext {
        contract: contract.clone(),
        codebase_context,
        task_sketch,
        warnings,
    }
}

/// Extract allowed change patterns from spec boundaries.
fn collect_allowed_patterns(sections: &[Section]) -> Vec<String> {
    let mut allowed = Vec::new();
    for section in sections {
        if let Section::Boundaries { items, .. } = section {
            for item in items {
                if item.category == BoundaryCategory::Allow && looks_like_path(&item.text) {
                    allowed.push(normalize_pattern(&item.text));
                }
            }
        }
    }
    allowed
}

fn looks_like_path(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('*')
        || trimmed.ends_with(".rs")
        || trimmed.ends_with(".ts")
        || trimmed.ends_with(".js")
        || trimmed.ends_with(".py")
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

/// Scan the codebase for files matching allowed patterns.
fn scan_codebase(
    code_dir: &Path,
    allowed_patterns: &[String],
    depth: ScanDepth,
    warnings: &mut Vec<String>,
) -> CodebaseContext {
    let gitignore_rules = load_gitignore(code_dir);

    let mut files = Vec::new();
    let mut test_functions = Vec::new();

    if allowed_patterns.is_empty() {
        return CodebaseContext {
            files,
            test_functions,
        };
    }

    for pattern in allowed_patterns {
        let base_dir = extract_base_dir(pattern);
        let scan_root = code_dir.join(&base_dir);

        if !scan_root.exists() {
            warnings.push(format!(
                "Allowed Changes path not found: {pattern} (resolved to {})",
                scan_root.display()
            ));
            continue;
        }

        let mut found_files = Vec::new();
        collect_matching_files(
            code_dir,
            &scan_root,
            pattern,
            &gitignore_rules,
            &mut found_files,
        );

        for file_path in found_files {
            let rel_path = file_path
                .strip_prefix(code_dir)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");

            // Skip if already added
            if files.iter().any(|f: &FileEntry| f.path == rel_path) {
                continue;
            }

            let (summary, pub_sigs, test_fns) = scan_file(&file_path, depth);

            files.push(FileEntry {
                path: rel_path.clone(),
                summary,
                pub_signatures: pub_sigs,
            });

            if !test_fns.is_empty() {
                test_functions.push(TestEntry {
                    file: rel_path,
                    function_names: test_fns,
                });
            }
        }
    }

    // Sort files for stable output
    files.sort_by(|a, b| a.path.cmp(&b.path));
    test_functions.sort_by(|a, b| a.file.cmp(&b.file));

    CodebaseContext {
        files,
        test_functions,
    }
}

/// Extract the static base directory from a glob pattern.
/// e.g., "src/spec_gateway/**" -> "src/spec_gateway"
fn extract_base_dir(pattern: &str) -> String {
    let parts: Vec<&str> = pattern.split('/').collect();
    let mut base = Vec::new();
    for part in parts {
        if part.contains('*') {
            break;
        }
        base.push(part);
    }
    if base.is_empty() {
        ".".to_string()
    } else {
        base.join("/")
    }
}

/// Recursively collect files matching a pattern, respecting gitignore.
fn collect_matching_files(
    code_dir: &Path,
    dir: &Path,
    pattern: &str,
    gitignore_rules: &[String],
    out: &mut Vec<PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let rel_path = path
            .strip_prefix(code_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        // Check gitignore
        if is_gitignored(&rel_path, gitignore_rules) {
            continue;
        }

        if path.is_dir() {
            collect_matching_files(code_dir, &path, pattern, gitignore_rules, out);
        } else if path_matches_pattern(pattern, &rel_path) {
            out.push(path);
        }
    }
}

/// Read a file and extract summary, pub signatures, and test function names.
fn scan_file(path: &Path, depth: ScanDepth) -> (String, Vec<String>, Vec<String>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (String::new(), Vec::new(), Vec::new()),
    };

    let lines: Vec<&str> = content.lines().collect();
    let summary = extract_summary(&lines);
    let test_fns = extract_test_functions(&lines);

    let pub_sigs = if depth == ScanDepth::Full {
        extract_pub_signatures(&lines)
    } else {
        Vec::new()
    };

    (summary, pub_sigs, test_fns)
}

/// Extract a one-line summary from the file.
/// Prefers `//!` doc comments, then first `pub fn/struct/enum/trait` line.
fn extract_summary(lines: &[&str]) -> String {
    // Check first 5 lines for module doc comments
    for line in lines.iter().take(5) {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") {
            return trimmed.trim_start_matches("//!").trim().to_string();
        }
    }

    // Fall back to first pub item
    for line in lines.iter().take(20) {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub trait ")
        {
            return trimmed.to_string();
        }
    }

    String::new()
}

/// Extract all pub fn/struct/enum/trait signatures.
fn extract_pub_signatures(lines: &[&str]) -> Vec<String> {
    let mut sigs = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("pub type ")
            || trimmed.starts_with("pub const ")
        {
            // Clean up: take just the signature part (up to `{` or end)
            let sig = trimmed
                .split('{')
                .next()
                .unwrap_or(trimmed)
                .trim()
                .trim_end_matches('{')
                .trim()
                .to_string();
            if !sigs.contains(&sig) {
                sigs.push(sig);
            }
        }
    }
    sigs
}

/// Extract test function names from a file.
fn extract_test_functions(lines: &[&str]) -> Vec<String> {
    let mut test_fns = Vec::new();
    let mut next_is_test = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "#[test]"
            || trimmed == "#[tokio::test]"
            || trimmed.starts_with("#[test]")
            || trimmed.starts_with("#[tokio::test]")
        {
            next_is_test = true;
            continue;
        }

        if next_is_test {
            if trimmed.starts_with("fn ") || trimmed.starts_with("async fn ") {
                let fn_name = trimmed
                    .trim_start_matches("async ")
                    .trim_start_matches("fn ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !fn_name.is_empty() {
                    test_fns.push(fn_name);
                }
            }
            next_is_test = false;
        }
    }

    test_fns
}

/// Load simple gitignore rules from the code directory.
fn load_gitignore(code_dir: &Path) -> Vec<String> {
    let gitignore_path = code_dir.join(".gitignore");
    let file = match std::fs::File::open(gitignore_path) {
        Ok(f) => f,
        Err(_) => {
            // Return default ignore rules
            return vec![
                "target/".to_string(),
                ".git/".to_string(),
                "node_modules/".to_string(),
            ];
        }
    };

    let mut rules: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    // Always include these
    if !rules.iter().any(|r| r.starts_with(".git")) {
        rules.push(".git/".to_string());
    }

    rules
}

/// Check if a relative path should be ignored based on gitignore rules.
fn is_gitignored(rel_path: &str, rules: &[String]) -> bool {
    for rule in rules {
        let rule = rule.trim_end_matches('/');
        if rel_path.starts_with(rule)
            || rel_path.contains(&format!("/{rule}/"))
            || rel_path.contains(&format!("/{rule}"))
        {
            return true;
        }
        // Simple glob: if rule ends with *, check prefix
        if let Some(prefix) = rule.strip_suffix('*')
            && rel_path.starts_with(prefix)
        {
            return true;
        }
    }
    false
}

/// Glob pattern matching (reuses logic from boundaries.rs).
fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&pattern_segments, &path_segments)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }

    if pattern[0] == "**" {
        return (0..=path.len()).any(|i| match_segments(&pattern[1..], &path[i..]));
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

/// Build a task sketch from scenario dependencies using topological sort.
fn build_task_sketch(scenarios: &[crate::spec_core::Scenario], sections: &[Section]) -> TaskSketch {
    if scenarios.is_empty() {
        return TaskSketch { groups: Vec::new() };
    }

    let n = scenarios.len();
    let name_to_idx: std::collections::HashMap<&str, usize> = scenarios
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.as_str(), i))
        .collect();

    // Build adjacency list and in-degree for Kahn's algorithm
    let mut in_degree = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

    for (i, scenario) in scenarios.iter().enumerate() {
        for dep in &scenario.depends_on {
            if let Some(&j) = name_to_idx.get(dep.as_str()) {
                adj[j].push(i);
                in_degree[i] += 1;
            }
        }
    }

    // Kahn's topological sort, collecting by layer
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    let mut groups = Vec::new();
    let mut order = 1;

    while !queue.is_empty() {
        // All nodes currently in the queue are at the same topological layer
        let layer_size = queue.len();
        let mut layer_scenarios = Vec::new();

        for _ in 0..layer_size {
            if let Some(u) = queue.pop_front() {
                layer_scenarios.push(u);
                for &v in &adj[u] {
                    in_degree[v] -= 1;
                    if in_degree[v] == 0 {
                        queue.push_back(v);
                    }
                }
            }
        }

        if !layer_scenarios.is_empty() {
            let scenario_names: Vec<String> = layer_scenarios
                .iter()
                .map(|&i| scenarios[i].name.clone())
                .collect();

            // Collect boundary paths from scenario step text
            let boundary_paths =
                collect_boundary_paths_for_scenarios(&layer_scenarios, scenarios, sections);

            // Collect test selectors
            let test_selectors: Vec<String> = layer_scenarios
                .iter()
                .filter_map(|&i| {
                    scenarios[i]
                        .test_selector
                        .as_ref()
                        .map(|s| s.filter.clone())
                })
                .collect();

            groups.push(TaskGroup {
                order,
                scenarios: scenario_names,
                boundary_paths,
                test_selectors,
            });
            order += 1;
        }
    }

    TaskSketch { groups }
}

/// Collect boundary paths relevant to a set of scenarios.
fn collect_boundary_paths_for_scenarios(
    scenario_indices: &[usize],
    scenarios: &[crate::spec_core::Scenario],
    sections: &[Section],
) -> Vec<String> {
    // Get all allowed change paths
    let allowed = collect_allowed_patterns(sections);

    // Check scenario steps for file path references
    let mut paths: Vec<String> = Vec::new();
    for &idx in scenario_indices {
        let scenario = &scenarios[idx];
        for step in &scenario.steps {
            for pattern in &allowed {
                if (step.text.contains(pattern) || pattern_base_mentioned(&step.text, pattern))
                    && !paths.contains(pattern)
                {
                    paths.push(pattern.clone());
                }
            }
        }
    }

    // If no specific paths found from steps, include all allowed paths
    if paths.is_empty() {
        return allowed;
    }

    paths
}

/// Check if a step mentions the base directory of a pattern.
fn pattern_base_mentioned(text: &str, pattern: &str) -> bool {
    let base = extract_base_dir(pattern);
    if base == "." {
        return false;
    }
    text.contains(&base)
}

// === Formatting functions ===

/// Format PlanContext as human-readable text.
pub fn format_plan_text(ctx: &PlanContext) -> String {
    let mut out = String::new();

    // Contract section
    out.push_str("=== Contract ===\n\n");
    out.push_str(&ctx.contract.to_prompt());

    // Codebase Context section
    out.push_str("=== Codebase Context ===\n\n");
    if ctx.codebase_context.files.is_empty() {
        out.push_str("(no matching files found)\n");
    } else {
        out.push_str(&format!("Files ({}):\n", ctx.codebase_context.files.len()));
        for file in &ctx.codebase_context.files {
            if file.summary.is_empty() {
                out.push_str(&format!("  - {}\n", file.path));
            } else {
                out.push_str(&format!("  - {} — {}\n", file.path, file.summary));
            }
            for sig in &file.pub_signatures {
                out.push_str(&format!("      {sig}\n"));
            }
        }
    }

    if !ctx.codebase_context.test_functions.is_empty() {
        out.push_str(&format!(
            "\nTest functions ({}):\n",
            ctx.codebase_context.test_functions.len()
        ));
        for entry in &ctx.codebase_context.test_functions {
            out.push_str(&format!("  {}:\n", entry.file));
            for name in &entry.function_names {
                out.push_str(&format!("    - {name}\n"));
            }
        }
    }
    out.push('\n');

    // Task Sketch section
    out.push_str("=== Task Sketch ===\n\n");
    if ctx.task_sketch.groups.is_empty() {
        out.push_str("(no scenarios found)\n");
    } else {
        for group in &ctx.task_sketch.groups {
            out.push_str(&format!("Group {} (order {}):\n", group.order, group.order));
            out.push_str("  Scenarios:\n");
            for name in &group.scenarios {
                out.push_str(&format!("    - {name}\n"));
            }
            if !group.boundary_paths.is_empty() {
                out.push_str("  Boundary paths:\n");
                for p in &group.boundary_paths {
                    out.push_str(&format!("    - {p}\n"));
                }
            }
            if !group.test_selectors.is_empty() {
                out.push_str("  Test selectors:\n");
                for s in &group.test_selectors {
                    out.push_str(&format!("    - {s}\n"));
                }
            }
            out.push('\n');
        }
    }

    // Warnings
    if !ctx.warnings.is_empty() {
        out.push_str("=== Warnings ===\n\n");
        for w in &ctx.warnings {
            out.push_str(&format!("  - {w}\n"));
        }
    }

    out
}

/// Format PlanContext as JSON.
pub fn format_plan_json(ctx: &PlanContext) -> String {
    serde_json::to_string_pretty(ctx).unwrap_or_default()
}

/// Format PlanContext as a self-contained AI prompt.
pub fn format_plan_prompt(ctx: &PlanContext) -> String {
    let mut out = String::new();

    out.push_str("You are implementing the following spec. Generate an implementation plan.\n\n");
    out.push_str("---\n\n");

    // Full contract with all inherited constraints
    out.push_str(&ctx.contract.to_prompt());
    out.push_str("---\n\n");

    // Codebase context
    out.push_str("# Codebase Context\n\n");
    if ctx.codebase_context.files.is_empty() {
        out.push_str("No matching files found in the allowed change paths.\n\n");
    } else {
        out.push_str("## Existing Files\n\n");
        for file in &ctx.codebase_context.files {
            if file.summary.is_empty() {
                out.push_str(&format!("- `{}`\n", file.path));
            } else {
                out.push_str(&format!("- `{}` — {}\n", file.path, file.summary));
            }
            for sig in &file.pub_signatures {
                out.push_str(&format!("  - `{sig}`\n"));
            }
        }
        out.push('\n');
    }

    if !ctx.codebase_context.test_functions.is_empty() {
        out.push_str("## Existing Tests\n\n");
        for entry in &ctx.codebase_context.test_functions {
            out.push_str(&format!("- `{}`:\n", entry.file));
            for name in &entry.function_names {
                out.push_str(&format!("  - `{name}`\n"));
            }
        }
        out.push('\n');
    }

    out.push_str("---\n\n");

    // Task sketch
    out.push_str("# Suggested Implementation Order\n\n");

    // Behavior rules: which examples prove which rule (Phase 1 BDD grouping).
    if !ctx.contract.rules.is_empty() {
        out.push_str("## Behavior Rules\n\n");
        for rule in &ctx.contract.rules {
            if rule.name == rule.key.id {
                out.push_str(&format!("- Rule `{}`\n", rule.key.id));
            } else {
                out.push_str(&format!("- Rule `{}` — {}\n", rule.key.id, rule.name));
            }
            for scenario in ctx
                .contract
                .completion_criteria
                .iter()
                .filter(|s| s.rule.as_deref() == Some(rule.key.id.as_str()))
            {
                out.push_str(&format!("  - {}\n", scenario.name));
            }
        }
        out.push('\n');
    }

    if ctx.task_sketch.groups.is_empty() {
        out.push_str("No scenario dependencies detected.\n\n");
    } else {
        for group in &ctx.task_sketch.groups {
            out.push_str(&format!("## Phase {}\n\n", group.order));
            out.push_str("Scenarios:\n");
            for name in &group.scenarios {
                out.push_str(&format!("- {name}\n"));
            }
            if !group.test_selectors.is_empty() {
                out.push_str("\nTest selectors:\n");
                for s in &group.test_selectors {
                    out.push_str(&format!("- `{s}`\n"));
                }
            }
            out.push('\n');
        }
    }

    // Warnings
    if !ctx.warnings.is_empty() {
        out.push_str("---\n\n");
        out.push_str("# Warnings\n\n");
        for w in &ctx.warnings {
            out.push_str(&format!("- {w}\n"));
        }
        out.push('\n');
    }

    out.push_str("---\n\n");
    out.push_str("Based on the contract, codebase context, and suggested order above, ");
    out.push_str("generate a detailed implementation plan with specific file changes, ");
    out.push_str("code snippets, and verification steps for each scenario group.\n\n");

    out.push_str("## Verification (MANDATORY)\n\n");
    out.push_str("After implementation, you MUST run:\n");
    out.push_str("```bash\nagent-spec lifecycle <spec> --code . --format json\n```\n");
    out.push_str("Do NOT claim completion until all scenarios show \"pass\".\n\n");

    out.push_str("## Execution Protocol\n\n");
    out.push_str("### Two-Stage Review\n");
    out.push_str("After implementing each scenario group:\n");
    out.push_str("1. **Spec compliance**: Execute `agent-spec lifecycle` with that group's test selectors. All must show `pass`.\n");
    out.push_str("2. **Code quality**: Review for dead code, `.unwrap()` in production paths, boundary violations, unnecessary complexity.\n\n");
    out.push_str(
        "Do NOT proceed to the next group until both stages pass for the current group.\n\n",
    );

    out.push_str("### Status Reporting\n");
    out.push_str("After each scenario group, report your status:\n");
    out.push_str("- **DONE** — All scenarios in this group pass lifecycle verification\n");
    out.push_str(
        "- **DONE_WITH_CONCERNS** — Scenarios pass but you have doubts (explain what and why)\n",
    );
    out.push_str("- **NEEDS_CONTEXT** — You need information not provided in the contract or codebase context\n");
    out.push_str("- **BLOCKED** — You cannot complete this group (explain the blocker)\n\n");
    out.push_str(
        "If BLOCKED: do not attempt workarounds. Report the blocker and wait for guidance.\n",
    );

    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_core::{
        Boundary, BoundaryCategory, ResolvedSpec, Scenario, Section, Span, SpecDocument, SpecLevel,
        SpecMeta, Step, StepKind, TestSelector,
    };

    fn make_test_contract() -> TaskContract {
        TaskContract {
            name: "Test Task".into(),
            intent: "Test intent".into(),
            must: vec!["must do X".into()],
            must_not: vec!["must not do Y".into()],
            decisions: vec!["decision A".into()],
            allowed_changes: vec!["src/spec_gateway/**".into()],
            forbidden: vec!["src/spec_parser/**".into()],
            out_of_scope: vec!["AI generation".into()],
            completion_criteria: Vec::new(),
            rules: vec![],
        }
    }

    fn make_test_resolved() -> ResolvedSpec {
        let scenarios = vec![
            Scenario {
                name: "scenario A".into(),
                steps: vec![Step {
                    kind: StepKind::When,
                    text: "execute plan".into(),
                    params: Vec::new(),
                    table: Vec::new(),
                    span: Span::default(),
                }],
                test_selector: Some(TestSelector {
                    filter: "test_a".into(),
                    package: Some("agent-spec".into()),
                    level: None,
                    test_double: None,
                    targets: None,
                }),
                tags: Vec::new(),
                review: crate::spec_core::ReviewMode::Auto,
                mode: crate::spec_core::ScenarioMode::Standard,
                depends_on: Vec::new(),
                rule: None,
                span: Span::default(),
            },
            Scenario {
                name: "scenario B".into(),
                steps: vec![],
                test_selector: None,
                tags: Vec::new(),
                review: crate::spec_core::ReviewMode::Auto,
                mode: crate::spec_core::ScenarioMode::Standard,
                depends_on: Vec::new(),
                rule: None,
                span: Span::default(),
            },
            Scenario {
                name: "scenario C".into(),
                steps: vec![],
                test_selector: Some(TestSelector {
                    filter: "test_c".into(),
                    package: Some("agent-spec".into()),
                    level: None,
                    test_double: None,
                    targets: None,
                }),
                tags: Vec::new(),
                review: crate::spec_core::ReviewMode::Auto,
                mode: crate::spec_core::ScenarioMode::Standard,
                depends_on: vec!["scenario A".into()],
                rule: None,
                span: Span::default(),
            },
            Scenario {
                name: "scenario D".into(),
                steps: vec![],
                test_selector: None,
                tags: Vec::new(),
                review: crate::spec_core::ReviewMode::Auto,
                mode: crate::spec_core::ScenarioMode::Standard,
                depends_on: Vec::new(),
                rule: None,
                span: Span::default(),
            },
        ];

        ResolvedSpec {
            task: SpecDocument {
                meta: SpecMeta {
                    level: SpecLevel::Task,
                    name: "Test Task".into(),
                    inherits: None,
                    lang: vec![],
                    tags: vec![],
                    depends: vec![],
                    estimate: None,
                    capability: None,
                    satisfies: vec![],                },
                sections: vec![Section::Boundaries {
                    items: vec![Boundary {
                        category: BoundaryCategory::Allow,
                        text: "src/spec_gateway/**".into(),
                        span: Span::default(),
                    }],
                    span: Span::default(),
                }],
                lint_acks: vec![],
                source_path: std::path::PathBuf::new(),
            },
            inherited_constraints: Vec::new(),
            inherited_decisions: Vec::new(),
            all_scenarios: scenarios,
        }
    }

    #[test]
    fn test_plan_includes_contract_section() {
        let contract = make_test_contract();
        let resolved = make_test_resolved();
        let ctx = build_plan_context(
            &contract,
            &resolved,
            std::path::Path::new("."),
            ScanDepth::Shallow,
        );

        assert_eq!(ctx.contract.name, "Test Task");
        assert_eq!(ctx.contract.intent, "Test intent");
        assert!(!ctx.contract.must.is_empty());
        assert!(!ctx.contract.decisions.is_empty());
        assert!(!ctx.contract.allowed_changes.is_empty());

        let text = format_plan_text(&ctx);
        assert!(text.contains("=== Contract ==="));
        assert!(text.contains("Intent"));
        assert!(text.contains("Boundaries"));
        assert!(text.contains("Completion Criteria") || text.contains("Must"));
    }

    #[test]
    fn test_plan_includes_codebase_context() {
        let contract = make_test_contract();
        let resolved = make_test_resolved();
        // Use actual project root so we scan real files
        let code_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let ctx = build_plan_context(&contract, &resolved, code_dir, ScanDepth::Shallow);

        assert!(!ctx.codebase_context.files.is_empty());
        // Check that files have summaries
        let has_summary = ctx
            .codebase_context
            .files
            .iter()
            .any(|f| !f.summary.is_empty());
        assert!(has_summary);

        let text = format_plan_text(&ctx);
        assert!(text.contains("=== Codebase Context ==="));
    }

    #[test]
    fn test_plan_includes_task_sketch() {
        let contract = make_test_contract();
        let resolved = make_test_resolved();
        let ctx = build_plan_context(
            &contract,
            &resolved,
            std::path::Path::new("."),
            ScanDepth::Shallow,
        );

        // Should have at least 2 groups: {A, B, D} and {C}
        assert!(ctx.task_sketch.groups.len() >= 2);

        // scenario A should be in an earlier group than scenario C
        let a_group = ctx
            .task_sketch
            .groups
            .iter()
            .find(|g| g.scenarios.contains(&"scenario A".to_string()))
            .map(|g| g.order);
        let c_group = ctx
            .task_sketch
            .groups
            .iter()
            .find(|g| g.scenarios.contains(&"scenario C".to_string()))
            .map(|g| g.order);

        assert!(a_group.is_some());
        assert!(c_group.is_some());
        assert!(a_group.unwrap() < c_group.unwrap());

        let text = format_plan_text(&ctx);
        assert!(text.contains("=== Task Sketch ==="));
    }

    #[test]
    fn test_plan_respects_gitignore() {
        let contract = TaskContract {
            name: "Test".into(),
            intent: "Test".into(),
            must: Vec::new(),
            must_not: Vec::new(),
            decisions: Vec::new(),
            allowed_changes: vec!["**".into()],
            forbidden: Vec::new(),
            out_of_scope: Vec::new(),
            completion_criteria: Vec::new(),
            rules: vec![],
        };

        let resolved = ResolvedSpec {
            task: SpecDocument {
                meta: SpecMeta {
                    level: SpecLevel::Task,
                    name: "Test".into(),
                    inherits: None,
                    lang: vec![],
                    tags: vec![],
                    depends: vec![],
                    estimate: None,
                    capability: None,
                    satisfies: vec![],                },
                sections: vec![Section::Boundaries {
                    items: vec![Boundary {
                        category: BoundaryCategory::Allow,
                        text: "**".into(),
                        span: Span::default(),
                    }],
                    span: Span::default(),
                }],
                lint_acks: vec![],
                source_path: PathBuf::new(),
            },
            inherited_constraints: Vec::new(),
            inherited_decisions: Vec::new(),
            all_scenarios: Vec::new(),
        };

        let code_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let ctx = build_plan_context(&contract, &resolved, code_dir, ScanDepth::Shallow);

        // None of the files should be under target/
        for file in &ctx.codebase_context.files {
            assert!(
                !file.path.starts_with("target/"),
                "target/ file should be gitignored: {}",
                file.path
            );
        }
    }

    #[test]
    fn test_plan_json_format_is_valid() {
        let contract = make_test_contract();
        let resolved = make_test_resolved();
        let ctx = build_plan_context(
            &contract,
            &resolved,
            std::path::Path::new("."),
            ScanDepth::Shallow,
        );

        let json = format_plan_json(&ctx);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("contract").is_some());
        assert!(parsed.get("codebase_context").is_some());
        assert!(parsed.get("task_sketch").is_some());
    }

    #[test]
    fn test_plan_prompt_format_is_self_contained() {
        let mut contract = make_test_contract();
        contract
            .must
            .push("inherited constraint from project".into());

        let resolved = make_test_resolved();
        let ctx = build_plan_context(
            &contract,
            &resolved,
            std::path::Path::new("."),
            ScanDepth::Shallow,
        );

        let prompt = format_plan_prompt(&ctx);

        // Should contain inherited constraints
        assert!(prompt.contains("inherited constraint from project"));

        // Should NOT contain CLI dependency instructions
        assert!(!prompt.contains("run agent-spec"));
        assert!(!prompt.contains("agent-spec plan"));

        // Should contain guiding instruction
        assert!(prompt.contains("implementation plan"));

        // Should contain verification guidance
        assert!(prompt.contains("Verification (MANDATORY)"));
        assert!(prompt.contains("agent-spec lifecycle"));

        // Should contain execution protocol (P6 Two-Stage Review + P7 Status Protocol)
        assert!(prompt.contains("Two-Stage Review"));
        assert!(prompt.contains("DONE_WITH_CONCERNS"));
        assert!(prompt.contains("BLOCKED"));
    }

    #[test]
    fn test_plan_prompt_includes_rule_grouping() {
        let doc = crate::spec_parser::parse_spec_from_str(
            r#"spec: task
name: "退款"
---

## 完成条件

### Rule: refund-must-be-idempotent — 退款幂等
场景: 首次退款成功
  测试: t1
  当 退款
  那么 成功
场景: 重复退款不重复扣减
  测试: t2
  当 再次退款
  那么 不重复
"#,
        )
        .unwrap();
        let contract = crate::spec_gateway::TaskContract::from_doc(&doc);
        let resolved = make_test_resolved();
        let ctx = build_plan_context(
            &contract,
            &resolved,
            std::path::Path::new("."),
            ScanDepth::Shallow,
        );
        let prompt = format_plan_prompt(&ctx);

        assert!(prompt.contains("## Behavior Rules"));
        assert!(prompt.contains("Rule `refund-must-be-idempotent` — 退款幂等"));
        // The two examples proving the rule are listed under it.
        let rule_pos = prompt.find("refund-must-be-idempotent").unwrap();
        let ex_pos = prompt.find("首次退款成功").unwrap();
        assert!(rule_pos < ex_pos, "examples listed under their rule");
    }

    #[test]
    fn test_plan_full_depth_includes_pub_signatures() {
        let contract = make_test_contract();
        let resolved = make_test_resolved();
        let code_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let ctx = build_plan_context(&contract, &resolved, code_dir, ScanDepth::Full);

        // At least some files should have pub signatures
        let has_sigs = ctx
            .codebase_context
            .files
            .iter()
            .any(|f| !f.pub_signatures.is_empty());
        assert!(has_sigs, "full depth should include pub signatures");
    }

    #[test]
    fn test_plan_warns_on_missing_boundary_path() {
        let contract = TaskContract {
            name: "Test".into(),
            intent: "Test".into(),
            must: Vec::new(),
            must_not: Vec::new(),
            decisions: Vec::new(),
            allowed_changes: vec!["src/nonexistent/**".into()],
            forbidden: Vec::new(),
            out_of_scope: Vec::new(),
            completion_criteria: Vec::new(),
            rules: vec![],
        };

        let resolved = ResolvedSpec {
            task: SpecDocument {
                meta: SpecMeta {
                    level: SpecLevel::Task,
                    name: "Test".into(),
                    inherits: None,
                    lang: vec![],
                    tags: vec![],
                    depends: vec![],
                    estimate: None,
                    capability: None,
                    satisfies: vec![],                },
                sections: vec![Section::Boundaries {
                    items: vec![Boundary {
                        category: BoundaryCategory::Allow,
                        text: "src/nonexistent/**".into(),
                        span: Span::default(),
                    }],
                    span: Span::default(),
                }],
                lint_acks: vec![],
                source_path: PathBuf::new(),
            },
            inherited_constraints: Vec::new(),
            inherited_decisions: Vec::new(),
            all_scenarios: Vec::new(),
        };

        let code_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let ctx = build_plan_context(&contract, &resolved, code_dir, ScanDepth::Shallow);

        assert!(!ctx.warnings.is_empty(), "should warn about missing path");
        assert!(
            ctx.warnings.iter().any(|w| w.contains("nonexistent")),
            "warning should mention the missing path"
        );
    }

    #[test]
    fn test_plan_lists_existing_test_functions() {
        let contract = make_test_contract();
        let resolved = make_test_resolved();
        let code_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let ctx = build_plan_context(&contract, &resolved, code_dir, ScanDepth::Shallow);

        // The spec_gateway directory should have test functions (at least in brief.rs or this file)
        // Since we're scanning src/spec_gateway/**, we should find tests
        let has_tests = !ctx.codebase_context.test_functions.is_empty();
        assert!(has_tests, "should list test functions from scanned files");

        // Check that function names are present
        for entry in &ctx.codebase_context.test_functions {
            assert!(!entry.function_names.is_empty());
        }
    }

    #[test]
    fn test_extract_summary_doc_comment() {
        let lines = vec!["//! Module documentation", "pub struct Foo;"];
        assert_eq!(extract_summary(&lines), "Module documentation");
    }

    #[test]
    fn test_extract_summary_pub_item() {
        let lines = vec!["use std::path::Path;", "", "pub struct MyStruct {"];
        assert_eq!(extract_summary(&lines), "pub struct MyStruct {");
    }

    #[test]
    fn test_extract_test_functions() {
        let lines = vec![
            "#[test]",
            "fn test_basic() {",
            "    assert!(true);",
            "}",
            "#[tokio::test]",
            "async fn test_async() {",
            "}",
        ];
        let fns = extract_test_functions(&lines);
        assert_eq!(fns, vec!["test_basic", "test_async"]);
    }

    #[test]
    fn test_extract_pub_signatures() {
        let lines = vec![
            "pub fn hello(name: &str) -> String {",
            "    format!(\"hello {name}\")",
            "}",
            "pub struct Foo {",
            "    bar: i32,",
            "}",
            "pub enum Color {",
        ];
        let sigs = extract_pub_signatures(&lines);
        assert_eq!(sigs.len(), 3);
        assert!(sigs[0].starts_with("pub fn hello"));
        assert!(sigs[1].starts_with("pub struct Foo"));
        assert!(sigs[2].starts_with("pub enum Color"));
    }

    #[test]
    fn test_is_gitignored() {
        let rules = vec!["target/".into(), ".git/".into(), "node_modules/".into()];
        assert!(is_gitignored("target/debug/build", &rules));
        assert!(is_gitignored(".git/config", &rules));
        assert!(!is_gitignored("src/main.rs", &rules));
    }

    #[test]
    fn test_extract_base_dir() {
        assert_eq!(extract_base_dir("src/spec_gateway/**"), "src/spec_gateway");
        assert_eq!(extract_base_dir("**"), ".");
        assert_eq!(extract_base_dir("src/*.rs"), "src");
    }
}
