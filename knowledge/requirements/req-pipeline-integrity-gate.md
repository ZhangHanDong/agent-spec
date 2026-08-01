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

[REQ-PIPELINE-INTEGRITY-GATE-ORPHAN] 当 `knowledge/requirements/` 含至少一份需求文档时，无 `satisfies:` 声明的任务 spec MUST 触发 `orphan-spec` 诊断；引入版本的严重级别 MUST 为 Info，且诊断文本 MUST 指名补救动作（声明 `satisfies: [REQ-*]` 或进基线）。

[REQ-PIPELINE-INTEGRITY-GATE-BASELINE] `orphan-spec` MUST 支持仓库内基线文件豁免存量 spec；基线中不存在的新孤儿 MUST 照常诊断；从基线移除的条目 MUST NOT 被自动加回。

[REQ-PIPELINE-INTEGRITY-GATE-KIND-MISMATCH] 需求文档 `## Dependencies` 中出现 `ADR-` 或 `LEP-` 前缀 id 时 MUST 触发 `dependency-kind-mismatch` 诊断，suggestion MUST 指名把该 id 移至 `## Source Trace`。

[REQ-PIPELINE-INTEGRITY-GATE-PRODUCES] status 为 accepted 的 proposal MUST 通过 `produces-link-integrity` 校验：`## Produces` 目标存在，且目标 decision 的 `## Source Trace` 回链该 proposal id，任一方向缺失均为 Warning 并指名缺失方向。

## Scenarios

Scenario: 门禁包含图诊断
  Given 一份 satisfies 指向不存在需求的 spec
  When lint-knowledge --gate 运行
  Then 输出含 dangling-spec-coverage 诊断且退出码非零

Scenario: 新孤儿 spec 被指名
  Given 需求语料非空且新增一份无 satisfies 的 spec 未进基线
  When lint-knowledge 运行
  Then 输出 orphan-spec 诊断并给出两条补救动作

Scenario: 基线豁免存量
  Given 基线文件列出全部 37 份存量孤儿 spec
  When lint-knowledge --gate 运行
  Then 存量不产生 orphan-spec 诊断且退出码为零

Scenario: Dependencies 里的 ADR 被纠偏
  Given 一份需求文档在 ## Dependencies 列出 ADR-001
  When lint-knowledge 运行
  Then 输出 dependency-kind-mismatch 且 suggestion 指名 Source Trace

Scenario: 断链的 Produces 被指名方向
  Given 一份 accepted proposal 的 Produces 指向存在但未回链的 decision
  When lint-knowledge 运行
  Then 输出 produces-link-integrity Warning 并指名缺失的回链方向

## Dependencies

- REQ-KNOWLEDGE-PREFIX-REGISTRY

## Source Trace

- decision: ADR-002（门禁合流与 orphan-spec 分阶段定案，2026-08-01）
- proposal: LEP-001
- 实测证据: specs/ 下 37/74 无 satisfies；graph/plan 验证器不在
  lint-knowledge --gate 路径（main.rs cmd_lint_knowledge 仅调 lint_corpus）
- 同类先例: agent-chat 工作区四份既有需求文档把 ADR 塞在 ## Dependencies
- staged contract: specs/roadmap/task-pipeline-integrity-gate.spec.md
