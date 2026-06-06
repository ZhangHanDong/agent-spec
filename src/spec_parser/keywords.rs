use crate::spec_core::StepKind;

/// Bilingual keyword recognition for BDD steps.
pub fn match_step_keyword(line: &str) -> Option<(StepKind, &str)> {
    let trimmed = line.trim();

    // Order matters: check longer keywords first to avoid partial matches.
    let mappings: &[(&str, StepKind)] = &[
        // Chinese
        ("假设 ", StepKind::Given),
        ("假设", StepKind::Given),
        ("当 ", StepKind::When),
        ("当", StepKind::When),
        ("那么 ", StepKind::Then),
        ("那么", StepKind::Then),
        ("并且 ", StepKind::And),
        ("并且", StepKind::And),
        ("但是 ", StepKind::But),
        ("但是", StepKind::But),
        // English (case-insensitive check below)
    ];

    for &(kw, kind) in mappings {
        if let Some(rest) = trimmed.strip_prefix(kw) {
            return Some((kind, rest.trim()));
        }
    }

    // English keywords (case-insensitive)
    let en_mappings: &[(&str, StepKind)] = &[
        ("given ", StepKind::Given),
        ("when ", StepKind::When),
        ("then ", StepKind::Then),
        ("and ", StepKind::And),
        ("but ", StepKind::But),
    ];

    let lower = trimmed.to_lowercase();
    for &(kw, kind) in en_mappings {
        if lower.starts_with(kw) {
            let rest = trimmed[kw.len()..].trim();
            return Some((kind, rest));
        }
    }

    None
}

/// Bilingual section header recognition.
pub fn match_section_header(line: &str) -> Option<SectionKind> {
    let trimmed = line.trim().trim_start_matches('#').trim();
    let lower = trimmed.to_lowercase();

    if lower.starts_with("意图") || lower.starts_with("intent") {
        Some(SectionKind::Intent)
    } else if lower.starts_with("约束") || lower.starts_with("constraint") {
        Some(SectionKind::Constraints)
    } else if lower.starts_with("已定决策")
        || lower.starts_with("决策")
        || lower.starts_with("decision")
    {
        Some(SectionKind::Decisions)
    } else if lower.starts_with("边界")
        || lower.starts_with("boundaries")
        || lower.starts_with("boundary")
    {
        Some(SectionKind::Boundaries)
    } else if lower.starts_with("验收标准")
        || lower.starts_with("acceptance criter")
        || lower.starts_with("完成条件")
        || lower.starts_with("completion criter")
    {
        Some(SectionKind::AcceptanceCriteria)
    } else if lower.starts_with("排除范围") || lower.starts_with("out of scope") {
        Some(SectionKind::OutOfScope)
    } else if lower.starts_with("问题")
        || lower.starts_with("待澄清")
        || lower.starts_with("questions")
    {
        Some(SectionKind::Questions)
    } else {
        None
    }
}

/// Scenario header recognition. `Example:` / `例子:` / `示例:` are accepted as
/// aliases of `Scenario:` / `场景:` (Cucumber treats Example and Scenario as
/// synonyms); the parser stores both as `Scenario`.
pub fn match_scenario_header(line: &str) -> Option<&str> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    // Chinese scenario / example aliases
    for prefix in ["场景:", "场景：", "示例:", "示例：", "例子:", "例子："] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some(rest.trim());
        }
    }

    // English keywords, accepting both ASCII `:` and full-width `：`
    // (common when authoring with a CJK IME on).
    let lower = trimmed.to_lowercase();
    for word in ["scenario", "example"] {
        for colon in [":", "："] {
            let prefix = format!("{word}{colon}");
            if lower.starts_with(&prefix) {
                return Some(trimmed[prefix.len()..].trim());
            }
        }
    }

    None
}

/// Behavior rule header recognition: `Rule:` / `规则:`.
/// Returns the raw content after the colon (id and optional display name);
/// the parser is responsible for splitting and validating the kebab-case id.
pub fn match_rule_header(line: &str) -> Option<&str> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    if let Some(rest) = trimmed
        .strip_prefix("规则:")
        .or_else(|| trimmed.strip_prefix("规则："))
    {
        return Some(rest.trim());
    }
    let lower = trimmed.to_lowercase();
    for colon in [":", "："] {
        let prefix = format!("rule{colon}");
        if lower.starts_with(&prefix) {
            return Some(trimmed[prefix.len()..].trim());
        }
    }
    None
}

/// Scenario-level test selector binding.
pub fn match_test_selector(line: &str) -> Option<&str> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    if let Some(rest) = trimmed
        .strip_prefix("测试:")
        .or_else(|| trimmed.strip_prefix("测试："))
    {
        Some(rest.trim())
    } else {
        let lower = trimmed.to_lowercase();
        if lower.starts_with("test:") {
            Some(trimmed["test:".len()..].trim())
        } else {
            None
        }
    }
}

/// Scenario-level tags line recognition (e.g., `标签: [critical]` or `Tags: [critical]`).
pub fn match_scenario_tags(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    let value = if let Some(rest) = trimmed
        .strip_prefix("标签:")
        .or_else(|| trimmed.strip_prefix("标签："))
    {
        Some(rest.trim())
    } else {
        let lower = trimmed.to_lowercase();
        if lower.starts_with("tags:") {
            Some(trimmed["tags:".len()..].trim())
        } else {
            None
        }
    };

    value.map(|v| {
        let v = v.trim_start_matches('[').trim_end_matches(']');
        v.split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestSelectorField {
    Package,
    Filter,
    Level,
    TestDouble,
    Targets,
}

/// Structured fields under a `Test:` / `测试:` selector block.
pub fn match_test_selector_field(line: &str) -> Option<(TestSelectorField, &str)> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    if let Some(rest) = trimmed
        .strip_prefix("包:")
        .or_else(|| trimmed.strip_prefix("包："))
    {
        return Some((TestSelectorField::Package, rest.trim()));
    }
    if let Some(rest) = trimmed
        .strip_prefix("过滤:")
        .or_else(|| trimmed.strip_prefix("过滤："))
    {
        return Some((TestSelectorField::Filter, rest.trim()));
    }
    if let Some(rest) = trimmed
        .strip_prefix("层级:")
        .or_else(|| trimmed.strip_prefix("层级："))
    {
        return Some((TestSelectorField::Level, rest.trim()));
    }
    if let Some(rest) = trimmed
        .strip_prefix("替身:")
        .or_else(|| trimmed.strip_prefix("替身："))
    {
        return Some((TestSelectorField::TestDouble, rest.trim()));
    }
    if let Some(rest) = trimmed
        .strip_prefix("命中:")
        .or_else(|| trimmed.strip_prefix("命中："))
    {
        return Some((TestSelectorField::Targets, rest.trim()));
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("package:") {
        return Some((
            TestSelectorField::Package,
            trimmed["package:".len()..].trim(),
        ));
    }
    if lower.starts_with("filter:") {
        return Some((TestSelectorField::Filter, trimmed["filter:".len()..].trim()));
    }
    if lower.starts_with("level:") {
        return Some((TestSelectorField::Level, trimmed["level:".len()..].trim()));
    }
    if lower.starts_with("test double:") {
        return Some((
            TestSelectorField::TestDouble,
            trimmed["test double:".len()..].trim(),
        ));
    }
    if lower.starts_with("targets:") {
        return Some((
            TestSelectorField::Targets,
            trimmed["targets:".len()..].trim(),
        ));
    }

    None
}

/// Review field recognition: `审核: human` / `Review: human`.
/// Returns Some("human") or Some("auto"), or None if not a review line.
pub fn match_review_field(line: &str) -> Option<&str> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    if let Some(rest) = trimmed
        .strip_prefix("审核:")
        .or_else(|| trimmed.strip_prefix("审核："))
    {
        return Some(rest.trim());
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("review:") {
        return Some(trimmed["review:".len()..].trim());
    }

    None
}

/// Mode field recognition: `模式: optimize` / `Mode: optimize`.
/// Returns Some("optimize") or Some("standard"), or None if not a mode line.
pub fn match_mode_field(line: &str) -> Option<&str> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    if let Some(rest) = trimmed
        .strip_prefix("模式:")
        .or_else(|| trimmed.strip_prefix("模式："))
    {
        return Some(rest.trim());
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("mode:") {
        return Some(trimmed["mode:".len()..].trim());
    }

    None
}

/// Depends field recognition: `前置: A, B` / `Depends: A, B`.
/// Returns Some("A, B") or None if not a depends line.
pub fn match_depends_field(line: &str) -> Option<&str> {
    let trimmed = line.trim().trim_start_matches('#').trim();

    if let Some(rest) = trimmed
        .strip_prefix("前置:")
        .or_else(|| trimmed.strip_prefix("前置："))
    {
        return Some(rest.trim());
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("depends:") {
        return Some(trimmed["depends:".len()..].trim());
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Intent,
    Constraints,
    Decisions,
    Boundaries,
    AcceptanceCriteria,
    OutOfScope,
    Questions,
}

/// Extract quoted parameters from step text.
/// e.g., `存在一笔金额为 "100.00" 元的交易 "TXN-001"` → ["100.00", "TXN-001"]
pub fn extract_params(text: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '"' || ch == '\u{201C}' || ch == '\u{201D}' {
            // collect until closing quote
            let mut param = String::new();
            for inner in chars.by_ref() {
                if inner == '"' || inner == '\u{201C}' || inner == '\u{201D}' {
                    break;
                }
                param.push(inner);
            }
            if !param.is_empty() {
                params.push(param);
            }
        }
    }
    params
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_match_step_chinese() {
        let (kind, rest) = match_step_keyword("  假设 数据库中存在用户").unwrap();
        assert_eq!(kind, StepKind::Given);
        assert_eq!(rest, "数据库中存在用户");
    }

    #[test]
    fn test_match_step_english() {
        let (kind, rest) = match_step_keyword("  Given a user exists").unwrap();
        assert_eq!(kind, StepKind::Given);
        assert_eq!(rest, "a user exists");
    }

    #[test]
    fn test_match_step_and() {
        let (kind, rest) = match_step_keyword("  并且 用户已登录").unwrap();
        assert_eq!(kind, StepKind::And);
        assert_eq!(rest, "用户已登录");
    }

    #[test]
    fn test_scenario_header_chinese() {
        assert_eq!(match_scenario_header("场景: 全额退款"), Some("全额退款"));
        assert_eq!(match_scenario_header("场景：全额退款"), Some("全额退款"));
    }

    #[test]
    fn test_scenario_header_english() {
        assert_eq!(
            match_scenario_header("Scenario: Full refund"),
            Some("Full refund")
        );
    }

    #[test]
    fn test_scenario_header_accepts_example_aliases() {
        assert_eq!(
            match_scenario_header("Example: Full refund"),
            Some("Full refund")
        );
        assert_eq!(match_scenario_header("示例: 余额不足"), Some("余额不足"));
        assert_eq!(match_scenario_header("例子: 余额充足"), Some("余额充足"));
        assert_eq!(
            match_scenario_header("### Example: Happy path"),
            Some("Happy path")
        );
    }

    #[test]
    fn test_match_rule_header() {
        assert_eq!(
            match_rule_header("Rule: auth-must-not-leak — 鉴权失败不得泄漏内部错误"),
            Some("auth-must-not-leak — 鉴权失败不得泄漏内部错误")
        );
        assert_eq!(
            match_rule_header("规则: vip-discount-priority"),
            Some("vip-discount-priority")
        );
        assert_eq!(
            match_rule_header("### Rule: refund-idempotent"),
            Some("refund-idempotent")
        );
        assert_eq!(match_rule_header("场景: 普通场景"), None);
        assert_eq!(match_rule_header("- 普通条目"), None);
    }

    #[test]
    fn test_scenario_header_accepts_markdown_heading() {
        assert_eq!(
            match_scenario_header("### Scenario: Full refund"),
            Some("Full refund")
        );
        assert_eq!(
            match_scenario_header("### 场景: 全额退款"),
            Some("全额退款")
        );
    }

    #[test]
    fn test_extract_params() {
        let params = extract_params(r#"金额为 "100.00" 元的交易 "TXN-001""#);
        assert_eq!(params, vec!["100.00", "TXN-001"]);
    }

    #[test]
    fn test_extract_params_chinese_quotes() {
        let params = extract_params("金额为\u{201C}100.00\u{201D}元");
        assert_eq!(params, vec!["100.00"]);
    }

    #[test]
    fn test_match_test_selector_chinese() {
        assert_eq!(
            match_test_selector("  测试: test_parse_contract"),
            Some("test_parse_contract")
        );
        assert_eq!(
            match_test_selector("  测试：test_parse_contract"),
            Some("test_parse_contract")
        );
    }

    #[test]
    fn test_match_test_selector_fields_support_verification_metadata() {
        assert_eq!(
            match_test_selector_field("  层级: integration"),
            Some((TestSelectorField::Level, "integration"))
        );
        assert_eq!(
            match_test_selector_field("  替身: local_http_stub"),
            Some((TestSelectorField::TestDouble, "local_http_stub"))
        );
        assert_eq!(
            match_test_selector_field("  命中: commands/update"),
            Some((TestSelectorField::Targets, "commands/update"))
        );
        assert_eq!(
            match_test_selector_field("  Level: integration"),
            Some((TestSelectorField::Level, "integration"))
        );
        assert_eq!(
            match_test_selector_field("  Test Double: local_http_stub"),
            Some((TestSelectorField::TestDouble, "local_http_stub"))
        );
        assert_eq!(
            match_test_selector_field("  Targets: commands/update"),
            Some((TestSelectorField::Targets, "commands/update"))
        );
    }

    #[test]
    fn test_match_test_selector_english() {
        assert_eq!(
            match_test_selector("  Test: test_parse_contract"),
            Some("test_parse_contract")
        );
    }

    #[test]
    fn test_match_test_selector_accepts_markdown_heading() {
        assert_eq!(
            match_test_selector("### Test: test_parse_contract"),
            Some("test_parse_contract")
        );
        assert_eq!(
            match_test_selector("### 测试: test_parse_contract"),
            Some("test_parse_contract")
        );
    }

    #[test]
    fn test_match_test_selector_field_chinese() {
        assert_eq!(
            match_test_selector_field("  包: spec-parser"),
            Some((TestSelectorField::Package, "spec-parser"))
        );
        assert_eq!(
            match_test_selector_field("  过滤: test_parse_contract"),
            Some((TestSelectorField::Filter, "test_parse_contract"))
        );
    }

    #[test]
    fn test_match_test_selector_field_english() {
        assert_eq!(
            match_test_selector_field("  Package: spec-parser"),
            Some((TestSelectorField::Package, "spec-parser"))
        );
        assert_eq!(
            match_test_selector_field("  Filter: test_parse_contract"),
            Some((TestSelectorField::Filter, "test_parse_contract"))
        );
    }

    #[test]
    fn test_match_test_selector_field_accepts_markdown_heading() {
        assert_eq!(
            match_test_selector_field("### Package: spec-parser"),
            Some((TestSelectorField::Package, "spec-parser"))
        );
        assert_eq!(
            match_test_selector_field("### 过滤: test_parse_contract"),
            Some((TestSelectorField::Filter, "test_parse_contract"))
        );
    }

    #[test]
    fn test_section_headers() {
        assert_eq!(match_section_header("## 意图"), Some(SectionKind::Intent));
        assert_eq!(match_section_header("## Intent"), Some(SectionKind::Intent));
        assert_eq!(
            match_section_header("## 约束"),
            Some(SectionKind::Constraints)
        );
        assert_eq!(
            match_section_header("## Constraints"),
            Some(SectionKind::Constraints)
        );
        assert_eq!(
            match_section_header("## 决策"),
            Some(SectionKind::Decisions)
        );
        assert_eq!(
            match_section_header("## Decisions"),
            Some(SectionKind::Decisions)
        );
        assert_eq!(
            match_section_header("## 边界"),
            Some(SectionKind::Boundaries)
        );
        assert_eq!(
            match_section_header("## Boundaries"),
            Some(SectionKind::Boundaries)
        );
        assert_eq!(
            match_section_header("## 验收标准"),
            Some(SectionKind::AcceptanceCriteria)
        );
        assert_eq!(
            match_section_header("## Acceptance Criteria"),
            Some(SectionKind::AcceptanceCriteria)
        );
        assert_eq!(
            match_section_header("## Completion Criteria"),
            Some(SectionKind::AcceptanceCriteria)
        );
    }

    #[test]
    fn test_not_a_step() {
        assert!(match_step_keyword("这是普通文字").is_none());
        assert!(match_step_keyword("- 约束条目").is_none());
    }
}
