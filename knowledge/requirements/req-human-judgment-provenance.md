---
kind: requirement
id: REQ-HUMAN-JUDGMENT-PROVENANCE
title: "Human Judgment Provenance"
status: accepted
liveness: auto
tags: [provenance, verification, audit, questions]
---

# Human Judgment Provenance

## Problem

验收里有些场景机器判不了，只能由人拍板。今天这类判定通过手写 decisions
JSON 进入报告后就消失了：审计时看不出哪些通过是人裁决的。ADR-003 裁决把
判定升为一等证据进入 run log 与 trace 链，同时必须绕开 ADR-001 的身份禁令
——记录判定的类别与内容，绝不记录身份。这条边界是本需求的核心：core 说
「此处由人裁决，内容为 X，绑定证据摘要为 Y」，永不说「某人批的」。

## Requirements

[REQ-HUMAN-JUDGMENT-PROVENANCE-RECORD] 人工判定 MUST 作为证据记录进入 run log 与 trace 证据链，携带 verdict、reasoning、被判定 scenario 标识与所依据证据的 digest。

[REQ-HUMAN-JUDGMENT-PROVENANCE-CLASS] 判定记录 MUST 以 `source: human` 一类的类别字段标明其来源类别，用以与机器判定区分。

[REQ-HUMAN-JUDGMENT-PROVENANCE-CLASS-RENDERED] `replay` 与 `explain-failure` 的输出 MUST 按记录的来源类别区分措辞，模型判定 MUST NOT 被呈现为人的判定。

[REQ-HUMAN-JUDGMENT-PROVENANCE-NO-IDENTITY] 判定记录 MUST NOT 包含 `actor`、`authority`、`approval` 或 `policy` 字段，亦 MUST NOT 以其他字段名承载审批者身份。

[REQ-HUMAN-JUDGMENT-PROVENANCE-DIGEST-BINDING] 判定记录 MUST 携带足以让外部系统按 digest 绑定审批人的稳定标识，绑定关系本身 MUST 留在外部存储。

[REQ-HUMAN-JUDGMENT-PROVENANCE-REPLAY] `requirements replay` 与 `requirements explain-failure` MUST 能显示某条通过是否依赖人工判定。

[REQ-HUMAN-JUDGMENT-PROVENANCE-FORBIDDEN-GATE] 机械检查 MUST 在判定记录出现被禁字段时失败并指名该字段，防止身份随实现演进渗入核心输出。

## Scenarios

Scenario: 人工判定进入证据链
  Given 一个 uncertain 场景被人判定为通过
  When 该判定合入验证报告并写入 run log
  Then 记录含 verdict、reasoning 与被判定 scenario 的证据 digest

Scenario: 判定按类别而非身份记录
  Given 一条人工判定记录
  When 读取该记录字段
  Then 记录含来源类别 human 且不含任何审批者姓名或标识

Scenario: 被禁字段被机械拦截
  Given 一条被注入 actor 字段的判定记录
  When 禁用字段检查运行
  Then 检查失败且错误信息指名 actor 字段

Scenario: replay 显示判定依赖
  Given 一份含人工判定通过的历史运行
  When requirements replay 针对该需求 id 运行
  Then 输出标明该通过依赖人工判定

Scenario: 无人工判定时输出不变
  Given 一份全部由机器判定的验证运行
  When run log 写入并被 replay 读取
  Then 输出不含任何人工判定标记

Scenario: 模型判定不被呈现为人的判定
  Given 一条来源类别为 model 的判定记录
  When replay 与 explain-failure 渲染该记录
  Then 两者均标注为模型判定且不含人工判定措辞

## Dependencies

- REQ-DECISION-POINT-EMISSION

## Source Trace

- decision: ADR-003（判定进 provenance 且绕开身份禁令，2026-08-04）
- proposal: LEP-002
- 上游约束: ADR-001 —— 「Core outputs and schemas are mechanically
  forbidden from carrying `actor`, `authority`, `approval`, or `policy`
  fields; external systems bind approvals to reported digests in their own
  stores.」
- 既有消费者: `requirements replay`、`requirements explain-failure`、
  `requirements trace-graph`

## Open Questions

None.
