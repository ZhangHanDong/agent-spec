//! Knowledge-layer data model (KLL P1): decisions, liveness states.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnowledgeKind {
    Decision,
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
pub struct DecisionDoc {
    pub meta: KnowledgeMeta,
    pub sections: Vec<KSection>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

impl DecisionDoc {
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
