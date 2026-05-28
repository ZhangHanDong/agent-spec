use crate::spec_core::{
    BehaviorRule, Boundary, BoundaryCategory, Constraint, ConstraintCategory, MalformedRule,
    ReviewMode, RuleKey, RuleScope, Scenario, ScenarioMode, Section, Span, SpecDocument, SpecError,
    SpecResult, Step, TestSelector,
};
use std::path::{Path, PathBuf};

use super::keywords::{
    SectionKind, TestSelectorField, extract_params, match_depends_field, match_mode_field,
    match_review_field, match_rule_header, match_scenario_header, match_scenario_tags,
    match_section_header, match_step_keyword, match_test_selector, match_test_selector_field,
};
use super::meta::parse_meta;

/// Parse a .spec/.spec.md file from disk.
pub fn parse_spec(path: &Path) -> SpecResult<SpecDocument> {
    let content = std::fs::read_to_string(path)?;
    let stem = task_stem_from_path(path);
    let mut doc = parse_spec_from_str_with_stem(&content, &stem)?;
    doc.source_path = path.to_path_buf();
    Ok(doc)
}

/// Derive the task scope namespace from a spec path: the file stem with any
/// `.spec.md` / `.spec` suffix removed (e.g. `task-foo.spec.md` -> `task-foo`).
pub fn task_stem_from_path(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    name.strip_suffix(".spec.md")
        .or_else(|| name.strip_suffix(".spec"))
        .unwrap_or(name)
        .to_string()
}

/// Parse a .spec/.spec.md string into a SpecDocument.
///
/// Task-scope behavior rules are namespaced by an empty stem; use
/// [`parse_spec`] (from a path) or [`parse_spec_from_str_with_stem`] when the
/// task stem matters.
pub fn parse_spec_from_str(input: &str) -> SpecResult<SpecDocument> {
    parse_spec_from_str_with_stem(input, "")
}

/// Parse a .spec/.spec.md string with an explicit task-scope namespace
/// (the spec file stem), used to build `RuleScope::Task(stem)`.
pub fn parse_spec_from_str_with_stem(input: &str, task_stem: &str) -> SpecResult<SpecDocument> {
    let lines: Vec<&str> = input.lines().collect();

    // Split on front-matter separator `---`
    let separator_pos = lines.iter().position(|l| l.trim() == "---");
    let (meta_lines, body_lines, body_offset) = match separator_pos {
        Some(pos) => (&lines[..pos], &lines[pos + 1..], pos + 1),
        None => {
            // No front-matter: try to parse entire content as body
            // with a minimal default meta
            return Err(SpecError::FrontMatter(
                "missing front-matter separator '---'".into(),
            ));
        }
    };

    let meta = parse_meta(meta_lines).map_err(SpecError::FrontMatter)?;

    let sections = parse_body(body_lines, body_offset, task_stem)?;

    Ok(SpecDocument {
        meta,
        sections,
        source_path: PathBuf::new(),
    })
}

/// Parse the body of a spec (after `---`) into sections.
fn parse_body(lines: &[&str], offset: usize, task_stem: &str) -> SpecResult<Vec<Section>> {
    let mut sections = Vec::new();
    let mut current_section: Option<(SectionKind, usize)> = None; // (kind, start_line)
    let mut section_lines: Vec<(usize, &str)> = Vec::new(); // (absolute_line, text)

    for (i, &line) in lines.iter().enumerate() {
        let abs_line = offset + i + 1; // 1-indexed

        if let Some(kind) = match_section_header(line) {
            // Flush previous section
            if let Some((prev_kind, start)) = current_section.take() {
                let section = build_section(prev_kind, &section_lines, start, task_stem)?;
                sections.push(section);
                section_lines.clear();
            }
            current_section = Some((kind, abs_line));
        } else if matches!(markdown_heading_level(line), Some(1 | 2)) {
            let header = line.trim().trim_start_matches('#').trim();
            return Err(SpecError::Parse {
                message: format!(
                    "unknown top-level section header '{header}' - use only Intent/Constraints/Decisions/Boundaries/Acceptance Criteria/Out of Scope"
                ),
                span: Span::line(abs_line),
            });
        } else if current_section.is_some() {
            section_lines.push((abs_line, line));
        }
    }

    // Flush last section
    if let Some((kind, start)) = current_section {
        let section = build_section(kind, &section_lines, start, task_stem)?;
        sections.push(section);
    }

    Ok(sections)
}

fn markdown_heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|&ch| ch == '#').count();
    if level == 0 || level == trimmed.len() {
        return None;
    }
    Some(level)
}

fn build_section(
    kind: SectionKind,
    lines: &[(usize, &str)],
    start_line: usize,
    task_stem: &str,
) -> SpecResult<Section> {
    let end_line = lines.last().map_or(start_line, |(ln, _)| *ln);
    let span = Span::new(start_line, 0, end_line, 0);

    match kind {
        SectionKind::Intent => {
            let content: String = lines
                .iter()
                .map(|(_, l)| *l)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
            Ok(Section::Intent { content, span })
        }
        SectionKind::Constraints => {
            let items = parse_constraints(lines);
            Ok(Section::Constraints { items, span })
        }
        SectionKind::Decisions => {
            let items = parse_string_list(lines);
            Ok(Section::Decisions { items, span })
        }
        SectionKind::Boundaries => {
            let items = parse_boundaries(lines);
            Ok(Section::Boundaries { items, span })
        }
        SectionKind::AcceptanceCriteria => {
            let (scenarios, rules, malformed_rules) = parse_scenarios(lines, task_stem)?;
            Ok(Section::AcceptanceCriteria {
                scenarios,
                rules,
                malformed_rules,
                span,
            })
        }
        SectionKind::OutOfScope => {
            let items = lines
                .iter()
                .filter_map(|(_, l)| {
                    let trimmed = l.trim().strip_prefix('-').map(str::trim);
                    trimmed.filter(|s| !s.is_empty()).map(String::from)
                })
                .collect();
            Ok(Section::OutOfScope { items, span })
        }
    }
}

fn parse_constraints(lines: &[(usize, &str)]) -> Vec<Constraint> {
    let mut constraints = Vec::new();
    let mut category = ConstraintCategory::General;

    for &(line_num, line) in lines {
        let trimmed = line.trim();

        // Sub-section headers for constraint categories
        if trimmed.starts_with("###") || trimmed.starts_with("### ") {
            let header = trimmed.trim_start_matches('#').trim().to_lowercase();
            if header.contains("必须做") || header.contains("must") && !header.contains("not") {
                category = ConstraintCategory::Must;
            } else if header.contains("禁止") || header.contains("must not") {
                category = ConstraintCategory::MustNot;
            } else if header.contains("已定") || header.contains("decided") {
                category = ConstraintCategory::Decided;
            }
            continue;
        }

        // Bullet items
        if let Some(text) = trimmed.strip_prefix('-') {
            let text = text.trim();
            if !text.is_empty() {
                constraints.push(Constraint {
                    text: text.to_string(),
                    category,
                    span: Span::line(line_num),
                });
            }
        }
    }

    constraints
}

fn parse_string_list(lines: &[(usize, &str)]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|(_, line)| line.trim().strip_prefix('-').map(str::trim))
        .filter(|text| !text.is_empty())
        .map(String::from)
        .collect()
}

fn parse_boundaries(lines: &[(usize, &str)]) -> Vec<Boundary> {
    let mut items = Vec::new();
    let mut category = BoundaryCategory::General;

    for &(line_num, line) in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("###") || trimmed.starts_with("### ") {
            let header = trimmed.trim_start_matches('#').trim().to_lowercase();
            if header.contains("允许修改") || header.contains("allowed") || header.contains("allow")
            {
                category = BoundaryCategory::Allow;
            } else if header.contains("禁止")
                || header.contains("forbidden")
                || header.contains("must not")
                || header.contains("disallow")
            {
                category = BoundaryCategory::Deny;
            }
            continue;
        }

        if let Some(text) = trimmed.strip_prefix('-') {
            let text = text.trim();
            if !text.is_empty() {
                items.push(Boundary {
                    text: text.to_string(),
                    category,
                    span: Span::line(line_num),
                });
            }
        }
    }

    items
}

type ParsedScenarios = (Vec<Scenario>, Vec<BehaviorRule>, Vec<MalformedRule>);

fn parse_scenarios(lines: &[(usize, &str)], task_stem: &str) -> SpecResult<ParsedScenarios> {
    let mut scenarios = Vec::new();
    let mut rules: Vec<BehaviorRule> = Vec::new();
    let mut malformed_rules: Vec<MalformedRule> = Vec::new();
    let mut current_name: Option<(String, usize)> = None;
    let mut current_steps: Vec<Step> = Vec::new();
    let mut current_test_selector: Option<TestSelectorDraft> = None;
    let mut current_tags: Vec<String> = Vec::new();
    let mut current_review: ReviewMode = ReviewMode::default();
    let mut current_mode: ScenarioMode = ScenarioMode::Standard;
    let mut current_depends_on: Vec<String> = Vec::new();
    let mut reading_test_selector_block = false;
    // The active behavior rule id for scenarios that follow a `Rule:` header.
    let mut current_rule_id: Option<String> = None;
    // The rule the in-progress scenario belongs to, captured at its start so
    // a later `Rule:` header does not retroactively re-group it.
    let mut current_scenario_rule: Option<String> = None;

    // Flush helper assigns the scenario's rule and links it back to the rule.
    macro_rules! push_scenario {
        ($name:expr, $start:expr, $end:expr, $steps:expr, $selector:expr,
         $tags:expr, $review:expr, $mode:expr, $depends:expr, $rule:expr) => {{
            let rule_id: Option<String> = $rule;
            if let Some(id) = &rule_id
                && let Some(r) = rules.iter_mut().find(|r| &r.key.id == id)
            {
                r.scenario_names.push($name.clone());
            }
            scenarios.push(Scenario {
                name: $name,
                steps: $steps,
                test_selector: $selector,
                tags: $tags,
                review: $review,
                mode: $mode,
                depends_on: $depends,
                rule: rule_id,
                span: Span::new($start, 0, $end, 0),
            });
        }};
    }

    for &(line_num, line) in lines {
        if let Some(raw) = match_rule_header(line) {
            // A `Rule:` header sets the active rule for subsequent scenarios.
            // Invalid (non-kebab) ids are dropped here; the `bdd-rule-id` lint
            // reports them from the raw source.
            let (id, name) = parse_rule_header_content(raw);
            match id {
                Some(id) => {
                    rules.push(BehaviorRule {
                        key: RuleKey {
                            scope: RuleScope::Task(task_stem.to_string()),
                            id: id.clone(),
                        },
                        name,
                        scenario_names: Vec::new(),
                        span: Span::line(line_num),
                    });
                    current_rule_id = Some(id);
                }
                None => {
                    current_rule_id = None;
                    malformed_rules.push(MalformedRule {
                        raw: raw.to_string(),
                        span: Span::line(line_num),
                    });
                }
            }
            continue;
        }
        if let Some(name) = match_scenario_header(line) {
            // Flush previous scenario
            if let Some((prev_name, start)) = current_name.take() {
                let end = current_steps.last().map_or(start, |s| s.span.end_line);
                push_scenario!(
                    prev_name,
                    start,
                    end,
                    std::mem::take(&mut current_steps),
                    finalize_test_selector(current_test_selector.take(), end)?,
                    std::mem::take(&mut current_tags),
                    std::mem::take(&mut current_review),
                    std::mem::take(&mut current_mode),
                    std::mem::take(&mut current_depends_on),
                    current_scenario_rule.take()
                );
            }
            current_name = Some((name.to_string(), line_num));
            current_tags = Vec::new();
            current_review = ReviewMode::default();
            current_mode = ScenarioMode::Standard;
            current_depends_on = Vec::new();
            current_scenario_rule = current_rule_id.clone();
            reading_test_selector_block = false;
        } else if let Some(tags) = match_scenario_tags(line) {
            if current_name.is_some() {
                current_tags = tags;
            }
        } else if let Some(review_value) = match_review_field(line) {
            if current_name.is_some() {
                let lower = review_value.to_lowercase();
                if lower == "human" {
                    current_review = ReviewMode::Human;
                } else {
                    current_review = ReviewMode::Auto;
                }
            }
        } else if let Some(mode_value) = match_mode_field(line) {
            if current_name.is_some() {
                let lower = mode_value.to_lowercase();
                if lower == "optimize" {
                    current_mode = ScenarioMode::Optimize;
                } else {
                    current_mode = ScenarioMode::Standard;
                }
            }
        } else if let Some(depends_value) = match_depends_field(line) {
            if current_name.is_some() {
                current_depends_on = depends_value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        } else if let Some(selector) = match_test_selector(line) {
            if current_name.is_some() {
                let draft = current_test_selector.get_or_insert_with(TestSelectorDraft::default);
                if selector.is_empty() {
                    reading_test_selector_block = true;
                } else {
                    draft.filter = Some(selector.to_string());
                    reading_test_selector_block = false;
                }
            }
        } else if reading_test_selector_block {
            if let Some((field, value)) = match_test_selector_field(line) {
                let draft = current_test_selector.get_or_insert_with(TestSelectorDraft::default);
                match field {
                    TestSelectorField::Package => draft.package = Some(value.to_string()),
                    TestSelectorField::Filter => draft.filter = Some(value.to_string()),
                    TestSelectorField::Level => draft.level = Some(value.to_string()),
                    TestSelectorField::TestDouble => draft.test_double = Some(value.to_string()),
                    TestSelectorField::Targets => draft.targets = Some(value.to_string()),
                }
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            reading_test_selector_block = false;
        }

        if let Some((kind, text)) = match_step_keyword(line) {
            let params = extract_params(text);
            current_steps.push(Step {
                kind,
                text: text.to_string(),
                params,
                table: Vec::new(),
                span: Span::line(line_num),
            });
        } else if let Some(row) = parse_table_row(line)
            && let Some(step) = current_steps.last_mut()
        {
            step.table.push(row);
            step.span.end_line = line_num;
        }
        // Ignore blank lines and non-step text inside scenarios
    }

    // Flush last scenario
    if let Some((name, start)) = current_name {
        let end = current_steps.last().map_or(start, |s| s.span.end_line);
        push_scenario!(
            name,
            start,
            end,
            current_steps,
            finalize_test_selector(current_test_selector, end)?,
            current_tags,
            current_review,
            current_mode,
            current_depends_on,
            current_scenario_rule.take()
        );
    }

    Ok((scenarios, rules, malformed_rules))
}

/// Split a `Rule:` header's raw content into `(id, display_name)`.
///
/// The id must be an explicit kebab-case identifier (`^[a-z][a-z0-9-]*$`).
/// Separator between id and display name is an em dash (`—`) or two-or-more
/// spaces. No auto-slugify: if the leading token is not a valid id, returns
/// `(None, raw)` so the `bdd-rule-id` lint can flag it and no rule is created.
fn parse_rule_header_content(raw: &str) -> (Option<String>, String) {
    let raw = raw.trim();
    let (id_part, name_part) = if let Some(idx) = raw.find('—') {
        (
            raw[..idx].trim(),
            raw[idx + '—'.len_utf8()..].trim(),
        )
    } else if let Some(idx) = raw.find("  ") {
        (raw[..idx].trim(), raw[idx..].trim())
    } else {
        (raw, "")
    };

    if is_valid_rule_id(id_part) {
        let name = if name_part.is_empty() {
            id_part.to_string()
        } else {
            name_part.to_string()
        };
        (Some(id_part.to_string()), name)
    } else {
        (None, raw.to_string())
    }
}

/// Validates a behavior rule id against `^[a-z][a-z0-9-]*$`.
fn is_valid_rule_id(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Default)]
struct TestSelectorDraft {
    package: Option<String>,
    filter: Option<String>,
    level: Option<String>,
    test_double: Option<String>,
    targets: Option<String>,
}

fn finalize_test_selector(
    draft: Option<TestSelectorDraft>,
    line_num: usize,
) -> SpecResult<Option<TestSelector>> {
    let Some(draft) = draft else {
        return Ok(None);
    };

    let Some(filter) = draft.filter else {
        return Err(SpecError::Parse {
            message: "test selector is missing required `Filter:` / `过滤:` field".into(),
            span: Span::line(line_num),
        });
    };

    Ok(Some(TestSelector {
        filter,
        package: draft.package,
        level: draft.level,
        test_double: draft.test_double,
        targets: draft.targets,
    }))
}

fn parse_table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return None;
    }

    let row: Vec<String> = trimmed
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .map(String::from)
        .collect();

    if row.is_empty() || row.iter().all(|cell| cell.is_empty()) {
        None
    } else {
        Some(row)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_core::StepKind;

    const SAMPLE_SPEC: &str = r#"spec: task
name: "退款功能"
inherits: project
tags: [payment, refund]
---

## 意图

为支付网关添加退款功能，支持全额和部分退款。

## 约束

- 退款金额不得超过原始交易金额
- 退款操作需要管理员权限
- 退款必须在原交易后 90 天内发起

## 验收标准

场景: 全额退款
  假设 存在一笔金额为 "100.00" 元的已完成交易 "TXN-001"
  并且 当前用户具有管理员权限
  当 用户对 "TXN-001" 发起全额退款
  那么 退款状态变为 "processing"
  并且 原始交易状态变为 "refunding"

场景: 退款拒绝 - 超期
  假设 存在一笔 91 天前完成的交易 "TXN-003"
  当 用户对 "TXN-003" 发起退款
  那么 系统拒绝退款
  并且 返回错误信息包含 "超过退款期限"

## 排除范围

- 登录功能
- 密码重置
"#;

    #[test]
    fn test_parse_full_spec() {
        let doc = parse_spec_from_str(SAMPLE_SPEC).unwrap();

        assert_eq!(doc.meta.name, "退款功能");
        assert_eq!(doc.meta.level, crate::spec_core::SpecLevel::Task);
        assert_eq!(doc.meta.inherits, Some("project".into()));
        assert_eq!(doc.meta.tags, vec!["payment", "refund"]);

        // Should have 4 sections: intent, constraints, acceptance, out-of-scope
        assert_eq!(doc.sections.len(), 4);

        // Intent
        match &doc.sections[0] {
            Section::Intent { content, .. } => {
                assert!(content.contains("退款功能"));
            }
            other => panic!("expected Intent, got {other:?}"),
        }

        // Constraints
        match &doc.sections[1] {
            Section::Constraints { items, .. } => {
                assert_eq!(items.len(), 3);
                assert!(items[0].text.contains("退款金额"));
            }
            other => panic!("expected Constraints, got {other:?}"),
        }

        // Scenarios
        match &doc.sections[2] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 2);

                let s1 = &scenarios[0];
                assert_eq!(s1.name, "全额退款");
                assert_eq!(s1.steps.len(), 5);
                assert_eq!(s1.steps[0].kind, StepKind::Given);
                assert_eq!(s1.steps[0].params, vec!["100.00", "TXN-001"]);
                assert_eq!(s1.steps[1].kind, StepKind::And);
                assert_eq!(s1.steps[2].kind, StepKind::When);
                assert_eq!(s1.steps[2].params, vec!["TXN-001"]);
                assert_eq!(s1.steps[3].kind, StepKind::Then);
                assert_eq!(s1.steps[4].kind, StepKind::And);

                let s2 = &scenarios[1];
                assert_eq!(s2.name, "退款拒绝 - 超期");
                assert_eq!(s2.steps.len(), 4);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }

        // Out of scope
        match &doc.sections[3] {
            Section::OutOfScope { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], "登录功能");
            }
            other => panic!("expected OutOfScope, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_english_spec() {
        let input = r#"spec: task
name: "User Registration"
---

## Intent

Implement user registration API.

## Constraints

- Passwords must be hashed with bcrypt
- Email must be unique

## Acceptance Criteria

Scenario: Successful registration
  Given no user with email "alice@example.com" exists
  When POST /api/v1/auth/register with email "alice@example.com"
  Then response status should be 201
  And response body should contain "id"
"#;
        let doc = parse_spec_from_str(input).unwrap();
        assert_eq!(doc.meta.name, "User Registration");
        assert_eq!(doc.sections.len(), 3);

        match &doc.sections[2] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 1);
                assert_eq!(scenarios[0].name, "Successful registration");
                assert_eq!(scenarios[0].steps.len(), 4);
                assert_eq!(scenarios[0].steps[0].params, vec!["alice@example.com"]);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_mixed_lang_spec() {
        let input = r#"spec: task
name: "混合语言测试"
---

## 验收标准

Scenario: 混合场景
  Given 用户已登录
  当 用户点击 "submit" 按钮
  Then 页面应显示成功消息
  并且 数据库中有新记录
"#;
        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                let s = &scenarios[0];
                assert_eq!(s.steps.len(), 4);
                assert_eq!(s.steps[0].kind, StepKind::Given);
                assert_eq!(s.steps[1].kind, StepKind::When);
                assert_eq!(s.steps[2].kind, StepKind::Then);
                assert_eq!(s.steps[3].kind, StepKind::And);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_step_table_and_preserve_json_output() {
        let input = r#"spec: task
name: "表格测试"
---

## 验收标准

场景: 注册请求
  当 发送 POST /api/v1/auth/register 请求:
    | field    | value             |
    | email    | alice@example.com |
    | password | Str0ng!Pass#2024  |
  那么 响应状态码应为 201
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                let when_step = &scenarios[0].steps[0];
                assert_eq!(when_step.kind, StepKind::When);
                assert_eq!(when_step.table.len(), 3);
                assert_eq!(when_step.table[0], vec!["field", "value"]);
                assert_eq!(when_step.table[1], vec!["email", "alice@example.com"]);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }

        let json = serde_json::to_string_pretty(&doc).unwrap();
        assert!(json.contains("\"table\""));
        assert!(json.contains("alice@example.com"));
        assert!(json.contains("Str0ng!Pass#2024"));
    }

    #[test]
    fn test_parse_scenario_without_table_stays_unchanged() {
        let input = r#"spec: task
name: "普通场景"
---

## 验收标准

场景: 无表格
  假设 用户已登录
  当 用户点击提交
  那么 页面显示成功
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                let scenario = &scenarios[0];
                assert_eq!(scenario.steps.len(), 3);
                assert!(scenario.steps.iter().all(|step| step.table.is_empty()));
                assert_eq!(scenario.steps[1].text, "用户点击提交");
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_task_contract_sections() {
        let input = r#"spec: task
name: "Contract"
---

## Intent

Implement the task safely.

## Decisions

- Use existing parser module

## Boundaries

### Allowed Changes
- crates/spec-parser/**

### Forbidden
- Do not modify crates/spec-verify/**

## Completion Criteria

Scenario: Parse succeeds
  Given a valid contract
  When the parser reads it
  Then the parser should succeed
"#;

        let doc = parse_spec_from_str(input).unwrap();
        assert_eq!(doc.sections.len(), 4);

        match &doc.sections[1] {
            Section::Decisions { items, .. } => {
                assert_eq!(items, &vec!["Use existing parser module".to_string()]);
            }
            other => panic!("expected Decisions, got {other:?}"),
        }

        match &doc.sections[2] {
            Section::Boundaries { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].category, BoundaryCategory::Allow);
                assert_eq!(items[1].category, BoundaryCategory::Deny);
            }
            other => panic!("expected Boundaries, got {other:?}"),
        }

        match &doc.sections[3] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 1);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_scenario_with_explicit_test_selector() {
        let input = r#"spec: task
name: "绑定测试"
---

## 完成条件

场景: 显式绑定
  测试: test_parse_scenario_with_explicit_test_selector
  假设 某个场景声明测试选择器
  当 parser 解析该场景
  那么 AST 中保留该 selector
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 1);
                assert_eq!(
                    scenarios[0]
                        .test_selector
                        .as_ref()
                        .map(|selector| selector.filter.as_str()),
                    Some("test_parse_scenario_with_explicit_test_selector")
                );
                assert_eq!(scenarios[0].steps.len(), 3);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }

        let json = serde_json::to_string_pretty(&doc).unwrap();
        assert!(json.contains("\"test_selector\""));
        assert!(json.contains("\"filter\""));
        assert!(json.contains("test_parse_scenario_with_explicit_test_selector"));
    }

    #[test]
    fn test_parse_structured_test_selector_block() {
        let input = r#"spec: task
name: "结构化绑定"
---

## 完成条件

场景: 结构化绑定
  测试:
    包: spec-parser
    过滤: test_parse_structured_test_selector_block
  假设 某个场景声明结构化测试选择器
  当 parser 解析该场景
  那么 AST 中保留结构化字段
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 1);
                let selector = scenarios[0].test_selector.as_ref().unwrap();
                assert_eq!(selector.package.as_deref(), Some("spec-parser"));
                assert_eq!(selector.filter, "test_parse_structured_test_selector_block");
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }

        let json = serde_json::to_string_pretty(&doc).unwrap();
        assert!(json.contains("\"package\""));
        assert!(json.contains("\"spec-parser\""));
        assert!(json.contains("\"filter\""));
        assert!(json.contains("test_parse_structured_test_selector_block"));
    }

    #[test]
    fn test_parse_scenario_verification_metadata_fields() {
        let input = r#"spec: task
name: "验证元数据"
---

## 完成条件

场景: 结构化验证强度
  测试:
    包: agent-spec
    过滤: test_parse_scenario_verification_metadata_fields
    层级: integration
    替身: local_http_stub
    命中: commands/update
  假设 某个场景声明验证元数据
  当 parser 解析该场景
  那么 AST 中保留这些字段
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                let selector = scenarios[0].test_selector.as_ref().unwrap();
                assert_eq!(selector.package.as_deref(), Some("agent-spec"));
                assert_eq!(
                    selector.filter,
                    "test_parse_scenario_verification_metadata_fields"
                );
                assert_eq!(selector.level.as_deref(), Some("integration"));
                assert_eq!(selector.test_double.as_deref(), Some("local_http_stub"));
                assert_eq!(selector.targets.as_deref(), Some("commands/update"));
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }

        let json = serde_json::to_string_pretty(&doc).unwrap();
        assert!(json.contains("\"level\""));
        assert!(json.contains("\"test_double\""));
        assert!(json.contains("\"targets\""));
    }

    #[test]
    fn test_parse_english_verification_metadata_fields() {
        let input = r#"spec: task
name: "verification metadata"
---

## Completion Criteria

Scenario: verification metadata
  Test:
    Package: agent-spec
    Filter: test_parse_english_verification_metadata_fields
    Level: integration
    Test Double: local_http_stub
    Targets: commands/update
  Given a scenario declares verification metadata
  When the parser reads it
  Then the AST keeps the metadata
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                let selector = scenarios[0].test_selector.as_ref().unwrap();
                assert_eq!(selector.level.as_deref(), Some("integration"));
                assert_eq!(selector.test_double.as_deref(), Some("local_http_stub"));
                assert_eq!(selector.targets.as_deref(), Some("commands/update"));
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_existing_specs_without_verification_metadata_remain_valid() {
        let input = r#"spec: task
name: "legacy selector"
---

## Completion Criteria

Scenario: legacy selector
  Test:
    Package: agent-spec
    Filter: test_existing_specs_without_verification_metadata_remain_valid
  Given a legacy spec
  When the parser reads it
  Then the selector remains valid
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                let selector = scenarios[0].test_selector.as_ref().unwrap();
                assert_eq!(selector.package.as_deref(), Some("agent-spec"));
                assert_eq!(
                    selector.filter,
                    "test_existing_specs_without_verification_metadata_remain_valid"
                );
                assert_eq!(selector.level, None);
                assert_eq!(selector.test_double, None);
                assert_eq!(selector.targets, None);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_shorthand_test_selector_as_filter_only() {
        let input = r#"spec: task
name: "单行绑定"
---

## 完成条件

场景: 单行绑定
  测试: test_parse_shorthand_test_selector_as_filter_only
  假设 某个场景继续使用单行测试绑定
  当 parser 解析该场景
  那么 filter 字段被保留
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                let selector = scenarios[0].test_selector.as_ref().unwrap();
                assert_eq!(
                    selector.filter,
                    "test_parse_shorthand_test_selector_as_filter_only"
                );
                assert_eq!(selector.package, None);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_missing_front_matter() {
        let input = "## Intent\nSome content\n";
        let result = parse_spec_from_str(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_top_level_section_header_is_rejected() {
        let input = r#"spec: task
name: "未知章节"
---

## Intent

Describe the task.

## Milestones

- phase 1
"#;

        let err = parse_spec_from_str(input).unwrap_err();
        match err {
            SpecError::Parse { message, span } => {
                assert!(message.contains("unknown top-level section header"));
                assert_eq!(span.start_line, 9);
            }
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn test_markdown_heading_scenarios_and_test_selectors_are_accepted() {
        let input = r#"spec: task
name: "Markdown Scenario"
---

## Completion Criteria

### Scenario: Happy path
  ### Test: test_markdown_heading_scenarios_and_test_selectors_are_accepted
  Given valid input
  When parser reads the scenario
  Then the scenario is preserved
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 1);
                assert_eq!(scenarios[0].name, "Happy path");
                assert_eq!(
                    scenarios[0]
                        .test_selector
                        .as_ref()
                        .map(|selector| selector.filter.as_str()),
                    Some("test_markdown_heading_scenarios_and_test_selectors_are_accepted")
                );
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let doc = parse_spec_from_str(SAMPLE_SPEC).unwrap();
        let json = serde_json::to_string_pretty(&doc).unwrap();
        let _: SpecDocument = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_parse_mode_field_in_scenario() {
        let input = r#"spec: task
name: "模式测试"
---

## 完成条件

场景: 优化场景
  模式: optimize
  测试: test_parse_mode_field_in_scenario
  假设 某个场景声明 optimize 模式
  当 parser 解析该场景
  那么 AST 中 mode 字段为 Optimize
"#;
        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 1);
                assert_eq!(scenarios[0].mode, crate::spec_core::ScenarioMode::Optimize);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_mode_field_english() {
        let input = r#"spec: task
name: "mode test"
---

## Completion Criteria

Scenario: optimize scenario
  Mode: optimize
  Given an optimize-mode scenario
  When parser reads it
  Then mode is Optimize
"#;
        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios[0].mode, crate::spec_core::ScenarioMode::Optimize);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_mode_field_standard_is_default() {
        let input = r#"spec: task
name: "default mode"
---

## Completion Criteria

Scenario: standard scenario
  Mode: standard
  Given a standard scenario
  When parser reads it
  Then mode is Standard

Scenario: no mode declared
  Given no mode field
  When parser reads it
  Then mode defaults to Standard
"#;
        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios[0].mode, crate::spec_core::ScenarioMode::Standard);
                assert_eq!(scenarios[1].mode, crate::spec_core::ScenarioMode::Standard);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_depends_field_in_scenario() {
        let input = r#"spec: task
name: "依赖测试"
---

## 完成条件

场景: 用户注册
  假设 注册表单已打开
  当 用户提交注册
  那么 注册成功

场景: 用户登录
  前置: 用户注册
  假设 已有注册用户
  当 用户登录
  那么 登录成功
"#;
        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 2);
                assert!(scenarios[0].depends_on.is_empty());
                assert_eq!(scenarios[1].depends_on, vec!["用户注册"]);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_depends_field_multiple() {
        let input = r#"spec: task
name: "multi depends"
---

## Completion Criteria

Scenario: A
  Given A
  When A
  Then A

Scenario: B
  Given B
  When B
  Then B

Scenario: C
  Depends: A, B
  Given C depends on A and B
  When parser reads it
  Then depends_on contains both
"#;
        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios[2].depends_on, vec!["A", "B"]);
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }

    // ---- BDD semantics v1: Rule / Example parsing ----

    use crate::spec_core::{BehaviorRule, RuleScope, Section as Sec};

    fn rules_of(doc: &SpecDocument) -> Vec<BehaviorRule> {
        doc.sections
            .iter()
            .find_map(|s| match s {
                Sec::AcceptanceCriteria { rules, .. } => Some(rules.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn scenarios_of(doc: &SpecDocument) -> Vec<Scenario> {
        doc.sections
            .iter()
            .find_map(|s| match s {
                Sec::AcceptanceCriteria { scenarios, .. } => Some(scenarios.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn test_parse_rule_header_creates_behavior_rule() {
        let input = r#"spec: task
name: "鉴权"
---

## 完成条件

### Rule: auth-must-not-leak — 鉴权失败不得泄漏内部错误
场景: 失败返回稳定错误
  测试: test_auth_stable_error
  假设 鉴权失败
  当 返回响应
  那么 不包含内部堆栈
"#;
        let doc = parse_spec_from_str_with_stem(input, "task-auth").unwrap();
        let rules = rules_of(&doc);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].key.id, "auth-must-not-leak");
        assert_eq!(rules[0].key.scope, RuleScope::Task("task-auth".into()));
        assert_eq!(rules[0].name, "鉴权失败不得泄漏内部错误");
        assert_eq!(rules[0].scenario_names, vec!["失败返回稳定错误".to_string()]);

        let scenarios = scenarios_of(&doc);
        assert_eq!(scenarios[0].rule.as_deref(), Some("auth-must-not-leak"));
    }

    #[test]
    fn test_parse_rule_header_without_display_name() {
        let input = r#"spec: task
name: "退款"
---

## 完成条件

### Rule: refund-must-be-idempotent
场景: 重复退款只生效一次
  测试: test_refund_idempotent
  假设 已退款
  当 再次退款
  那么 不重复扣减
"#;
        let doc = parse_spec_from_str_with_stem(input, "task-refund").unwrap();
        let rules = rules_of(&doc);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].key.id, "refund-must-be-idempotent");
        assert_eq!(rules[0].name, "refund-must-be-idempotent");
    }

    #[test]
    fn test_parse_chinese_rule_alias() {
        let input = r#"spec: task
name: "促销"
---

## 完成条件

规则: vip-discount-priority — VIP 折扣优先级高于促销
示例: VIP 用户折扣优先
  测试: test_vip_priority
  假设 用户是 VIP
  当 同时存在促销
  那么 应用 VIP 折扣
"#;
        let doc = parse_spec_from_str_with_stem(input, "task-promo").unwrap();
        let rules = rules_of(&doc);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].key.id, "vip-discount-priority");
        assert_eq!(rules[0].name, "VIP 折扣优先级高于促销");
        let scenarios = scenarios_of(&doc);
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].name, "VIP 用户折扣优先");
        assert_eq!(scenarios[0].rule.as_deref(), Some("vip-discount-priority"));
    }

    #[test]
    fn test_parse_example_alias_as_scenario() {
        let input = r#"spec: task
name: "提现"
---

## 完成条件

Example: 余额充足时提现成功
  测试: test_withdraw_ok
  假设 余额 "200"
  当 提现 "100"
  那么 成功

示例: 余额不足时提现失败
  测试: test_withdraw_insufficient
  假设 余额 "50"
  当 提现 "100"
  那么 拒绝
"#;
        let doc = parse_spec_from_str_with_stem(input, "task-withdraw").unwrap();
        let scenarios = scenarios_of(&doc);
        assert_eq!(scenarios.len(), 2);
        assert_eq!(scenarios[0].name, "余额充足时提现成功");
        assert_eq!(scenarios[1].name, "余额不足时提现失败");
        // No new "Example" AST node — both are Scenario, serialized as such.
        let json = serde_json::to_string(&doc).unwrap();
        assert!(!json.contains("\"Example\""));
        assert!(!json.contains("\"example\""));
    }

    #[test]
    fn test_legacy_spec_without_rule_compat() {
        // A spec with no Rule line: scenarios carry rule == None, no rules emitted.
        let doc = parse_spec_from_str(SAMPLE_SPEC).unwrap();
        let scenarios = scenarios_of(&doc);
        assert!(!scenarios.is_empty());
        assert!(scenarios.iter().all(|s| s.rule.is_none()));
        assert!(rules_of(&doc).is_empty());
    }

    #[test]
    fn test_rule_scope_serializes_to_json() {
        let input = r#"spec: task
name: "鉴权"
---

## 完成条件

### Rule: auth-must-not-leak — 鉴权失败不得泄漏内部错误
场景: 失败返回稳定错误
  测试: test_auth_stable_error
  假设 鉴权失败
  当 返回响应
  那么 不包含内部堆栈
"#;
        let doc = parse_spec_from_str_with_stem(input, "task-auth").unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        // RuleScope::Task(stem) serializes as { "task": "<stem>" }
        assert!(json.contains(r#""scope":{"task":"task-auth"}"#));
        assert!(json.contains(r#""id":"auth-must-not-leak""#));
        assert!(json.contains(r#""rule":"auth-must-not-leak""#));
    }

    #[test]
    fn test_capability_scope_is_reserved_in_v1() {
        // Capability / Project scope are declared and (de)serialize cleanly,
        // but the v1 parser never produces them.
        let doc = parse_spec_from_str_with_stem(
            r#"spec: task
name: "鉴权"
---

## 完成条件

### Rule: auth-ok
场景: 通过
  测试: test_ok
  假设 a
  当 b
  那么 c
"#,
            "task-auth",
        )
        .unwrap();
        assert!(
            rules_of(&doc)
                .iter()
                .all(|r| matches!(r.key.scope, RuleScope::Task(_)))
        );

        // Reserved variants round-trip through serde.
        let cap = RuleScope::Capability("ecosystem-import".into());
        let cap_json = serde_json::to_string(&cap).unwrap();
        assert_eq!(cap_json, r#"{"capability":"ecosystem-import"}"#);
        let back: RuleScope = serde_json::from_str(&cap_json).unwrap();
        assert_eq!(back, cap);
        let proj_json = serde_json::to_string(&RuleScope::Project).unwrap();
        assert_eq!(proj_json, r#""project""#);
        let proj_back: RuleScope = serde_json::from_str(&proj_json).unwrap();
        assert_eq!(proj_back, RuleScope::Project);
    }

    #[test]
    fn test_json_output_additive_only() {
        // A legacy spec (no Rule) must not emit `rules` or scenario `rule`
        // keys, so v0.2.7 consumers see an unchanged shape.
        let doc = parse_spec_from_str(SAMPLE_SPEC).unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        assert!(!json.contains("\"rules\""));
        assert!(!json.contains("\"rule\""));
    }

    #[test]
    fn test_parse_review_field_in_scenario() {
        let input = r#"spec: task
name: "审核测试"
---

## 完成条件

场景: 需要人类审核
  审核: human
  测试: test_parse_review_field_in_scenario
  假设 某个场景声明审核为 human
  当 parser 解析该场景
  那么 AST 中 review 字段为 Human

场景: 默认自动审核
  测试: test_default_auto_review
  假设 某个场景不声明审核字段
  当 parser 解析该场景
  那么 AST 中 review 字段为 Auto
"#;

        let doc = parse_spec_from_str(input).unwrap();
        match &doc.sections[0] {
            Section::AcceptanceCriteria { scenarios, .. } => {
                assert_eq!(scenarios.len(), 2);
                assert_eq!(
                    scenarios[0].review,
                    crate::spec_core::ReviewMode::Human,
                    "scenario with '审核: human' should have ReviewMode::Human"
                );
                assert_eq!(
                    scenarios[1].review,
                    crate::spec_core::ReviewMode::Auto,
                    "scenario without review field should default to ReviewMode::Auto"
                );
            }
            other => panic!("expected AcceptanceCriteria, got {other:?}"),
        }
    }
}
