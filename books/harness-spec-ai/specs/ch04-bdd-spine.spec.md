spec: task
name: "第 4 章: BDD 脊柱 - Rule、Example 与可执行规格"
inherits: book
tags: [book, chapter, bdd, formulation]
---

## Intent

把 BDD/Cucumber 的原始语义接入本书主线：Rule 表达规则，Example/Scenario 证明规则，Given/When/Then 约束上下文、事件与结果。读者应理解为什么 BDD 是 spec-driven AI development 的语义脊柱。

## Decisions

- 以 Dan North 的 BDD 起源和 Cucumber 的 Discovery/Formulation/Automation 三层实践为依据
- 明确 `Example` 与 `Scenario` 在 Cucumber 语境中同义
- 用 `Rule -> Example -> Test Selector -> Verdict` 连接到 agent-spec BDD-spine
- 不把本章写成 Cucumber 使用教程
- 图文预算遵循 `visual-budget.md` 第 4 章: 6,000-7,000 字符、5-6 个视觉单元、至少 2 个 Mermaid 图

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch04-bdd-spine.md

### Forbidden
- Do not reduce BDD to test automation
- Do not introduce Gherkin features that agent-spec does not plan to absorb
- Do not overuse UI click/type examples as good BDD style

## Completion Criteria

Rule: bdd-three-practices-are-connected - 三层实践必须闭环

Scenario: Discovery Formulation Automation are explained as a loop
  Review: human
  Test: ch04_bdd_three_practices_form_loop
  Given the BDD overview section
  When the three practices are introduced
  Then Discovery, Formulation, and Automation each have a precise role
  And the text shows how Automation feeds back into Discovery

Rule: rule-example-is-core - Rule 与 Example 是本章核心

Scenario: Rule Example relationship is explicit
  Review: human
  Test: ch04_rule_example_relationship_is_explicit
  Given the Formulation section
  When Cucumber/Gherkin semantics are explained
  Then Rule is defined as behavior constraint
  And Example/Scenario is defined as a concrete proof case

Rule: agent-spec-link-is-concrete - 必须落到 agent-spec

Scenario: BDD spine maps to agent-spec
  Review: human
  Test: ch04_bdd_spine_maps_to_agent_spec
  Given the agent-spec subsection
  When the mapping is shown
  Then `Rule -> Example -> Test Selector -> Verdict` appears
  And the reader understands why this is stronger than prose acceptance criteria

## Out of Scope

- Full Cucumber setup
- `.feature` import/export
- Scenario Outline expansion
