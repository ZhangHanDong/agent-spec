spec: task
name: "List Item Continuation"
tags: [parser, decisions, constraints, lint, bugfix]
satisfies: [REQ-LIST-ITEM-CONTINUATION]
risk: B
estimate: 0.5d
---

## Intent

让多行 bullet 被完整读出：Decisions、Constraints、Boundaries、Out of Scope、
Questions 五个列表段里，紧随 bullet 的缩进续行并入该条目，缩进子 bullet 作为
父条目的一部分保留而不是提升为同级。这是 parser 层的修复，`explain` 各格式与
基于条目文本的 lint 自动受益。来源是 robrix2 反馈 B5（升 P1）。

## Decisions

- 在 `src/spec_parser/parser.rs` 新增 `group_list_lines(lines) -> Vec<(usize, ListLine)>`，
  `ListLine` 为 `Header(String)` / `Item(String)` 二选一；五个列表段的解析器都改为
  先经它分组再各自处理 Header（类别切换）与 Item。
- 续行判定：非空、有缩进、不以 `-` 开头、且当前有未结束条目 → 以单个空格接到条目末尾。
- 子 bullet 判定：以 `-` 开头且缩进深于当前条目的 bullet 缩进 → 以 `\n  - ` 前缀接到父条目；
  子 bullet 自己的续行同样以空格接在子 bullet 之后。
- 终止条件：空行、`###` 子标题、HTML 注释（含跨多行的注释块）、非缩进的非 bullet 行
  都结束当前条目；`###` 产出 `Header`，其余被忽略。
- 条目 span 为其 bullet 所在行；合并不改变现有单行条目的文本。
- 不改变 Acceptance Criteria 段的解析。

## Boundaries

### Allowed Changes
- src/spec_parser/parser.rs
- specs/task-list-item-continuation.spec.md
- knowledge/requirements/req-list-item-continuation.md
- .agent-spec/wiki/**
- CHANGELOG.md
- skills/agent-spec-authoring/references/patterns.md
- book/src/**

### Forbidden
- 不改变 Acceptance Criteria 场景与步骤的解析
- 不改变单行条目的文本
- 不引入新的 AST 类型

## Out of Scope

- `explain` 渲染格式本身的调整
- 段落级（非列表）内容的续行

## Completion Criteria

### Rule: merge — 续行并入条目

场景: 三行 Decision 被完整读出
  测试: test_list_continuation_merges_wrapped_decision
  假设 一条 Decision 写成 bullet 行加两行缩进续行
  当 解析该 spec
  那么 decisions 中该条目文本等于三行以空格连接的结果

场景: 单行条目文本不变
  测试: test_list_continuation_leaves_single_line_items_unchanged
  假设 三条各占一行的 Decision
  当 经 `src/spec_parser/parser.rs` 的 `group_list_lines` 分组后解析该 spec
  那么 decisions 输出三条且文本逐字与源文件相同

场景: Acceptance Criteria 解析不受影响
  测试: test_list_continuation_does_not_touch_acceptance_criteria
  假设 一个场景的步骤带缩进表格与续行
  当 解析该 spec
  那么 该场景的步骤数与表格行数与修改前一致

### Rule: nested — 子 bullet 留在父条目内

场景: 子 bullet 附在父条目之后
  测试: test_list_continuation_keeps_nested_bullets_in_parent
  假设 一条 Decision 下有两个缩进子 bullet 且第二个子 bullet 有一行续行
  当 解析该 spec
  那么 decisions 只有一个元素且其文本包含两个以 "\n  - " 开头的片段并且第二个片段含续行内容

### Rule: terminators — 终止条件

场景: 空行与注释后的缩进行不被并入
  测试: test_list_continuation_stops_at_blank_comment_and_unindented_lines
  假设 bullet 之后依次是空行、HTML 注释与一行缩进文本
  当 解析该 spec
  那么 该 bullet 的文本不包含那行缩进文本且 decisions 只有一个元素

场景: 多行注释内的行不并入
  测试: test_list_continuation_ignores_multiline_comment_body
  假设 bullet 之后是一段跨三行的 HTML 注释其中间行带缩进
  当 解析该 spec
  那么 该 bullet 的文本不包含注释内容

### Rule: all-lists — 五个列表段一致

场景: 约束与禁止条目同样合并
  测试: test_list_continuation_applies_to_constraints_and_boundaries
  假设 Constraints 与 Boundaries Forbidden 各有一条两行的条目
  当 解析该 spec
  那么 constraints 与 boundaries 输出的对应条目文本均包含第二行内容

场景: 子标题切换类别时不吞并前后条目
  测试: test_list_continuation_respects_subsection_headers
  假设 Constraints 有 `### Must` 与 `### Must Not` 两个子标题各含一条两行条目
  当 解析该 spec
  那么 两条 constraint 类别分别为 must 与 must_not 且文本各含自己的第二行

场景: Out of Scope 与 Questions 同样合并
  测试: test_list_continuation_applies_to_out_of_scope_and_questions
  假设 Out of Scope 与 Questions 各有一条两行的条目
  当 解析该 spec
  那么 两段输出的条目文本均包含第二行内容

### Rule: span-and-lint — span 与 lint 可见性

场景: span 指向 bullet 行
  测试: test_list_continuation_span_points_at_bullet_line
  假设 一条从某行开始并续到后两行的 Constraint
  当 解析该 spec
  那么 该 constraint 的 span 行号等于 bullet 行号

场景: 续行里的标识符参与 decision-coverage
  测试: test_list_continuation_decision_coverage_sees_wrapped_identifier
  假设 一条 Decision 的关键反引号标识符只出现在续行且有场景步骤引用同一标识符
  当 运行 DecisionCoverageLinter
  那么 不输出针对该 Decision 的 decision-coverage 诊断
