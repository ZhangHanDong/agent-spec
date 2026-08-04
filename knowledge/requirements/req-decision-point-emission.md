---
kind: requirement
id: REQ-DECISION-POINT-EMISSION
title: "Decision Point Emission"
status: accepted
liveness: auto
tags: [knowledge, questions, cli, verification]
---

# Decision Point Emission

## Problem

信封定了形状，还需要三个阶段真正把问题发出来。ADR-003 裁决用三个阶段局部
表面而非统一命名空间：`requirements questions` 填充候选、新增
`knowledge questions <id>` 从提案与决策提取选型、新增
`verify --emit-questions` 把机器判不了的场景变成待判定问题——后者正是
`resolve-ai` 的逆向，发出 `resolve-ai` 本就吃的东西。

## Requirements

[REQ-DECISION-POINT-EMISSION-REQUIREMENTS] `requirements questions` MUST 输出 REQ-DECISION-POINT-ENVELOPE 定义的信封，kind 标记为 requirements，并接受 agent 起草的候选回填后校验。

[REQ-DECISION-POINT-EMISSION-KNOWLEDGE] `knowledge questions <id>` MUST 从 proposal 的 `## Unresolved Questions` 与 decision 的 `## Alternatives Considered` 提取问题，每条产出一个 kind 标记为 knowledge 的信封。

[REQ-DECISION-POINT-EMISSION-RECOMMENDATION] 当来源文档明示推荐项时，`knowledge questions` MUST 在对应候选上标记推荐；未明示时 MUST NOT 自行推断推荐项。

[REQ-DECISION-POINT-EMISSION-VERIFY] `verify --emit-questions` MUST 为每个 skip、uncertain 或 pending_review 场景产出 kind 标记为 verification 的信封，携带场景文本、已收集证据与 verdict 词汇表作为候选。

[REQ-DECISION-POINT-EMISSION-MECHANICAL-MOAT] 发出的判定问题 MUST NOT 覆盖机械已判定的 pass 或 fail 场景；`resolve-ai` 只应用 skip 场景的判定这一既有保护 MUST 保持不变。

[REQ-DECISION-POINT-EMISSION-DECISIONS-OUT] 验收判定的答案 MUST 能转为 `resolve-ai` 既有 decisions JSON 格式而无需人工改写字段名。

[REQ-DECISION-POINT-EMISSION-NO-TTY] 三个发射点 MUST NOT 在 CLI 内发起交互式提问或等待标准输入。

[REQ-DECISION-POINT-EMISSION-EMPTY] 无待决问题时三个发射点 MUST 输出空集合并以零退出码结束。

## Scenarios

Scenario: 逆向访谈问题带阶段标记
  Given 一份含 compound-clause 诊断的需求文档
  When requirements questions 以 json 输出
  Then 对应信封的 kind 为 requirements 且诊断码保留在 diagnostic_code

Scenario: 提案未决问题被提取
  Given 一份未决问题含两条的提案
  When knowledge questions 针对该提案 id 运行
  Then 输出两个 kind 为 knowledge 的信封且各自 source 指向该提案路径

Scenario: 未明示推荐时不标记推荐
  Given 一份备选中未写明推荐项的决策
  When knowledge questions 针对该决策 id 运行
  Then 输出候选均不带推荐标记

Scenario: 待判定场景带证据发出
  Given 一份验证报告含一个未定论场景
  When verify --emit-questions 运行
  Then 输出信封的 kind 为 verification 且携带该场景文本与已收集证据

Scenario: 机械已判定场景不发问题
  Given 一份验证报告含一个机械判定为 pass 的场景
  When verify --emit-questions 运行
  Then 该场景不产生任何判定问题

Scenario: 判定答案可直接喂给 resolve-ai
  Given 一组已回答的验收判定信封
  When 转换为 decisions JSON
  Then resolve-ai 读取该文件成功且不需要人工改写字段名

Scenario: CLI 不发起交互
  Given 任一发射点在无终端环境运行
  When 命令执行
  Then 命令不读取标准输入且不阻塞等待

Scenario: 无待决问题时输出空集合
  Given 一个无任何待决问题的工作区
  When 三个发射点分别运行
  Then 各自输出空集合且退出码为零

## Dependencies

- REQ-DECISION-POINT-ENVELOPE

## Source Trace

- decision: ADR-003（三个阶段局部表面定案，2026-08-04）
- proposal: LEP-002
- 既有反向半边: `resolve-ai` 与 `AiDecision { model, confidence, verdict,
  reasoning }`（src/spec_core/verify.rs）
- 上游约束: ADR-001（编排中立，审批工作流不进核心）

## Open Questions

None.
