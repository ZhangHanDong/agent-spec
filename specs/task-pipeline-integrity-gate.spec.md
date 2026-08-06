spec: task
name: "Pipeline Integrity Gate"
tags: [knowledge, lint, gate]
satisfies: [REQ-PIPELINE-INTEGRITY-GATE]
depends: [task-knowledge-prefix-registry]
risk: A
---

## Intent

把三套知识图验证器（corpus、requirement graph、requirement plan）的诊断
合流进 `lint-knowledge --gate`，并新增 `orphan-spec`（带基线、Info 起步）、
`dependency-kind-mismatch`、`produces-link-integrity` 三条流水线完整性
规则，让跳层从主观判断变成机械诊断。

## Decisions

- 合流方式：`cmd_lint_knowledge` 在 corpus lint 之后追加调用
  `build_requirement_graph` 与 `build_requirement_plan`，诊断并入同一份
  输出与 SARIF，规则名保持既有字符串不变。
- `orphan-spec`：需求语料非空且**任务合约**无 `satisfies:` 时触发；引入版本
  严重级别为 Info；诊断文本给出两条补救动作（声明 satisfies 或进基线）。
  project、org 与 capability 层级的 spec 不承载 satisfies，不在范围内。
- 基线条目按路径身份匹配（规范化后比较），支持相对仓库根、相对基线文件与
  绝对路径三种写法。
- 基线文件：`.agent-spec/orphan-baseline.json`，仅允许人为缩小；lint 启动
  时读取，缺失视为空基线。
- `dependency-kind-mismatch`：`## Dependencies` 行内 id 前缀命中 `ADR-` 或
  `LEP-` 即触发 Warning，suggestion 固定为「move to `## Source Trace`」。
- `produces-link-integrity`：仅对 status accepted 的 proposal 生效；目标
  不存在沿用既有 `produces-dangling`，目标存在但 decision `## Source Trace`
  未含该 proposal id 时新发 Warning 并指名缺失方向。

## Boundaries

### Allowed Changes
- src/spec_knowledge/lint.rs
- src/spec_knowledge/governance.rs
- src/spec_knowledge/proposal.rs
- src/spec_knowledge/requirement_graph.rs
- src/spec_knowledge/requirement_plan.rs
- src/spec_knowledge/mod.rs
- src/main.rs
- .agent-spec/orphan-baseline.json
- fixtures/**
- specs/task-pipeline-integrity-gate.spec.md

### Forbidden
- 不改变既有规则名与既有诊断的严重级别
- 不在本合约内把 orphan-spec 升为 Warning 或 Error
- 不自动写入或扩充基线文件

## Out of Scope

- `knowledge` 命令空间重组（ADR-002 已裁决另立提案）
- orphan-spec 的 Warning/Error 升级（后续版本合约）
- scenario 级 satisfies 粒度

## Completion Criteria

场景: 门禁吸收图诊断
  测试: test_gate_includes_graph_and_plan_diagnostics
  假设 fixture 中一份 spec 的 satisfies 指向不存在的需求
  当 lint-knowledge --gate 运行
  那么 输出含 dangling-spec-coverage 且退出码非零

场景: 基线内存量孤儿放行
  测试: test_orphan_baseline_exempts_listed_specs
  假设 基线文件列出 fixture 的全部存量孤儿 spec
  当 lint-knowledge --gate 运行
  那么 无 orphan-spec 诊断且退出码为零

场景: 基线外新孤儿被指名
  测试: test_new_orphan_spec_diagnosed_with_remedies
  假设 需求语料非空且新增一份无 satisfies 的 spec 未进基线
  当 lint-knowledge 运行
  那么 输出 orphan-spec Info 且文本含两条补救动作

场景: Dependencies 中的决策 id 被纠偏
  测试: test_dependency_kind_mismatch_suggests_source_trace
  假设 fixture 需求文档的 Dependencies 列出 ADR-001
  当 lint-knowledge 运行
  那么 输出 dependency-kind-mismatch 且 suggestion 含 Source Trace

场景: 缺失回链被指名方向
  测试: test_produces_link_integrity_names_missing_backlink
  假设 accepted proposal 的 Produces 指向存在但未回链的 decision
  当 lint-knowledge 运行
  那么 输出 produces-link-integrity Warning 且指名缺失的回链方向

场景: 需求语料为空时不误报
  测试: test_orphan_spec_silent_without_requirements
  假设 工作区无 knowledge/requirements/ 文档
  当 lint-knowledge 运行
  那么 无 orphan-spec 诊断

场景: 非任务合约不算孤儿
  测试: test_orphan_spec_applies_only_to_task_contracts
  假设 specs 下存在一份 project 层级的 spec 且它不带 satisfies
  当 lint-knowledge 运行
  那么 该 project spec 不产生 orphan-spec 诊断
