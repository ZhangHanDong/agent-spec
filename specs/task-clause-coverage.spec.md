spec: task
name: "Clause Coverage"
tags: [knowledge, lint, coverage, governance]
satisfies: [REQ-CLAUSE-COVERAGE]
risk: A
---

## Intent

把 MUST 条款与场景之间的空白连上：需求文档用 BDD `Rule: <条款 id>` 声明
场景归属，新规则 `clause-uncovered` 报出无人认领的 MUST 条款，覆盖率报在
`requirements status`。只认显式归属、skip 不算覆盖——本规则存在的理由就是
「看上去绿」曾掩盖过一条被推翻的治理条款。

## Decisions

- 归属解析复用需求文档 `## Scenarios` 段内的 `Rule:` 行：Rule id 即条款 id，
  其后的场景归属于该条款，直到下一个 `Rule:` 行。不新增语法。
- `clause_coverage(doc) -> ClauseCoverage`：返回每条 MUST 条款的
  `covered` / `uncovered` / `attributed_but_skipped`，以及归属到不存在
  条款 id 的 `unknown_rule_ids`。
- 覆盖只认显式归属：不做关键词、文本相似度或位置邻近推断。
- skip 判定的处理分两层：静态 lint 只能看到「有无归属场景」，因此
  `clause-uncovered` 在静态层判定未归属；「归属场景判定为 skip」由消费
  verdict 的一侧（status）判定为未覆盖。二者用同一个 `ClauseCoverage` 结构，
  由调用方决定是否传入 verdict。
- `clause-uncovered` 为 Info，基线文件 `.agent-spec/clause-baseline.json`
  形状与 orphan-baseline 一致（`{"clauses": ["REQ-X-ALPHA", ...]}`），只准
  人为缩小。
- 归属到不存在条款 id 的 Rule 触发 `clause-attribution-unknown`（Warning），
  与未覆盖区分：前者是写错了，后者是没写。
- `requirements status` 增覆盖行；`trace` 不动。

## Boundaries

### Allowed Changes
- src/spec_knowledge/lint.rs
- src/spec_knowledge/requirement.rs
- src/spec_knowledge/status.rs
- src/spec_knowledge/mod.rs
- src/main.rs
- .agent-spec/clause-baseline.json
- fixtures/**
- specs/task-clause-coverage.spec.md

### Forbidden
- 不以关键词或文本相似度推断覆盖
- 不改变 trace 的输出
- 不自动写入或扩充基线文件
- 不在本合约内把 clause-uncovered 升为 Warning 或 Error

## Out of Scope

- 存量 553 条条款的补覆盖工作
- SHOULD/MAY 条款的覆盖
- 严重级别升级的版本与前提（后续决策）
- 归属正确性的语义校验

## Completion Criteria

场景: 场景按条款 id 归属
  测试: test_clause_coverage_attributes_by_rule_id
  假设 一份需求文档的场景归属在 Rule REQ-X-ALPHA 之下
  当 计算该文档的条款覆盖
  那么 条款 REQ-X-ALPHA 出现在 covered 集合中

场景: 文字相近但未归属不算覆盖
  测试: test_clause_coverage_ignores_textual_similarity
  假设 一条 MUST 条款与一个措辞高度相近但未归属任何 Rule 的场景
  当 计算条款覆盖
  那么 该条款出现在 uncovered 集合中

场景: 判定为 skip 的归属场景不计覆盖
  测试: test_clause_coverage_excludes_skipped_scenarios
  假设 一条条款的唯一归属场景带有 skip 判定
  当 以 verdict 计算条款覆盖
  那么 该条款不出现在 covered 集合中

场景: 未覆盖条款被指名
  测试: test_clause_uncovered_names_clause_and_document
  假设 一份含无归属场景的 MUST 条款的需求文档
  当 lint_requirement 运行
  那么 输出 clause-uncovered 诊断且消息含该条款 id

场景: 基线豁免存量条款
  测试: test_clause_baseline_exempts_listed_clauses
  假设 基线文件列出某未覆盖条款
  当 lint-knowledge --gate 运行
  那么 该条款不产生诊断且退出码为零

场景: 归属到不存在的条款被指名
  测试: test_clause_attribution_unknown_id_is_named
  假设 一个 Rule id 在该文档条款列表中不存在
  当 计算条款覆盖
  那么 输出 clause-attribution-unknown 诊断且指名该 id

场景: 覆盖率报在 status 而非 trace
  测试: test_status_reports_coverage_and_trace_unchanged
  假设 一份条款部分覆盖的需求
  当 requirements status 与 trace 分别运行
  那么 status 输出含覆盖计数而 trace 输出不含覆盖字样

场景: 非 MUST 条款不触发诊断
  测试: test_clause_uncovered_skips_should_and_may
  假设 一份只含 SHOULD 与 MAY 条款且无场景的需求文档
  当 lint_requirement 运行
  那么 不输出 clause-uncovered 诊断

场景: 引入严重级别为 Info
  测试: test_clause_uncovered_ships_at_info
  假设 一条未覆盖且未进基线的 MUST 条款
  当 lint_requirement 运行
  那么 该诊断的严重级别为 Info
