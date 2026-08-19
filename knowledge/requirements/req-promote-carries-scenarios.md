---
kind: requirement
id: REQ-PROMOTE-CARRIES-SCENARIOS
title: "Promote Carries Scenarios"
status: proposed
liveness: auto
tags: [promote, capability, bdd]
---

# Promote Carries Scenarios

## Problem

`agent-spec promote <task> --rule <id> --to <cap>` 通过门禁（Rule 至少一个
Example 且全部 pass）后，写进能力 spec 的只有一行 provenance 注释与
`### Rule:` 标题：`upsert_capability_rule` 根本没有接收场景。结果是能力库里
出现一个没有任何 Example 的 Rule——正是 `audit` 判为 unproven、lint 判为
`orphan-rule` 的形态；robrix2 只能手写 `specs/capabilities/dm-encryption.spec.md`。
另外新建能力 spec 的骨架固定使用中文段名 `## 意图` / `## 完成条件`，与英文源
spec 不一致。robrix2 反馈 C1（P0）。

## Requirements

[REQ-PROMOTE-CARRIES-SCENARIOS-VERBATIM] promote MUST 把 Rule 下每个 Example 的源文本（`Scenario:` 行到最后一步，含 Tags、Test/结构化选择器、Review、Mode、Depends 与步骤表格）逐字写入能力 spec 的该 Rule 之下。

[REQ-PROMOTE-CARRIES-SCENARIOS-ORDER] 写入的 Example MUST 保持源 spec 中的文档顺序。

[REQ-PROMOTE-CARRIES-SCENARIOS-IDEMPOTENT] 能力 spec 已含该 Rule id 时 promote MUST 保持文件内容不变。

[REQ-PROMOTE-CARRIES-SCENARIOS-LANG] 新建能力 spec 的段名 MUST 跟随源 spec 语言：源 spec 使用英文段头时用 `## Intent` / `## Completion Criteria`，否则用 `## 意图` / `## 完成条件`。

[REQ-PROMOTE-CARRIES-SCENARIOS-REPARSE] promote 写出的能力 spec MUST 能被解析器读回，且该 Rule 的 scenario 名单与源 spec 一致。

[REQ-PROMOTE-CARRIES-SCENARIOS-GATE-UNCHANGED] promote 门禁 MUST 保持"至少一个 Example 且全部 pass"不变。

## Scenarios

Rule: REQ-PROMOTE-CARRIES-SCENARIOS-VERBATIM

Scenario: 场景块整段搬运
  Given 一个 Rule 下有两个带 Tags 与结构化 Test 选择器的 Example
  When 对该 Rule 执行 promote
  Then 能力 spec 文件包含两个 Example 的全部源行

Rule: REQ-PROMOTE-CARRIES-SCENARIOS-ORDER

Scenario: 顺序保持
  Given Rule 下 Example 依次为 A、B
  When 执行 promote
  Then 能力 spec 中 A 出现在 B 之前

Rule: REQ-PROMOTE-CARRIES-SCENARIOS-IDEMPOTENT

Scenario: 二次 promote 不变
  Given 能力 spec 已含该 Rule
  When 再次执行 promote
  Then 文件内容逐字节不变

Rule: REQ-PROMOTE-CARRIES-SCENARIOS-LANG

Scenario: 英文源生成英文骨架
  Given 源 spec 使用 `## Intent` 与 `## Acceptance Criteria`
  When promote 新建能力 spec
  Then 文件包含 `## Intent` 与 `## Completion Criteria` 且不含 `## 意图`

Rule: REQ-PROMOTE-CARRIES-SCENARIOS-REPARSE

Scenario: 写出的能力 spec 可读回
  Given promote 刚写出的能力 spec
  When 解析该文件
  Then 该 Rule 的 scenario_names 输出与源 spec 相同

Rule: REQ-PROMOTE-CARRIES-SCENARIOS-GATE-UNCHANGED

Scenario: 未通过的 Example 仍阻止 promote
  Given Rule 下有一个 verdict 为 fail 的 Example
  When 执行 promote
  Then 命令返回错误且能力 spec 文件不被创建

## Dependencies

None.

## Source Trace

- practice feedback: robrix2 docs/agent-spec-feedback-2026-08.md C1（P0）
- triage: docs/robrix2-feedback-triage-2026-08.md 队列 1
- code: src/main.rs upsert_capability_rule / cmd_promote

## Open Questions

None.

## Next

Single exit: compile this requirement into a task contract with
`agent-spec requirements draft-specs`.
