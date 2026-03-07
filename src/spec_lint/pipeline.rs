use crate::spec_core::{LintDiagnostic, LintReport, QualityScore, Section, Severity, SpecDocument};

/// Trait for individual spec linters.
pub trait SpecLinter: Send + Sync {
    fn name(&self) -> &str;
    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic>;
}

/// Pipeline that runs all registered linters and produces a report.
pub struct LintPipeline {
    linters: Vec<Box<dyn SpecLinter>>,
}

impl LintPipeline {
    pub fn new() -> Self {
        Self {
            linters: Vec::new(),
        }
    }

    /// Create a pipeline with all built-in linters.
    pub fn with_defaults() -> Self {
        let mut p = Self::new();
        p.add(Box::new(super::linters::VagueVerbLinter));
        p.add(Box::new(super::linters::UnquantifiedLinter));
        p.add(Box::new(super::linters::TestabilityLinter));
        p.add(Box::new(super::linters::CoverageLinter));
        p.add(Box::new(super::linters::DeterminismLinter));
        p.add(Box::new(super::linters::ImplicitDepLinter));
        p.add(Box::new(super::linters::ExplicitTestBindingLinter));
        p.add(Box::new(super::linters::SycophancyLinter));
        p
    }

    pub fn add(&mut self, linter: Box<dyn SpecLinter>) {
        self.linters.push(linter);
    }

    pub fn run(&self, doc: &SpecDocument) -> LintReport {
        let mut diagnostics = Vec::new();
        for linter in &self.linters {
            diagnostics.extend(linter.lint(doc));
        }

        let quality_score = compute_quality(doc, &diagnostics);

        LintReport {
            spec_name: doc.meta.name.clone(),
            diagnostics,
            quality_score,
        }
    }
}

impl Default for LintPipeline {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Cross-check multiple specs for mechanical boundary and decision conflicts.
pub fn cross_check(docs: &[SpecDocument]) -> Vec<LintDiagnostic> {
    use crate::spec_core::Span;

    let mut diags = Vec::new();

    // Collect boundaries and decisions per spec
    let mut spec_boundaries: Vec<(&str, Vec<(String, crate::spec_core::BoundaryCategory)>)> = Vec::new();
    let mut spec_decisions: Vec<(&str, Vec<String>)> = Vec::new();

    for doc in docs {
        let name = doc.meta.name.as_str();
        let mut boundaries = Vec::new();
        let mut decisions = Vec::new();

        for section in &doc.sections {
            match section {
                Section::Boundaries { items, .. } => {
                    for b in items {
                        boundaries.push((b.text.clone(), b.category));
                    }
                }
                Section::Decisions { items, .. } => {
                    decisions.extend(items.clone());
                }
                _ => {}
            }
        }

        spec_boundaries.push((name, boundaries));
        spec_decisions.push((name, decisions));
    }

    // Check boundary conflicts: one spec allows, another denies the same path
    for i in 0..spec_boundaries.len() {
        for j in (i + 1)..spec_boundaries.len() {
            let (name_a, bounds_a) = &spec_boundaries[i];
            let (name_b, bounds_b) = &spec_boundaries[j];

            for (text_a, cat_a) in bounds_a {
                for (text_b, cat_b) in bounds_b {
                    if text_a == text_b
                        && *cat_a != *cat_b
                        && ((*cat_a == crate::spec_core::BoundaryCategory::Allow
                            && *cat_b == crate::spec_core::BoundaryCategory::Deny)
                            || (*cat_a == crate::spec_core::BoundaryCategory::Deny
                                && *cat_b == crate::spec_core::BoundaryCategory::Allow))
                    {
                        diags.push(LintDiagnostic {
                            rule: "cross-check-boundary".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "boundary conflict: '{name_a}' allows '{text_a}' but '{name_b}' forbids it"
                            ),
                            span: Span::line(0),
                            suggestion: Some(
                                "reconcile the conflicting boundary rules between these specs".into(),
                            ),
                        });
                    }
                }
            }
        }
    }

    // Check decision conflicts: contradictory decisions across specs
    for i in 0..spec_decisions.len() {
        for j in (i + 1)..spec_decisions.len() {
            let (name_a, decs_a) = &spec_decisions[i];
            let (name_b, decs_b) = &spec_decisions[j];

            for dec_a in decs_a {
                for dec_b in decs_b {
                    if decisions_contradict(dec_a, dec_b) {
                        diags.push(LintDiagnostic {
                            rule: "cross-check-decision".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "decision conflict between '{name_a}' and '{name_b}': '{}' vs '{}'",
                                truncate_cross(dec_a, 50),
                                truncate_cross(dec_b, 50),
                            ),
                            span: Span::line(0),
                            suggestion: Some(
                                "reconcile the conflicting decisions between these specs".into(),
                            ),
                        });
                    }
                }
            }
        }
    }

    diags
}

/// Simple mechanical check: two decisions contradict if one negates the other.
fn decisions_contradict(a: &str, b: &str) -> bool {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Check negation patterns
    let negation_pairs = [
        ("use ", "do not use "),
        ("使用 ", "不使用 "),
        ("enable ", "disable "),
        ("启用", "禁用"),
        ("allow ", "forbid "),
        ("允许", "禁止"),
    ];

    for (pos, neg) in negation_pairs {
        if (a_lower.contains(pos) && b_lower.contains(neg))
            || (a_lower.contains(neg) && b_lower.contains(pos))
        {
            // Check if they share a common subject
            let a_words: Vec<&str> = a_lower.split_whitespace().collect();
            let b_words: Vec<&str> = b_lower.split_whitespace().collect();
            let shared = a_words
                .iter()
                .filter(|w| w.len() > 3)
                .any(|w| b_words.contains(w));
            if shared {
                return true;
            }
        }
    }

    false
}

fn truncate_cross(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{truncated}...")
    }
}

fn compute_quality(doc: &SpecDocument, diagnostics: &[LintDiagnostic]) -> QualityScore {
    let constraint_count = doc
        .sections
        .iter()
        .filter_map(|s| match s {
            Section::Constraints { items, .. } => Some(items.len()),
            _ => None,
        })
        .sum::<usize>();

    let scenario_count = doc
        .sections
        .iter()
        .filter_map(|s| match s {
            Section::AcceptanceCriteria { scenarios, .. } => Some(scenarios.len()),
            _ => None,
        })
        .sum::<usize>();

    // Determinism: penalty for each determinism warning
    let det_issues = diagnostics
        .iter()
        .filter(|d| d.rule == "determinism")
        .count();
    let determinism = if scenario_count == 0 {
        0.0
    } else {
        (1.0 - det_issues as f64 / scenario_count.max(1) as f64).max(0.0)
    };

    // Testability: penalty for each testability warning
    let test_issues = diagnostics
        .iter()
        .filter(|d| d.rule == "testability")
        .count();
    let step_count: usize = doc
        .sections
        .iter()
        .filter_map(|s| match s {
            Section::AcceptanceCriteria { scenarios, .. } => {
                Some(scenarios.iter().map(|sc| sc.steps.len()).sum::<usize>())
            }
            _ => None,
        })
        .sum();
    let testability = if step_count == 0 {
        0.0
    } else {
        (1.0 - test_issues as f64 / step_count.max(1) as f64).max(0.0)
    };

    // Coverage: ratio of constraints with at least one covering scenario
    let coverage_issues = diagnostics.iter().filter(|d| d.rule == "coverage").count();
    let coverage = if constraint_count == 0 {
        1.0
    } else {
        (1.0 - coverage_issues as f64 / constraint_count as f64).max(0.0)
    };

    QualityScore::compute(determinism, testability, coverage)
}
