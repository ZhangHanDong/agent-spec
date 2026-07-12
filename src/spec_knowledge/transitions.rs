//! Explicit requirement governance transitions.
//!
//! Governance status answers whether a requirement may enter executable
//! lowering. Per `docs/intent-compiler/architecture.md`, compilation never
//! mutates this status; the only sanctioned writers are the human-invoked
//! `requirements transition` and `requirements supersede` commands below.
//! Rewrites are line-precise: only the frontmatter `status:` (and, for
//! supersession, `supersedes:`) line changes.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceError {
    pub message: String,
}

impl std::fmt::Display for GovernanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GovernanceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionOutcome {
    pub id: String,
    pub path: PathBuf,
    pub old_status: Option<String>,
    pub new_status: String,
}

/// Apply a legal governance transition to the requirement with `id` under
/// `knowledge_dir`, rewriting only its frontmatter `status:` line.
pub fn transition_requirement(
    knowledge_dir: &Path,
    id: &str,
    to: &str,
) -> Result<TransitionOutcome, GovernanceError> {
    let to = to.trim().to_ascii_lowercase();
    match to.as_str() {
        "proposed" | "accepted" | "rejected" | "deprecated" => {}
        "superseded" => {
            return Err(err(
                "status `superseded` is set through `requirements supersede <OLD> --by <NEW>`, not a direct transition".to_string(),
            ));
        }
        other => {
            return Err(err(format!(
                "unknown governance status `{other}`; expected proposed, accepted, rejected, or deprecated"
            )));
        }
    }

    let (path, content, old_status) = find_requirement(knowledge_dir, id)?;
    let legal = matches!(
        (old_status.as_deref(), to.as_str()),
        (None, "proposed" | "accepted")
            | (Some("proposed"), "accepted" | "rejected")
            | (Some("accepted"), "deprecated")
    );
    if !legal {
        return Err(err(format!(
            "illegal governance transition for {}: {} -> {to}",
            id.to_ascii_uppercase(),
            old_status.as_deref().unwrap_or("(missing)")
        )));
    }

    let rewritten = set_frontmatter_line(&content, "status", &to)?;
    std::fs::write(&path, rewritten)
        .map_err(|error| err(format!("cannot write {}: {error}", path.display())))?;
    Ok(TransitionOutcome {
        id: id.to_ascii_uppercase(),
        path,
        old_status,
        new_status: to,
    })
}

/// Mark `old_id` superseded by `new_id`: sets `status: superseded` on the old
/// document and records `supersedes: <OLD>` in the new document. Atomic: a
/// failure leaves both documents unchanged.
pub fn supersede_requirement(
    knowledge_dir: &Path,
    old_id: &str,
    new_id: &str,
) -> Result<(TransitionOutcome, PathBuf), GovernanceError> {
    let (old_path, old_content, old_status) = find_requirement(knowledge_dir, old_id)?;
    let (new_path, new_content, new_status) = find_requirement(knowledge_dir, new_id)?;
    if old_path == new_path {
        return Err(err(format!(
            "a requirement cannot supersede itself: {}",
            old_id.to_ascii_uppercase()
        )));
    }
    if matches!(new_status.as_deref(), Some("superseded") | Some("rejected")) {
        return Err(err(format!(
            "replacement {} is `{}` and cannot supersede {}",
            new_id.to_ascii_uppercase(),
            new_status.as_deref().unwrap_or_default(),
            old_id.to_ascii_uppercase()
        )));
    }

    // Compute both rewrites before the first write so failures change nothing.
    let old_rewritten = set_frontmatter_line(&old_content, "status", "superseded")?;
    let new_rewritten =
        set_frontmatter_line(&new_content, "supersedes", &old_id.to_ascii_uppercase())?;

    std::fs::write(&old_path, &old_rewritten)
        .map_err(|error| err(format!("cannot write {}: {error}", old_path.display())))?;
    if let Err(error) = std::fs::write(&new_path, &new_rewritten) {
        // restore the first write so the pair stays consistent
        let _ = std::fs::write(&old_path, &old_content);
        return Err(err(format!("cannot write {}: {error}", new_path.display())));
    }
    Ok((
        TransitionOutcome {
            id: old_id.to_ascii_uppercase(),
            path: old_path,
            old_status,
            new_status: "superseded".to_string(),
        },
        new_path,
    ))
}

fn err(message: String) -> GovernanceError {
    GovernanceError { message }
}

/// Locate a requirement document by id under `<knowledge_dir>/requirements/**`.
/// Returns its path, full content, and current frontmatter status value.
fn find_requirement(
    knowledge_dir: &Path,
    id: &str,
) -> Result<(PathBuf, String, Option<String>), GovernanceError> {
    let wanted = id.trim().to_ascii_uppercase();
    crate::spec_knowledge::validate_knowledge_id(&wanted).map_err(err)?;
    let root = knowledge_dir.join("requirements");
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|error| err(format!("cannot read {}: {error}", dir.display())))?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let content = std::fs::read_to_string(&path)
                .map_err(|error| err(format!("cannot read {}: {error}", path.display())))?;
            let Some(front) = frontmatter(&content) else {
                continue;
            };
            let doc_id = value_of(front, "id").map(|v| v.to_ascii_uppercase());
            if doc_id.as_deref() == Some(wanted.as_str()) {
                let status = value_of(front, "status").map(|v| v.to_ascii_lowercase());
                return Ok((path, content, status));
            }
        }
    }
    Err(err(format!(
        "no requirement document under {} declares id {wanted}",
        root.display()
    )))
}

fn frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

fn value_of<'a>(front: &'a str, key: &str) -> Option<&'a str> {
    front.lines().find_map(|line| {
        line.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix(':'))
            .map(str::trim)
    })
}

/// Replace the frontmatter `<key>:` line with `<key>: <value>`, inserting it
/// after `title:` (or `id:`) when absent. Every other byte is preserved.
fn set_frontmatter_line(content: &str, key: &str, value: &str) -> Result<String, GovernanceError> {
    let Some(front) = frontmatter(content) else {
        return Err(err("document has no frontmatter block".to_string()));
    };
    let front_len = "---\n".len() + front.len();
    let (head, tail) = content.split_at(front_len);

    let mut lines: Vec<String> = head.split_inclusive('\n').map(str::to_string).collect();
    let prefix = format!("{key}:");
    if let Some(line) = lines.iter_mut().find(|line| line.starts_with(&prefix)) {
        *line = format!("{key}: {value}\n");
        return Ok(lines.concat() + tail);
    }
    let anchor = lines
        .iter()
        .position(|line| line.starts_with("title:"))
        .or_else(|| lines.iter().position(|line| line.starts_with("id:")));
    let Some(anchor) = anchor else {
        return Err(err(format!(
            "frontmatter has no `title:` or `id:` line to anchor `{key}:`"
        )));
    };
    lines.insert(anchor + 1, format!("{key}: {value}\n"));
    Ok(lines.concat() + tail)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("requirements")).unwrap();
        dir
    }

    fn write_req(dir: &Path, file: &str, id: &str, status: Option<&str>) -> PathBuf {
        let status_line = status.map(|s| format!("status: {s}\n")).unwrap_or_default();
        let content = format!(
            "---\nkind: requirement\nid: {id}\ntitle: \"T {id}\"\n{status_line}liveness: auto\ntags: []\n---\n\n# T\n\n## Problem\n\np\n\n## Requirements\n\n[{id}-ONE] The system MUST hold the first obligation.\n\n## Scenarios\n\nScenario: holds\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n"
        );
        let path = dir.join("requirements").join(file);
        fs::write(&path, &content).unwrap();
        path
    }

    #[test]
    fn test_requirement_graph_reports_missing_governance_status() {
        let dir = make_temp_tree("gov-missing-status");
        write_req(&dir, "req-a.md", "REQ-GOV-A", None);
        let graph = crate::spec_knowledge::build_requirement_graph(&dir);
        let mut diagnostics = graph.diagnostics.clone();
        diagnostics.extend(crate::spec_knowledge::validate_requirement_graph(&graph));
        assert!(
            diagnostics.iter().any(|d| d.severity == "error"
                && d.code == "requirement-governance-missing"
                && d.message.contains("REQ-GOV-A")),
            "missing status must be an error diagnostic: {diagnostics:?}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_work_units_demote_missing_status_to_informational() {
        let dir = make_temp_tree("gov-units-missing");
        write_req(&dir, "req-a.md", "REQ-GOV-A", None);
        let graph = crate::spec_knowledge::build_requirement_graph(&dir);
        let units = crate::spec_knowledge::build_work_units(&graph);
        let unit = units
            .units
            .iter()
            .find(|u| u.requirement_id == "REQ-GOV-A")
            .unwrap();
        assert_eq!(
            unit.status,
            crate::spec_knowledge::WorkUnitStatus::Informational,
            "missing status must never schedule work"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_markdown_intake_emits_proposed_status() {
        let input = "<!-- agent-spec:requirement id=REQ-GOV-M title=\"Marked\" -->\n## Problem\n\np\n\n## Requirements\n\n[REQ-GOV-M] The system MUST parse marked blocks.\n<!-- /agent-spec:requirement -->";
        let blocks = crate::spec_knowledge::parse_requirement_blocks(input, "prd.md").unwrap();
        let rendered = crate::spec_knowledge::render_requirement_artifact(&blocks[0]);
        assert!(
            rendered.contains("\nstatus: proposed\n"),
            "intake must emit proposed candidates: {rendered}"
        );
    }

    #[test]
    fn test_repo_requirements_declare_governance_status() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        for entry in fs::read_dir(repo.join("knowledge/requirements")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let doc = crate::spec_knowledge::parse_requirement(&path)
                .unwrap_or_else(|e| panic!("{} must parse: {e}", path.display()));
            assert!(
                doc.meta.status.is_some(),
                "{} must declare a governance status",
                path.display()
            );
            checked += 1;
        }
        assert!(
            checked >= 8,
            "expected the repository corpus, saw {checked}"
        );
    }

    #[test]
    fn test_requirements_transition_rewrites_only_status_line() {
        let dir = make_temp_tree("gov-transition-ok");
        let path = write_req(&dir, "req-a.md", "REQ-GOV-A", Some("proposed"));
        let before = fs::read_to_string(&path).unwrap();

        let outcome = transition_requirement(&dir, "REQ-GOV-A", "accepted").unwrap();
        assert_eq!(outcome.old_status.as_deref(), Some("proposed"));
        assert_eq!(outcome.new_status, "accepted");

        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(
            after,
            before.replace("status: proposed", "status: accepted"),
            "only the status line may change"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirements_transition_rejects_illegal_transition() {
        let dir = make_temp_tree("gov-transition-illegal");
        let path = write_req(&dir, "req-a.md", "REQ-GOV-A", Some("accepted"));
        let before = fs::read_to_string(&path).unwrap();

        let err = transition_requirement(&dir, "REQ-GOV-A", "proposed").unwrap_err();
        assert!(
            err.to_string().contains("accepted") && err.to_string().contains("proposed"),
            "diagnostic must name the illegal transition: {err}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirements_transition_rejects_unknown_id() {
        let dir = make_temp_tree("gov-transition-unknown");
        write_req(&dir, "req-a.md", "REQ-GOV-A", Some("proposed"));
        let err = transition_requirement(&dir, "REQ-GHOST", "accepted").unwrap_err();
        assert!(err.to_string().contains("REQ-GHOST"), "{err}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirements_supersede_updates_both_documents() {
        let dir = make_temp_tree("gov-supersede-ok");
        let old_path = write_req(&dir, "req-old.md", "REQ-GOV-OLD", Some("accepted"));
        let new_path = write_req(&dir, "req-new.md", "REQ-GOV-NEW", Some("accepted"));

        let (outcome, replacement) =
            supersede_requirement(&dir, "REQ-GOV-OLD", "REQ-GOV-NEW").unwrap();
        assert_eq!(outcome.new_status, "superseded");
        assert_eq!(replacement, new_path);

        let old_doc = fs::read_to_string(&old_path).unwrap();
        assert!(old_doc.contains("status: superseded"));
        let new_doc = fs::read_to_string(&new_path).unwrap();
        assert!(new_doc.contains("supersedes: REQ-GOV-OLD"));

        let docs = crate::spec_knowledge::collect_knowledge(&dir);
        let findings = crate::spec_knowledge::lint_corpus(&docs);
        assert!(
            !findings.iter().any(|f| f.message.contains("supersedes")
                && f.severity == crate::spec_core::Severity::Error),
            "supersession pair must satisfy governance lint: {findings:?}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_requirements_supersede_rejects_unknown_target_atomically() {
        let dir = make_temp_tree("gov-supersede-unknown");
        let old_path = write_req(&dir, "req-old.md", "REQ-GOV-OLD", Some("accepted"));
        let before = fs::read_to_string(&old_path).unwrap();

        let err = supersede_requirement(&dir, "REQ-GOV-OLD", "REQ-GHOST").unwrap_err();
        assert!(err.to_string().contains("REQ-GHOST"), "{err}");
        assert_eq!(
            fs::read_to_string(&old_path).unwrap(),
            before,
            "a failed supersession must change neither document"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_compiler_reads_do_not_mutate_governance_status() {
        let dir = make_temp_tree("gov-pure-reads");
        write_req(&dir, "req-a.md", "REQ-GOV-A", Some("accepted"));
        write_req(&dir, "req-b.md", "REQ-GOV-B", Some("proposed"));
        write_req(&dir, "req-c.md", "REQ-GOV-C", None);
        let snapshot: Vec<(PathBuf, String)> = fs::read_dir(dir.join("requirements"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .map(|p| (p.clone(), fs::read_to_string(&p).unwrap()))
            .collect();

        fs::create_dir_all(dir.join("specs")).unwrap();
        let graph = crate::spec_knowledge::build_requirement_graph(&dir);
        let _ = crate::spec_knowledge::validate_requirement_graph(&graph);
        let _ = crate::spec_knowledge::build_work_units(&graph);
        let _ = crate::spec_knowledge::build_requirement_plan(&dir, &dir.join("specs"));

        for (path, content) in snapshot {
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                content,
                "compiler reads must not mutate {}",
                path.display()
            );
        }
        fs::remove_dir_all(dir).ok();
    }
}
