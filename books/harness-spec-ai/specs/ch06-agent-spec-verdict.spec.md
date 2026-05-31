spec: task
name: "第 6 章: 从 Spec 到 Verdict - agent-spec 的任务合约"
inherits: book
tags: [book, chapter, agent-spec, verdict]
---

## Intent

把本仓库 agent-spec 作为核心案例，说明 spec 如何从文档变成可执行门禁。读者应理解 Task Contract、Rule/Example、Test selector、coverage matrix、lifecycle、guard、explain 之间的闭环。

## Decisions

- 以本仓库当前 BDD-spine 分支为基线
- 对比 Spec Kit 和 OpenSpec 时只保留与 agent-spec 差异化直接相关的部分
- 强调 `skip != pass` 和 `provenance` 区分 computational / inferential evidence
- coverage matrix 是 observability，不在本章宣称它改变 gate 语义
- 图文预算遵循 `visual-budget.md` 第 6 章: 6,000-7,000 字符、5-6 个视觉单元、至少 2 个 Mermaid 图

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch06-agent-spec-verdict.md

### Forbidden
- Do not claim agent-spec replaces all planning tools for every team
- Do not treat AI verifier output as mechanical proof
- Do not hide current Rust/Cargo bias of TestVerifier

## Completion Criteria

Rule: task-contract-is-explained - Task Contract 架构要完整

Scenario: contract parts are mapped to review responsibility
  Review: human
  Test: ch06_contract_parts_map_to_review
  Given the Task Contract section
  When Intent, Decisions, Boundaries, and Completion Criteria are introduced
  Then each part states who reviews it and what failure it prevents

Rule: lifecycle-verdict-is-central - verdict 通道是中心

Scenario: lifecycle flow ends in verdict
  Review: human
  Test: ch06_lifecycle_flow_ends_in_verdict
  Given the lifecycle section
  When pass, fail, skip, uncertain, and pending_review are explained
  Then `skip != pass` is explicit
  And the reader knows why all scenarios must be covered

Rule: matrix-explains-traceability - 覆盖矩阵说明可追溯性

Scenario: coverage matrix is shown as traceability
  Review: human
  Test: ch06_coverage_matrix_explains_traceability
  Given the coverage matrix section
  When a sample matrix is shown
  Then Rule, Scenario, Test, Verdict, and Provenance columns are explained
  And matrix is labeled as observability rather than gate

## Out of Scope

- Implementing new agent-spec features
- Full source-code walkthrough of every verifier
- Cross-language runner design
