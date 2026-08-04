---
kind: requirement
id: REQ-DECISION-POINT-ENVELOPE
title: "Decision Point Envelope"
status: accepted
liveness: auto
tags: [knowledge, questions, interop, agent-ux]
---

# Decision Point Envelope

## Problem

流水线三处等人拍板的时刻各说各话：`ClarificationQuestion` 有 `options`
字段却两处构造点硬编码空表（本仓库 119 个问题零候选），LEP 的未决问题与
ADR 的备选是纯散文，验收判定没有任何机器可读形状。ADR-003 裁决：设一个
跨三处共享的决策点信封，候选由 agent 起草、CLI 只校验形状，使任何 harness
都能原样渲染而无需解释自由文本。

## Requirements

[REQ-DECISION-POINT-ENVELOPE-SHAPE] 决策点信封 MUST 携带 id、target_id、diagnostic_code、blocking、prompt、source、kind 与 options；kind MUST 标明发问的流水线阶段。

[REQ-DECISION-POINT-ENVELOPE-OPTION-FIELDS] 每个候选 MUST 携带 label、description 与 write-back 值三个字段；bare string 形式 MUST NOT 再作为候选表示。

[REQ-DECISION-POINT-ENVELOPE-BOUNDS] 候选数量 MUST 不超过四项；超出上限的信封 MUST 被校验拒绝并指名超限的问题 id。

[REQ-DECISION-POINT-ENVELOPE-FREEFORM] 信封 MUST NOT 声明候选表已穷尽答案空间；自由作答路径 MUST 始终可用，空候选表 MUST 是合法状态而非缺陷。

[REQ-DECISION-POINT-ENVELOPE-VALIDATE] CLI MUST 提供对 agent 起草候选的形状校验，拒绝缺 label、缺 description 或超限的候选，错误信息 MUST 指名违规字段。

[REQ-DECISION-POINT-ENVELOPE-VERSION] questions JSON MUST 携带信封版本字段，使旧读者遇到结构化候选时显式失败而非静默误读。

## Scenarios

Scenario: 信封携带阶段与结构化候选
  Given 一个由 requirements 阶段发出的问题
  When 以 json 格式输出
  Then 输出含 kind 字段与每项带 label 和 description 的候选表

Scenario: 超过四项候选被拒绝
  Given agent 起草了五项候选的信封
  When CLI 校验该信封
  Then 校验失败且错误信息含该问题 id 与候选数上限

Scenario: 缺 description 的候选被指名
  Given 一项只有 label 的候选
  When CLI 校验该信封
  Then 校验失败且错误信息指名 description 字段

Scenario: 空候选表合法通过
  Given 一个诊断无法给出候选的问题
  When CLI 校验该信封
  Then 校验通过且该问题保持自由作答

Scenario: 旧读者遇结构化候选显式失败
  Given 一份按 bare string 解析候选的旧消费者
  When 读取带版本字段的新 questions JSON
  Then 消费者依版本字段报错而非静默误读

## Dependencies

None.

## Source Trace

- decision: ADR-003（信封形状与「agent 起草、CLI 校验」定案，2026-08-04）
- proposal: LEP-002
- 实测证据: src/spec_knowledge/questions.rs 两处构造点硬编码
  `options: Vec::new()`；本仓库 `requirements questions --format json`
  产出 119 个问题、0 个带候选
- harness 形状约束: Claude Code AskUserQuestion 与 Codex 等价物均为
  2–4 个带描述的选项加恒常自由输入

## Open Questions

None.
