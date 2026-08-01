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
const ADR_TEMPLATE: &str = "---\nkind: decision\nid: ADR-NNN\nstatus: Proposed\n---\n\n## Context\n\n## Decision\n\n## Consequences\n\nGood, because …\nBad, because …\n\n## Alternatives Considered\n";
const REQUIREMENTS_README: &str = "# Requirements\n\nEARS/29148-style requirement records. Use one artifact per stable requirement or grouping requirement.\n\nRequired shape:\n- `title:` is the canonical human-readable title used by graph, work-unit, and spec draft generation.\n- `## Problem` explains the user or system problem.\n- `## Requirements` is the normative source, with one `[REQ-NNN] ... MUST/SHOULD/MAY ...` clause per line.\n- `## Scenarios` supplies the work-unit and draft-spec BDD source.\n- `## Dependencies` declares ordering edges to other requirement ids.\n- `## Open Questions` blocks executable work-unit generation when it contains real questions.\n\nSpecs link back via `satisfies:`.\n";
const REQ_TEMPLATE: &str = "---\nkind: requirement\nid: REQ-NNN\ntitle: \"Requirement Title\"\nliveness: auto\ntags: []\n---\n\n## Problem\n\nDescribe the user or system problem this requirement solves.\n\n## Requirements\n\n[REQ-NNN] The system MUST produce an observable response.\n\n## Scenarios\n\nScenario: Main behavior\n  Given a concrete starting state\n  When a concrete action occurs\n  Then a concrete observable outcome occurs\n\n## Dependencies\n\nNone.\n\n## Source Trace\n\n- issue:#NNN\n\n## Open Questions\n\nNone.\n";
const PROPOSALS_README: &str = "# Proposals\n\nGovernance proposals (LEP-style). `liveness: n/a` — never enters the code gate.\nLink the decisions a proposal spawns with `## Produces: ADR-NNN`.\n";
const LEP_TEMPLATE: &str = include_str!("../../knowledge/proposals/proposal-template.md");
const GUIDANCE_README: &str = "# Guidance\n\nAgent-facing guidance + skill designation. `liveness: n/a`. Projected into\nCLAUDE.md/AGENTS.md via `gen-integrations --with-guidance` and served live via\nMCP `guidance.for`.\n";
const GUIDANCE_TEMPLATE: &str = "---\nkind: guidance\nid: G-NNN\nliveness: n/a\ntags: []\n---\n\n## Scope\n\n## Instructions\n\n## Applies To\n\n## Skills\n";
const CONTEXT_README: &str = "# Context (free-form)\n\nEscape hatch: arbitrary agent-context. Served read-only, NOT linted, no schema.\n";
const ARTIFACT_TYPES: &str = "# Artifact types (canon)\n\n- decision — `## Context · ## Decision · ## Consequences`; recommended `## Alternatives Considered`; `supersedes:`.\n- requirement — `## Problem · ## Requirements` ([REQ-NNN] MUST/SHOULD/MAY); BCP-14/29148/EARS quality lint.\n- guidance — `## Scope · ## Instructions`; `## Applies To · ## Skills`; `liveness: n/a`.\n- proposal — MADR shape; `liveness: n/a`; `## Produces:` edge to decisions/requirements.\n- context — free-form, untyped, unlinted (escape hatch).\n\nThis canon documents the schema the lint enforces. It is exempt from artifact lint.\n";
const CONFIG_YAML: &str = "paths:\n  knowledge: knowledge\n  specs: specs\nliveness:\n  gate:\n    violated: error\n    unproven: warning\n";

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
