spec: task
name: "第 2 章: Harness 工程 - 给 Agent 建造可工作的世界"
inherits: book
tags: [book, chapter, harness, architecture]
---

## Intent

把 Harness 工程定义为围绕 agent 的工程操作系统：工具、沙箱、上下文、权限、记忆、反馈和观测共同把不稳定推理变成可持续工程行为。本章为后续 spec 与 Rust 实战提供环境模型。

## Decisions

- 用五层模型组织本章：Model、Agent Loop、Tools、Repo Knowledge、Feedback
- 对比 Prompt engineering、Context engineering、Agent engineering、Harness engineering
- 引入 Anthropic long-running harness 与 Claude Agent SDK 作为设计参照
- 不把 harness 缩减为某个 SDK、CLI 或平台

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch02-harness-engineering.md

### Forbidden
- Do not write a vendor product survey
- Do not conflate Harness.io with the AI harness concept
- Do not give implementation code before chapter 8

## Completion Criteria

Rule: harness-layer-model-is-clear - Harness 五层模型必须清晰

Scenario: five-layer model is explained
  Review: human
  Test: ch02_five_layer_model_is_explained
  Given the chapter architecture section
  When the reader scans the model
  Then the five layers are named and ordered
  And each layer has one concrete artifact example

Rule: harness-boundaries-are-precise - 相邻概念边界必须讲清楚

Scenario: adjacent concepts are distinguished
  Review: human
  Test: ch02_adjacent_concepts_are_distinguished
  Given the comparison table
  When Prompt, Context, Agent, and Harness engineering are compared
  Then each concept has a different concern and output artifact
  And Harness is presented as the system-level discipline

Rule: long-running-agent-patterns-are-actionable - 长任务模式要能落地

Scenario: long-running harness pattern is reusable
  Review: human
  Test: ch02_long_running_pattern_is_reusable
  Given the section on long-running agents
  When the reader applies the pattern
  Then they can identify initializer state, progress file, task queue, and handoff rules

## Out of Scope

- Full Claude Agent SDK tutorial
- MCP protocol internals
- Rust code implementation

