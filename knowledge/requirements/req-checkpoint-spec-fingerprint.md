---
kind: requirement
id: REQ-CHECKPOINT-SPEC-FINGERPRINT
title: "Checkpoint Spec Fingerprint"
status: proposed
liveness: auto
tags: [lifecycle, resume, checkpoint, drift]
---

# Checkpoint Spec Fingerprint

## Problem

`lifecycle --resume incremental` 把 checkpoint 里 verdict 为 pass 的场景直接标为
pass 并跳过重跑。checkpoint（`Checkpoint { spec_name, timestamp, vcs_ref, scenarios }`）
只按场景名匹配，不记录 spec 内容指纹，也不校验 spec 名：spec 在两次运行之间被
改动（步骤改写、测试选择器换绑、甚至同名场景语义完全变化）时，旧的 pass 仍被
"carried forward from checkpoint"，一次改 spec 让门禁变绿的操作不会被发现。
run log 早已记录 `spec_fingerprint`（`RunLogEntry`），checkpoint 没有对齐。robrix2
反馈 D1 的 checkpoint 部分，二次审查判为正确性问题并前移。

## Requirements

[REQ-CHECKPOINT-SPEC-FINGERPRINT-RECORD] 写入 checkpoint 时 MUST 记录当前 spec 文件的内容指纹（与 run log 使用同一算法）。

[REQ-CHECKPOINT-SPEC-FINGERPRINT-STALE-IGNORED] 恢复时 checkpoint 的指纹与当前 spec 指纹不一致 MUST 使 checkpoint 被整体忽略，任何 verdict 都不得 carried forward。

[REQ-CHECKPOINT-SPEC-FINGERPRINT-NAME-CHECK] 恢复时 checkpoint 的 spec_name 与当前 spec 名不一致 MUST 同样使 checkpoint 被整体忽略。

[REQ-CHECKPOINT-SPEC-FINGERPRINT-LEGACY] 没有指纹字段的旧 checkpoint MUST 视为过期而被忽略。

[REQ-CHECKPOINT-SPEC-FINGERPRINT-DIAGNOSTIC] checkpoint 被忽略时 lifecycle MUST 输出 warning 级 `checkpoint_diagnostic`（json 字段；text 模式写 stderr），消息说明原因。

[REQ-CHECKPOINT-SPEC-FINGERPRINT-FRESH-UNCHANGED] 指纹与名字都匹配时 incremental 与 conservative 的合并行为 MUST 与之前逐字节相同。

## Scenarios

Rule: REQ-CHECKPOINT-SPEC-FINGERPRINT-RECORD

Scenario: checkpoint 含指纹
  Given 一次带 run-log-dir 的 lifecycle 运行
  When 读取写出的 checkpoint.json
  Then 文件包含非空的 spec_fingerprint 字段

Rule: REQ-CHECKPOINT-SPEC-FINGERPRINT-STALE-IGNORED

Scenario: spec 改动后旧 pass 不再前移
  Given checkpoint 记录场景 A 为 pass 且当前 spec 指纹与之不同
  When 以 incremental 模式合并
  Then 场景 A 的 verdict 输出为本次运行的实际结果且不含 checkpoint:incremental 证据

Rule: REQ-CHECKPOINT-SPEC-FINGERPRINT-NAME-CHECK

Scenario: 换了 spec 名的 checkpoint 被忽略
  Given checkpoint 的 spec_name 与当前 spec 不同
  When 以 incremental 模式合并
  Then 输出的 verdict 均为本次运行的实际结果

Rule: REQ-CHECKPOINT-SPEC-FINGERPRINT-LEGACY

Scenario: 旧格式 checkpoint 被忽略
  Given 一份不含 spec_fingerprint 字段的 checkpoint.json
  When 加载并判定
  Then 判定结果输出为 stale

Rule: REQ-CHECKPOINT-SPEC-FINGERPRINT-DIAGNOSTIC

Scenario: 忽略时有诊断
  Given 一份 stale 的 checkpoint
  When lifecycle 以 --resume incremental --format json 运行
  Then 输出 json 包含 checkpoint_diagnostic 且 message 含 "spec content changed"

Rule: REQ-CHECKPOINT-SPEC-FINGERPRINT-FRESH-UNCHANGED

Scenario: 新鲜 checkpoint 行为不变
  Given checkpoint 指纹与名字都匹配
  When 以 incremental 模式合并
  Then 场景 verdict 输出为 pass 且带 checkpoint:incremental 证据

## Dependencies

None.

## Source Trace

- practice feedback: robrix2 docs/agent-spec-feedback-2026-08.md D1
- triage: docs/robrix2-feedback-triage-2026-08.md 队列 2（D1-checkpoint，前移）
- code: src/spec_core/verify.rs Checkpoint；src/main.rs save_checkpoint_with_timestamp / merge_checkpoint_results

## Open Questions

None.

## Next

Single exit: compile this requirement into a task contract with
`agent-spec requirements draft-specs`.
