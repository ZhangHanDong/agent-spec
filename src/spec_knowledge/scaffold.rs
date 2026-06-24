//! `init --workspace` scaffold (§11). Idempotent: create-if-missing only.

use std::io;
use std::path::Path;

/// Files created by the workspace scaffold, relative to root.
const FILES: &[(&str, &str)] = &[
    ("knowledge/decisions/README.md", DECISIONS_README),
    ("knowledge/decisions/adr-template.md", ADR_TEMPLATE),
    ("knowledge/guidance/README.md", GUIDANCE_README),
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
const GUIDANCE_README: &str = "# Guidance\n\n[P2] Agent-facing guidance + skill designation (typed, governance tier). Empty in P1.\n";
const CONTEXT_README: &str = "# Context (free-form)\n\nEscape hatch: arbitrary agent-context. Served read-only, NOT linted, no schema.\n";
const ARTIFACT_TYPES: &str = "# Artifact types (canon)\n\nDecision (P1): required `## Context · ## Decision · ## Consequences`; recommended `## Status · ## Category · ## Alternatives Considered`; `## Supersedes`.\nThis canon documents the schema the lint enforces. It is exempt from artifact lint.\n";
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
