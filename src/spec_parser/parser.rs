use crate::spec_core::{
    Boundary, BoundaryCategory, Constraint, ConstraintCategory, Scenario, Section, Span,
    SpecDocument, SpecError, SpecResult, Step, TestSelector,
};
use std::path::{Path, PathBuf};

use super::keywords::{
    SectionKind, TestSelectorField, extract_params, match_scenario_header, match_section_header,
    match_step_keyword, match_test_selector, match_test_selector_field,
};
use super::meta::parse_meta;

/// Parse a .spec file from disk.
pub fn parse_spec(path: &Path) -> SpecResult<SpecDocument> {
    let content = std::fs::read_to_string(path)?;
    let mut doc = parse_spec_from_str(&content)?;
    doc.source_path = path.to_path_buf();
    Ok(doc)
}

/// Parse a .spec string into a SpecDocument.
pub fn parse_spec_from_str(input: &str) -> SpecResult<SpecDocument> {
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

    let sections = parse_body(body_lines, body_offset)?;

    Ok(SpecDocument {
        meta,
        sections,
        source_path: PathBuf::new(),
    })
}

/// Parse the body of a spec (after `---`) into sections.
fn parse_body(lines: &[&str], offset: usize) -> SpecResult<Vec<Section>> {
    let mut sections = Vec::new();
    let mut current_section: Option<(SectionKind, usize)> = None; // (kind, start_line)
    let mut section_lines: Vec<(usize, &str)> = Vec::new(); // (absolute_line, text)

    for (i, &line) in lines.iter().enumerate() {
        let abs_line = offset + i + 1; // 1-indexed

        if let Some(kind) = match_section_header(line) {
            // Flush previous section
            if let Some((prev_kind, start)) = current_section.take() {
                let section = build_section(prev_kind, &section_lines, start)?;
                sections.push(section);
                section_lines.clear();
            }
            current_section = Some((kind, abs_line));
        } else if current_section.is_some() {
            section_lines.push((abs_line, line));
        }
    }

    // Flush last section
    if let Some((kind, start)) = current_section {
        let section = build_section(kind, &section_lines, start)?;
        sections.push(section);
    }

    Ok(sections)
}

fn build_section(
    kind: SectionKind,
    lines: &[(usize, &str)],
    start_line: usize,
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
            let scenarios = parse_scenarios(lines)?;
            Ok(Section::AcceptanceCriteria { scenarios, span })
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

fn parse_scenarios(lines: &[(usize, &str)]) -> SpecResult<Vec<Scenario>> {
    let mut scenarios = Vec::new();
    let mut current_name: Option<(String, usize)> = None;
    let mut current_steps: Vec<Step> = Vec::new();
    let mut current_test_selector: Option<TestSelectorDraft> = None;
    let mut reading_test_selector_block = false;

    for &(line_num, line) in lines {
        if let Some(name) = match_scenario_header(line) {
            // Flush previous scenario
            if let Some((prev_name, start)) = current_name.take() {
                let end = current_steps.last().map_or(start, |s| s.span.end_line);
                scenarios.push(Scenario {
                    name: prev_name,
                    steps: std::mem::take(&mut current_steps),
                    test_selector: finalize_test_selector(current_test_selector.take(), end)?,
                    tags: Vec::new(),
                    span: Span::new(start, 0, end, 0),
                });
            }
            current_name = Some((name.to_string(), line_num));
            reading_test_selector_block = false;
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
        scenarios.push(Scenario {
            name,
            steps: current_steps,
            test_selector: finalize_test_selector(current_test_selector, end)?,
            tags: Vec::new(),
            span: Span::new(start, 0, end, 0),
        });
    }

    Ok(scenarios)
}

#[derive(Default)]
struct TestSelectorDraft {
    package: Option<String>,
    filter: Option<String>,
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
                assert_eq!(selector.filter, "test_parse_shorthand_test_selector_as_filter_only");
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
    fn test_serialization_roundtrip() {
        let doc = parse_spec_from_str(SAMPLE_SPEC).unwrap();
        let json = serde_json::to_string_pretty(&doc).unwrap();
        let _: SpecDocument = serde_json::from_str(&json).unwrap();
    }
}
