spec: task
name: "第 7 章: Rust 实战一 - 解剖 codex-rs 的工程规约"
inherits: book
tags: [book, chapter, rust, codex-rs, agents-md]
---

## Intent

用 codex-rs 的 AGENTS.md 作为工业级 Rust agent harness 样本，逐条解释强约束如何让 agent 更可靠。读者应看到 Rust 类型系统、模块边界、测试命令和项目规约如何共同构成 harness。

## Decisions

- 本章围绕 AGENTS.md 约束解读，不做 codex-rs 全源码分析
- 重点讲 Rust trait、模块边界、测试反馈、crate 组织和反模式警告
- 引用 OpenAI Codex 仓库规则时使用短摘录和转述，避免长段复制
- 以工程判断解释规则价值，而不是做风格偏好争论
- 图文预算遵循 `visual-budget.md` 第 7 章: 6,000-7,000 字符、4-5 个视觉单元、至少 1 个 Mermaid 图

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch07-codex-rs-rust-contracts.md

### Forbidden
- Do not quote large blocks from third-party AGENTS.md
- Do not present codex-rs rules as universal Rust law
- Do not write a full Codex CLI architecture chapter here

## Completion Criteria

Rule: codex-agents-md-is-treated-as-harness - AGENTS.md 被作为 harness 样本解读

Scenario: rules are explained by reliability effect
  Review: human
  Test: ch07_rules_explained_by_reliability_effect
  Given a codex-rs AGENTS.md rule is mentioned
  When the text analyzes it
  Then the analysis states what agent failure mode the rule reduces
  And avoids pure style commentary

Rule: rust-specific-constraints-are-concrete - Rust 约束要具体

Scenario: Rust trait and module constraints are explained
  Review: human
  Test: ch07_rust_constraints_are_concrete
  Given the Rust-specific section
  When RPITIT, async_trait avoidance, bool parameter avoidance, and module size limits are discussed
  Then each topic has a concrete before/after or rationale

Rule: feedback-loop-is-visible - 工具链反馈环必须出现

Scenario: just cargo insta feedback loop is described
  Review: human
  Test: ch07_feedback_loop_is_visible
  Given the testing section
  When `just`, `cargo test`, and snapshot testing are introduced
  Then they are framed as harness feedback, not just developer convenience

## Out of Scope

- OpenAI API usage
- Implementing a Rust agent
- Detailed codex-core module map
