spec: task
name: "输出保真度分级（Context Fidelity）"
inherits: project
tags: [done, bootstrap, lifecycle, report, phase7]
depends: []
estimate: 0.5d
---

## 意图

为 lifecycle 的 `--format` 增加 `compact` 和 `diagnostic` 两个输出级别，
让 agent 在多轮 ReAct 循环中按需选择信息密度——
首轮用 `json` 获取完整 evidence，后续轮用 `compact` 节省 token。

灵感来源：Attractor 的 6 级 context fidelity mode。

## 已定决策

- `compact` 格式输出单行摘要，每个场景用 `✓`/`✗`/`⊘`/`?` 标记
- `diagnostic` 格式在 `json` 基础上追加完整 test stdout/stderr
- 现有 `json`、`text`、`markdown` 格式行为不变
- `compact` 作为新格式独立存在，不修改 `text` 的行为

## 边界

### 允许修改
- src/spec_report/**
- src/main.rs

### 禁止做
- 不要修改现有 `json`、`text`、`markdown` 格式的输出
- 不要让 `compact` 格式丢失 pass/fail 计数
- 不要让 `diagnostic` 格式在无测试输出时报错

## 完成条件

场景: compact 格式输出单行摘要
  测试:
    包: agent-spec
    过滤: test_compact_format_outputs_single_line_summary
  假设 某个任务级 spec 有 3 个场景，verdict 分别为 pass、fail、skip
  当 lifecycle 使用 `--format compact` 输出
  那么 输出包含 `✓` `✗` `⊘` 标记
  并且 输出包含 pass/fail/skip 计数
  并且 输出不超过 3 行

场景: diagnostic 格式包含测试原始输出
  测试:
    包: agent-spec
    过滤: test_diagnostic_format_includes_raw_test_output
  假设 某个场景绑定了真实测试且测试 fail
  当 lifecycle 使用 `--format diagnostic` 输出
  那么 输出包含 JSON 结构
  并且 evidence 中包含完整的 test stdout 和 stderr

场景: 现有 json 格式不受影响
  测试:
    包: agent-spec
    过滤: test_existing_json_format_unchanged
  假设 某个任务级 spec 的 lifecycle 已有测试通过
  当 lifecycle 使用 `--format json` 输出
  那么 输出结构与引入新格式前一致
