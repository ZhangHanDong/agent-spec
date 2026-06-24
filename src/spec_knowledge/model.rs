//! Knowledge-layer data model (KLL P1): decisions, liveness states.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnowledgeKind {
    Decision,
    Requirement,
    Guidance,
    Proposal,
}

impl KnowledgeKind {
    /// Parse a frontmatter `kind:` value (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "decision" => Some(KnowledgeKind::Decision),
            "requirement" => Some(KnowledgeKind::Requirement),
            "guidance" => Some(KnowledgeKind::Guidance),
            "proposal" => Some(KnowledgeKind::Proposal),
            _ => None,
        }
    }

    /// The conventional subdirectory under `knowledge/` for this kind.
    pub fn dir(self) -> &'static str {
        match self {
            KnowledgeKind::Decision => "decisions",
            KnowledgeKind::Requirement => "requirements",
            KnowledgeKind::Guidance => "guidance",
            KnowledgeKind::Proposal => "proposals",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Proposed,
    Accepted,
    Superseded,
    Deprecated,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LivenessDeclared {
    #[default]
    Auto,
    Na,
}

/// Derived liveness state (never stored; §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Liveness {
    Honored,
    Violated,
    Unproven,
    Na,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeMeta {
    pub kind: KnowledgeKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DecisionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub liveness: LivenessDeclared,
}

/// One `## Heading` block and its raw body text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KSection {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeDoc {
    pub meta: KnowledgeMeta,
    pub sections: Vec<KSection>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

/// Back-compat alias: a decision is just a knowledge doc with `kind: decision`.
pub type DecisionDoc = KnowledgeDoc;

impl KnowledgeDoc {
    /// Find a section by case-insensitive heading match.
    pub fn section(&self, heading: &str) -> Option<&KSection> {
        self.sections
            .iter()
            .find(|s| s.heading.eq_ignore_ascii_case(heading))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_section_lookup_is_case_insensitive() {
        let doc = DecisionDoc {
            meta: KnowledgeMeta {
                kind: KnowledgeKind::Decision,
                id: "ADR-001".into(),
                status: Some(DecisionStatus::Accepted),
                supersedes: None,
                liveness: LivenessDeclared::Auto,
            },
            sections: vec![KSection {
                heading: "Context".into(),
                body: "x".into(),
            }],
            source_path: PathBuf::new(),
        };
        assert!(doc.section("context").is_some());
        assert!(doc.section("Decision").is_none());
    }
}
