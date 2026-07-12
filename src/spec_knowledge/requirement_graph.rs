//! Requirement graph extraction from KLL requirement artifacts.

use crate::spec_knowledge::{
    KnowledgeKind, KnowledgeParseError, NormativeKeyword, collect_knowledge_checked,
    extract_requirements,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementGraph {
    pub nodes: Vec<RequirementNode>,
    pub diagnostics: Vec<RequirementGraphDiagnostic>,
    pub parse_errors: Vec<KnowledgeParseErrorView>,
}

impl RequirementGraph {
    pub fn node(&self, requirement_id: &str) -> Option<&RequirementNode> {
        self.nodes
            .iter()
            .find(|node| node.id.eq_ignore_ascii_case(requirement_id))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementNode {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<crate::spec_knowledge::DecisionStatus>,
    pub source_path: PathBuf,
    pub problem: String,
    pub clauses: Vec<RequirementClauseView>,
    pub dependencies: Vec<String>,
    pub children: Vec<String>,
    pub scenarios: Vec<RequirementScenario>,
    pub source_trace: Vec<String>,
    pub open_questions: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementClauseView {
    pub id: Option<String>,
    pub keyword: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementScenario {
    pub name: String,
    pub steps: Vec<RequirementStep>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementStep {
    pub keyword: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequirementGraphDiagnostic {
    pub code: String,
    pub severity: String,
    pub requirement_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KnowledgeParseErrorView {
    pub path: PathBuf,
    pub message: String,
}

pub fn build_requirement_graph(knowledge_dir: &Path) -> RequirementGraph {
    let collection = collect_knowledge_checked(knowledge_dir);
    let parse_errors = collection
        .parse_errors
        .iter()
        .map(parse_error_view)
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut nodes = collection
        .docs
        .into_iter()
        .filter(|doc| doc.meta.kind == KnowledgeKind::Requirement)
        .map(|doc| {
            if doc.meta.status.is_none() {
                diagnostics.push(diag(
                    "requirement-governance-missing",
                    Some(doc.meta.id.clone()),
                    format!(
                        "{} has no governance status; declare `status:` (proposed|accepted|superseded|deprecated|rejected) — missing status fails the governance gate",
                        doc.meta.id
                    ),
                ));
            }
            let title = if let Some(title) = &doc.meta.title {
                title.clone()
            } else {
                diagnostics.push(diag(
                    "missing-title",
                    Some(doc.meta.id.clone()),
                    format!("{} has no frontmatter title", doc.meta.id),
                ));
                doc.meta.id.clone()
            };
            RequirementNode {
                id: doc.meta.id.clone(),
                title,
                status: doc.meta.status,
                source_path: doc.source_path.clone(),
                problem: section_body(&doc, "Problem"),
                clauses: extract_requirements(&doc)
                    .into_iter()
                    .map(|clause| RequirementClauseView {
                        id: clause.id,
                        keyword: clause.keyword.map(normative_keyword_string),
                        text: clause.text,
                    })
                    .collect(),
                dependencies: extract_id_lines(&section_body(&doc, "Dependencies")),
                children: extract_id_lines(&section_body(&doc, "Child Requirements")),
                scenarios: parse_scenarios(&section_body(&doc, "Scenarios")),
                source_trace: extract_plain_lines(&section_body(&doc, "Source Trace")),
                open_questions: parse_open_questions(&section_body(&doc, "Open Questions")),
                tags: doc.meta.tags.clone(),
            }
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then_with(|| a.source_path.cmp(&b.source_path))
    });
    diagnostics.sort_by(diag_sort);
    RequirementGraph {
        nodes,
        diagnostics,
        parse_errors,
    }
}

pub fn validate_requirement_graph(graph: &RequirementGraph) -> Vec<RequirementGraphDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut by_id: BTreeMap<&str, Vec<&RequirementNode>> = BTreeMap::new();
    for node in &graph.nodes {
        by_id.entry(node.id.as_str()).or_default().push(node);
    }

    for (id, nodes) in &by_id {
        if nodes.len() > 1 {
            diagnostics.push(diag(
                "duplicate-requirement-id",
                Some((*id).to_string()),
                format!("{id} is declared by {} files", nodes.len()),
            ));
        }
    }

    let known = by_id.keys().copied().collect::<BTreeSet<_>>();
    for node in &graph.nodes {
        for dep in &node.dependencies {
            if !known.contains(dep.as_str()) {
                diagnostics.push(diag(
                    "dangling-dependency",
                    Some(node.id.clone()),
                    format!("{} depends on missing requirement {dep}", node.id),
                ));
            }
        }
        for child in &node.children {
            if !known.contains(child.as_str()) {
                diagnostics.push(diag(
                    "dangling-child",
                    Some(node.id.clone()),
                    format!("{} references missing child requirement {child}", node.id),
                ));
            }
        }
        if node.children.is_empty() && node.scenarios.is_empty() && node.open_questions.is_empty() {
            diagnostics.push(diag(
                "missing-scenarios",
                Some(node.id.clone()),
                format!("{} is a leaf requirement with no scenarios", node.id),
            ));
        }
        if !node.open_questions.is_empty() {
            diagnostics.push(diag(
                "blocked-open-questions",
                Some(node.id.clone()),
                format!("{} has open questions", node.id),
            ));
        }
    }

    let mut child_parent: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for node in &graph.nodes {
        for child in &node.children {
            child_parent
                .entry(child.as_str())
                .or_default()
                .push(node.id.as_str());
        }
    }
    for (child, parents) in child_parent {
        if parents.len() > 1 {
            diagnostics.push(diag(
                "duplicate-child-parent",
                Some(child.to_string()),
                format!(
                    "{child} is listed under multiple parents: {}",
                    parents.join(", ")
                ),
            ));
        }
    }

    diagnostics.extend(find_dependency_cycles(graph));
    diagnostics.extend(find_child_cycles(graph));
    diagnostics.sort_by(diag_sort);
    diagnostics
}

fn parse_error_view(error: &KnowledgeParseError) -> KnowledgeParseErrorView {
    KnowledgeParseErrorView {
        path: error.path.clone(),
        message: error.message.clone(),
    }
}

fn section_body(doc: &crate::spec_knowledge::KnowledgeDoc, heading: &str) -> String {
    doc.section(heading)
        .map(|section| section.body.clone())
        .unwrap_or_default()
}

fn normative_keyword_string(keyword: NormativeKeyword) -> String {
    match keyword {
        NormativeKeyword::Must => "MUST",
        NormativeKeyword::MustNot => "MUST NOT",
        NormativeKeyword::Should => "SHOULD",
        NormativeKeyword::ShouldNot => "SHOULD NOT",
        NormativeKeyword::May => "MAY",
    }
    .to_string()
}

fn extract_id_lines(body: &str) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for line in body.lines() {
        let line = line.trim().trim_start_matches('-').trim();
        if line.is_empty()
            || line.eq_ignore_ascii_case("none.")
            || line.eq_ignore_ascii_case("none")
        {
            continue;
        }
        for token in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
            if is_requirement_id_token(token) {
                ids.insert(token.to_ascii_uppercase());
            }
        }
    }
    ids.into_iter().collect()
}

fn is_requirement_id_token(token: &str) -> bool {
    let token = token.trim();
    !token.is_empty()
        && token.contains('-')
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn extract_plain_lines(body: &str) -> Vec<String> {
    body.lines()
        .map(|line| line.trim().trim_start_matches('-').trim())
        .filter(|line| !line.is_empty())
        .filter(|line| !line.eq_ignore_ascii_case("none") && !line.eq_ignore_ascii_case("none."))
        .map(str::to_string)
        .collect()
}

fn parse_open_questions(body: &str) -> Vec<String> {
    let normalized = body.trim();
    if normalized.is_empty()
        || normalized.eq_ignore_ascii_case("none")
        || normalized.eq_ignore_ascii_case("none.")
        || normalized == "无"
        || normalized == "无。"
    {
        Vec::new()
    } else {
        extract_plain_lines(body)
    }
}

fn parse_scenarios(body: &str) -> Vec<RequirementScenario> {
    let mut scenarios = Vec::new();
    let mut current: Option<RequirementScenario> = None;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix("Scenario:")
            .or_else(|| line.strip_prefix("场景:"))
        {
            if let Some(scenario) = current.take() {
                scenarios.push(scenario);
            }
            current = Some(RequirementScenario {
                name: name.trim().to_string(),
                steps: Vec::new(),
            });
        } else if let Some(step) = parse_step(line)
            && let Some(scenario) = current.as_mut()
        {
            scenario.steps.push(step);
        }
    }
    if let Some(scenario) = current.take() {
        scenarios.push(scenario);
    }
    scenarios
}

fn parse_step(line: &str) -> Option<RequirementStep> {
    const KEYWORDS: &[&str] = &[
        "Given", "When", "Then", "And", "But", "假设", "当", "那么", "并且", "但是",
    ];
    for keyword in KEYWORDS {
        if let Some(content) = line.strip_prefix(keyword) {
            let content = content.trim();
            if !content.is_empty() {
                return Some(RequirementStep {
                    keyword: (*keyword).to_string(),
                    content: content.to_string(),
                });
            }
        }
    }
    None
}

fn find_dependency_cycles(graph: &RequirementGraph) -> Vec<RequirementGraphDiagnostic> {
    let adjacency = graph
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.as_str(),
                node.dependencies
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    let mut reported = BTreeSet::new();
    for node in &graph.nodes {
        let mut visiting = BTreeSet::new();
        let mut path = Vec::new();
        detect_cycle(
            node.id.as_str(),
            &adjacency,
            &mut visiting,
            &mut path,
            &mut reported,
            &mut diagnostics,
        );
    }
    diagnostics
}

fn detect_cycle<'a>(
    id: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    path: &mut Vec<&'a str>,
    reported: &mut BTreeSet<String>,
    diagnostics: &mut Vec<RequirementGraphDiagnostic>,
) {
    if let Some(pos) = path.iter().position(|existing| *existing == id) {
        let mut cycle = path[pos..].to_vec();
        cycle.push(id);
        let key = canonical_cycle_key(&cycle);
        if reported.insert(key) {
            diagnostics.push(diag(
                "dependency-cycle",
                Some(id.to_string()),
                format!("dependency cycle detected: {}", cycle.join(" -> ")),
            ));
        }
        return;
    }
    if !visiting.insert(id) {
        return;
    }
    path.push(id);
    for dep in adjacency.get(id).into_iter().flatten() {
        if adjacency.contains_key(dep) {
            detect_cycle(dep, adjacency, visiting, path, reported, diagnostics);
        }
    }
    path.pop();
    visiting.remove(id);
}

fn canonical_cycle_key(cycle: &[&str]) -> String {
    let mut ids = cycle[..cycle.len().saturating_sub(1)].to_vec();
    ids.sort_unstable();
    ids.join("|")
}

fn find_child_cycles(graph: &RequirementGraph) -> Vec<RequirementGraphDiagnostic> {
    let adjacency = graph
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.as_str(),
                node.children.iter().map(String::as_str).collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    let mut reported = BTreeSet::new();
    for node in &graph.nodes {
        let mut visiting = BTreeSet::new();
        let mut path = Vec::new();
        detect_child_cycle(
            node.id.as_str(),
            &adjacency,
            &mut visiting,
            &mut path,
            &mut reported,
            &mut diagnostics,
        );
    }
    diagnostics
}

fn detect_child_cycle<'a>(
    id: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    path: &mut Vec<&'a str>,
    reported: &mut BTreeSet<String>,
    diagnostics: &mut Vec<RequirementGraphDiagnostic>,
) {
    if let Some(pos) = path.iter().position(|existing| *existing == id) {
        let mut cycle = path[pos..].to_vec();
        cycle.push(id);
        let key = canonical_cycle_key(&cycle);
        if reported.insert(key) {
            diagnostics.push(diag(
                "child-cycle",
                Some(id.to_string()),
                format!("child cycle detected: {}", cycle.join(" -> ")),
            ));
        }
        return;
    }
    if !visiting.insert(id) {
        return;
    }
    path.push(id);
    for child in adjacency.get(id).into_iter().flatten() {
        if adjacency.contains_key(child) {
            detect_child_cycle(child, adjacency, visiting, path, reported, diagnostics);
        }
    }
    path.pop();
    visiting.remove(id);
}

fn diag(code: &str, requirement_id: Option<String>, message: String) -> RequirementGraphDiagnostic {
    RequirementGraphDiagnostic {
        code: code.to_string(),
        severity: severity_for(code).to_string(),
        requirement_id,
        message,
    }
}

fn severity_for(code: &str) -> &'static str {
    match code {
        "duplicate-requirement-id"
        | "dangling-dependency"
        | "dangling-child"
        | "dependency-cycle"
        | "child-cycle"
        | "duplicate-child-parent"
        | "requirement-governance-missing" => "error",
        "missing-title" | "missing-scenarios" | "blocked-open-questions" => "warning",
        _ => "warning",
    }
}

fn diag_sort(a: &RequirementGraphDiagnostic, b: &RequirementGraphDiagnostic) -> std::cmp::Ordering {
    a.requirement_id
        .cmp(&b.requirement_id)
        .then_with(|| a.code.cmp(&b.code))
        .then_with(|| a.message.cmp(&b.message))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("agent-spec-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("requirements")).unwrap();
        dir
    }

    #[test]
    fn test_requirement_graph_extracts_dependencies_scenarios_and_open_questions() {
        let dir = temp_dir("req-graph-ok");
        fs::write(
            dir.join("requirements/req-100-session.md"),
            "---\nkind: requirement\nid: REQ-100\ntitle: \"Session Persistence\"\n---\n## Problem\nSession exists.\n## Requirements\n[REQ-100] The session service MUST persist sessions.\n## Scenarios\nScenario: Persist session\n  Given valid session data\n  When the service stores the session\n  Then the session is available for later reads\n## Open Questions\nNone.\n",
        )
        .unwrap();
        fs::write(
            dir.join("requirements/req-101-login.md"),
            "---\nkind: requirement\nid: REQ-101\ntitle: \"User Login\"\ntags: [auth]\n---\n## Problem\nLogin.\n## Requirements\n[REQ-101] The authentication service MUST create a login session.\n## Dependencies\n- REQ-100\n## Scenarios\nScenario: Valid login\n  Given a valid account\n  When valid credentials are submitted\n  Then a login session is created\n## Open Questions\nNone.\n",
        )
        .unwrap();

        let graph = build_requirement_graph(&dir);
        assert!(graph.parse_errors.is_empty());
        assert_eq!(graph.nodes.len(), 2);
        let login = graph.node("REQ-101").unwrap();
        assert_eq!(login.title, "User Login");
        assert_eq!(login.dependencies, vec!["REQ-100"]);
        assert_eq!(login.tags, vec!["auth"]);
        assert_eq!(login.scenarios.len(), 1);
        assert!(login.open_questions.is_empty());
        assert!(validate_requirement_graph(&graph).is_empty());
    }

    #[test]
    fn test_requirement_graph_reports_dangling_dependency_and_cycle() {
        let dir = temp_dir("req-graph-bad");
        fs::write(
            dir.join("requirements/req-201-a.md"),
            "---\nkind: requirement\nid: REQ-201\n---\n## Problem\nA.\n## Requirements\n[REQ-201] The service MUST do A.\n## Dependencies\n- REQ-202\n- REQ-999\n",
        )
        .unwrap();
        fs::write(
            dir.join("requirements/req-202-b.md"),
            "---\nkind: requirement\nid: REQ-202\n---\n## Problem\nB.\n## Requirements\n[REQ-202] The service MUST do B.\n## Dependencies\n- REQ-201\n",
        )
        .unwrap();

        let graph = build_requirement_graph(&dir);
        let diagnostics = validate_requirement_graph(&graph);
        assert!(diagnostics.iter().any(|d| d.code == "dangling-dependency"));
        assert!(diagnostics.iter().any(|d| d.code == "dependency-cycle"));
    }

    #[test]
    fn test_requirement_graph_reports_child_cycle_and_duplicate_parent() {
        let graph = RequirementGraph {
            nodes: vec![
                RequirementNode {
                    id: "REQ-A".into(),
                    title: "A".into(),
                    status: None,
                    source_path: PathBuf::from("knowledge/requirements/req-a.md"),
                    problem: "A".into(),
                    clauses: Vec::new(),
                    dependencies: Vec::new(),
                    children: vec!["REQ-B".into(), "REQ-C".into()],
                    scenarios: Vec::new(),
                    source_trace: Vec::new(),
                    open_questions: Vec::new(),
                    tags: Vec::new(),
                },
                RequirementNode {
                    id: "REQ-B".into(),
                    title: "B".into(),
                    status: None,
                    source_path: PathBuf::from("knowledge/requirements/req-b.md"),
                    problem: "B".into(),
                    clauses: Vec::new(),
                    dependencies: Vec::new(),
                    children: vec!["REQ-A".into()],
                    scenarios: Vec::new(),
                    source_trace: Vec::new(),
                    open_questions: Vec::new(),
                    tags: Vec::new(),
                },
                RequirementNode {
                    id: "REQ-C".into(),
                    title: "C".into(),
                    status: None,
                    source_path: PathBuf::from("knowledge/requirements/req-c.md"),
                    problem: "C".into(),
                    clauses: Vec::new(),
                    dependencies: Vec::new(),
                    children: Vec::new(),
                    scenarios: Vec::new(),
                    source_trace: Vec::new(),
                    open_questions: Vec::new(),
                    tags: Vec::new(),
                },
                RequirementNode {
                    id: "REQ-D".into(),
                    title: "D".into(),
                    status: None,
                    source_path: PathBuf::from("knowledge/requirements/req-d.md"),
                    problem: "D".into(),
                    clauses: Vec::new(),
                    dependencies: Vec::new(),
                    children: vec!["REQ-C".into()],
                    scenarios: Vec::new(),
                    source_trace: Vec::new(),
                    open_questions: Vec::new(),
                    tags: Vec::new(),
                },
            ],
            diagnostics: Vec::new(),
            parse_errors: Vec::new(),
        };

        let diagnostics = validate_requirement_graph(&graph);
        assert!(diagnostics.iter().any(|diag| diag.code == "child-cycle"));
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.code == "duplicate-child-parent")
        );
    }
}
