use serde::{Deserialize, Serialize};
use crate::spec_core::{
    BoundaryCategory, Constraint, ConstraintCategory, ResolvedSpec, Scenario, Section, SpecDocument,
};

/// Primary task contract projection for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContract {
    pub name: String,
    pub intent: String,
    pub must: Vec<String>,
    pub must_not: Vec<String>,
    pub decisions: Vec<String>,
    pub allowed_changes: Vec<String>,
    pub forbidden: Vec<String>,
    pub out_of_scope: Vec<String>,
    pub completion_criteria: Vec<Scenario>,
}

/// Legacy compatibility summary for older brief-based integrations.
///
/// Prefer [`TaskContract`] for new integrations. `SpecBrief` remains available so
/// older callers can keep working while the contract model becomes the only
/// first-class planning surface.
#[deprecated(note = "Use TaskContract and SpecGateway::plan()/contract() instead")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecBrief {
    pub name: String,
    pub intent: String,
    pub must: Vec<String>,
    pub must_not: Vec<String>,
    pub decided: Vec<String>,
    pub scenario_names: Vec<String>,
    pub out_of_scope: Vec<String>,
}

#[allow(deprecated)]
impl SpecBrief {
    /// Build a brief from a parsed spec document.
    pub fn from_doc(doc: &SpecDocument) -> Self {
        let mut intent = String::new();
        let mut must = Vec::new();
        let mut must_not = Vec::new();
        let mut decided = Vec::new();
        let mut scenario_names = Vec::new();
        let mut out_of_scope = Vec::new();

        for section in &doc.sections {
            match section {
                Section::Intent { content, .. } => {
                    intent = content.clone();
                }
                Section::Constraints { items, .. } => {
                    for c in items {
                        match c.category {
                            ConstraintCategory::Must => must.push(c.text.clone()),
                            ConstraintCategory::MustNot => must_not.push(c.text.clone()),
                            ConstraintCategory::Decided => decided.push(c.text.clone()),
                            ConstraintCategory::General => must.push(c.text.clone()),
                        }
                    }
                }
                Section::Decisions { items, .. } => {
                    for item in items {
                        decided.push(item.clone());
                    }
                }
                Section::Boundaries { items, .. } => {
                    for item in items {
                        match item.category {
                            BoundaryCategory::Allow => must.push(item.text.clone()),
                            BoundaryCategory::Deny | BoundaryCategory::General => {
                                must_not.push(item.text.clone())
                            }
                        }
                    }
                }
                Section::AcceptanceCriteria { scenarios, .. } => {
                    for s in scenarios {
                        scenario_names.push(s.name.clone());
                    }
                }
                Section::OutOfScope { items, .. } => {
                    out_of_scope.clone_from(items);
                }
            }
        }

        Self {
            name: doc.meta.name.clone(),
            intent,
            must,
            must_not,
            decided,
            scenario_names,
            out_of_scope,
        }
    }

    /// Build a brief from a fully resolved spec, including inherited constraints.
    pub fn from_resolved(resolved: &ResolvedSpec) -> Self {
        let mut brief = Self {
            name: resolved.task.meta.name.clone(),
            intent: String::new(),
            must: Vec::new(),
            must_not: Vec::new(),
            decided: Vec::new(),
            scenario_names: resolved
                .all_scenarios
                .iter()
                .map(|scenario| scenario.name.clone())
                .collect(),
            out_of_scope: Vec::new(),
        };

        for constraint in &resolved.inherited_constraints {
            push_constraint_into_brief(&mut brief, constraint);
        }

        for section in &resolved.task.sections {
            match section {
                Section::Intent { content, .. } => {
                    brief.intent = content.clone();
                }
                Section::Constraints { items, .. } => {
                    for constraint in items {
                        push_constraint_into_brief(&mut brief, constraint);
                    }
                }
                Section::Decisions { items, .. } => {
                    for item in items {
                        push_unique(&mut brief.decided, item);
                    }
                }
                Section::Boundaries { items, .. } => {
                    for item in items {
                        match item.category {
                            BoundaryCategory::Allow => push_unique(&mut brief.must, &item.text),
                            BoundaryCategory::Deny | BoundaryCategory::General => {
                                push_unique(&mut brief.must_not, &item.text)
                            }
                        }
                    }
                }
                Section::OutOfScope { items, .. } => {
                    brief.out_of_scope.clone_from(items);
                }
                Section::AcceptanceCriteria { .. } => {}
            }
        }

        brief
    }

    pub fn from_contract(contract: &TaskContract) -> Self {
        let mut must = contract.must.clone();
        for item in &contract.allowed_changes {
            push_unique(&mut must, item);
        }
        let mut must_not = contract.must_not.clone();
        for item in &contract.forbidden {
            push_unique(&mut must_not, item);
        }

        Self {
            name: contract.name.clone(),
            intent: contract.intent.clone(),
            must,
            must_not,
            decided: contract.decisions.clone(),
            scenario_names: contract
                .completion_criteria
                .iter()
                .map(|scenario| scenario.name.clone())
                .collect(),
            out_of_scope: contract.out_of_scope.clone(),
        }
    }

    /// Render as a compact system prompt fragment for agent consumption.
    pub fn to_prompt(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Task: {}\n\n", self.name));
        out.push_str(&format!("## Intent\n{}\n\n", self.intent));

        if !self.must.is_empty() {
            out.push_str("## Must\n");
            for item in &self.must {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        if !self.must_not.is_empty() {
            out.push_str("## Must NOT\n");
            for item in &self.must_not {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        if !self.decided.is_empty() {
            out.push_str("## Already Decided\n");
            for item in &self.decided {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        if !self.scenario_names.is_empty() {
            out.push_str("## Acceptance Scenarios\n");
            for name in &self.scenario_names {
                out.push_str(&format!("- {name}\n"));
            }
            out.push('\n');
        }

        if !self.out_of_scope.is_empty() {
            out.push_str("## Out of Scope (do NOT implement)\n");
            for item in &self.out_of_scope {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        out
    }

    /// Render as JSON for structured agent consumption.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

impl TaskContract {
    pub fn from_doc(doc: &SpecDocument) -> Self {
        let mut contract = Self {
            name: doc.meta.name.clone(),
            intent: String::new(),
            must: Vec::new(),
            must_not: Vec::new(),
            decisions: Vec::new(),
            allowed_changes: Vec::new(),
            forbidden: Vec::new(),
            out_of_scope: Vec::new(),
            completion_criteria: Vec::new(),
        };

        for section in &doc.sections {
            match section {
                Section::Intent { content, .. } => {
                    contract.intent = content.clone();
                }
                Section::Constraints { items, .. } => {
                    for constraint in items {
                        push_constraint_into_contract(&mut contract, constraint);
                    }
                }
                Section::Decisions { items, .. } => {
                    for item in items {
                        push_unique(&mut contract.decisions, item);
                    }
                }
                Section::Boundaries { items, .. } => {
                    for item in items {
                        match item.category {
                            BoundaryCategory::Allow => {
                                push_unique(&mut contract.allowed_changes, &item.text)
                            }
                            BoundaryCategory::Deny => {
                                push_unique(&mut contract.forbidden, &item.text)
                            }
                            BoundaryCategory::General => {
                                push_unique(&mut contract.forbidden, &item.text)
                            }
                        }
                    }
                }
                Section::AcceptanceCriteria { scenarios, .. } => {
                    contract.completion_criteria = scenarios.clone();
                }
                Section::OutOfScope { items, .. } => {
                    contract.out_of_scope.clone_from(items);
                }
            }
        }

        contract
    }

    pub fn from_resolved(resolved: &ResolvedSpec) -> Self {
        let mut contract = Self {
            name: resolved.task.meta.name.clone(),
            intent: String::new(),
            must: Vec::new(),
            must_not: Vec::new(),
            decisions: Vec::new(),
            allowed_changes: Vec::new(),
            forbidden: Vec::new(),
            out_of_scope: Vec::new(),
            completion_criteria: Vec::new(),
        };

        for constraint in &resolved.inherited_constraints {
            push_constraint_into_contract(&mut contract, constraint);
        }
        for decision in &resolved.inherited_decisions {
            push_unique(&mut contract.decisions, decision);
        }

        for section in &resolved.task.sections {
            match section {
                Section::Intent { content, .. } => {
                    contract.intent = content.clone();
                }
                Section::Constraints { items, .. } => {
                    for constraint in items {
                        push_constraint_into_contract(&mut contract, constraint);
                    }
                }
                Section::Decisions { items, .. } => {
                    for item in items {
                        push_unique(&mut contract.decisions, item);
                    }
                }
                Section::Boundaries { items, .. } => {
                    for item in items {
                        match item.category {
                            BoundaryCategory::Allow => {
                                push_unique(&mut contract.allowed_changes, &item.text)
                            }
                            BoundaryCategory::Deny | BoundaryCategory::General => {
                                push_unique(&mut contract.forbidden, &item.text)
                            }
                        }
                    }
                }
                Section::AcceptanceCriteria { scenarios, .. } => {
                    contract.completion_criteria = scenarios.clone();
                }
                Section::OutOfScope { items, .. } => {
                    contract.out_of_scope.clone_from(items);
                }
            }
        }

        contract
    }

    pub fn to_prompt(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Task Contract: {}\n\n", self.name));
        out.push_str(&format!("## Intent\n{}\n\n", self.intent));

        if !self.must.is_empty() {
            out.push_str("## Must\n");
            for item in &self.must {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        if !self.must_not.is_empty() {
            out.push_str("## Must NOT\n");
            for item in &self.must_not {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        if !self.decisions.is_empty() {
            out.push_str("## Decisions\n");
            for item in &self.decisions {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        if !self.allowed_changes.is_empty()
            || !self.forbidden.is_empty()
            || !self.out_of_scope.is_empty()
        {
            out.push_str("## Boundaries\n");
            if !self.allowed_changes.is_empty() {
                out.push_str("Allowed changes:\n");
                for item in &self.allowed_changes {
                    out.push_str(&format!("- {item}\n"));
                }
            }
            if !self.forbidden.is_empty() {
                out.push_str("Forbidden:\n");
                for item in &self.forbidden {
                    out.push_str(&format!("- {item}\n"));
                }
            }
            if !self.out_of_scope.is_empty() {
                out.push_str("Out of scope:\n");
                for item in &self.out_of_scope {
                    out.push_str(&format!("- {item}\n"));
                }
            }
            out.push('\n');
        }

        if !self.completion_criteria.is_empty() {
            out.push_str("## Completion Criteria\n");
            for scenario in &self.completion_criteria {
                render_scenario(&mut out, scenario);
            }
        }

        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

fn push_constraint_into_contract(contract: &mut TaskContract, constraint: &Constraint) {
    match constraint.category {
        ConstraintCategory::Must => push_unique(&mut contract.must, &constraint.text),
        ConstraintCategory::MustNot => push_unique(&mut contract.must_not, &constraint.text),
        ConstraintCategory::Decided => push_unique(&mut contract.decisions, &constraint.text),
        ConstraintCategory::General => push_unique(&mut contract.must, &constraint.text),
    }
}

fn render_scenario(out: &mut String, scenario: &Scenario) {
    out.push_str(&format!("Scenario: {}\n", scenario.name));
    if let Some(selector) = &scenario.test_selector {
        out.push_str("  Test:\n");
        if let Some(package) = &selector.package {
            out.push_str(&format!("    Package: {package}\n"));
        }
        out.push_str(&format!("    Filter: {}\n", selector.filter));
    }

    for step in &scenario.steps {
        out.push_str(&format!(
            "  {} {}\n",
            render_step_keyword(step.kind),
            step.text
        ));
        for row in &step.table {
            out.push_str("    |");
            for cell in row {
                out.push(' ');
                out.push_str(cell);
                out.push(' ');
                out.push('|');
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

fn render_step_keyword(kind: crate::spec_core::StepKind) -> &'static str {
    match kind {
        crate::spec_core::StepKind::Given => "Given",
        crate::spec_core::StepKind::When => "When",
        crate::spec_core::StepKind::Then => "Then",
        crate::spec_core::StepKind::And => "And",
        crate::spec_core::StepKind::But => "But",
    }
}

fn push_unique(bucket: &mut Vec<String>, value: &str) {
    if !bucket.iter().any(|item| item == value) {
        bucket.push(value.to_string());
    }
}

#[allow(deprecated)]
fn push_constraint_into_brief(brief: &mut SpecBrief, constraint: &Constraint) {
    match constraint.category {
        ConstraintCategory::Must => push_unique(&mut brief.must, &constraint.text),
        ConstraintCategory::MustNot => push_unique(&mut brief.must_not, &constraint.text),
        ConstraintCategory::Decided => push_unique(&mut brief.decided, &constraint.text),
        ConstraintCategory::General => push_unique(&mut brief.must, &constraint.text),
    }
}
