spec: task
name: "AiVerifier 最小骨架"
inherits: project
tags: [done, bootstrap, verify, ai, gateway, report, phase4]
---

## 意图

为 `agent-spec` 建立一个可追踪的 `AiVerifier` 最小骨架，
先把 AI 证据模型、CLI/gateway 开关和 `uncertain` 语义定清楚，
而不是直接引入真实模型调用。

## 已定决策

- `AiVerifier` 首批只支持 `off` 与 `stub` 两种模式
- `stub` 模式不会声称通过，只会把未被机械 verifier 覆盖的场景标成 `uncertain`
- `uncertain` 结果必须附带结构化 `AiAnalysis` 证据，说明尚未配置真实 AI backend

## 边界

### 允许修改
- src/spec_core/**
- src/spec_verify/**
- src/spec_gateway/**
- src/spec_report/**
- src/**
- specs/**
- README.md

### 禁止做
- 不要在 `stub` 模式下把场景判成 `pass`
- 不要改变默认验证行为为自动启用 AI verifier
- 不要输出没有证据的 `uncertain`

## 完成条件

场景: stub 模式把未覆盖场景标成 uncertain
  测试:
    过滤: test_verify_with_ai_mode_stub_marks_uncovered_scenarios_uncertain
  假设 某个任务级 spec 的场景未被机械 verifier 覆盖
  当 gateway 使用 `AiMode::Stub` 执行验证
  那么 场景 verdict 为 `uncertain`
  并且 结果包含 `AiAnalysis` 证据

场景: 默认 off 模式保持 skip 语义
  测试:
    过滤: test_verify_default_keeps_uncovered_scenarios_skipped
  假设 同一个未被覆盖的场景
  当 gateway 使用默认 AI 模式执行验证
  那么 场景 verdict 仍然是 `skip`
  并且 不会附带 `AiAnalysis` 证据

场景: 文本报告输出 AI 证据
  测试:
    过滤: test_format_verification_text_includes_ai_analysis_evidence
  假设 某个验证结果包含 `AiAnalysis` 证据
  当 report 以 text 格式输出
  那么 输出中包含 AI model 与 confidence
  并且 输出中包含 reasoning 摘要
