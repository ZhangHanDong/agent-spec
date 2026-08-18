spec: task
name: "Promote Carries Scenarios"
tags: [promote, capability, bdd, bugfix]
satisfies: [REQ-PROMOTE-CARRIES-SCENARIOS]
risk: B
estimate: 0.5d
---

## Intent

修复 `agent-spec promote`：通过门禁后把 Rule 下全部 Example 的源文本逐字搬进能力
spec，而不是只写一行 `### Rule:` 标题留下一个 unproven 的 Rule；新建能力 spec 的
段名跟随源 spec 的语言。门禁语义（至少一个 Example 且全部 pass）与幂等性不变。
来源是 robrix2 反馈 C1（P0）。

## Decisions

- 新增 `rule_scenario_blocks(source, doc, rule_id) -> Vec<String>`：按每个 Example 的
  `Scenario.span`（`start_line..=end_line`，覆盖 `Scenario:` 行到最后一步含表格）切源文本
  行，去掉共同的前导空白后作为一个块；顺序即文档顺序。
- `upsert_capability_rule` 增加 `scenario_blocks: &[String]` 与 `lang: CapabilityLang` 参数：
  写入形状为 provenance 注释、`### Rule:` 行、空行、各块以空行分隔。
- `CapabilityLang` 由源 spec 文本判定：含 `## Intent`、`## Acceptance Criteria` 或
  `## Completion Criteria` 任一英文段头 → `En`（骨架用 `## Intent` / `## Completion Criteria`），
  否则 `Zh`（`## 意图` / `## 完成条件`）。
- 能力 spec 已含该 Rule id 时整份内容原样返回（不追加、不去重场景）。
- 门禁函数 `promote_gate_ok` 与 `examples_all_pass` 不改。

## Boundaries

### Allowed Changes
- src/main.rs
- specs/task-promote-carries-scenarios.spec.md
- knowledge/requirements/req-promote-carries-scenarios.md
- .agent-spec/wiki/**
- CHANGELOG.md
- skills/agent-spec-authoring/SKILL.md
- skills/agent-spec-tool-first/references/commands.md
- book/src/**

### Forbidden
- 不改变 promote 门禁语义
- 不修改解析器
- 不在能力 spec 中改写 Example 的文本

## Out of Scope

- 把 Invariant 随 Rule 搬运（A8，Invariant 尚无正式语法）
- 从能力 spec 反向同步回任务 spec
- 多个任务 spec 向同一 Rule 追加 Example 的合并策略

## Completion Criteria

### Rule: verbatim — 场景块逐字搬运

场景: 场景块整段搬运
  测试: test_promote_carries_scenario_blocks_verbatim
  假设 一个 Rule 下有两个 Example 且各带 Tags 与结构化 Test 选择器与步骤表格
  当 用 `rule_scenario_blocks` 取块并经 `upsert_capability_rule` 写入
  那么 输出内容包含两个 Example 从 `Scenario:` 到最后一步的全部源行

场景: 顺序保持
  测试: test_promote_keeps_example_document_order
  假设 Rule 下 Example 依次为 A、B
  当 写入能力 spec
  那么 A 的 `Scenario:` 行出现在 B 之前

### Rule: reparse — 写出的能力 spec 可读回

场景: 写出的能力 spec 可读回
  测试: test_promote_output_reparses_with_same_scenario_names
  假设 promote 刚写出的能力 spec 内容
  当 解析该内容
  那么 该 Rule 的 scenario_names 与源 spec 相同且 level 为 capability

场景: 追加到已有能力文件时场景落在 Rule 之下
  测试: test_promote_appends_scenarios_under_rule_in_existing_file
  假设 一份已有其它 Rule 的能力 spec
  当 追加新 Rule 与其两个 Example
  那么 解析后新 Rule 的 scenario_names 有两个且旧 Rule 不变

### Rule: idempotent-and-gate — 幂等与门禁不变

场景: 二次 promote 不变
  测试: test_promote_with_scenarios_is_idempotent
  假设 能力 spec 已含该 Rule
  当 再次以相同场景块调用 `upsert_capability_rule`
  那么 返回内容与输入逐字节相同

场景: 未通过的 Example 仍阻止 promote
  测试: test_promote_gate_still_blocks_failing_example
  假设 Rule 下有一个 verdict 为 fail 的 Example
  当 调用 `promote_gate_ok`
  那么 返回 Err 且消息含 "not all examples pass"

### Rule: lang — 段名跟随源语言

场景: 英文源生成英文骨架
  测试: test_promote_new_capability_uses_english_headers_for_english_source
  假设 源 spec 使用 `## Intent` 与 `## Acceptance Criteria`
  当 `CapabilityLang` 判定后新建能力 spec
  那么 内容包含 `## Intent` 与 `## Completion Criteria` 且不含 `## 意图`

场景: 中文源生成中文骨架
  测试: test_promote_new_capability_uses_chinese_headers_for_chinese_source
  假设 源 spec 使用 `## 意图` 与 `## 完成条件`
  当 新建能力 spec
  那么 内容包含 `## 意图` 与 `## 完成条件` 且不含 `## Intent`
