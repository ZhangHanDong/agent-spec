---
kind: requirement
id: REQ-LIST-ITEM-CONTINUATION
title: "List Item Continuation"
status: proposed
liveness: auto
tags: [parser, decisions, constraints, lint]
---

# List Item Continuation

## Problem

`## Decisions`、`## Constraints`、`## Boundaries`、`## Out of Scope`、`## Questions`
的条目解析只保留以 `-` 开头的那一行：`parse_string_list` 对每行做
`strip_prefix('-')`，续行（缩进的非 bullet 行）被 `filter_map` 直接丢弃，缩进的
子 bullet 被提升为同级条目。一条写成三行的 Decision 只剩前三分之一，`explain`
的 json/text/markdown 全部照此输出，`decision-coverage`、
`precedence-fallback-coverage`、`observable-decision-coverage` 也只对半句做关键词
匹配。本仓库 28 份 spec 共 268 行续行受影响，robrix2 有 475 行续行与 107 个缩进
子 bullet。robrix2 反馈 B5，二次审查升为 P1。

## Requirements

[REQ-LIST-ITEM-CONTINUATION-MERGE] 列表段中紧随 bullet 的缩进非 bullet 行 MUST 并入该 bullet 的文本，行间以单个空格连接。

[REQ-LIST-ITEM-CONTINUATION-NESTED] 缩进深于所属 bullet 的子 bullet MUST 作为父条目的一部分保留，以换行加 `  - ` 前缀附在父文本之后，而不是提升为同级条目。

[REQ-LIST-ITEM-CONTINUATION-TERMINATORS] 空行、`###` 子标题、HTML 注释与非缩进的非 bullet 行 MUST 结束当前条目，使其后的缩进行不再并入前一条目。

[REQ-LIST-ITEM-CONTINUATION-ALL-LISTS] 合并规则 MUST 同时适用于 Decisions、Constraints、Boundaries、Out of Scope 与 Questions 五个列表段。

[REQ-LIST-ITEM-CONTINUATION-SPAN] 合并后条目的 span MUST 指向其 bullet 所在行。

[REQ-LIST-ITEM-CONTINUATION-LINT-SEES-ALL] 基于条目文本做关键词匹配的 lint MUST 看到合并后的完整文本。

## Scenarios

Rule: REQ-LIST-ITEM-CONTINUATION-MERGE

Scenario: 三行 Decision 被完整读出
  Given 一条 Decision 写成 bullet 行加两行缩进续行
  When 解析该 spec 并输出 json
  Then decisions 数组中该条目包含三行合并后的完整文本

Rule: REQ-LIST-ITEM-CONTINUATION-NESTED

Scenario: 子 bullet 留在父条目内
  Given 一条 Decision 下有两个缩进子 bullet
  When 解析该 spec
  Then decisions 数组只有一个元素且其文本包含两个以 `  - ` 开头的行

Rule: REQ-LIST-ITEM-CONTINUATION-TERMINATORS

Scenario: 空行与注释后的缩进行不被并入
  Given bullet 之后依次是空行、HTML 注释与一行缩进文本
  When 解析该 spec
  Then 该 bullet 的文本不包含那行缩进文本

Rule: REQ-LIST-ITEM-CONTINUATION-ALL-LISTS

Scenario: 约束与禁止条目同样合并
  Given Constraints 与 Boundaries Forbidden 各有一条两行的条目
  When 解析该 spec
  Then constraints 与 boundaries 输出的对应条目文本均包含第二行内容

Rule: REQ-LIST-ITEM-CONTINUATION-SPAN

Scenario: span 指向 bullet 行
  Given 一条从第 12 行开始、续到第 14 行的 Constraint
  When 解析该 spec
  Then 该 constraint 的 span 行号输出为 12

Rule: REQ-LIST-ITEM-CONTINUATION-LINT-SEES-ALL

Scenario: 续行里的标识符参与 decision-coverage
  Given 一条 Decision 的关键标识符只出现在续行且有场景覆盖该标识符
  When 运行 lint
  Then 不输出针对该 Decision 的 decision-coverage 诊断

## Dependencies

None.

## Source Trace

- practice feedback: robrix2 docs/agent-spec-feedback-2026-08.md B5
- triage: docs/robrix2-feedback-triage-2026-08.md 队列 1
- code: src/spec_parser/parser.rs parse_string_list / parse_constraints / parse_boundaries

## Open Questions

None.

## Next

Single exit: compile this requirement into a task contract with
`agent-spec requirements draft-specs`.
