---
kind: requirement
id: REQ-VERIFICATION-DIAGNOSTICS
title: "Verification Diagnostics"
status: proposed
liveness: auto
tags: [verification, lifecycle, guard, diagnostics]
---

# Verification Diagnostics

## Problem

三处诊断质量问题让使用者无法从输出定位原因（robrix2 反馈 C2 / A5 / A4）：

- `Package:` 选择器指向非 workspace 成员时，cargo 以非零退出且零测试运行，
  verifier 给出与真正编译失败完全相同的 reason "cargo exited before any test
  ran (build/toolchain failure)"，拼错的包名与坏掉的构建无法区分。
- 边界层、atlas-symbols 层与 complexity 层各以一条合成 `ScenarioResult`
  （名字以 `[layer]` 开头）进入 `all_results`，`from_results` 直接计数，10 个场景的
  spec 显示 `passed: 11`；汇总没有把"场景"与"层"分开。
- `guard --change-scope` 的 help 只写 `staged, worktree`，解析器实际接受
  `none | staged | worktree | jj`。

## Requirements

[REQ-VERIFICATION-DIAGNOSTICS-PACKAGE-MEMBERSHIP] 场景绑定带 `Package:` 时 verifier MUST 先用 `cargo metadata --no-deps` 校验该包是 workspace 成员。

[REQ-VERIFICATION-DIAGNOSTICS-PACKAGE-REASON] 非成员包 MUST 得到 Uncertain verdict，且 reason 说明 "not a member of the cargo workspace" 并列出可用成员名。

[REQ-VERIFICATION-DIAGNOSTICS-PACKAGE-NO-RUN] 非成员包 MUST NOT 触发 `cargo test` 运行。

[REQ-VERIFICATION-DIAGNOSTICS-METADATA-FALLBACK] `cargo metadata` 不可用时 verifier MUST 回退到现有行为。

[REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-SCENARIOS] `VerificationSummary` MUST 新增 `scenarios` 计数（total/passed/failed/skipped/uncertain/pending_review），只统计名字不以 `[` 开头的真实场景。

[REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-LAYERS] `VerificationSummary` MUST 新增 `layers` 列表，每个合成层一项 `{name, verdict}`。

[REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-GATE-UNCHANGED] 顶层 `total/passed/failed/skipped/uncertain/pending_review` 的取值 MUST 保持不变以维持门禁与既有消费者的语义。

[REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-JSON-ORDER] struct 直接序列化时新增字段 MUST 位于既有字段之后。

[REQ-VERIFICATION-DIAGNOSTICS-TEXT-SUMMARY] 文本与 run log 的汇总行 MUST 分开显示场景计数与各层 verdict。

[REQ-VERIFICATION-DIAGNOSTICS-GUARD-HELP] `guard --change-scope` 的 help 文本 MUST 列出全部可接受值 `none, staged, worktree, jj`。

## Scenarios

Rule: REQ-VERIFICATION-DIAGNOSTICS-PACKAGE-MEMBERSHIP

Scenario: 成员校验发生在运行前
  Given 一个 `Package:` 指向不存在包的场景
  When verifier 运行
  Then 输出的 evidence 中不包含 cargo test 的 stdout

Rule: REQ-VERIFICATION-DIAGNOSTICS-PACKAGE-REASON

Scenario: 非成员包被指名
  Given `Package: not-a-crate`
  When verifier 运行
  Then step reason 文本包含 "not a member of the cargo" 并且列出真实成员名

Rule: REQ-VERIFICATION-DIAGNOSTICS-PACKAGE-NO-RUN

Scenario: 非成员包不跑测试
  Given `Package: not-a-crate`
  When verifier 运行
  Then verdict 输出为 uncertain 且 duration 小于 cargo 构建时间

Rule: REQ-VERIFICATION-DIAGNOSTICS-METADATA-FALLBACK

Scenario: 无 metadata 时行为不变
  Given 成员名单不可得
  When verifier 运行带 `Package:` 的场景
  Then 输出与之前相同的 reason 文本

Rule: REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-SCENARIOS

Scenario: 场景计数不含层
  Given 10 个真实场景与 1 个 `[boundaries]` 层结果
  When 计算 summary
  Then scenarios.total 输出为 10

Rule: REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-LAYERS

Scenario: 层单独列出
  Given 一个 `[boundaries]` 层结果 verdict 为 fail
  When 计算 summary
  Then layers 输出包含 name 为 boundaries 且 verdict 为 fail 的项

Rule: REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-GATE-UNCHANGED

Scenario: 顶层计数不变
  Given 10 个 pass 场景与 1 个 fail 的层结果
  When 计算 summary
  Then total 输出为 11 且 failed 输出为 1

Rule: REQ-VERIFICATION-DIAGNOSTICS-SUMMARY-JSON-ORDER

Scenario: 新字段在旧字段之后
  Given 一份含层结果的 summary
  When 以 struct 直接序列化为 json
  Then 字符串中 "failed" 首次出现的位置在 "scenarios" 之前

Rule: REQ-VERIFICATION-DIAGNOSTICS-TEXT-SUMMARY

Scenario: 文本汇总分行
  Given 10 个场景与 1 个 boundaries 层
  When 渲染文本报告
  Then 输出包含 "10/10 scenarios" 与 "boundaries: pass"

Rule: REQ-VERIFICATION-DIAGNOSTICS-GUARD-HELP

Scenario: guard help 列全
  Given guard 子命令
  When 渲染 --help
  Then 输出包含 "none, staged, worktree, jj"

## Dependencies

None.

## Source Trace

- practice feedback: robrix2 docs/agent-spec-feedback-2026-08.md C2 / A5 / A4
- triage: docs/robrix2-feedback-triage-2026-08.md 队列 2
- code: src/spec_verify/test_verifier.rs；src/spec_core/verify.rs from_results；src/main.rs Guard clap

## Open Questions

None.

## Next

Single exit: compile this requirement into a task contract with
`agent-spec requirements draft-specs`.
