spec: task
name: "第 1 章: AI Coding 的瓶颈已经从代码转向环境"
inherits: book
tags: [book, chapter, harness, thesis]
---

## Intent

写出全书的开场论点：AI coding 的主要瓶颈已经从模型能否写代码，转向环境能否给 agent 提供清晰上下文、约束和反馈。读者读完本章应理解为什么本书不讲提示词技巧，而讲 Harness 工程和 Spec 驱动。

## Decisions

- 以 OpenAI Codex 团队 harness engineering 案例作为开场事实，但不把内部吞吐量外推为普遍承诺
- 用 `vibe coding -> spec-driven -> harness engineering` 建立叙事递进
- 明确 Harness.io 公司与 AI harness 工程范式同名但无技术血缘
- 本章只建立问题和主线，不展开工具谱系细节
- 图文预算遵循 `visual-budget.md` 第 1 章: 4,000-5,000 字符、3-4 个视觉单元、至少 1 个 Mermaid 图

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch01-environment-bottleneck.md

### Forbidden
- Do not introduce a general AI history section
- Do not overclaim productivity numbers as enterprise baselines
- Do not discuss Rust implementation details in this chapter

## Completion Criteria

Rule: chapter-frames-the-bottleneck - 开章必须把瓶颈定位到环境

Scenario: thesis is stated in the first section
  Review: human
  Test: ch01_thesis_states_environment_bottleneck
  Given the chapter introduction
  When the reader finishes the first 800 Chinese characters
  Then the reader can restate that AI coding reliability depends on environment, not only model strength
  And the phrase `Harness = 装备 + 约束 + 反馈环` or an equivalent definition appears

Rule: chapter-uses-case-with-caveat - 案例有证据也有边界

Scenario: Codex case is used as evidence, not promise
  Review: human
  Test: ch01_codex_case_has_caveat
  Given the chapter cites OpenAI harness engineering
  When the throughput numbers are mentioned
  Then the text states they come from one internal team context
  And the text does not imply every team can reproduce the same numbers

Rule: chapter-defines-book-map - 读者知道后续路线

Scenario: chapter ends with book map
  Review: human
  Test: ch01_ends_with_book_map
  Given the chapter conclusion
  When the next chapters are previewed
  Then Harness, Spec, BDD, agent-spec, and Rust practice each get one concise positioning sentence

## Out of Scope

- Detailed Spec Kit command walkthrough
- Domestic IDE market comparison
- Writing chapter 2 material inside chapter 1
