//! `init --workspace` scaffold (§11). Idempotent: create-if-missing only.

use std::io;
use std::path::Path;

/// Files created by the workspace scaffold, relative to root.
const FILES: &[(&str, &str)] = &[
    ("knowledge/decisions/README.md", DECISIONS_README),
    ("knowledge/decisions/adr-template.md", ADR_TEMPLATE),
    ("knowledge/requirements/README.md", REQUIREMENTS_README),
    ("knowledge/requirements/req-template.md", REQ_TEMPLATE),
    ("knowledge/proposals/README.md", PROPOSALS_README),
    ("knowledge/proposals/lep-template.md", LEP_TEMPLATE),
    ("knowledge/guidance/README.md", GUIDANCE_README),
    ("knowledge/guidance/guidance-template.md", GUIDANCE_TEMPLATE),
    ("knowledge/context/README.md", CONTEXT_README),
    (
        "knowledge/standards/canon/artifact-types.md",
        ARTIFACT_TYPES,
    ),
    (".agent-spec/config.yaml", CONFIG_YAML),
];

/// Create the canonical workspace tree under `root`. Returns the list of
/// paths actually created (skips existing files).
pub fn scaffold_workspace(root: &Path) -> io::Result<Vec<String>> {
    let mut created = Vec::new();
    for (rel, contents) in FILES {
        let path = root.join(rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
        created.push((*rel).to_string());
    }
    Ok(created)
}

const DECISIONS_README: &str = "# Decisions\n\nMADR-style decision records. One decision per file, `NNNNN-slug.md`.\nWhen NOT to use: routine implementation choices with no real trade-off — leave those in code/comments.\n";
const ADR_TEMPLATE: &str = "---\nkind: decision\nid: ADR-NNN\nstatus: Proposed\n---\n\n## Context\n\n## Decision\n\n## Consequences\n\nGood, because …\nBad, because …\n\n## Alternatives Considered\n\n## Next\n\nSingle exit: govern this decision with a requirement document that task\ncontracts can satisfy.\n";
const REQUIREMENTS_README: &str = "# Requirements\n\nEARS/29148-style requirement records. Use one artifact per stable requirement or grouping requirement.\n\nRequired shape:\n- `title:` is the canonical human-readable title used by graph, work-unit, and spec draft generation.\n- `## Problem` explains the user or system problem.\n- `## Requirements` is the normative source, with one `[REQ-NNN] ... MUST/SHOULD/MAY ...` clause per line.\n- `## Scenarios` supplies the work-unit and draft-spec BDD source.\n- `## Dependencies` declares ordering edges to other requirement ids.\n- `## Open Questions` blocks executable work-unit generation when it contains real questions.\n\nSpecs link back via `satisfies:`.\n";
const REQ_TEMPLATE: &str = "---\nkind: requirement\nid: REQ-NNN\ntitle: \"Requirement Title\"\nliveness: auto\ntags: []\n---\n\n## Problem\n\nDescribe the user or system problem this requirement solves.\n\n## Requirements\n\n[REQ-NNN] The system MUST produce an observable response.\n\n## Scenarios\n\nScenario: Main behavior\n  Given a concrete starting state\n  When a concrete action occurs\n  Then a concrete observable outcome occurs\n\n## Dependencies\n\nNone.\n\n## Source Trace\n\n- issue:#NNN\n\n## Open Questions\n\nNone.\n\n## Next\n\nSingle exit: compile this requirement into a task contract with\n`agent-spec requirements draft-specs`.\n";
const PROPOSALS_README: &str = "# Proposals\n\nGovernance proposals (LEP-style). `liveness: n/a` — never enters the code gate.\nLink the decisions a proposal spawns with `## Produces: ADR-NNN`.\n";
const LEP_TEMPLATE: &str = include_str!("../../knowledge/proposals/proposal-template.md");
const GUIDANCE_README: &str = "# Guidance\n\nAgent-facing guidance + skill designation. `liveness: n/a`. Projected into\nCLAUDE.md/AGENTS.md via `gen-integrations --with-guidance` and served live via\nMCP `guidance.for`.\n";
const GUIDANCE_TEMPLATE: &str = "---\nkind: guidance\nid: G-NNN\nliveness: n/a\ntags: []\n---\n\n## Scope\n\n## Instructions\n\n## Applies To\n\n## Skills\n";
const CONTEXT_README: &str = "# Context (free-form)\n\nEscape hatch: arbitrary agent-context. Served read-only, NOT linted, no schema.\n";
const ARTIFACT_TYPES: &str = "# Artifact types (canon)\n\n- decision — `## Context · ## Decision · ## Consequences`; recommended `## Alternatives Considered`; `supersedes:`.\n- requirement — `## Problem · ## Requirements` ([REQ-NNN] MUST/SHOULD/MAY); BCP-14/29148/EARS quality lint.\n- guidance — `## Scope · ## Instructions`; `## Applies To · ## Skills`; `liveness: n/a`.\n- proposal — MADR shape; `liveness: n/a`; `## Produces:` edge to decisions/requirements.\n- context — free-form, untyped, unlinted (escape hatch).\n\nThis canon documents the schema the lint enforces. It is exempt from artifact lint.\n";
const CONFIG_YAML: &str = "paths:\n  knowledge: knowledge\n  specs: specs\nliveness:\n  gate:\n    violated: error\n    unproven: warning\n";

/// Kinds `knowledge new` can scaffold. `context` is free-form (no template)
/// and `guidance` is deferred; the forward-walking pipeline needs these three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeNewKind {
    Proposal,
    Decision,
    Requirement,
}

impl KnowledgeNewKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "proposal" => Some(Self::Proposal),
            "decision" => Some(Self::Decision),
            "requirement" => Some(Self::Requirement),
            _ => None,
        }
    }

    /// Id prefix per the registry (knowledge/standards/operational/id-registry.md).
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Proposal => "LEP-",
            Self::Decision => "ADR-",
            Self::Requirement => "REQ-",
        }
    }

    pub fn dir(self) -> &'static str {
        match self {
            Self::Proposal => "proposals",
            Self::Decision => "decisions",
            Self::Requirement => "requirements",
        }
    }

    fn template(self) -> &'static str {
        match self {
            Self::Proposal => LEP_TEMPLATE,
            Self::Decision => ADR_TEMPLATE,
            Self::Requirement => REQ_TEMPLATE,
        }
    }

    fn placeholder_id(self) -> &'static str {
        match self {
            Self::Proposal => "LEP-NNN",
            Self::Decision => "ADR-NNN",
            Self::Requirement => "REQ-NNN",
        }
    }
}

/// Scaffold one lint-clean knowledge artifact. `date` is the caller-supplied
/// `YYYY-MM-DD` used for proposal filenames (injected so tests stay
/// deterministic). Never overwrites: an existing target is an error.
pub fn knowledge_new(
    root: &Path,
    kind: KnowledgeNewKind,
    id: &str,
    title: Option<&str>,
    date: &str,
) -> Result<std::path::PathBuf, String> {
    if !root.is_dir() {
        return Err(format!(
            "knowledge root {} does not exist; run `agent-spec init --workspace` first",
            root.display()
        ));
    }
    let prefix = kind.prefix();
    let rest = id.strip_prefix(prefix).unwrap_or_default();
    if rest.is_empty()
        || !rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(format!(
            "id `{id}` does not match the registry prefix for this kind; valid prefix: {prefix} (see knowledge/standards/operational/id-registry.md)"
        ));
    }

    let lower_id = id.to_ascii_lowercase();
    let slug = title.map(slugify).filter(|s| !s.is_empty());
    let file_name = match (kind, &slug) {
        (KnowledgeNewKind::Proposal, Some(s)) => format!("{date}-{s}.md"),
        (KnowledgeNewKind::Proposal, None) => format!("{date}-{lower_id}.md"),
        (_, Some(s)) => format!("{lower_id}-{s}.md"),
        (_, None) => format!("{lower_id}.md"),
    };
    let dir = root.join(kind.dir());
    let path = dir.join(file_name);
    if path.exists() {
        return Err(format!("refusing to overwrite existing {}", path.display()));
    }

    let mut contents = kind.template().replace(kind.placeholder_id(), id);
    if kind == KnowledgeNewKind::Requirement {
        // The requirement template carries no governance status; a fresh
        // artifact enters the pipeline as proposed.
        contents = contents.replacen("\nliveness: auto\n", "\nstatus: proposed\nliveness: auto\n", 1);
    }
    if let Some(t) = title {
        for placeholder in ["Proposal Title", "Requirement Title"] {
            contents = contents.replace(placeholder, t);
        }
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    std::fs::write(&path, &contents).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Today's civil date as `YYYY-MM-DD` (UTC), via Hinnant's civil-from-days.
pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_scaffold_is_idempotent() {
        let root = std::env::temp_dir().join(format!("kll-scaffold-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();

        let first = scaffold_workspace(&root).unwrap();
        assert!(
            first
                .iter()
                .any(|p| p == "knowledge/decisions/adr-template.md")
        );
        assert!(root.join(".agent-spec/config.yaml").exists());
        let req_template =
            std::fs::read_to_string(root.join("knowledge/requirements/req-template.md")).unwrap();
        assert!(req_template.contains("title: \"Requirement Title\""));
        assert!(req_template.contains("## Scenarios"));
        assert!(req_template.contains("## Open Questions"));

        // Second run creates nothing.
        let second = scaffold_workspace(&root).unwrap();
        assert!(second.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    use crate::spec_core::Severity;
    use crate::spec_knowledge::governance::lint_doc;
    use crate::spec_knowledge::parser::parse_knowledge_str;
    use std::path::Path;

    fn repo_file(rel: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
    }

    fn instantiation_errors(contents: &str, name: &str) -> Vec<String> {
        let doc = parse_knowledge_str(contents, Path::new(name))
            .unwrap_or_else(|e| panic!("{name}: template instantiation must parse: {e}"));
        lint_doc(&doc)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| format!("{}: {}", d.rule, d.message))
            .collect()
    }

    #[test]
    fn test_repo_templates_match_scaffold() {
        assert_eq!(
            repo_file("knowledge/proposals/proposal-template.md"),
            LEP_TEMPLATE,
            "knowledge/proposals/proposal-template.md must match scaffold LEP_TEMPLATE byte for byte"
        );
    }

    #[test]
    fn test_id_registry_doc_lists_all_prefixes() {
        let registry = repo_file("knowledge/standards/operational/id-registry.md");
        for (prefix, dir) in [
            ("`LEP-`", "knowledge/proposals/"),
            ("`ADR-`", "knowledge/decisions/"),
            ("`REQ-`", "knowledge/requirements/"),
            ("`task-`", "specs/"),
        ] {
            assert!(
                registry.contains(prefix) && registry.contains(dir),
                "registry must map {prefix} to {dir}"
            );
        }
        assert!(registry.contains("YYYY-MM-DD"), "filename rule missing");
        assert!(registry.contains("frontmatter"), "id-in-frontmatter rule missing");
    }

    #[test]
    fn test_proposal_template_instantiation_lints_clean() {
        let errors = instantiation_errors(&LEP_TEMPLATE.replace("LEP-NNN", "LEP-999"), "lep-999.md");
        assert!(errors.is_empty(), "proposal template must lint clean, got {errors:?}");
    }

    #[test]
    fn test_scaffold_templates_instantiate_clean() {
        for (contents, name) in [
            (ADR_TEMPLATE.replace("ADR-NNN", "ADR-999"), "adr-999.md"),
            (REQ_TEMPLATE.replace("REQ-NNN", "REQ-999"), "req-999.md"),
            (GUIDANCE_TEMPLATE.replace("G-NNN", "G-999"), "g-999.md"),
        ] {
            let errors = instantiation_errors(&contents, name);
            assert!(errors.is_empty(), "{name} must lint clean, got {errors:?}");
        }
    }

    fn new_root(prefix: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("kll-knew-{prefix}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("knowledge/proposals")).unwrap();
        std::fs::create_dir_all(root.join("knowledge/decisions")).unwrap();
        std::fs::create_dir_all(root.join("knowledge/requirements")).unwrap();
        root
    }

    #[test]
    fn test_knowledge_new_proposal_lints_clean() {
        let root = new_root("prop");
        let path = knowledge_new(
            &root.join("knowledge"),
            KnowledgeNewKind::Proposal,
            "LEP-002",
            None,
            "2026-08-01",
        )
        .unwrap();
        assert!(path.starts_with(root.join("knowledge/proposals")));
        let errors = instantiation_errors(&std::fs::read_to_string(&path).unwrap(), "lep-002.md");
        assert!(errors.is_empty(), "scaffolded proposal must lint clean: {errors:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_knowledge_new_requirement_has_single_exit() {
        let root = new_root("req");
        let path = knowledge_new(
            &root.join("knowledge"),
            KnowledgeNewKind::Requirement,
            "REQ-X",
            None,
            "2026-08-01",
        )
        .unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let next = contents.split("## Next").nth(1).expect("skeleton ends with a ## Next exit");
        assert!(
            next.contains("requirements draft-specs"),
            "requirement exit points at draft-specs"
        );
        assert!(
            !contents.contains("## Produces"),
            "a requirement skeleton has no second exit"
        );
        assert!(contents.contains("status: proposed"), "governance status prefilled");
        let errors = instantiation_errors(&contents, "req-x.md");
        assert!(errors.is_empty(), "scaffolded requirement must lint clean: {errors:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_knowledge_new_rejects_prefix_mismatch() {
        let root = new_root("prefix");
        let err = knowledge_new(
            &root.join("knowledge"),
            KnowledgeNewKind::Decision,
            "LEP-9",
            None,
            "2026-08-01",
        )
        .unwrap_err();
        assert!(err.contains("ADR-"), "error lists the valid prefix for the kind: {err}");
        assert!(err.contains("LEP-9"), "error names the offending id: {err}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_knowledge_new_refuses_to_clobber() {
        let root = new_root("clobber");
        let knowledge = root.join("knowledge");
        let first = knowledge_new(&knowledge, KnowledgeNewKind::Decision, "ADR-9", None, "2026-08-01")
            .unwrap();
        let before = std::fs::read_to_string(&first).unwrap();
        let err = knowledge_new(&knowledge, KnowledgeNewKind::Decision, "ADR-9", None, "2026-08-01")
            .unwrap_err();
        assert!(err.contains(&first.display().to_string()), "error names the existing path: {err}");
        assert_eq!(std::fs::read_to_string(&first).unwrap(), before, "file is untouched");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_knowledge_new_without_workspace_names_init() {
        let root = std::env::temp_dir().join(format!("kll-knew-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let err = knowledge_new(
            &root.join("knowledge"),
            KnowledgeNewKind::Proposal,
            "LEP-002",
            None,
            "2026-08-01",
        )
        .unwrap_err();
        assert!(err.contains("init --workspace"), "error names the recovery command: {err}");
    }

    #[test]
    fn test_skill_routing_tables_match_registry() {
        let registry = repo_file("knowledge/standards/operational/id-registry.md");
        for skill in [
            "skills/agent-spec-authoring/SKILL.md",
            "skills/agent-spec-intent-compiler/SKILL.md",
        ] {
            let text = repo_file(skill);
            for (dir, prefix) in [
                ("`knowledge/proposals/`", "`LEP-`"),
                ("`knowledge/decisions/`", "`ADR-`"),
                ("`knowledge/requirements/`", "`REQ-`"),
                ("`specs/`", "`task-`"),
            ] {
                assert!(
                    registry.contains(dir) && registry.contains(prefix),
                    "registry must hold {dir} {prefix}"
                );
                assert!(
                    text.contains(dir) && text.contains(prefix),
                    "{skill} routing table must hold {dir} {prefix}"
                );
            }
        }
    }

    #[test]
    fn test_no_prop_prefix_remains() {
        let sources = [
            (
                "knowledge/proposals/proposal-template.md",
                repo_file("knowledge/proposals/proposal-template.md"),
            ),
            ("scaffold LEP_TEMPLATE", LEP_TEMPLATE.to_string()),
            ("scaffold PROPOSALS_README", PROPOSALS_README.to_string()),
            ("scaffold ARTIFACT_TYPES", ARTIFACT_TYPES.to_string()),
        ];
        for (name, contents) in sources {
            assert!(
                !contents.contains("PROP-"),
                "{name} still carries the retired PROP- prefix (ADR-002 ratified LEP-NNN)"
            );
        }
    }
}
