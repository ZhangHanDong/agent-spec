---
kind: requirement
id: REQ-ORPHAN-BASELINE-RETIREMENT
title: "Orphan Baseline Retirement"
status: accepted
liveness: auto
tags: [knowledge, governance, migration, orphan]
---

# Orphan Baseline Retirement

## Problem

`orphan-spec` 在 1.3.0 以 Info 和 36 项存量基线上线，避免一次性打断已有
仓库；如果基线继续具有豁免能力，过渡机制就会成为永久逃生口。ADR-002 已经
裁决基线只准缩小，并在后续 minor 把诊断升为 Warning。现在 36 份合约的场景
都有真实测试且 lifecycle 全部通过，应把完成的历史合约移出活动扫描集，并让
空基线成为不可逆的仓库状态。

## Requirements

[REQ-ORPHAN-BASELINE-RETIREMENT-ZERO] 仓库内 `.agent-spec/orphan-baseline.json` 的 `specs` 列表 MUST 为空。

[REQ-ORPHAN-BASELINE-RETIREMENT-DISPOSITION] 原基线中的 36 份任务合约 MUST 全部离开活动 spec 集；每份 MUST 在 `.agent-spec/archive/specs/` 保留历史合约，且归档摘要 MUST 保留其路径与验证证据。

[REQ-ORPHAN-BASELINE-RETIREMENT-EVIDENCE] 非重复的完成合约 MUST 只在当前路径与内容指纹匹配的 lifecycle 证据为全 Pass 后归档；已经存在字节等价归档副本的重复活动合约 MAY 删除重复副本。

[REQ-ORPHAN-BASELINE-RETIREMENT-NO-RESURRECTION] 非空 orphan baseline MUST 触发 `orphan-baseline-retired` Error，且其中的路径 MUST NOT 再豁免 `orphan-spec`。

[REQ-ORPHAN-BASELINE-RETIREMENT-SEVERITY] baseline 清零后的 `orphan-spec` MUST 为 Warning；升级为 Error MUST 留到 ADR-002 规定的下一个 major。

[REQ-ORPHAN-BASELINE-RETIREMENT-REMEDIATION] `orphan-spec` 诊断 MUST 给出两条当前合法出口：声明真实的 `satisfies: [REQ-*]`，或在当前 lifecycle 证据全 Pass 后归档已完成合约；MUST NOT 再建议把路径加入已退休 baseline。

[REQ-ORPHAN-BASELINE-RETIREMENT-ACTIVE] 本仓库活动扫描集中的每份 task spec MUST 声明至少一个存在的 requirement id；工具 MUST NOT 自动猜测 `satisfies:`、自动扩充 baseline 或无证据归档。

## Scenarios

Rule: REQ-ORPHAN-BASELINE-RETIREMENT-ZERO

Scenario: 仓库 baseline 清零
  Given 仓库的 orphan baseline 已完成迁移
  When 检查 baseline 的 specs 列表
  Then 列表为空

Rule: REQ-ORPHAN-BASELINE-RETIREMENT-DISPOSITION

Scenario: 三十六份存量均有历史出口
  Given 1.3.0 记录的 36 份存量孤儿合约
  When 检查活动与归档 spec 集及归档摘要
  Then 活动集不再含这些路径且归档集与摘要逐份保留

Rule: REQ-ORPHAN-BASELINE-RETIREMENT-EVIDENCE

Scenario: 归档只接受当前全绿证据
  Given 一份非重复的存量合约
  When 它进入 archive 计划
  Then 当前路径和内容指纹匹配的 lifecycle 结果全部为 Pass

Rule: REQ-ORPHAN-BASELINE-RETIREMENT-NO-RESURRECTION

Scenario: 非空 baseline 被拒绝且不豁免
  Given fixture baseline 重新加入一份孤儿合约
  When lint-knowledge 门禁收集诊断
  Then 输出 orphan-baseline-retired Error 与 orphan-spec Warning

Rule: REQ-ORPHAN-BASELINE-RETIREMENT-SEVERITY

Scenario: 新孤儿升级为 Warning
  Given 需求语料非空且新增任务合约没有 satisfies
  When lint-knowledge 运行
  Then orphan-spec 的严重级别为 Warning 而非 Info 或 Error

Rule: REQ-ORPHAN-BASELINE-RETIREMENT-REMEDIATION

Scenario: 诊断只给当前合法出口
  Given 一份没有 satisfies 的活动任务合约
  When 输出 orphan-spec 诊断
  Then 消息指向真实 requirement 链接或带全绿证据的归档且不建议加入 baseline

Rule: REQ-ORPHAN-BASELINE-RETIREMENT-ACTIVE

Scenario: 活动任务合约全部进入需求图
  Given 当前仓库的活动 spec 集
  When 构建 requirement plan
  Then 每份 task spec 的 satisfies 非空且不存在 dangling requirement id

## Dependencies

- REQ-PIPELINE-INTEGRITY-GATE
- REQ-REQUIREMENTS-COMPILER-PLAN-DAG

## Source Trace

- decision: ADR-002（Info → Warning → Error 分阶段升级与 shrink-only baseline）
- proposal: LEP-001
- release evidence: 1.3.0 release commit 88b794a 明确留下 36 项 baseline，待后续版本缩小后升级
- staged contract: specs/task-orphan-baseline-retirement.spec.md

## Open Questions

None.
