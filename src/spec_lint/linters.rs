use crate::spec_core::{
    LintDiagnostic, Scenario, Section, Severity, Span, SpecDocument, SpecLevel, StepKind,
    TestSelector,
};
use std::collections::{HashMap, HashSet};

use super::pipeline::SpecLinter;

// =============================================================================
// 1. VagueVerbLinter - detects vague/imprecise verbs in constraints and steps
// =============================================================================

pub struct VagueVerbLinter;

const VAGUE_VERBS_ZH: &[&str] = &["处理", "管理", "支持", "优化", "改善", "增强", "完善"];

const VAGUE_VERBS_EN: &[&str] = &[
    "handle", "manage", "support", "process", "optimize", "improve", "enhance",
];

impl SpecLinter for VagueVerbLinter {
    fn name(&self) -> &str {
        "vague-verb"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        for section in &doc.sections {
            match section {
                Section::Constraints { items, .. } => {
                    for c in items {
                        if let Some(verb) = find_vague_verb(&c.text) {
                            diags.push(LintDiagnostic {
                                rule: "vague-verb".into(),
                                severity: Severity::Warning,
                                message: format!(
                                    "constraint uses vague verb '{verb}' - use precise verbs like create/delete/validate"
                                ),
                                span: c.span,
                                suggestion: Some(
                                    "replace with specific action: 创建/删除/校验/查询 or create/delete/validate/query".into(),
                                ),
                            });
                        }
                    }
                }
                Section::Intent { content, span } => {
                    if let Some(verb) = find_vague_verb(content) {
                        diags.push(LintDiagnostic {
                            rule: "vague-verb".into(),
                            severity: Severity::Info,
                            message: format!(
                                "intent uses vague verb '{verb}' - consider being more specific"
                            ),
                            span: *span,
                            suggestion: None,
                        });
                    }
                }
                _ => {}
            }
        }

        diags
    }
}

fn find_vague_verb(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for &v in VAGUE_VERBS_ZH {
        if text.contains(v) {
            return Some(v.to_string());
        }
    }
    for &v in VAGUE_VERBS_EN {
        if lower.contains(v) {
            return Some(v.to_string());
        }
    }
    None
}

// =============================================================================
// 2. UnquantifiedLinter - detects constraints without measurable values
// =============================================================================

pub struct UnquantifiedLinter;

const VAGUE_QUALIFIERS_ZH: &[&str] = &["快速", "高效", "及时", "合理", "适当", "足够", "良好"];

const VAGUE_QUALIFIERS_EN: &[&str] = &[
    "fast",
    "efficient",
    "timely",
    "reasonable",
    "appropriate",
    "sufficient",
    "good",
    "quickly",
];

impl SpecLinter for UnquantifiedLinter {
    fn name(&self) -> &str {
        "unquantified"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        for section in &doc.sections {
            if let Section::Constraints { items, .. } = section {
                for c in items {
                    if let Some(qualifier) = find_vague_qualifier(&c.text) {
                        diags.push(LintDiagnostic {
                            rule: "unquantified".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "constraint uses vague qualifier '{qualifier}' without a measurable value"
                            ),
                            span: c.span,
                            suggestion: Some(
                                "add a measurable threshold: e.g., '< 200ms', '>= 80%', '不超过 5 次'".into(),
                            ),
                        });
                    }
                }
            }
        }

        diags
    }
}

fn find_vague_qualifier(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for &q in VAGUE_QUALIFIERS_ZH {
        if text.contains(q) {
            return Some(q.to_string());
        }
    }
    for &q in VAGUE_QUALIFIERS_EN {
        if lower.contains(q) {
            return Some(q.to_string());
        }
    }
    None
}

// =============================================================================
// 3. TestabilityLinter - checks if Then steps are mechanically verifiable
// =============================================================================

pub struct TestabilityLinter;

const UNTESTABLE_ZH: &[&str] = &["美观", "友好", "直观", "舒适", "合适", "自然"];

const UNTESTABLE_EN: &[&str] = &[
    "beautiful",
    "user-friendly",
    "intuitive",
    "comfortable",
    "natural",
    "clean",
    "nice",
];

impl SpecLinter for TestabilityLinter {
    fn name(&self) -> &str {
        "testability"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for scenario in scenarios {
                    for step in &scenario.steps {
                        if (step.kind == StepKind::Then || step.kind == StepKind::And)
                            && let Some(term) = find_untestable_term(&step.text)
                        {
                            diags.push(LintDiagnostic {
                                rule: "testability".into(),
                                severity: Severity::Warning,
                                message: format!(
                                    "step uses subjective term '{term}' that cannot be mechanically verified"
                                ),
                                span: step.span,
                                suggestion: Some(
                                    "replace with a measurable assertion: score >= 90, contains 'X', status == 200".into(),
                                ),
                            });
                        }
                    }
                }
            }
        }

        diags
    }
}

fn find_untestable_term(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for &t in UNTESTABLE_ZH {
        if text.contains(t) {
            return Some(t.to_string());
        }
    }
    for &t in UNTESTABLE_EN {
        if lower.contains(t) {
            return Some(t.to_string());
        }
    }
    None
}

// =============================================================================
// 4. CoverageLinter - checks if constraints are covered by scenarios
// =============================================================================

pub struct CoverageLinter;

impl SpecLinter for CoverageLinter {
    fn name(&self) -> &str {
        "coverage"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        let all_step_text: Vec<&str> = doc
            .sections
            .iter()
            .filter_map(|s| match s {
                Section::AcceptanceCriteria { scenarios, .. } => Some(
                    scenarios
                        .iter()
                        .flat_map(|sc| sc.steps.iter().map(|st| st.text.as_str())),
                ),
                _ => None,
            })
            .flatten()
            .collect();

        for section in &doc.sections {
            if let Section::Constraints { items, .. } = section {
                for c in items {
                    let keywords = extract_keywords(&c.text);
                    let covered = keywords.iter().any(|kw| {
                        all_step_text
                            .iter()
                            .any(|step| step.to_lowercase().contains(&kw.to_lowercase()))
                    });

                    if !covered && !keywords.is_empty() {
                        diags.push(LintDiagnostic {
                            rule: "coverage".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "constraint '{}' has no matching scenario step",
                                truncate(&c.text, 60),
                            ),
                            span: c.span,
                            suggestion: Some("add a scenario that verifies this constraint".into()),
                        });
                    }
                }
            }
        }

        diags
    }
}

fn extract_keywords(text: &str) -> Vec<String> {
    let stop_words = [
        "的", "是", "在", "了", "和", "与", "或", "为", "被", "将", "不", "应", "必须", "使用",
        "所有", "每个", "a", "the", "is", "are", "must", "should", "all", "be", "to", "in", "of",
        "and", "or", "not", "no", "with", "for", "by",
    ];

    text.split(|c: char| c.is_whitespace() || c == ',' || c == '、' || c == '。')
        .filter(|w| {
            let w_lower = w.to_lowercase();
            w.len() > 1 && !stop_words.iter().any(|sw| w_lower == *sw)
        })
        .map(String::from)
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{truncated}...")
    }
}

// =============================================================================
// 5. DeterminismLinter
// =============================================================================

pub struct DeterminismLinter;

const NONDETERMINISTIC_ZH: &[&str] = &["大约", "大概", "可能", "也许", "随机", "有时"];

const NONDETERMINISTIC_EN: &[&str] = &[
    "approximately",
    "roughly",
    "maybe",
    "possibly",
    "random",
    "sometimes",
    "might",
    "could",
    "about",
];

impl SpecLinter for DeterminismLinter {
    fn name(&self) -> &str {
        "determinism"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for scenario in scenarios {
                    for step in &scenario.steps {
                        if let Some(term) = find_nondeterministic(&step.text) {
                            diags.push(LintDiagnostic {
                                rule: "determinism".into(),
                                severity: Severity::Warning,
                                message: format!(
                                    "step uses non-deterministic term '{term}' - outcomes should be exact"
                                ),
                                span: step.span,
                                suggestion: Some(
                                    "use exact values: '== 100', 'contains X', 'status is 200'".into(),
                                ),
                            });
                        }
                    }
                }
            }
        }

        diags
    }
}

fn find_nondeterministic(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for &t in NONDETERMINISTIC_ZH {
        if text.contains(t) {
            return Some(t.to_string());
        }
    }
    for &t in NONDETERMINISTIC_EN {
        if lower.contains(t) {
            return Some(t.to_string());
        }
    }
    None
}

// =============================================================================
// 6. ImplicitDepLinter
// =============================================================================

pub struct ImplicitDepLinter;

impl SpecLinter for ImplicitDepLinter {
    fn name(&self) -> &str {
        "implicit-dep"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for scenario in scenarios {
                    let given_entities: Vec<String> = scenario
                        .steps
                        .iter()
                        .filter(|s| s.kind == StepKind::Given || s.kind == StepKind::And)
                        .flat_map(|s| s.params.clone())
                        .collect();

                    let mut seen_when = false;
                    for step in &scenario.steps {
                        if step.kind == StepKind::When {
                            seen_when = true;
                        }
                        if seen_when {
                            for param in &step.params {
                                if !given_entities.contains(param) && !is_likely_literal(param) {
                                    diags.push(LintDiagnostic {
                                        rule: "implicit-dep".into(),
                                        severity: Severity::Info,
                                        message: format!(
                                            "parameter '{param}' referenced but not defined in Given steps"
                                        ),
                                        span: step.span,
                                        suggestion: Some(
                                            "add a Given step that establishes this value".into(),
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        diags
    }
}

fn is_likely_literal(value: &str) -> bool {
    value.parse::<f64>().is_ok()
        || value.chars().all(|c| c.is_ascii_digit() || c == '.')
        || value.starts_with("http")
        || value.contains('@')
}

// =============================================================================
// 7. ExplicitTestBindingLinter
// =============================================================================

pub struct ExplicitTestBindingLinter;

impl SpecLinter for ExplicitTestBindingLinter {
    fn name(&self) -> &str {
        "explicit-test-binding"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        if doc.meta.level != SpecLevel::Task {
            return diags;
        }

        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for scenario in scenarios {
                    if scenario.test_selector.is_none() {
                        diags.push(LintDiagnostic {
                            rule: "explicit-test-binding".into(),
                            severity: Severity::Error,
                            message: format!(
                                "scenario '{}' is missing an explicit test selector",
                                scenario.name
                            ),
                            span: scenario.span,
                            suggestion: Some(
                                "add `测试: test_name` or `Test: test_name` directly under the scenario header".into(),
                            ),
                        });
                    }
                }
            }
        }

        diags
    }
}

// =============================================================================
// 8. ScenarioPresenceLinter
// =============================================================================

pub struct ScenarioPresenceLinter;

impl SpecLinter for ScenarioPresenceLinter {
    fn name(&self) -> &str {
        "scenario-presence"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let acceptance_sections: Vec<_> = doc
            .sections
            .iter()
            .filter_map(|section| match section {
                Section::AcceptanceCriteria { scenarios, span, .. } => Some((scenarios, span)),
                _ => None,
            })
            .collect();

        if acceptance_sections.is_empty() {
            return vec![LintDiagnostic {
                rule: "scenario-presence".into(),
                severity: Severity::Error,
                message: "task spec is missing an Acceptance Criteria / Completion Criteria section"
                    .into(),
                span: crate::spec_core::Span::line(0),
                suggestion: Some(
                    "add `## 验收标准` / `## Completion Criteria` with at least one `场景:` / `Scenario:` block".into(),
                ),
            }];
        }

        let scenario_count = acceptance_sections
            .iter()
            .map(|(scenarios, _)| scenarios.len())
            .sum::<usize>();

        if scenario_count == 0 {
            return vec![LintDiagnostic {
                rule: "scenario-presence".into(),
                severity: Severity::Error,
                message:
                    "task spec has an Acceptance Criteria section but no parseable scenarios".into(),
                span: *acceptance_sections[0].1,
                suggestion: Some(
                    "write scenarios using bare `场景:` / `Scenario:` lines, or run `agent-spec parse` to inspect the AST".into(),
                ),
            }];
        }

        Vec::new()
    }
}

// =============================================================================
// 9. SycophancyLinter
// =============================================================================

pub struct SycophancyLinter;

const SYCOPHANCY_ZH: &[&str] = &[
    "找出所有",
    "必须找到",
    "尽可能多地发现",
    "不要遗漏任何",
    "确保发现所有",
];

const SYCOPHANCY_EN: &[&str] = &[
    "find all bugs",
    "find every bug",
    "must find",
    "discover as many",
    "do not miss any",
    "ensure you find all",
    "catch all issues",
    "identify all problems",
    "find all issues",
];

impl SpecLinter for SycophancyLinter {
    fn name(&self) -> &str {
        "sycophancy"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        for section in &doc.sections {
            let (texts, span): (Vec<&str>, _) = match section {
                Section::Intent { content, span } => (vec![content.as_str()], *span),
                Section::Constraints { items, span } => {
                    (items.iter().map(|c| c.text.as_str()).collect(), *span)
                }
                Section::AcceptanceCriteria { scenarios, span, .. } => {
                    let texts: Vec<&str> = scenarios
                        .iter()
                        .flat_map(|s| s.steps.iter().map(|st| st.text.as_str()))
                        .collect();
                    (texts, *span)
                }
                _ => continue,
            };

            for text in texts {
                if let Some(phrase) = find_sycophancy_phrase(text) {
                    diags.push(LintDiagnostic {
                        rule: "sycophancy".into(),
                        severity: Severity::Warning,
                        message: format!(
                            "spec uses bug-finding bias phrase '{phrase}' which may induce sycophantic AI behavior"
                        ),
                        span,
                        suggestion: Some(
                            "use neutral language: 'verify the contract holds' instead of 'find all bugs'".into(),
                        ),
                    });
                }
            }
        }

        diags
    }
}

fn find_sycophancy_phrase(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for &p in SYCOPHANCY_ZH {
        if text.contains(p) {
            return Some(p.to_string());
        }
    }
    for &p in SYCOPHANCY_EN {
        if lower.contains(p) {
            return Some(p.to_string());
        }
    }
    None
}

// =============================================================================
// 10. DecisionCoverageLinter - checks if decisions are covered by scenarios
// =============================================================================

pub struct DecisionCoverageLinter;

impl SpecLinter for DecisionCoverageLinter {
    fn name(&self) -> &str {
        "decision-coverage"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        // Only check task-level specs (decisions in project specs are inherited)
        if doc.meta.level != SpecLevel::Task {
            return diags;
        }

        // Collect all step text from scenarios
        let all_step_text: Vec<&str> = doc
            .sections
            .iter()
            .filter_map(|s| match s {
                Section::AcceptanceCriteria { scenarios, .. } => Some(
                    scenarios
                        .iter()
                        .flat_map(|sc| sc.steps.iter().map(|st| st.text.as_str())),
                ),
                _ => None,
            })
            .flatten()
            .collect();

        // Also collect scenario names (decisions are often reflected in scenario names)
        let scenario_names: Vec<&str> = doc
            .sections
            .iter()
            .filter_map(|s| match s {
                Section::AcceptanceCriteria { scenarios, .. } => {
                    Some(scenarios.iter().map(|sc| sc.name.as_str()))
                }
                _ => None,
            })
            .flatten()
            .collect();

        for section in &doc.sections {
            if let Section::Decisions { items, span } = section {
                for (i, decision) in items.iter().enumerate() {
                    let keywords = extract_decision_keywords(decision);
                    if keywords.is_empty() {
                        continue;
                    }

                    let covered_by_step = keywords.iter().any(|kw| {
                        all_step_text
                            .iter()
                            .any(|step| step.to_lowercase().contains(&kw.to_lowercase()))
                    });

                    let covered_by_name = keywords.iter().any(|kw| {
                        scenario_names
                            .iter()
                            .any(|name| name.to_lowercase().contains(&kw.to_lowercase()))
                    });

                    if !covered_by_step && !covered_by_name {
                        diags.push(LintDiagnostic {
                            rule: "decision-coverage".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "decision '{}' has no matching scenario",
                                truncate(decision, 60),
                            ),
                            span: Span::new(span.start_line + i + 1, 0, span.start_line + i + 1, 0),
                            suggestion: Some(
                                "add a scenario that verifies this decision is implemented correctly".into(),
                            ),
                        });
                    }
                }
            }
        }

        diags
    }
}

/// Extract meaningful keywords from a decision text, filtering common verbs and articles.
fn extract_decision_keywords(text: &str) -> Vec<String> {
    let stop_words = [
        // Chinese
        "的", "是", "在", "了", "和", "与", "或", "为", "被", "将", "不", "应", "必须", "使用",
        "所有", "每个", "通过", "可以", "需要", "这个", "那个", "一个", // English
        "a", "the", "is", "are", "must", "should", "all", "be", "to", "in", "of", "and", "or",
        "not", "no", "with", "for", "by", "use", "using", "this", "that", "when", "if", "then",
        "will", "does", "do", "has", "have", "can",
    ];

    // Extract backtick-quoted identifiers first (highest signal)
    let mut keywords: Vec<String> = Vec::new();
    let mut in_backtick = false;
    let mut current = String::new();
    for c in text.chars() {
        if c == '`' {
            if in_backtick && !current.is_empty() {
                keywords.push(current.clone());
                current.clear();
            }
            in_backtick = !in_backtick;
        } else if in_backtick {
            current.push(c);
        }
    }

    // Also extract regular words (lower priority)
    let words: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == '、' || c == '。' || c == '`')
        .filter(|w| {
            let w_lower = w.to_lowercase();
            w.len() > 2 && !stop_words.iter().any(|sw| w_lower == *sw)
        })
        .map(String::from)
        .collect();

    keywords.extend(words);
    keywords
}

// =============================================================================
// 10b. ObservableDecisionCoverageLinter - behavioral decisions need explicit
//      scenario coverage, not just structural mention overlap
// =============================================================================

pub struct ObservableDecisionCoverageLinter;

const OBSERVABLE_DECISION_KEYWORDS: &[&str] = &[
    "stdout",
    "stderr",
    "--json",
    "-o",
    "--output",
    "output",
    "fallback",
    "precedence",
    "priority",
    "cache",
    "local",
    "remote",
    "bundle",
    "timeout",
    "env",
    "force",
    "冷启动",
    "缓存",
    "本地",
    "远端",
    "远程",
    "回退",
    "优先",
    "顺序",
    "输出",
    "环境变量",
    "超时",
];

impl SpecLinter for ObservableDecisionCoverageLinter {
    fn name(&self) -> &str {
        "observable-decision-coverage"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let scenario_text = collect_all_scenario_text(doc);
        let mut diags = Vec::new();

        for section in &doc.sections {
            if let Section::Decisions { items, span } = section {
                for (i, decision) in items.iter().enumerate() {
                    if !contains_observable_keyword(decision) {
                        continue;
                    }

                    let keywords = extract_decision_keywords(decision);
                    let covered = keywords
                        .iter()
                        .any(|kw| text_set_contains(&scenario_text, kw));

                    if !covered {
                        diags.push(LintDiagnostic {
                            rule: "observable-decision-coverage".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "behavioral decision '{}' lacks an explicit scenario covering its observable behavior",
                                truncate(decision, 60),
                            ),
                            span: Span::new(span.start_line + i + 1, 0, span.start_line + i + 1, 0),
                            suggestion: Some(
                                "add a scenario that verifies the user-visible behavior for this decision (stdout/stderr, json, output files, fallback, precedence, cache, timeout, or env handling)".into(),
                            ),
                        });
                    }
                }
            }
        }

        diags
    }
}

// =============================================================================
// 10c. OutputModeCoverageLinter - when specs mention multiple output modes,
//      scenarios must cover them explicitly
// =============================================================================

pub struct OutputModeCoverageLinter;

impl SpecLinter for OutputModeCoverageLinter {
    fn name(&self) -> &str {
        "output-mode-coverage"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let mut required_modes = Vec::new();
        let mut spans = Vec::new();
        for section in &doc.sections {
            match section {
                Section::Intent { content, span } => {
                    required_modes.extend(detect_output_modes(content));
                    spans.push(*span);
                }
                Section::Constraints { items, .. } => {
                    for item in items {
                        required_modes.extend(detect_output_modes(&item.text));
                        spans.push(item.span);
                    }
                }
                Section::Decisions { items, span } => {
                    for (i, item) in items.iter().enumerate() {
                        required_modes.extend(detect_output_modes(item));
                        spans.push(Span::new(
                            span.start_line + i + 1,
                            0,
                            span.start_line + i + 1,
                            0,
                        ));
                    }
                }
                _ => {}
            }
        }

        dedup_strings(&mut required_modes);
        if required_modes.is_empty() {
            return Vec::new();
        }

        let scenario_text = collect_all_scenario_text(doc);
        let missing: Vec<String> = required_modes
            .into_iter()
            .filter(|mode| !scenario_covers_output_mode(&scenario_text, mode))
            .collect();

        if missing.is_empty() {
            return Vec::new();
        }

        let span = spans.first().copied().unwrap_or_else(|| Span::line(0));
        vec![LintDiagnostic {
            rule: "output-mode-coverage".into(),
            severity: Severity::Warning,
            message: format!(
                "spec mentions output behavior but missing explicit scenario coverage for mode(s): {}",
                missing.join(", ")
            ),
            span,
            suggestion: Some(
                "add scenarios that verify each mentioned mode, such as human output, JSON output, file output, and stdout/stderr cleanliness".into(),
            ),
        }]
    }
}

// =============================================================================
// 10d. PrecedenceFallbackCoverageLinter - ordered behavior must be verified
// =============================================================================

pub struct PrecedenceFallbackCoverageLinter;

impl SpecLinter for PrecedenceFallbackCoverageLinter {
    fn name(&self) -> &str {
        "precedence-fallback-coverage"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let scenario_text = collect_all_scenario_text(doc);
        let mut diags = Vec::new();

        for section in &doc.sections {
            match section {
                Section::Decisions { items, span } => {
                    for (i, item) in items.iter().enumerate() {
                        if let Some(chain_terms) = extract_ordered_behavior_terms(item)
                            && !ordered_behavior_is_covered(&scenario_text, &chain_terms)
                        {
                            diags.push(LintDiagnostic {
                                rule: "precedence-fallback-coverage".into(),
                                severity: Severity::Warning,
                                message: format!(
                                    "ordered behavior '{}' has no scenario that verifies the precedence/fallback chain",
                                    truncate(item, 60),
                                ),
                                span: Span::new(
                                    span.start_line + i + 1,
                                    0,
                                    span.start_line + i + 1,
                                    0,
                                ),
                                suggestion: Some(
                                    "add a scenario that exercises the documented precedence or fallback order".into(),
                                ),
                            });
                        }
                    }
                }
                Section::Constraints { items, .. } => {
                    for item in items {
                        if let Some(chain_terms) = extract_ordered_behavior_terms(&item.text)
                            && !ordered_behavior_is_covered(&scenario_text, &chain_terms)
                        {
                            diags.push(LintDiagnostic {
                                rule: "precedence-fallback-coverage".into(),
                                severity: Severity::Warning,
                                message: format!(
                                    "ordered behavior '{}' has no scenario that verifies the precedence/fallback chain",
                                    truncate(&item.text, 60),
                                ),
                                span: item.span,
                                suggestion: Some(
                                    "add a scenario that exercises the documented precedence or fallback order".into(),
                                ),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        diags
    }
}

// =============================================================================
// 10e. ExternalIoErrorStrengthLinter - high-risk I/O error scenarios should
//      avoid mock-only verification unless a realistic boundary is also named
// =============================================================================

pub struct ExternalIoErrorStrengthLinter;

const EXTERNAL_IO_TERMS: &[&str] = &[
    "http",
    "network",
    "registry.json",
    "bundle.tar.gz",
    "bundle",
    "filesystem",
    "file system",
    "disk",
    "path",
    "stdio",
    "protocol",
    "json-rpc",
    "4xx",
    "5xx",
    "non-2xx",
    "timeout",
    "网络",
    "文件系统",
    "磁盘",
    "路径",
    "协议",
    "超时",
    "4xx/5xx",
];

const WEAK_IO_TERMS: &[&str] = &[
    "mock",
    "mock-only",
    "inject",
    "injected",
    "closure",
    "stub only",
    "模拟",
    "注入",
    "闭包",
];

const STRONG_IO_TERMS: &[&str] = &[
    "fixture",
    "temp dir",
    "temporary directory",
    "local stub",
    "stub server",
    "http stub",
    "filesystem fixture",
    "real bytes",
    "真实",
    "临时目录",
    "本地 stub",
    "本地替身",
    "fixture 文件",
];

impl SpecLinter for ExternalIoErrorStrengthLinter {
    fn name(&self) -> &str {
        "external-io-error-strength"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let mut diags = Vec::new();
        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for scenario in scenarios {
                    let scenario_text = scenario_text_blob(scenario);
                    if !contains_any_term(&scenario_text, EXTERNAL_IO_TERMS)
                        || !contains_any_term(&scenario_text, ERROR_PATH_INDICATORS_EN)
                            && !contains_any_term(&scenario_text, ERROR_PATH_INDICATORS_ZH)
                    {
                        continue;
                    }

                    let selector_label = scenario
                        .test_selector
                        .as_ref()
                        .map(TestSelector::label)
                        .unwrap_or_default();
                    let combined = if selector_label.is_empty() {
                        scenario_text.clone()
                    } else {
                        format!("{scenario_text} {selector_label}")
                    };

                    if contains_any_term(&combined, WEAK_IO_TERMS)
                        && !contains_any_term(&combined, STRONG_IO_TERMS)
                    {
                        diags.push(LintDiagnostic {
                            rule: "external-io-error-strength".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "scenario '{}' describes external I/O failure handling but appears to rely on mock-only verification",
                                scenario.name
                            ),
                            span: scenario.span,
                            suggestion: Some(
                                "prefer a local HTTP stub, fixture filesystem, temporary directory, or another realistic boundary in the scenario/test selector".into(),
                            ),
                        });
                    }
                }
            }
        }
        diags
    }
}

pub struct VerificationMetadataSuggestionLinter;

impl SpecLinter for VerificationMetadataSuggestionLinter {
    fn name(&self) -> &str {
        "verification-metadata-suggestion"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let mut diags = Vec::new();
        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for scenario in scenarios {
                    let text = scenario_text_blob(scenario);
                    if !contains_any_term(&text, EXTERNAL_IO_TERMS) {
                        continue;
                    }

                    let missing_metadata = scenario.test_selector.as_ref().is_none_or(|selector| {
                        selector.level.is_none()
                            && selector.test_double.is_none()
                            && selector.targets.is_none()
                    });

                    if missing_metadata {
                        diags.push(LintDiagnostic {
                            rule: "verification-metadata-suggestion".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "scenario '{}' covers external I/O behavior without verification-strength metadata",
                                scenario.name
                            ),
                            span: scenario.span,
                            suggestion: Some(
                                "add `Level:` / `层级:`, `Test Double:` / `替身:`, or `Targets:` / `命中:` to clarify test strength".into(),
                            ),
                        });
                    }
                }
            }
        }
        diags
    }
}

// =============================================================================
// 11. ErrorPathLinter - checks if scenarios include error/failure paths
// =============================================================================

pub struct ErrorPathLinter;

const ERROR_PATH_INDICATORS_ZH: &[&str] = &[
    "错误",
    "失败",
    "拒绝",
    "异常",
    "超时",
    "不存在",
    "无效",
    "禁止",
    "返回错误",
    "命令失败",
    "返回 error",
    "isError",
];

const ERROR_PATH_INDICATORS_EN: &[&str] = &[
    "error",
    "fail",
    "reject",
    "invalid",
    "forbidden",
    "timeout",
    "not found",
    "not exist",
    "denied",
    "unauthorized",
    "4xx",
    "5xx",
    "non-2xx",
    "panic",
    "abort",
];

impl SpecLinter for ErrorPathLinter {
    fn name(&self) -> &str {
        "error-path"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        if doc.meta.level != SpecLevel::Task {
            return diags;
        }

        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, span, .. } = section {
                if scenarios.is_empty() {
                    continue;
                }

                let has_error_scenario = scenarios.iter().any(|sc| {
                    let name_lower = sc.name.to_lowercase();
                    let has_error_name = ERROR_PATH_INDICATORS_ZH
                        .iter()
                        .any(|ind| sc.name.contains(ind))
                        || ERROR_PATH_INDICATORS_EN
                            .iter()
                            .any(|ind| name_lower.contains(ind));

                    let has_error_step = sc.steps.iter().any(|step| {
                        let text_lower = step.text.to_lowercase();
                        ERROR_PATH_INDICATORS_ZH
                            .iter()
                            .any(|ind| step.text.contains(ind))
                            || ERROR_PATH_INDICATORS_EN
                                .iter()
                                .any(|ind| text_lower.contains(ind))
                    });

                    has_error_name || has_error_step
                });

                if !has_error_scenario {
                    diags.push(LintDiagnostic {
                        rule: "error-path".into(),
                        severity: Severity::Warning,
                        message: format!(
                            "no error/failure path scenarios found ({} scenarios are all happy paths)",
                            scenarios.len()
                        ),
                        span: *span,
                        suggestion: Some(
                            "add at least one scenario that tests error handling (e.g., invalid input, network failure, malformed data)".into(),
                        ),
                    });
                }
            }
        }

        diags
    }
}

// =============================================================================
// 12. UniversalClaimLinter - decisions/constraints with universal quantifiers
//     must be backed by proportional scenario coverage
// =============================================================================

pub struct UniversalClaimLinter;

/// Patterns that indicate a universal claim (must apply to ALL instances).
const UNIVERSAL_ZH: &[&str] = &[
    "所有入口",
    "每个入口",
    "所有二进制",
    "每个二进制",
    "所有 bin",
    "每个 bin",
    "所有模块",
    "每个模块",
    "所有调用方",
    "每个调用方",
    "所有实现",
    "每个实现",
    "统一行为",
    "行为一致",
    "保持一致",
];

const UNIVERSAL_EN: &[&str] = &[
    "all entry points",
    "every entry point",
    "all binaries",
    "every binary",
    "all callers",
    "every caller",
    "all implementations",
    "every implementation",
    "all modules",
    "every module",
    "consistent behavior",
    "consistent behaviour",
    "behave identically",
    "behave consistently",
];

impl SpecLinter for UniversalClaimLinter {
    fn name(&self) -> &str {
        "universal-claim"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        if doc.meta.level != SpecLevel::Task {
            return diags;
        }

        let scenario_count: usize = doc
            .sections
            .iter()
            .filter_map(|s| match s {
                Section::AcceptanceCriteria { scenarios, .. } => Some(scenarios.len()),
                _ => None,
            })
            .sum();

        // Check Decisions for universal claims
        for section in &doc.sections {
            if let Section::Decisions { items, span } = section {
                for (i, decision) in items.iter().enumerate() {
                    if let Some(claim) = find_universal_claim(decision)
                        && scenario_count < 2
                    {
                        diags.push(LintDiagnostic {
                            rule: "universal-claim".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "decision claims '{}' but only {} scenario(s) exist — universal claims need multiple scenarios to verify each instance",
                                claim, scenario_count
                            ),
                            span: Span::new(
                                span.start_line + i + 1,
                                0,
                                span.start_line + i + 1,
                                0,
                            ),
                            suggestion: Some(
                                "add scenarios for each entry point / implementation that the universal claim covers".into(),
                            ),
                        });
                    }
                }
            }

            if let Section::Constraints { items, .. } = section {
                for c in items {
                    if let Some(claim) = find_universal_claim(&c.text)
                        && scenario_count < 2
                    {
                        diags.push(LintDiagnostic {
                            rule: "universal-claim".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "constraint claims '{}' but only {} scenario(s) exist — universal claims need multiple scenarios",
                                claim, scenario_count
                            ),
                            span: c.span,
                            suggestion: Some(
                                "add scenarios for each instance that the universal claim covers".into(),
                            ),
                        });
                    }
                }
            }
        }

        diags
    }
}

fn find_universal_claim(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for &p in UNIVERSAL_ZH {
        if text.contains(p) {
            return Some(p.to_string());
        }
    }
    for &p in UNIVERSAL_EN {
        if lower.contains(p) {
            return Some(p.to_string());
        }
    }
    None
}

fn collect_all_scenario_text(doc: &SpecDocument) -> Vec<String> {
    doc.sections
        .iter()
        .filter_map(|s| match s {
            Section::AcceptanceCriteria { scenarios, .. } => Some(
                scenarios
                    .iter()
                    .flat_map(|scenario| {
                        let mut texts = vec![scenario.name.to_lowercase()];
                        texts.extend(scenario.steps.iter().map(|step| step.text.to_lowercase()));
                        texts
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

fn contains_observable_keyword(text: &str) -> bool {
    contains_any_term(text, OBSERVABLE_DECISION_KEYWORDS)
}

fn contains_any_term(text: &str, terms: &[&str]) -> bool {
    let lower = text.to_lowercase();
    terms.iter().any(|term| {
        let t = term.to_lowercase();
        lower.contains(&t)
    })
}

fn text_set_contains(texts: &[String], term: &str) -> bool {
    let lower = term.to_lowercase();
    texts.iter().any(|text| text.contains(&lower))
}

fn detect_output_modes(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut modes = Vec::new();

    if lower.contains("--json")
        || text.contains("JSON 输出")
        || lower.contains("json output")
        || lower.contains("json mode")
        || text.contains("JSON 模式")
    {
        modes.push("json".to_string());
    }
    if lower.contains("-o")
        || lower.contains("--output")
        || lower.contains("写文件")
        || lower.contains("write to file")
    {
        modes.push("file-output".to_string());
    }
    if lower.contains("stdout") {
        modes.push("stdout".to_string());
    }
    if lower.contains("stderr") {
        modes.push("stderr".to_string());
    }
    if lower.contains("human output") || text.contains("人类模式") || text.contains("默认输出")
    {
        modes.push("human".to_string());
    }

    dedup_strings(&mut modes);
    modes
}

fn scenario_covers_output_mode(scenario_text: &[String], mode: &str) -> bool {
    match mode {
        "json" => scenario_text
            .iter()
            .any(|text| text.contains("--json") || text.contains("json")),
        "file-output" => scenario_text.iter().any(|text| {
            text.contains("-o")
                || text.contains("--output")
                || text.contains("写文件")
                || text.contains("write to file")
                || text.contains("output path")
        }),
        "stdout" => scenario_text.iter().any(|text| text.contains("stdout")),
        "stderr" => scenario_text.iter().any(|text| text.contains("stderr")),
        "human" => scenario_text.iter().any(|text| {
            text.contains("human output") || text.contains("人类模式") || text.contains("默认输出")
        }),
        _ => false,
    }
}

fn dedup_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn extract_ordered_behavior_terms(text: &str) -> Option<Vec<String>> {
    let lower = text.to_lowercase();
    if text.contains("->") {
        let terms: Vec<String> = text
            .split("->")
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_lowercase())
            .collect();
        if terms.len() >= 2 {
            return Some(terms);
        }
    }

    let has_order_language = lower.contains("fallback")
        || lower.contains("precedence")
        || lower.contains("priority")
        || lower.contains("prefer")
        || text.contains("回退")
        || text.contains("优先")
        || text.contains("顺序");

    if has_order_language {
        let keywords = extract_decision_keywords(text);
        let filtered: Vec<String> = keywords
            .into_iter()
            .filter(|kw| kw.len() > 2)
            .take(4)
            .collect();
        if filtered.len() >= 2 {
            return Some(filtered);
        }
    }

    None
}

fn ordered_behavior_is_covered(scenario_text: &[String], chain_terms: &[String]) -> bool {
    scenario_text.iter().any(|text| {
        let matches = chain_terms
            .iter()
            .filter(|term| text.contains(*term))
            .count();
        matches >= 2
            || text.contains("fallback")
            || text.contains("precedence")
            || text.contains("priority")
            || text.contains("回退")
            || text.contains("优先")
            || text.contains("顺序")
    })
}

fn scenario_text_blob(scenario: &Scenario) -> String {
    let mut parts = vec![scenario.name.clone()];
    parts.extend(scenario.steps.iter().map(|step| step.text.clone()));
    parts.join(" ").to_lowercase()
}

// =============================================================================
// 13. BoundaryEntryPointLinter - warns when Boundaries list multiple entry
//     points (bin/, main.rs) but scenarios don't reference each one
// =============================================================================

pub struct BoundaryEntryPointLinter;

/// Patterns that indicate an entry point file in Boundaries.
const ENTRY_POINT_PATTERNS: &[&str] = &[
    "bin/",
    "main.rs",
    "main.py",
    "main.ts",
    "main.go",
    "index.ts",
    "index.js",
    "cli.rs",
    "server.rs",
];

impl SpecLinter for BoundaryEntryPointLinter {
    fn name(&self) -> &str {
        "boundary-entry-point"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        if doc.meta.level != SpecLevel::Task {
            return diags;
        }

        // Collect entry point paths from Boundaries
        let mut entry_points: Vec<(String, Span)> = Vec::new();
        for section in &doc.sections {
            if let Section::Boundaries { items, .. } = section {
                for boundary in items {
                    if boundary.category == crate::spec_core::BoundaryCategory::Allow {
                        let text_lower = boundary.text.to_lowercase();
                        if ENTRY_POINT_PATTERNS.iter().any(|p| text_lower.contains(p)) {
                            entry_points.push((boundary.text.clone(), boundary.span));
                        }
                    }
                }
            }
        }

        // Only warn when there are 2+ entry points
        if entry_points.len() < 2 {
            return diags;
        }

        // Collect all scenario step text and names
        let all_scenario_text: Vec<String> = doc
            .sections
            .iter()
            .filter_map(|s| match s {
                Section::AcceptanceCriteria { scenarios, .. } => Some(
                    scenarios
                        .iter()
                        .flat_map(|sc| {
                            let mut texts = vec![sc.name.to_lowercase()];
                            texts.extend(sc.steps.iter().map(|st| st.text.to_lowercase()));
                            texts
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .flatten()
            .collect();

        // Check which entry points are referenced in scenarios
        for (ep_text, ep_span) in &entry_points {
            // Extract the filename or last path segment
            let filename = ep_text
                .rsplit('/')
                .next()
                .unwrap_or(ep_text)
                .trim_end_matches('*')
                .trim_end_matches('.');
            let filename_lower = filename.to_lowercase();

            // Also try the stem without extension
            let stem = filename.split('.').next().unwrap_or(filename);
            let stem_lower = stem.to_lowercase();

            if filename_lower.is_empty() && stem_lower.is_empty() {
                continue;
            }

            let referenced = all_scenario_text.iter().any(|text| {
                (!filename_lower.is_empty() && text.contains(&filename_lower))
                    || (!stem_lower.is_empty()
                        && stem_lower.len() > 2
                        && text.contains(&stem_lower))
            });

            if !referenced {
                diags.push(LintDiagnostic {
                    rule: "boundary-entry-point".into(),
                    severity: Severity::Warning,
                    message: format!(
                        "entry point '{}' is in Boundaries but no scenario references it — shared logic across entry points needs separate verification",
                        ep_text
                    ),
                    span: *ep_span,
                    suggestion: Some(
                        "add a scenario that tests behavior through this specific entry point, or extract shared logic into a common function tested once".into(),
                    ),
                });
            }
        }

        diags
    }
}

// =============================================================================
// 14. FlagCombinationCoverageLinter - when a CLI command has multiple flags
//     that affect output behavior, require scenarios covering key combinations
//     (e.g., multi-ID + -o, single-ID + --full + -o)
// =============================================================================

pub struct FlagCombinationCoverageLinter;

/// CLI flags that affect output behavior and should be tested in combination.
const COMBINATION_FLAGS_ZH: &[(&str, &[&str])] = &[
    ("多 ID", &["多个", "多 ID", "batch", "多条目"]),
    (
        "-o/--output",
        &["-o", "--output", "写文件", "写入文件", "输出到文件"],
    ),
    ("--full", &["--full", "全部文件", "所有文件"]),
    ("--json", &["--json", "JSON 模式", "JSON 输出"]),
];

const COMBINATION_FLAGS_EN: &[(&str, &[&str])] = &[
    ("multi-ID", &["multiple", "multi", "batch", "ids..."]),
    (
        "-o/--output",
        &["-o", "--output", "write to file", "output path"],
    ),
    ("--full", &["--full", "all files", "full mode"]),
    ("--json", &["--json", "json mode", "json output"]),
];

impl SpecLinter for FlagCombinationCoverageLinter {
    fn name(&self) -> &str {
        "flag-combination-coverage"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let mut diags = Vec::new();

        // Detect which flags are mentioned in decisions/constraints
        let mut mentioned_flags: Vec<String> = Vec::new();
        let mut decision_span: Option<Span> = None;

        for section in &doc.sections {
            let texts: Vec<&str> = match section {
                Section::Decisions { items, span } => {
                    decision_span = Some(*span);
                    items.iter().map(|s| s.as_str()).collect()
                }
                Section::Constraints { items, .. } => {
                    items.iter().map(|c| c.text.as_str()).collect()
                }
                _ => continue,
            };

            for text in texts {
                let lower = text.to_lowercase();
                for (flag_name, indicators) in COMBINATION_FLAGS_ZH
                    .iter()
                    .chain(COMBINATION_FLAGS_EN.iter())
                {
                    if indicators
                        .iter()
                        .any(|ind| lower.contains(&ind.to_lowercase()))
                    {
                        let name = flag_name.to_string();
                        if !mentioned_flags.contains(&name) {
                            mentioned_flags.push(name);
                        }
                    }
                }
            }
        }

        // Only check when 2+ flags are mentioned
        if mentioned_flags.len() < 2 {
            return diags;
        }

        // Check if scenarios cover any combination (2+ flags in same scenario)
        let scenarios = collect_scenario_texts_per_scenario(doc);
        let has_combination_scenario = scenarios.iter().any(|texts| {
            let mut flags_in_scenario = 0;
            for flag_name in &mentioned_flags {
                let indicators = get_flag_indicators(flag_name);
                if indicators.iter().any(|ind| {
                    texts
                        .iter()
                        .any(|t| t.to_lowercase().contains(&ind.to_lowercase()))
                }) {
                    flags_in_scenario += 1;
                }
            }
            flags_in_scenario >= 2
        });

        if !has_combination_scenario {
            let span = decision_span.unwrap_or(Span::line(0));
            diags.push(LintDiagnostic {
                rule: "flag-combination-coverage".into(),
                severity: Severity::Warning,
                message: format!(
                    "decisions mention {} output-affecting flags ({}) but no scenario tests a combination of 2+ flags together",
                    mentioned_flags.len(),
                    mentioned_flags.join(", ")
                ),
                span,
                suggestion: Some(
                    "add scenarios that test flag combinations (e.g., multi-ID + -o, single entry + --full + -o) — individual flag tests miss interaction bugs".into(),
                ),
            });
        }

        diags
    }
}

/// Collect scenario texts grouped by scenario (not flattened).
fn collect_scenario_texts_per_scenario(doc: &SpecDocument) -> Vec<Vec<String>> {
    doc.sections
        .iter()
        .filter_map(|s| match s {
            Section::AcceptanceCriteria { scenarios, .. } => Some(
                scenarios
                    .iter()
                    .map(|sc| {
                        let mut texts = vec![sc.name.clone()];
                        texts.extend(sc.steps.iter().map(|st| st.text.clone()));
                        texts
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Get all indicator strings for a flag name (both languages).
fn get_flag_indicators(flag_name: &str) -> Vec<String> {
    let mut indicators = Vec::new();
    for (name, inds) in COMBINATION_FLAGS_ZH
        .iter()
        .chain(COMBINATION_FLAGS_EN.iter())
    {
        if *name == flag_name {
            indicators.extend(inds.iter().map(|s| s.to_string()));
        }
    }
    indicators
}

// =============================================================================
// 15. PlatformDecisionTagLinter - detects decisions that reference platform-
//     specific concepts (npm, pip, cargo, bundled dist, etc.) without explicit
//     platform tags, warning about phantom requirements in parity rewrites
// =============================================================================

pub struct PlatformDecisionTagLinter;

/// Platform-specific terms that may indicate a decision copied from a reference
/// implementation without considering if it applies to the current platform.
const PLATFORM_SPECIFIC_TERMS: &[(&str, &str)] = &[
    ("npm", "Node.js/npm"),
    ("node_modules", "Node.js"),
    ("dist/", "npm/bundled"),
    ("bundled dist", "npm/bundled"),
    ("package.json", "Node.js"),
    ("pip", "Python/pip"),
    ("site-packages", "Python"),
    ("__pycache__", "Python"),
    ("setup.py", "Python"),
    ("cargo install", "Rust/cargo"),
    ("crate.io", "Rust/cargo"),
    ("gem install", "Ruby"),
    ("nuget", ".NET"),
    ("maven", "Java"),
    ("go install", "Go"),
    ("apt-get", "Debian/Ubuntu"),
    ("brew install", "macOS/Homebrew"),
];

/// Tags that indicate the author is aware of the platform dependency.
const PLATFORM_AWARENESS_MARKERS: &[&str] = &[
    "[js-only]",
    "[node-only]",
    "[npm-only]",
    "[python-only]",
    "[rust-only]",
    "[platform-specific]",
    "[不适用]",
    "[JS 特有]",
    "[Node 特有]",
    "[Python 特有]",
    "[Rust 特有]",
    "[平台特有]",
    "不实现",
    "not applicable",
    "n/a",
    "skipped",
    "跳过",
];

impl SpecLinter for PlatformDecisionTagLinter {
    fn name(&self) -> &str {
        "platform-decision-tag"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        if doc.meta.level != SpecLevel::Task {
            return Vec::new();
        }

        let mut diags = Vec::new();

        for section in &doc.sections {
            if let Section::Decisions { items, span } = section {
                for (i, decision) in items.iter().enumerate() {
                    let lower = decision.to_lowercase();

                    // Check if the decision references a platform-specific term
                    let platform_match = PLATFORM_SPECIFIC_TERMS
                        .iter()
                        .find(|(term, _)| lower.contains(&term.to_lowercase()));

                    if let Some((term, platform)) = platform_match {
                        // Check if the author has already tagged it
                        let is_tagged = PLATFORM_AWARENESS_MARKERS
                            .iter()
                            .any(|marker| lower.contains(&marker.to_lowercase()));

                        if !is_tagged {
                            diags.push(LintDiagnostic {
                                rule: "platform-decision-tag".into(),
                                severity: Severity::Info,
                                message: format!(
                                    "decision references '{}' ({}), which may be platform-specific — if this is a parity rewrite, mark with [platform-specific] or state whether it applies",
                                    term, platform
                                ),
                                span: Span::new(
                                    span.start_line + i + 1,
                                    0,
                                    span.start_line + i + 1,
                                    0,
                                ),
                                suggestion: Some(format!(
                                    "add a platform tag like [JS-only] or explicitly state that {platform} behavior is not applicable in the current platform"
                                )),
                            });
                        }
                    }
                }
            }
        }

        diags
    }
}

/// Collect all scenarios from a SpecDocument.
fn collect_scenarios(doc: &SpecDocument) -> Vec<&crate::spec_core::Scenario> {
    doc.sections
        .iter()
        .filter_map(|section| match section {
            Section::AcceptanceCriteria { scenarios, .. } => {
                Some(scenarios.iter().collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect()
}

// =============================================================================
// CircularDependencyLinter - detects circular dependencies in scenario depends_on
// =============================================================================

pub struct CircularDependencyLinter;

impl SpecLinter for CircularDependencyLinter {
    fn name(&self) -> &str {
        "circular-dependency"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();
        let scenarios: Vec<&Scenario> = doc
            .sections
            .iter()
            .filter_map(|s| match s {
                Section::AcceptanceCriteria { scenarios, .. } => Some(scenarios.iter()),
                _ => None,
            })
            .flatten()
            .collect();

        // Build adjacency map: name -> depends_on names
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut span_map: HashMap<&str, Span> = HashMap::new();
        for s in &scenarios {
            adj.insert(
                s.name.as_str(),
                s.depends_on.iter().map(|d| d.as_str()).collect(),
            );
            span_map.insert(s.name.as_str(), s.span);
        }

        // DFS cycle detection
        let mut visited: HashSet<&str> = HashSet::new();
        let mut on_stack: HashSet<&str> = HashSet::new();

        for s in &scenarios {
            if !visited.contains(s.name.as_str()) {
                let mut path = Vec::new();
                if let Some(cycle) =
                    dfs_find_cycle(s.name.as_str(), &adj, &mut visited, &mut on_stack, &mut path)
                {
                    let cycle_display = cycle.join(" -> ");
                    let span = span_map
                        .get(s.name.as_str())
                        .copied()
                        .unwrap_or_default();
                    diags.push(LintDiagnostic {
                        rule: "circular-dependency".into(),
                        severity: Severity::Error,
                        message: format!("circular dependency detected: {cycle_display}"),
                        span,
                        suggestion: Some(
                            "remove or restructure dependencies to break the cycle".into(),
                        ),
                    });
                }
            }
        }

        diags
    }
}

fn dfs_find_cycle<'a>(
    node: &'a str,
    adj: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    on_stack: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
) -> Option<Vec<String>> {
    visited.insert(node);
    on_stack.insert(node);
    path.push(node);

    if let Some(deps) = adj.get(node) {
        for &dep in deps {
            if !visited.contains(dep) {
                if let Some(cycle) = dfs_find_cycle(dep, adj, visited, on_stack, path) {
                    return Some(cycle);
                }
            } else if on_stack.contains(dep) {
                // Found a cycle: extract the cycle portion
                let cycle_start = path.iter().position(|&n| n == dep).unwrap_or(0);
                let mut cycle: Vec<String> =
                    path[cycle_start..].iter().map(|s| s.to_string()).collect();
                cycle.push(dep.to_string());
                return Some(cycle);
            }
        }
    }

    on_stack.remove(node);
    path.pop();
    None
}

// =============================================================================
// BDD semantics v1 linters (Phase 1). All warning/info — never Error, so they
// stay out of the lifecycle pass/fail verdict. Diagnostics carry agent-readable
// self-correction guidance: why the rule exists, how to fix, when an exception
// is legitimate, and how to leave a trace if not fixing.
// =============================================================================

/// `bdd-rule-id`: a `Rule:` / `规则:` line whose leading token is not a valid
/// kebab-case id is dropped by the parser (no auto-slugify). Warn so the author
/// gives it a stable id or removes it.
pub struct BddRuleIdLinter;

impl SpecLinter for BddRuleIdLinter {
    fn name(&self) -> &str {
        "bdd-rule-id"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();
        for section in &doc.sections {
            if let Section::AcceptanceCriteria { malformed_rules, .. } = section {
                for m in malformed_rules {
                    diags.push(LintDiagnostic {
                        rule: "bdd-rule-id".into(),
                        severity: Severity::Warning,
                        message: format!(
                            "Rule line '{}' has no explicit kebab-case id, so no rule was created and its scenarios are ungrouped",
                            truncate_bdd(&m.raw, 50)
                        ),
                        span: m.span,
                        suggestion: Some(
                            "A Rule needs a stable id (^[a-z][a-z0-9-]$) because it survives display-name edits and lifts to capability scope unchanged. \
Fix: `Rule: kebab-id — 显示名` (em dash or two-space separator), or `Rule: kebab-id` alone. \
If you don't want a rule here, delete the line — scenarios stay flat. \
If you must keep it as-is for now, leave `<!-- lint-ack: bdd-rule-id — <reason> -->` (full lint-ack lands in Phase 5).".into(),
                        ),
                    });
                }
            }
        }
        diags
    }
}

/// `bdd-rule-grouping`: suggests grouping when many scenarios are ungrouped, and
/// warns on a `Rule:` that has no scenarios proving it.
pub struct BddRuleGroupingLinter;

impl SpecLinter for BddRuleGroupingLinter {
    fn name(&self) -> &str {
        "bdd-rule-grouping"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();
        for section in &doc.sections {
            if let Section::AcceptanceCriteria {
                scenarios,
                rules,
                span,
                ..
            } = section
            {
                // Empty rule: declared but proven by nothing.
                for r in rules {
                    if r.scenario_names.is_empty() {
                        diags.push(LintDiagnostic {
                            rule: "bdd-rule-grouping".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "Rule '{}' has no scenarios proving it",
                                r.key.id
                            ),
                            span: r.span,
                            suggestion: Some(
                                "A rule is a promise; an unproven promise is just prose. \
Add at least one `Scenario:`/`Example:` under it (with a `Test:` selector), or remove the Rule line. \
To keep it deliberately empty for now, leave `<!-- lint-ack: bdd-rule-grouping — <reason> -->`.".into(),
                            ),
                        });
                    }
                }

                // Many ungrouped scenarios: suggest introducing rules.
                let ungrouped = scenarios.iter().filter(|s| s.rule.is_none()).count();
                if rules.is_empty() && ungrouped >= 3 {
                    diags.push(LintDiagnostic {
                        rule: "bdd-rule-grouping".into(),
                        severity: Severity::Info,
                        message: format!(
                            "{ungrouped} scenarios with no Rule grouping"
                        ),
                        span: *span,
                        suggestion: Some(
                            "Grouping scenarios under `Rule: <id> — <name>` makes the behavior they prove explicit and lets the rule lift to capability scope later. \
Add `Rule:` headers above related scenarios. \
This is a suggestion, not a gate — if flat structure is right for this spec, ignore it or leave `<!-- lint-ack: bdd-rule-grouping — <reason> -->`.".into(),
                        ),
                    });
                }
            }
        }
        diags
    }
}

/// `bdd-scenario-shape`: each scenario should have When + Then, and should not
/// open with And/But (a continuation with nothing to continue).
pub struct BddScenarioShapeLinter;

impl SpecLinter for BddScenarioShapeLinter {
    fn name(&self) -> &str {
        "bdd-scenario-shape"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();
        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for sc in scenarios {
                    if sc.steps.is_empty() {
                        continue;
                    }
                    let has_when = sc.steps.iter().any(|s| s.kind == StepKind::When);
                    let has_then = sc.steps.iter().any(|s| s.kind == StepKind::Then);
                    if !has_when || !has_then {
                        let missing = match (has_when, has_then) {
                            (false, false) => "When and Then",
                            (false, true) => "When",
                            (true, false) => "Then",
                            _ => unreachable!(),
                        };
                        diags.push(LintDiagnostic {
                            rule: "bdd-scenario-shape".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "scenario '{}' is missing {missing}",
                                sc.display_name()
                            ),
                            span: sc.span,
                            suggestion: Some(
                                "A behavioral example needs a trigger (When/当) and an observable outcome (Then/那么) because without them the verifier cannot bind it to behavior. \
Fix: add the missing step(s). \
If this is intentionally a setup-only fragment, reconsider whether it should be a Scenario at all. \
To keep it as-is for now, leave `<!-- lint-ack: bdd-scenario-shape — <reason> -->` (full lint-ack lands in Phase 5).".into(),
                            ),
                        });
                    }
                    if matches!(
                        sc.steps[0].kind,
                        StepKind::And | StepKind::But
                    ) {
                        diags.push(LintDiagnostic {
                            rule: "bdd-scenario-shape".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "scenario '{}' opens with And/But (并且/但是) — nothing to continue from",
                                sc.display_name()
                            ),
                            span: sc.steps[0].span,
                            suggestion: Some(
                                "And/But continue a prior Given/When/Then, because an opening continuation has nothing to continue from. \
Fix: start the scenario with Given/When/Then (假设/当/那么) instead. \
If the leading And/But is deliberate, leave `<!-- lint-ack: bdd-scenario-shape — <reason> -->` (full lint-ack lands in Phase 5).".into(),
                            ),
                        });
                    }
                }
            }
        }
        diags
    }
}

/// `bdd-implementation-detail-step`: steps that describe UI/script procedure
/// (click/type/visit/CSS selectors, 点击/输入/访问...) instead of behavior.
pub struct BddImplementationDetailStepLinter;

const BDD_IMPL_DETAIL_EN: &[&str] = &[
    "click", "type ", "visit", "press ", "data-testid", "css selector", ".class", "#id",
    "navigate to",
];

const BDD_IMPL_DETAIL_ZH: &[&str] = &[
    "点击", "输入", "访问", "填写", "选择", "打开页面", "查看页面", "拖拽",
];

impl SpecLinter for BddImplementationDetailStepLinter {
    fn name(&self) -> &str {
        "bdd-implementation-detail-step"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();
        for section in &doc.sections {
            if let Section::AcceptanceCriteria { scenarios, .. } = section {
                for sc in scenarios {
                    for step in &sc.steps {
                        let lower = step.text.to_lowercase();
                        let hit_en = BDD_IMPL_DETAIL_EN.iter().find(|kw| lower.contains(*kw));
                        let hit_zh = BDD_IMPL_DETAIL_ZH.iter().find(|kw| step.text.contains(*kw));
                        if let Some(kw) = hit_en.or(hit_zh) {
                            diags.push(LintDiagnostic {
                                rule: "bdd-implementation-detail-step".into(),
                                severity: Severity::Info,
                                message: format!(
                                    "step describes UI/implementation procedure ('{kw}') rather than behavior"
                                ),
                                span: step.span,
                                suggestion: Some(
                                    "Behavioral steps survive UI refactors; procedural ones break when the page changes. \
Prefer `When the user signs in` over `When the user clicks the #login button`. \
If this step genuinely must assert UI mechanics, leave it and (Phase 5) `<!-- lint-ack: bdd-implementation-detail-step — <reason> -->`.".into(),
                                ),
                            });
                            break; // one hint per step
                        }
                    }
                }
            }
        }
        diags
    }
}

/// Truncate a string to `max` chars for diagnostic messages.
fn truncate_bdd(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::len_zero, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_parser::parse_spec_from_str;

    #[test]
    fn test_vague_verb_linter() {
        let input = r#"spec: task
name: "test"
---

## 约束

- 系统应处理用户请求
- 退款金额不得超过原始交易金额
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = VagueVerbLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("处理"));
    }

    #[test]
    fn test_unquantified_linter() {
        let input = r#"spec: task
name: "test"
---

## Constraints

- Response should be fast
- Timeout must be less than 500ms
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = UnquantifiedLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("fast"));
    }

    #[test]
    fn test_testability_linter() {
        let input = r#"spec: task
name: "test"
---

## 验收标准

场景: UI测试
  假设 用户已登录
  当 用户打开页面
  那么 界面应该美观
  并且 响应状态码为 200
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = TestabilityLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("美观"));
    }

    #[test]
    fn test_determinism_linter() {
        let input = r#"spec: task
name: "test"
---

## Acceptance Criteria

Scenario: test
  Given a user exists
  When user sends request
  Then response should take approximately 100ms
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = DeterminismLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("approximately"));
    }

    #[test]
    fn test_full_pipeline() {
        let input = r#"spec: task
name: "退款功能"
---

## 意图

为支付网关添加退款功能。

## 约束

- 退款金额不得超过原始交易金额
- 退款操作需要管理员权限

## 验收标准

场景: 全额退款
  测试: test_full_refund
  假设 存在一笔金额为 "100.00" 元的已完成交易 "TXN-001"
  当 用户对 "TXN-001" 发起全额退款
  那么 退款状态变为 "processing"
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let pipeline = crate::spec_lint::LintPipeline::with_defaults();
        let report = pipeline.run(&doc);
        assert!(!report.spec_name.is_empty());
        assert!(report.quality_score.overall >= 0.0);
        assert!(report.quality_score.overall <= 1.0);
    }

    #[test]
    fn test_explicit_test_binding_linter_requires_task_scenario_selectors() {
        let input = r#"spec: task
name: "test"
---

## 完成条件

场景: 缺失绑定
  假设 存在某个任务
  当 verifier 检查规格
  那么 应报告缺少 selector
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ExplicitTestBindingLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(
            diags[0]
                .message
                .contains("missing an explicit test selector")
        );
    }

    #[test]
    fn test_sycophancy_linter_flags_bug_finding_bias() {
        let input = r#"spec: task
name: "test"
---

## Intent

Review the code to find all bugs and catch all issues.

## Constraints

- You must find every bug in the implementation
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = SycophancyLinter.lint(&doc);
        assert!(
            !diags.is_empty(),
            "should flag sycophancy-inducing language"
        );
        assert!(diags.iter().any(|d| d.rule == "sycophancy"));
        assert!(diags.iter().any(|d| d.suggestion.is_some()));
    }

    #[test]
    fn test_quality_report_scores_testability_and_smells() {
        let input = r#"spec: task
name: "quality"
---

## Constraints

- Response should be fast and efficient
- All errors must use structured types

## Acceptance Criteria

Scenario: good path
  Test: test_quality_report_scores_testability_and_smells
  Given a user exists
  When user submits a request
  Then response status should be 200
  And the UI should look beautiful
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let pipeline = crate::spec_lint::LintPipeline::with_defaults();
        let report = pipeline.run(&doc);

        assert!(
            report.diagnostics.iter().any(|d| d.rule == "testability"),
            "should flag untestable term"
        );
        assert!(
            report.diagnostics.iter().any(|d| d.rule == "unquantified"),
            "should flag unquantified qualifier"
        );
        assert!(
            report.quality_score.testability < 1.0,
            "testability penalized"
        );
        assert!(report.quality_score.overall > 0.0, "overall score positive");
        assert!(
            report.quality_score.overall < 1.0,
            "overall score penalized"
        );
    }

    #[test]
    fn test_cross_check_reports_boundary_and_decision_conflicts() {
        let spec_a = parse_spec_from_str(
            r#"spec: task
name: "Spec A"
---

## Decisions

- Use tokio for async runtime

## Boundaries

### Allowed Changes
- crates/spec-core/**
"#,
        )
        .unwrap();

        let spec_b = parse_spec_from_str(
            r#"spec: task
name: "Spec B"
---

## Decisions

- Do not use tokio for async runtime

## Boundaries

### Forbidden
- crates/spec-core/**
"#,
        )
        .unwrap();

        let diags = crate::spec_lint::cross_check(&[spec_a, spec_b]);

        assert!(
            diags.iter().any(|d| d.rule == "cross-check-boundary"),
            "should detect boundary conflict: {:?}",
            diags
        );

        assert!(
            diags.iter().any(|d| d.rule == "cross-check-decision"),
            "should detect decision conflict: {:?}",
            diags
        );
    }

    #[test]
    fn test_explicit_test_binding_linter_accepts_explicit_selector() {
        let input = r#"spec: task
name: "test"
---

## 完成条件

场景: 显式绑定
  测试: test_explicit_test_binding_linter_accepts_explicit_selector
  假设 存在某个任务
  当 verifier 检查规格
  那么 不应报告绑定错误
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ExplicitTestBindingLinter.lint(&doc);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_scenario_presence_linter_requires_acceptance_criteria() {
        let input = r#"spec: task
name: "missing scenarios"
---

## Intent

Describe the task.
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ScenarioPresenceLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("missing an Acceptance Criteria"));
    }

    // ── DecisionCoverageLinter tests ────────────────────────────────

    #[test]
    fn test_decision_coverage_warns_on_uncovered_decision() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 使用 `BTreeMap` 确保输出顺序确定性。
- 本地源优先从 `source.path` 读取 registry。

## 验收标准

场景: 输出是确定性的
  测试: output_is_deterministic
  假设 已构建注册表
  当 运行 build 命令
  那么 输出使用 BTreeMap 排序
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = DecisionCoverageLinter.lint(&doc);
        // "BTreeMap" is covered by the scenario, but "source.path" / "本地源" is NOT
        assert!(
            diags.len() >= 1,
            "should warn about uncovered 'source.path' decision, got {} diags",
            diags.len()
        );
        assert!(diags.iter().any(|d| d.rule == "decision-coverage"));
    }

    #[test]
    fn test_decision_coverage_passes_when_all_covered() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 使用 `BTreeMap` 确保输出顺序确定性。

## 验收标准

场景: 输出是确定性的
  测试: output_is_deterministic
  假设 已构建注册表
  当 运行 build 命令
  那么 输出使用 BTreeMap 排序
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = DecisionCoverageLinter.lint(&doc);
        assert!(diags.is_empty(), "all decisions covered, got: {:?}", diags);
    }

    #[test]
    fn test_observable_decision_coverage_warns_when_behavioral_decisions_lack_scenarios() {
        let input = r#"spec: task
name: "test"
---

## 决策

- `--json` 模式下 stdout 只能输出 JSON，fallback 顺序必须保持稳定。

## 验收标准

场景: 默认输出可用
  测试: human_output_works
  假设 用户运行默认命令
  当 输出结果
  那么 人类模式返回文本
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ObservableDecisionCoverageLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "observable-decision-coverage");
    }

    #[test]
    fn test_output_mode_coverage_warns_when_json_or_output_flags_are_uncovered() {
        let input = r#"spec: task
name: "test"
---

## 决策

- `get --json` 返回结构化输出，`-o/--output` 用于写文件。

## 验收标准

场景: 默认 human 输出可用
  测试: human_output_works
  假设 用户运行默认命令
  当 输出结果
  那么 返回默认 human 输出
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = OutputModeCoverageLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "output-mode-coverage");
        assert!(diags[0].message.contains("json"));
        assert!(diags[0].message.contains("file-output"));
    }

    #[test]
    fn test_precedence_fallback_coverage_warns_when_ordered_behavior_has_no_scenario() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 读取顺序为 `local -> cache -> remote`。

## 验收标准

场景: 远端读取成功
  测试: remote_read_success
  假设 用户请求内容
  当 运行读取命令
  那么 返回文档内容
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = PrecedenceFallbackCoverageLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "precedence-fallback-coverage");
    }

    #[test]
    fn test_external_io_error_strength_warns_on_weak_mock_only_http_scenarios() {
        let input = r#"spec: task
name: "test"
---

## 验收标准

场景: HTTP 4xx 返回错误
  测试: mock_only_http_error
  假设 通过注入 mock closure 模拟 404 HTTP 响应
  当 运行 update
  那么 返回 HTTP error
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ExternalIoErrorStrengthLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "external-io-error-strength");
    }

    #[test]
    fn test_behavior_completeness_linters_do_not_flag_plain_implementation_choices() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 使用 `BTreeMap` 和 `serde_json`。
- 目录结构维持 `src/**` 和 `specs/**`。

## 验收标准

场景: 输出是确定性的
  测试: output_is_deterministic
  假设 已构建注册表
  当 运行 build 命令
  那么 输出使用 BTreeMap 排序
"#;
        let doc = parse_spec_from_str(input).unwrap();
        assert!(ObservableDecisionCoverageLinter.lint(&doc).is_empty());
        assert!(OutputModeCoverageLinter.lint(&doc).is_empty());
        assert!(PrecedenceFallbackCoverageLinter.lint(&doc).is_empty());
        assert!(ExternalIoErrorStrengthLinter.lint(&doc).is_empty());
    }

    #[test]
    fn test_lint_suggests_verification_metadata_for_external_io_scenarios() {
        let input = r#"spec: task
name: "test"
---

## 验收标准

场景: HTTP 4xx 返回错误
  测试: update_http_error
  假设 远端 HTTP 请求返回 404
  当 运行 update
  那么 返回 HTTP error
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = VerificationMetadataSuggestionLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "verification-metadata-suggestion");
    }

    // ── ErrorPathLinter tests ────────────────────────────────────────

    #[test]
    fn test_error_path_warns_on_all_happy_paths() {
        let input = r#"spec: task
name: "test"
---

## 验收标准

场景: 成功创建用户
  测试: create_user_success
  假设 数据库可用
  当 提交有效用户数据
  那么 用户被创建

场景: 成功查询用户
  测试: query_user_success
  假设 用户已存在
  当 查询用户列表
  那么 返回用户数据
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ErrorPathLinter.lint(&doc);
        assert_eq!(diags.len(), 1, "should warn about missing error paths");
        assert_eq!(diags[0].rule, "error-path");
        assert!(diags[0].message.contains("happy paths"));
    }

    #[test]
    fn test_error_path_passes_with_error_scenario() {
        let input = r#"spec: task
name: "test"
---

## 验收标准

场景: 成功创建用户
  测试: create_user_success
  假设 数据库可用
  当 提交有效用户数据
  那么 用户被创建

场景: 无效数据返回错误
  测试: create_user_invalid_error
  假设 数据库可用
  当 提交无效用户数据
  那么 返回错误消息
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ErrorPathLinter.lint(&doc);
        assert!(diags.is_empty(), "has error scenario, should pass");
    }

    #[test]
    fn test_error_path_detects_english_error_indicators() {
        let input = r#"spec: task
name: "test"
---

## Completion Criteria

Scenario: successful operation
  Test: op_success
  Given a valid input
  When the operation runs
  Then it returns 200

Scenario: rejects invalid input
  Test: op_rejects_invalid
  Given an invalid input
  When the operation runs
  Then it returns an error response
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ErrorPathLinter.lint(&doc);
        assert!(diags.is_empty(), "has 'error' in step text, should pass");
    }

    // ── UniversalClaimLinter tests ─────────────────────────────────

    #[test]
    fn test_universal_claim_warns_single_scenario_for_all_entry_points() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 所有入口点必须使用相同的合并逻辑

## 验收标准

场景: CLI 合并正确
  测试: cli_merge
  假设 有多个源
  当 CLI 执行搜索
  那么 返回合并结果
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = UniversalClaimLinter.lint(&doc);
        assert_eq!(diags.len(), 1, "should warn: universal claim + 1 scenario");
        assert_eq!(diags[0].rule, "universal-claim");
        assert!(diags[0].message.contains("所有入口"));
    }

    #[test]
    fn test_universal_claim_passes_with_multiple_scenarios() {
        let input = r#"spec: task
name: "test"
---

## Decisions

- All entry points must use the same merge logic

## Completion Criteria

Scenario: CLI merges correctly
  Test: cli_merge
  Given multiple sources
  When CLI runs search
  Then merged results returned

Scenario: MCP merges correctly
  Test: mcp_merge
  Given multiple sources
  When MCP server runs search
  Then merged results returned
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = UniversalClaimLinter.lint(&doc);
        assert!(
            diags.is_empty(),
            "2 scenarios for universal claim should pass, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_universal_claim_ignores_non_universal_decisions() {
        let input = r#"spec: task
name: "test"
---

## Decisions

- Use BTreeMap for deterministic output

## Completion Criteria

Scenario: output is sorted
  Test: sorted_output
  Given a registry
  When build runs
  Then output is deterministic
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = UniversalClaimLinter.lint(&doc);
        assert!(diags.is_empty(), "no universal claim, should pass");
    }

    // ── BoundaryEntryPointLinter tests ──────────────────────────────

    #[test]
    fn test_boundary_entry_point_warns_uncovered_entry() {
        let input = r#"spec: task
name: "test"
---

## Boundaries

### Allowed Changes
- src/bin/chub.rs
- src/bin/chub_mcp.rs

## Completion Criteria

Scenario: CLI search works
  Test: cli_search
  Given a registry
  When chub search runs
  Then results returned
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = BoundaryEntryPointLinter.lint(&doc);
        assert_eq!(
            diags.len(),
            1,
            "should warn about chub_mcp.rs not covered, got: {:?}",
            diags
        );
        assert!(diags[0].message.contains("chub_mcp"));
    }

    #[test]
    fn test_boundary_entry_point_passes_all_covered() {
        let input = r#"spec: task
name: "test"
---

## Boundaries

### Allowed Changes
- src/bin/chub.rs
- src/bin/chub_mcp.rs

## Completion Criteria

Scenario: CLI search works
  Test: cli_search
  Given a registry
  When chub search runs
  Then results returned

Scenario: MCP search works via chub_mcp
  Test: mcp_search
  Given a registry
  When chub_mcp handles search tool call
  Then results returned
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = BoundaryEntryPointLinter.lint(&doc);
        assert!(diags.is_empty(), "both entry points covered, should pass");
    }

    #[test]
    fn test_boundary_entry_point_ignores_single_entry() {
        let input = r#"spec: task
name: "test"
---

## Boundaries

### Allowed Changes
- src/main.rs
- src/lib.rs

## Completion Criteria

Scenario: app works
  Test: app_works
  Given valid config
  When app starts
  Then it runs
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = BoundaryEntryPointLinter.lint(&doc);
        // src/lib.rs is not an entry point pattern, so only 1 entry point (main.rs)
        // Single entry point should not trigger warning
        assert!(diags.is_empty(), "single entry point should not warn");
    }

    // ── FlagCombinationCoverageLinter tests ────────────────────────

    #[test]
    fn test_flag_combination_warns_when_multiple_flags_but_no_combo_scenario() {
        let input = r#"spec: task
name: "test"
---

## 决策

- `get -o` 用于写文件，`--json` 返回结构化输出。
- 多 ID 时合并所有内容写入单文件。

## 验收标准

场景: 单 ID -o 写文件
  测试: single_id_output
  假设 存在一个条目
  当 运行 get -o out.md
  那么 文件被写入

场景: JSON 模式输出
  测试: json_output
  假设 存在一个条目
  当 运行 get --json
  那么 返回 JSON
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = FlagCombinationCoverageLinter.lint(&doc);
        assert_eq!(
            diags.len(),
            1,
            "should warn: 3 flags but no combo scenario, got: {:?}",
            diags
        );
        assert_eq!(diags[0].rule, "flag-combination-coverage");
    }

    #[test]
    fn test_flag_combination_passes_when_combo_scenario_exists() {
        let input = r#"spec: task
name: "test"
---

## 决策

- `get -o` 用于写文件，`--json` 返回结构化输出。
- 多 ID 时合并所有内容写入单文件。

## 验收标准

场景: 单 ID -o 写文件
  测试: single_id_output
  假设 存在一个条目
  当 运行 get -o out.md
  那么 文件被写入

场景: 多 ID -o 合并写入
  测试: multi_id_output
  假设 存在多个条目
  当 运行 get a b -o combined.md
  那么 合并内容写入单文件
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = FlagCombinationCoverageLinter.lint(&doc);
        assert!(
            diags.is_empty(),
            "combo scenario exists (多 ID + -o), should pass, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_flag_combination_ignores_single_flag_specs() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 使用 BTreeMap 确保输出顺序。

## 验收标准

场景: 输出有序
  测试: ordered_output
  假设 已构建注册表
  当 运行 build
  那么 输出是确定性的
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = FlagCombinationCoverageLinter.lint(&doc);
        assert!(
            diags.is_empty(),
            "single flag should not trigger, got: {:?}",
            diags
        );
    }

    // ── PlatformDecisionTagLinter tests ─────────────────────────────

    #[test]
    fn test_platform_tag_warns_on_untagged_npm_reference() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 读取顺序为 local source -> 本地缓存 -> npm bundled dist -> 远端下载。
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = PlatformDecisionTagLinter.lint(&doc);
        assert_eq!(
            diags.len(),
            1,
            "should warn about untagged npm reference, got: {:?}",
            diags
        );
        assert_eq!(diags[0].rule, "platform-decision-tag");
        assert!(diags[0].message.contains("npm"));
    }

    #[test]
    fn test_platform_tag_passes_when_tagged() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 读取顺序为 local source -> 本地缓存 -> 远端下载。
  （chub-rs 不实现 npm bundled dist 路径，这是 JS 特有 的包分发机制。）
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = PlatformDecisionTagLinter.lint(&doc);
        assert!(
            diags.is_empty(),
            "tagged with 不实现, should pass, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_platform_tag_ignores_non_platform_decisions() {
        let input = r#"spec: task
name: "test"
---

## 决策

- 使用 BTreeMap 和 serde_json。
- 搜索优先使用预构建 BM25 index。
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = PlatformDecisionTagLinter.lint(&doc);
        assert!(
            diags.is_empty(),
            "no platform terms, should pass, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_scenario_presence_linter_rejects_empty_acceptance_criteria() {
        let input = r#"spec: task
name: "empty scenarios"
---

## Completion Criteria
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = ScenarioPresenceLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("no parseable scenarios"));
    }

    #[test]
    fn test_lint_detects_circular_dependency() {
        let input = r#"spec: task
name: "circular deps"
---

## Completion Criteria

Scenario: A
  Depends: B
  Given A
  When A
  Then A

Scenario: B
  Depends: A
  Given B
  When B
  Then B
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = CircularDependencyLinter.lint(&doc);
        assert!(
            !diags.is_empty(),
            "should detect circular dependency, got no diagnostics"
        );
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("circular dependency"));
    }

    #[test]
    fn test_lint_no_circular_dependency_for_linear_chain() {
        let input = r#"spec: task
name: "linear deps"
---

## Completion Criteria

Scenario: A
  Given A
  When A
  Then A

Scenario: B
  Depends: A
  Given B
  When B
  Then B

Scenario: C
  Depends: B
  Given C
  When C
  Then C
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = CircularDependencyLinter.lint(&doc);
        assert!(
            diags.is_empty(),
            "should not detect cycle in linear chain, got: {:?}",
            diags
        );
    }

    // ---- BDD semantics v1 lints ----


    fn guidance_has_four_elements(s: &str) -> bool {
        // (1) why exists  (2) how to fix  (3) when exception ok  (4) lint-ack trace
        let l = s.to_lowercase();
        let why = l.contains("because")
            || l.contains("survives")
            || l.contains("promise")
            || l.contains("behavioral")
            || l.contains("cannot");
        let how =
            l.contains("fix") || l.contains("add") || l.contains("prefer") || l.contains("start");
        let exception =
            l.contains("if ") || l.contains("ignore") || l.contains("reconsider");
        let ack = l.contains("lint-ack");
        why && how && exception && ack
    }

    #[test]
    fn test_freeform_rule_emits_warning_and_does_not_group_scenarios() {
        let input = r#"spec: task
name: "促销"
---

## 完成条件

规则: VIP 折扣优先于促销叠加
场景: 折扣场景
  测试: test_discount
  假设 a
  当 b
  那么 c
"#;
        let doc = parse_spec_from_str(input).unwrap();
        // No BehaviorRule created; scenario ungrouped.
        match &doc.sections[0] {
            Section::AcceptanceCriteria {
                rules, scenarios, ..
            } => {
                assert!(rules.is_empty(), "invalid id must not create a rule");
                assert!(scenarios[0].rule.is_none());
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
        let diags = BddRuleIdLinter.lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "bdd-rule-id");
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn test_bdd_rule_grouping_suggests_when_three_or_more_scenarios_uncategorized() {
        let input = r#"spec: task
name: "无分组"
---

## 完成条件

场景: 一
  测试: t1
  当 a
  那么 b
场景: 二
  测试: t2
  当 a
  那么 b
场景: 三
  测试: t3
  当 a
  那么 b
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = BddRuleGroupingLinter.lint(&doc);
        assert!(diags.iter().any(|d| d.rule == "bdd-rule-grouping"
            && d.severity == Severity::Info));
    }

    #[test]
    fn test_bdd_rule_grouping_warns_on_empty_rule() {
        let input = r#"spec: task
name: "空规则"
---

## 完成条件

### Rule: orphan-rule — 没有场景的规则
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = BddRuleGroupingLinter.lint(&doc);
        let warn = diags
            .iter()
            .find(|d| d.rule == "bdd-rule-grouping" && d.severity == Severity::Warning)
            .expect("empty rule should warn");
        assert!(warn.message.contains("orphan-rule"));
    }

    #[test]
    fn test_bdd_scenario_shape_flags_missing_when_or_then() {
        let input = r#"spec: task
name: "缺步骤"
---

## 完成条件

场景: 只有假设
  测试: t1
  假设 用户已登录
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = BddScenarioShapeLinter.lint(&doc);
        assert!(diags.iter().any(|d| d.rule == "bdd-scenario-shape"
            && d.severity == Severity::Warning));
    }

    #[test]
    fn test_bdd_scenario_shape_flags_leading_and_or_but() {
        let input = r#"spec: task
name: "首步And"
---

## 完成条件

场景: 错误开头
  测试: t1
  并且 这是一个错误的开头
  当 触发
  那么 结果
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let diags = BddScenarioShapeLinter.lint(&doc);
        assert!(diags.iter().any(|d| d.rule == "bdd-scenario-shape"
            && d.message.contains("And/But")));
    }

    #[test]
    fn test_bdd_implementation_detail_flags_ui_verbs_en_and_zh() {
        let en = r#"spec: task
name: "ui en"
---

## Completion Criteria

Scenario: login flow
  Test: t1
  Given the login page
  When the user clicks the #login button
  Then a welcome page shows
"#;
        let doc = parse_spec_from_str(en).unwrap();
        let diags = BddImplementationDetailStepLinter.lint(&doc);
        assert!(diags.iter().any(|d| d.rule == "bdd-implementation-detail-step"
            && d.severity == Severity::Info));

        let zh = r#"spec: task
name: "ui zh"
---

## 完成条件

场景: 登录流程
  测试: t1
  假设 登录页已打开
  当 用户点击登录按钮
  那么 显示欢迎页
"#;
        let doc = parse_spec_from_str(zh).unwrap();
        let diags = BddImplementationDetailStepLinter.lint(&doc);
        assert!(diags.iter().any(|d| d.rule == "bdd-implementation-detail-step"));
    }

    #[test]
    fn test_new_bdd_lints_emit_self_correction_guidance() {
        // Each bdd-* lint's suggestion must carry the four-element guidance.
        let input = r#"spec: task
name: "guidance"
---

## 完成条件

规则: NOT-kebab
### Rule: orphan-rule — 空规则
场景: 缺then
  测试: t1
  当 用户点击按钮
"#;
        let doc = parse_spec_from_str(input).unwrap();
        let mut all = Vec::new();
        all.extend(BddRuleIdLinter.lint(&doc));
        all.extend(BddRuleGroupingLinter.lint(&doc));
        all.extend(BddScenarioShapeLinter.lint(&doc));
        all.extend(BddImplementationDetailStepLinter.lint(&doc));
        assert!(
            all.iter().any(|d| d.rule == "bdd-rule-id"),
            "expected bdd-rule-id diagnostic"
        );
        for d in &all {
            let g = d.suggestion.as_deref().unwrap_or("");
            assert!(
                guidance_has_four_elements(g),
                "lint {} suggestion missing self-correction guidance element: {g}",
                d.rule
            );
        }
    }
}
