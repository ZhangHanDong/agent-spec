spec: task
name: "Intent-Aware Affected Projection"
tags: [atlas, intent-compiler, traceability]
satisfies: [REQ-INTENT-AWARE-AFFECTED]
depends: [task-atlas-explore-flow-impact, task-code-graph-ir-bindings, task-requirements-compiler-plan-dag]
risk: A
---

## Intent

把 provider-neutral code impact 与 agent-spec 的 binding、requirement plan、work unit、
Task Contract、scenario、显式 test selector、obligation、worktree 和 VCS 事实连接成一个
确定性报告，并把所有断链保留为 typed gap。

## Decisions

- `src/spec_knowledge/code_graph.rs` 定义 provider-neutral impact contract；Rust Atlas adapter
  只负责投影，不把 `rust_atlas` 类型泄漏到消费层。
- `src/spec_knowledge/intent_impact.rs` 是唯一 join 层，输出 schema
  `agent-spec/intent-compiler/intent-impact-v1`。
- authoritative selector 只能来自解析后的 Task Contract；test obligation 的 slug 只能标为
  candidate。
- provider error、stale、fingerprint mismatch、truncation、unbound node、missing plan/spec/scenario/
  selector/obligation、unobserved worktree/VCS 都使用稳定 gap code。

## Boundaries

### Allowed Changes
- src/spec_knowledge/code_graph.rs
- src/spec_knowledge/intent_impact.rs
- src/spec_knowledge/test_obligations.rs
- src/spec_knowledge/mod.rs
- src/main.rs
- knowledge/requirements/req-intent-aware-affected.md
- specs/task-intent-aware-affected.spec.md
- docs/atlas-roadmap.md
- README.md
- AGENTS.md

### Symbols
- rust-atlas: rust_atlas::affected::affected_paths
- rust-atlas: rust_atlas::impact::impact

### Forbidden
- 不从文件名、scenario 名或 source path 推断 authoritative test
- 不在 join 层重新构建或修改 provider graph
- 不把缺失链路过滤掉

## Out of Scope

- 执行 quality provider
- 重跑 lifecycle 或测试
- 自动创建 Git worktree
- 非 Rust provider 实现

## Completion Criteria

场景: 完整意图链确定性投影
  测试: test_intent_aware_affected_projects_full_chain_deterministically
  假设 affected node 有 current binding、plan、spec、scenario、selector、obligation、worktree 与 VCS
  当 intent-aware join 连续运行两次
  那么 两次 JSON byte-identical 且完整链路字段相同

场景: 未绑定 node 保持可见
  测试: test_intent_aware_affected_reports_unbound_node_gap
  假设 provider 返回一个 bindings 中不存在的 node
  当 intent-aware join 运行
  那么 node 仍在 affected 列表且 gaps 含 affected-node-unbound

场景: 缺失显式 selector 不做推断
  测试: test_intent_aware_affected_reports_missing_explicit_selector
  假设 scenario 没有 Task Contract Test selector
  当 join 消费 test obligation
  那么 scenario selector 为空且 gaps 含 selector-missing

场景: provider authority 与 truncation 不丢失
  测试: test_intent_aware_affected_preserves_provider_and_truncation_gaps
  假设 provider unavailable、stale、binding fingerprint mismatch 或 impact truncated
  当报告序列化
  那么 对应 typed gaps 均保留且排序稳定

场景: 文件名永不生成 authoritative test
  测试: test_intent_aware_affected_never_infers_tests_from_filenames
  假设 affected path 命名为 tests/feature_test.rs
  当 join 生成报告
  那么 authoritative selectors 仍只来自显式 contract selector

场景: 缺失执行上下文保持可见
  测试: intent_aware_affected_reports_missing_worktree_vcs_and_obligation_context
  假设 worktree manifest、VCS context 或 scenario test obligation 不存在
  当 intent-aware join 运行
  那么 对应 worktree-manifest-missing、worktree-unobserved、vcs-unobserved 与 obligation-unmapped gaps 均保留
