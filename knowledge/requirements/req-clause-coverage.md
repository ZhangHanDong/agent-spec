---
kind: requirement
id: REQ-CLAUSE-COVERAGE
title: "Clause Coverage"
status: accepted
liveness: auto
tags: [knowledge, lint, coverage, governance]
---

# Clause Coverage

## Problem

`liveness=Honored` 声称需求的义务成立，但没有任何机械物把 MUST 条款与场景
连起来：本仓库 553 条 MUST 条款对 255 个场景，且场景从不引用条款 id。既有的
`requirement-must-needs-scenario` 只在完全没有场景时才报，一个场景即可让十二
条义务过关。2026-08-07 的实证是
`REQ-DECISION-POINT-EMISSION-MECHANICAL-MOAT` 被实现推翻期间需求持续显示
Honored。ADR-004 裁决：以 BDD `Rule: <条款 id>` 显式归属，不做关键词推断，
skip 不算覆盖，覆盖率报在 `requirements status`。

## Requirements

[REQ-CLAUSE-COVERAGE-ATTRIBUTION] 需求文档 MUST 支持以 `Rule: <条款 id>` 将场景归属到某条条款，Rule id 即该条款的 id。

[REQ-CLAUSE-COVERAGE-EXPLICIT-ONLY] 覆盖判定 MUST 只依据显式归属；MUST NOT 以关键词、文本相似度或位置邻近推断覆盖关系。

[REQ-CLAUSE-COVERAGE-SKIP-NOT-COVERED] 归属场景的判定为 skip 时该条款 MUST NOT 计为已覆盖。

[REQ-CLAUSE-COVERAGE-DIAGNOSTIC] 无归属场景的 MUST 条款 MUST 触发 `clause-uncovered` 诊断，文本指名条款 id 与所在文档路径。

[REQ-CLAUSE-COVERAGE-SEVERITY] `clause-uncovered` 引入版本的严重级别 MUST 为 Info，且 MUST 支持只准缩小的基线文件豁免存量条款。

[REQ-CLAUSE-COVERAGE-UNKNOWN-ID] 归属到文档中不存在的条款 id 的 Rule MUST 触发诊断并指名该 id，MUST NOT 被静默忽略。

[REQ-CLAUSE-COVERAGE-STATUS-REPORT] `requirements status` MUST 报告该需求的条款覆盖情况；`trace` 的输出 MUST NOT 因本需求而改变。

[REQ-CLAUSE-COVERAGE-NON-MUST] SHOULD 与 MAY 条款 MUST NOT 触发 `clause-uncovered`。

## Scenarios

Rule: REQ-CLAUSE-COVERAGE-ATTRIBUTION

Scenario: 场景按条款 id 归属
  Given 一份需求文档的场景归属在 Rule REQ-X-ALPHA 之下
  When 计算该文档的条款覆盖
  Then 条款 REQ-X-ALPHA 被计为已覆盖

Rule: REQ-CLAUSE-COVERAGE-EXPLICIT-ONLY

Scenario: 文字相近但未归属不算覆盖
  Given 一条 MUST 条款与一个措辞高度相近但未归属任何 Rule 的场景
  When 计算条款覆盖
  Then 该条款仍被计为未覆盖

Rule: REQ-CLAUSE-COVERAGE-SKIP-NOT-COVERED

Scenario: 判定为 skip 的归属场景不计覆盖
  Given 一条条款的唯一归属场景在验证中判定为 skip
  When 计算条款覆盖
  Then 该条款被计为未覆盖

Rule: REQ-CLAUSE-COVERAGE-DIAGNOSTIC

Scenario: 未覆盖条款被指名
  Given 一份含无归属场景的 MUST 条款的需求文档
  When lint-knowledge 运行
  Then 输出 clause-uncovered 诊断且含该条款 id 与文档路径

Rule: REQ-CLAUSE-COVERAGE-SEVERITY

Scenario: 引入版本为 Info 且基线可豁免
  Given 基线文件列出某未覆盖条款
  When lint-knowledge --gate 运行
  Then 该条款不产生诊断且退出码为零

Rule: REQ-CLAUSE-COVERAGE-UNKNOWN-ID

Scenario: 归属到不存在的条款被指名
  Given 一个 Rule id 在该文档条款列表中不存在
  When 计算条款覆盖
  Then 输出诊断且指名该不存在的 id

Rule: REQ-CLAUSE-COVERAGE-STATUS-REPORT

Scenario: 覆盖率报在 status 而非 trace
  Given 一份条款部分覆盖的需求
  When requirements status 与 trace 分别运行
  Then status 报告覆盖情况而 trace 输出不含覆盖信息

Rule: REQ-CLAUSE-COVERAGE-NON-MUST

Scenario: 非 MUST 条款不触发诊断
  Given 一份只含 SHOULD 与 MAY 条款且无场景的需求文档
  When lint-knowledge 运行
  Then 不输出 clause-uncovered 诊断

## Dependencies

None.

## Source Trace

- decision: ADR-004（归属语法、只认显式、skip 不算、报在 status，2026-08-08）
- proposal: LEP-003
- 实测证据: knowledge/requirements/ 下 553 条 MUST 条款对 255 个场景；
  req-decision-point-emission.md 的 Scenarios 段零 `REQ-` 引用
- 失效实证: REQ-DECISION-POINT-EMISSION-MECHANICAL-MOAT 被推翻期间
  需求持续 Honored（2026-08-07 审查）
- 先例: spec_lint 的 decision-coverage；BDD `Rule: <id>` 与 bdd-rule-id；
  ADR-002 的 orphan-spec 分阶段与只准缩小基线

## Open Questions

None.
