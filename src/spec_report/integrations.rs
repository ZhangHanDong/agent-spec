//! Single-source multi-tool integration generation (Phase 6).
//!
//! One canonical [`integration_body`] is rendered into per-tool formats so the
//! agent instructions never drift across `AGENTS.md`, `.cursorrules`, and the
//! Claude skill.

/// The canonical tool-first integration instructions — the single source.
pub fn integration_body() -> String {
    "\
agent-spec is an AI-native BDD/spec verification tool. Use it tool-first:\n\
\n\
1. Read the Task Contract: `agent-spec contract <spec>`.\n\
2. Generate plan context: `agent-spec plan <spec> --code . --format prompt`.\n\
3. Implement within the contract's Boundaries.\n\
4. Verify: `agent-spec lifecycle <spec> --code . --format json` — fix until all\n\
   scenarios pass (failed/skipped/uncertain all 0). Do not edit the spec to pass.\n\
5. Repo-level gate before committing: `agent-spec guard --spec-dir specs --code .`.\n\
6. Render the PR acceptance summary: `agent-spec explain <spec> --code . --format markdown`.\n\
\n\
The machine verifies whether the code satisfies the contract; you implement\n\
against it, and a human reviews the contract."
        .to_string()
}

/// Integration output targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationTarget {
    Agents,
    Cursor,
    Claude,
}

impl IntegrationTarget {
    /// Parse a target name. `Err` for unknown targets (never panics).
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "agents" => Ok(Self::Agents),
            "cursor" => Ok(Self::Cursor),
            "claude" => Ok(Self::Claude),
            other => Err(format!(
                "unknown integration target `{other}` (expected agents/cursor/claude)"
            )),
        }
    }

    /// Conventional output filename for this target.
    pub fn filename(self) -> &'static str {
        match self {
            Self::Agents => "AGENTS.md",
            Self::Cursor => ".cursorrules",
            Self::Claude => "agent-spec-tool-first.md",
        }
    }
}

/// Render the canonical body into the given target's format.
pub fn render_target(target: IntegrationTarget, body: &str) -> String {
    match target {
        IntegrationTarget::Agents => {
            format!("# agent-spec — Agent Instructions\n\n{body}\n")
        }
        IntegrationTarget::Cursor => {
            format!("# agent-spec rules (Cursor)\n\n{body}\n")
        }
        IntegrationTarget::Claude => {
            format!(
                "---\nname: agent-spec-tool-first\ndescription: Use agent-spec as a CLI tool to verify code against Task Contracts.\n---\n\n{body}\n"
            )
        }
    }
}

/// Render a target by name (single entry point used by the CLI).
pub fn render_named(target: &str) -> Result<String, String> {
    let t = IntegrationTarget::parse(target)?;
    Ok(render_target(t, &integration_body()))
}

/// Whether existing content has drifted from the freshly-rendered output.
pub fn has_drifted(existing: &str, rendered: &str) -> bool {
    existing.trim() != rendered.trim()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_all_targets_share_integration_body() {
        let body = integration_body();
        for t in [
            IntegrationTarget::Agents,
            IntegrationTarget::Cursor,
            IntegrationTarget::Claude,
        ] {
            let out = render_target(t, &body);
            assert!(
                out.contains("lifecycle"),
                "{t:?} must carry the shared body"
            );
            assert!(out.contains("guard"), "{t:?} must carry the shared body");
        }
    }

    #[test]
    fn test_integration_body_is_tool_first() {
        let b = integration_body();
        assert!(b.contains("contract"));
        assert!(b.contains("lifecycle"));
        assert!(b.contains("guard"));
    }

    #[test]
    fn test_claude_target_has_frontmatter() {
        let out = render_target(IntegrationTarget::Claude, &integration_body());
        assert!(
            out.starts_with("---"),
            "claude target must start with frontmatter"
        );
        assert!(out.contains("name:"));
    }

    #[test]
    fn test_agents_target_is_plain_markdown() {
        let out = render_target(IntegrationTarget::Agents, &integration_body());
        assert!(
            !out.starts_with("---"),
            "agents target must not be frontmatter"
        );
        assert!(out.contains("# "));
    }

    #[test]
    fn test_unknown_target_errors() {
        assert!(IntegrationTarget::parse("vim").is_err());
        assert!(render_named("vim").is_err());
    }

    #[test]
    fn test_check_passes_when_content_matches() {
        let rendered = render_named("agents").unwrap();
        assert!(
            !has_drifted(&rendered, &rendered),
            "identical content must not drift"
        );
    }

    #[test]
    fn test_check_reports_drift_when_different() {
        let rendered = render_named("agents").unwrap();
        assert!(has_drifted("stale hand-edited content", &rendered));
    }
}
