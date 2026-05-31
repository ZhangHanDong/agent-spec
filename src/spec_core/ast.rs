use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Spec hierarchy level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpecLevel {
    Org,
    Project,
    /// Capability scope: a long-lived living-spec holding Rules proven by tasks (Phase 3).
    Capability,
    Task,
}

/// Language used in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Zh,
    En,
}

/// Front-matter metadata of a spec file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecMeta {
    pub level: SpecLevel,
    pub name: String,
    pub inherits: Option<String>,
    pub lang: Vec<Lang>,
    pub tags: Vec<String>,
    /// Spec-level dependencies: names of other specs this spec depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    /// Estimated effort (e.g., "0.5d", "2d", "1w").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// Capability this task contributes to (Phase 3). Additive; `None` for
    /// specs that declare no `capability:`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
}

/// A parsed .spec document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDocument {
    pub meta: SpecMeta,
    pub sections: Vec<Section>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

/// A top-level section in the spec body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Section {
    Intent {
        content: String,
        span: Span,
    },
    Constraints {
        items: Vec<Constraint>,
        span: Span,
    },
    Decisions {
        items: Vec<String>,
        span: Span,
    },
    Boundaries {
        items: Vec<Boundary>,
        span: Span,
    },
    AcceptanceCriteria {
        scenarios: Vec<Scenario>,
        /// BDD behavior rules grouping scenarios under this section.
        /// Additive (Phase 1 BDD semantics); empty for specs without `Rule:` lines.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rules: Vec<BehaviorRule>,
        /// `Rule:` lines whose id was not a valid kebab-case identifier.
        /// Retained (not promoted to a [`BehaviorRule`]) so the `bdd-rule-id`
        /// lint can flag them. Additive.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        malformed_rules: Vec<MalformedRule>,
        span: Span,
    },
    OutOfScope {
        items: Vec<String>,
        span: Span,
    },
}

/// A single constraint line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub text: String,
    pub category: ConstraintCategory,
    pub span: Span,
}

/// Constraint categories matching the DSL sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintCategory {
    Must,
    MustNot,
    Decided,
    General,
}

/// A task boundary item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boundary {
    pub text: String,
    pub category: BoundaryCategory,
    pub span: Span,
}

/// Boundary categories for task contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryCategory {
    Allow,
    Deny,
    General,
}

/// Scenario execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioMode {
    #[default]
    Standard,
    Optimize,
}

impl ScenarioMode {
    pub fn is_standard(&self) -> bool {
        *self == Self::Standard
    }
}

/// Review mode for a scenario: whether it needs human review after passing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewMode {
    Auto,
    Human,
}

impl Default for ReviewMode {
    fn default() -> Self {
        Self::Auto
    }
}

impl ReviewMode {
    pub fn is_auto(&self) -> bool {
        *self == Self::Auto
    }
}

/// Scope of a BDD behavior rule. The keystone of the BDD-spine roadmap:
/// the same Rule primitive lifts across task/capability/project scope by
/// changing only this field, never the stable `id`.
///
/// Phase 1 only produces `Task`; `Capability` and `Project` are reserved
/// variants written into the AST but not parsed/loaded/promoted yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleScope {
    /// Task scope, namespaced by the spec file stem (not the human display name).
    Task(String),
    /// Capability scope (reserved; Phase 3).
    Capability(String),
    /// Project scope (reserved; Phase 3).
    Project,
}

/// Stable identity of a behavior rule: `{ scope, id }`.
/// `id` is the stable kebab-case identifier; the human display text lives in
/// `BehaviorRule.name` and may change freely without breaking references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleKey {
    pub scope: RuleScope,
    pub id: String,
}

/// Lifecycle event in a behavior rule's provenance log (Phase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleEventKind {
    Created,
    Promoted,
    Affirmed,
    Deprecated,
}

/// A single provenance event on a behavior rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEvent {
    pub kind: RuleEventKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A BDD behavior rule: a promise the system should keep, proven by one or
/// more scenarios (examples). Formulation-layer primitive (Phase 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorRule {
    pub key: RuleKey,
    /// Human-readable display text. Defaults to `key.id` when no display name given.
    pub name: String,
    /// Names of scenarios grouped under this rule, in document order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenario_names: Vec<String>,
    /// Provenance event log (Phase 3). Additive; empty for freshly-parsed rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<RuleEvent>,
    pub span: Span,
}

/// A `Rule:` header whose leading token was not a valid kebab-case id, so it
/// was not promoted to a [`BehaviorRule`]. Carries the raw content for the
/// `bdd-rule-id` lint to report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MalformedRule {
    pub raw: String,
    pub span: Span,
}

/// A BDD scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_selector: Option<TestSelector>,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "ReviewMode::is_auto")]
    pub review: ReviewMode,
    #[serde(default, skip_serializing_if = "ScenarioMode::is_standard")]
    pub mode: ScenarioMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// `id` of the owning [`BehaviorRule`], if this scenario is grouped under one.
    /// Additive (Phase 1 BDD semantics); `None` for ungrouped/legacy scenarios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    pub span: Span,
}

impl Scenario {
    /// Returns `true` if this scenario is marked as critical — either via a
    /// `critical` tag or a `(critical)` / `（critical）` name suffix (case-insensitive).
    pub fn is_critical(&self) -> bool {
        let has_tag = self
            .tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case("critical"));
        if has_tag {
            return true;
        }
        let lower = self.name.to_lowercase();
        lower.ends_with("(critical)") || lower.ends_with("（critical）")
    }

    /// Returns the scenario name with any trailing `(critical)` / `（critical）`
    /// suffix stripped, suitable for display purposes.
    pub fn display_name(&self) -> &str {
        let name = self.name.trim_end();
        // Try ASCII parentheses first
        if let Some(idx) = name.rfind('(') {
            let suffix = &name[idx..];
            if suffix.to_lowercase() == "(critical)" {
                return name[..idx].trim_end();
            }
        }
        // Try fullwidth parentheses
        if let Some(idx) = name.rfind('（') {
            let suffix = &name[idx..];
            if suffix.to_lowercase() == "（critical）" {
                return name[..idx].trim_end();
            }
        }
        name
    }
}

/// Structured test selector for binding a scenario to test execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSelector {
    pub filter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_double: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<String>,
}

impl TestSelector {
    pub fn filter_only(filter: impl Into<String>) -> Self {
        Self {
            filter: filter.into(),
            package: None,
            level: None,
            test_double: None,
            targets: None,
        }
    }

    pub fn label(&self) -> String {
        match &self.package {
            Some(package) => format!("{package}::{}", self.filter),
            None => self.filter.clone(),
        }
    }
}

/// BDD step keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepKind {
    Given,
    When,
    Then,
    And,
    But,
}

/// A single Given/When/Then step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    pub text: String,
    pub params: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub table: Vec<Vec<String>>,
    pub span: Span,
}

/// Source location span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl Span {
    pub fn new(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Self {
        Self {
            start_line,
            end_line,
            start_col,
            end_col,
        }
    }

    pub fn line(line: usize) -> Self {
        Self {
            start_line: line,
            end_line: line,
            start_col: 0,
            end_col: 0,
        }
    }
}

/// A resolved spec with inherited constraints merged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedSpec {
    pub task: SpecDocument,
    pub inherited_constraints: Vec<Constraint>,
    pub inherited_decisions: Vec<String>,
    pub all_scenarios: Vec<Scenario>,
}
