spec: task
name: "第 3 章: Spec 驱动开发 - 把意图变成可传递工件"
inherits: book
tags: [book, chapter, spec-driven, tools]
---

## Intent

解释 Spec 驱动开发如何把人类意图拆成可传递、可审查、可执行的工件链。读者应理解 Spec Kit、Kiro、AGENTS.md、SKILL.md 和任务 spec 的分工，而不是把它们看成互相竞争的模板格式。

## Decisions

- 以 GitHub Spec Kit 的 `Spec -> Plan -> Tasks -> Implement` 作为主线
- Kiro/EARS 作为产品化 spec-driven IDE 的对照案例
- AGENTS.md、SKILL.md、task spec 作为不同粒度的推理时规约
- 只保留国内 IDE 对 spec 标准化支持的一张映射表，不写全景报道
- 图文预算遵循 `visual-budget.md` 第 3 章: 6,000-7,000 字符、5-6 个视觉单元、至少 1 个 Mermaid 图

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch03-spec-driven-development.md

### Forbidden
- Do not list every Spec Kit command without explaining its role
- Do not rely on stale star counts as a main argument
- Do not present vendor self-reported data as independent measurement

## Completion Criteria

Rule: spec-chain-is-explained - Spec 工件链必须成体系

Scenario: Spec Kit flow is mapped to intent transfer
  Review: human
  Test: ch03_speckit_flow_maps_to_intent_transfer
  Given the Spec Kit section
  When the four-step flow is explained
  Then each step states its input, output, and handoff target
  And the reader sees why Markdown artifacts matter

Rule: spec-artifacts-have-roles - 不同 spec 工件分工明确

Scenario: AGENTS Skills and task specs are separated
  Review: human
  Test: ch03_agent_artifacts_have_distinct_roles
  Given the artifact comparison table
  When AGENTS.md, SKILL.md, MCP, and task spec are compared
  Then each row answers what context it carries and when it is loaded

Rule: caveats-are-visible - 动态事实必须有警示

Scenario: market facts are labeled as snapshots
  Review: human
  Test: ch03_market_facts_are_labeled_snapshots
  Given the chapter mentions stars, releases, or vendor claims
  When the reader checks footnotes or inline caveats
  Then dates and source types are visible
  And uncertain claims are not used as structural proof

## Out of Scope

- Tool installation tutorials
- Deep product review of every IDE
- Model training alignment details
