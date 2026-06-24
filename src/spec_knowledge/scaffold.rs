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
const REQUIREMENTS_README: &str = "# Requirements\n\nEARS/29148-style requirement records. `## Problem` + `## Requirements`, one\n`[REQ-NNN] … MUST/SHOULD/MAY …` clause per line. Specs link back via `satisfies:`.\n";
const REQ_TEMPLATE: &str = "---\nkind: requirement\nid: REQ-NNN\n---\n\n## Problem\n\n## Requirements\n\n[REQ-NNN] The <system> MUST <response>.\n\n## Success Metrics\n";
const PROPOSALS_README: &str = "# Proposals\n\nGovernance proposals (LEP-style). `liveness: n/a` — never enters the code gate.\nLink the decisions a proposal spawns with `## Produces: ADR-NNN`.\n";
const LEP_TEMPLATE: &str = "---\nkind: proposal\nid: LEP-NNN\nstatus: Proposed\nliveness: n/a\n---\n\n## Context\n\n## Decision\n\n## Consequences\n\nGood, because …\nBad, because …\n\n## Produces: ADR-NNN\n";
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

        // Second run creates nothing.
        let second = scaffold_workspace(&root).unwrap();
        assert!(second.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }
}
