spec: task
name: "第 5 章: Agent-Spec 标准家族 - AGENTS.md、SKILL.md、MCP"
inherits: book
tags: [book, chapter, agents-md, skills, mcp]
---

## Intent

说明 agent 时代的上下文标准家族如何分层协作：AGENTS.md 放项目稳态约束，SKILL.md 放可复用流程，MCP 给 agent 调用外部能力，task spec 描述当前任务。读者应能为自己的项目选择正确工件。

## Decisions

- 用上下文加载时机组织本章：always-on、on-demand、tool capability、per-task
- Skills 采用 progressive disclosure 解释，不展开每个厂商实现
- MCP 只讲能力边界，不讲协议细节
- 强调普通 Markdown + YAML frontmatter 的低接入成本

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch05-agent-spec-standards.md

### Forbidden
- Do not turn the chapter into standards history
- Do not duplicate chapter 3 tool taxonomy
- Do not give unsupported claims about future standard unification

## Completion Criteria

Rule: context-layers-are-actionable - 上下文分层要可操作

Scenario: four artifacts have different loading semantics
  Review: human
  Test: ch05_four_artifacts_have_loading_semantics
  Given the artifact table
  When the reader compares AGENTS.md, SKILL.md, MCP, and task spec
  Then each artifact has one loading moment and one authoring guideline

Rule: skill-pattern-is-reusable - SKILL.md 模板要能复用

Scenario: high-quality skill template is described
  Review: human
  Test: ch05_skill_template_is_reusable
  Given the SKILL.md section
  When the reader finishes it
  Then they know required frontmatter, optional scripts, references, and assets
  And they understand progressive disclosure

Rule: mcp-skill-division-is-clear - MCP 与 Skills 分工清楚

Scenario: MCP is capability and Skill is procedure
  Review: human
  Test: ch05_mcp_skill_division_is_clear
  Given the MCP subsection
  When MCP and Skills are compared
  Then MCP is described as callable capability
  And Skills are described as procedure and domain knowledge

## Out of Scope

- MCP implementation tutorial
- Vendor-specific adapter walkthrough
- Full AGENTS.md specification reproduction

