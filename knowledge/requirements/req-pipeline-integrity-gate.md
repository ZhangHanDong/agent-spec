---
kind: requirement
id: REQ-PIPELINE-INTEGRITY-GATE
title: "Pipeline Integrity Gate"
status: accepted
liveness: auto
tags: [knowledge, lint, gate, satisfies, orphan]
---

# Pipeline Integrity Gate

## Problem

知识层的机械门禁抓不住跳层：一份跳过 proposal/decision/requirement 三层的
孤儿合约能在 `lint` 拿满分。图完整性检查散在三套验证器
（`lint_corpus`、`build_requirement_graph`、`build_requirement_plan`）里，
各有独立诊断词汇表，而 `lint-knowledge --gate` 只跑第一套——
`{spec} satisfies missing requirement` 这类检查存在却不在门禁路径上。
本仓库 74 个 spec 中 37 个无 `satisfies:`。ADR-002 裁决：合流进
`lint-knowledge --gate`，orphan-spec 按 Info → Warning → Error 分阶段升级，
存量记基线。

## Requirements

[REQ-PIPELINE-INTEGRITY-GATE-MERGE] `lint-knowledge --gate` MUST 包含 requirement graph 与 requirement plan 两套验证器的全部诊断，规则名与既有输出保持一致，Error 存在时退出码非零。

[REQ-PIPELINE-INTEGRITY-GATE-ORPHAN] 当 `knowledge/requirements/` 含至少一份需求文档时，无 `satisfies:` 声明的任务 spec MUST 触发 `orphan-spec` 诊断；1.3.0 引入版本的严重级别 MUST 为 Info。迁移完成后的严重级别与补救动作由 `REQ-ORPHAN-BASELINE-RETIREMENT` 约束。

[REQ-PIPELINE-INTEGRITY-GATE-ORPHAN-SCOPE] `orphan-spec` MUST 只针对任务合约；project、org 与 capability 层级的 spec 不承载 `satisfies:`，MUST NOT 被诊断为孤儿。

[REQ-PIPELINE-INTEGRITY-GATE-BASELINE] 1.3.0 迁移期的 `orphan-spec` MUST 支持仓库内基线文件豁免存量 spec；基线中不存在的新孤儿 MUST 照常诊断；从基线移除的条目 MUST NOT 被自动加回。迁移后的空基线与防复活规则由 `REQ-ORPHAN-BASELINE-RETIREMENT` 约束。

[REQ-PIPELINE-INTEGRITY-GATE-BASELINE-PATHS] 1.3.0 迁移期的基线条目 MUST 按路径身份而非字符串字面量匹配，使相对于仓库根、相对于基线文件与绝对路径三种写法指向同一 spec 时等效。

[REQ-PIPELINE-INTEGRITY-GATE-KIND-MISMATCH] 需求文档 `## Dependencies` 中出现 `ADR-` 或 `LEP-` 前缀 id 时 MUST 触发 `dependency-kind-mismatch` 诊断，suggestion MUST 指名把该 id 移至 `## Source Trace`。

[REQ-PIPELINE-INTEGRITY-GATE-PRODUCES] status 为 accepted 的 proposal MUST 通过 `produces-link-integrity` 校验：`## Produces` 目标存在，且目标 decision 的 `## Source Trace` 回链该 proposal id，任一方向缺失均为 Warning 并指名缺失方向。

[REQ-PIPELINE-INTEGRITY-GATE-PRODUCES-PARSE] `## Produces` 的产出物 MUST 只取自内联标题与列表项开头的 id；节内散文（含列表项 id 之后的说明文字）中引用的其他 id MUST NOT 被当作产出物。

## Scenarios

Scenario: 门禁包含图诊断
  Given 一份 satisfies 指向不存在需求的 spec
  When lint-knowledge --gate 运行
  Then 输出含 dangling-spec-coverage 诊断且退出码非零

Scenario: 新孤儿 spec 被指名
  Given 需求语料非空且新增一份无 satisfies 的 spec 未进基线
  When lint-knowledge 运行
  Then 输出 orphan-spec 诊断；当前严重级别与补救动作遵循退休需求

Scenario: 迁移基线退休
  Given 1.3.0 存量孤儿已经逐份完成证据化处置
  When lint-knowledge --gate 运行
  Then 基线保持空且任意复活条目被拒绝

Scenario: 非任务合约不算孤儿
  Given specs 下存在一份 project 层级的 spec 且它不带 satisfies
  When lint-knowledge 运行
  Then 该 project spec 不产生 orphan-spec 诊断

Scenario: Dependencies 里的 ADR 被纠偏
  Given 一份需求文档在 ## Dependencies 列出 ADR-001
  When lint-knowledge 运行
  Then 输出 dependency-kind-mismatch 且 suggestion 指名 Source Trace

Scenario: 断链的 Produces 被指名方向
  Given 一份 accepted proposal 的 Produces 指向存在但未回链的 decision
  When lint-knowledge 运行
  Then 输出 produces-link-integrity Warning 并指名缺失的回链方向

Scenario: Produces 段的散文引用不算产出
  Given 一份 Produces 段在说明文字里引用了另一个决策 id 的提案
  When 解析该提案的产出物
  Then 只有内联标题与列表项开头的 id 被计为产出

## Dependencies

- REQ-KNOWLEDGE-PREFIX-REGISTRY

## Source Trace

- decision: ADR-002（门禁合流与 orphan-spec 分阶段定案，2026-08-01）
- proposal: LEP-001
- 实测证据: specs/ 下 37/74 无 satisfies；graph/plan 验证器不在
  lint-knowledge --gate 路径（main.rs cmd_lint_knowledge 仅调 lint_corpus）
- 同类先例: agent-chat 工作区四份既有需求文档把 ADR 塞在 ## Dependencies
- staged contract: specs/task-pipeline-integrity-gate.spec.md
- retirement contract: specs/task-orphan-baseline-retirement.spec.md
