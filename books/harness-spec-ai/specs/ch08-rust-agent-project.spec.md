spec: task
name: "第 8 章: Rust 实战二 - 从零搭一个 Spec 驱动 Agent 项目"
inherits: book
tags: [book, chapter, rust, agent, practical]
---

## Intent

设计一个最小但完整的 Rust agent 项目实战：AGENTS.md 约束项目，task spec 定义行为，provider trait 抽象模型，工具/MCP 接入能力，测试和 lifecycle 形成反馈。读者应能照着搭出自己的 v1 harness。

## Decisions

- 实战项目以最小可运行 agent harness 为目标，不追求生产全功能
- SDK 现状必须诚实说明：OpenAI/Anthropic 官方 Rust SDK 生态不如 Python/TypeScript 完整
- 默认示例用 provider trait 隔离具体模型 SDK
- 代码章节后续必须附完整 spec、plan、tasks、AGENTS.md 片段
- 图文预算遵循 `visual-budget.md` 第 8 章: 7,000-8,000 字符、6-7 个视觉单元、至少 2 个 Mermaid 图

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch08-rust-agent-project.md

### Forbidden
- Do not choose a framework solely by popularity
- Do not hide unofficial SDK maintenance risk
- Do not build a multi-agent platform in this chapter

## Completion Criteria

Rule: project-setup-is-spec-first - 实战必须 spec-first

Scenario: project starts from AGENTS and task spec
  Review: human
  Test: ch08_project_starts_from_specs
  Given the practical workflow
  When the first implementation step is described
  Then AGENTS.md and task spec appear before Rust implementation code
  And the first test selector is declared before final code

Rule: provider-abstraction-is-honest - Provider 抽象说明真实 SDK 风险

Scenario: SDK caveat and provider trait are both present
  Review: human
  Test: ch08_sdk_caveat_and_provider_trait_present
  Given the model provider section
  When SDK options are discussed
  Then unofficial SDK risk is named
  And a replaceable provider trait is used to isolate that risk

Rule: feedback-loop-reaches-verdict - 实战闭环到 verdict

Scenario: example ends with lifecycle check
  Review: human
  Test: ch08_example_ends_with_lifecycle
  Given the chapter walkthrough
  When the v1 feature is complete
  Then the reader sees lint, tests, and lifecycle results
  And failure handling describes how to iterate from evidence

## Out of Scope

- Production deployment
- Full MCP server implementation
- Benchmark and observability stack
