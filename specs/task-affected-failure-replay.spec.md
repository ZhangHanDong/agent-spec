spec: task
name: "Affected Failure Explanation and Replay"
tags: [atlas, intent-compiler, trace]
satisfies: [REQ-AFFECTED-FAILURE-REPLAY]
depends: [task-intent-aware-affected, task-affected-execution-bundle, task-requirements-compiler-plan-dag]
risk: A
---

## Intent

把 intent-impact 与 quality decision 作为 trace schema v2 的附加确定性证据保存，使 replay、
failure explanation 和 trace graph 能回答 requirement 到代码路径、worktree、commit 与失败
verdict 的完整链路，而不重新运行任何分析器或模型。

## Decisions

- trace ledger v2 对 v1 additive backward-compatible；v1 缺少 affected context 时返回 typed gap。
- report digest 基于 canonical compact JSON；保存完整 selector、path/span、link gap、worktree、
  observed VCS 与 normalized quality outcome。
- `requirements replay` 与 `explain-failure` 读取最新存储记录，不调用 provider 或 verifier。

## Boundaries

### Allowed Changes
- src/spec_knowledge/intent_impact.rs
- src/spec_knowledge/trace_ledger.rs
- src/spec_knowledge/mod.rs
- src/main.rs
- knowledge/requirements/req-affected-failure-replay.md
- specs/task-affected-failure-replay.spec.md
- docs/atlas-roadmap.md
- README.md
- AGENTS.md

### Forbidden
- replay 时不调用 Atlas、Git diff、test、quality provider、skill 或模型
- 不从 v1 record 猜测 affected path
- 不把 missing trace 当作 empty success

## Out of Scope

- 分布式 trace store
- 远程 telemetry
- 自动修复失败

## Completion Criteria

场景: 最新 affected 链完整重放
  测试: test_affected_failure_replay_returns_latest_full_chain
  假设同一 requirement 有多条 v2 affected trace
  当 replay 查询该 requirement
  那么 只返回最新完整 chain 及其 digest、selector、paths、worktree、VCS 和 verdict

场景: lifecycle 与 quality 失败共同解释
  测试: test_affected_failure_replay_includes_lifecycle_and_quality_failures
  假设最新 record 同时有 lifecycle fail 与 quality fail
  当 explain-failure 查询
  那么 两类失败与 affected code context 同时出现

场景: gaps 原样重放
  测试: test_affected_failure_replay_preserves_link_gaps
  假设保存的 report 含 binding、selector 与 provider gaps
  当 replay 查询
  那么 typed gap codes 与保存值一致

场景: v1 trace 明示缺少 affected context
  测试: test_affected_failure_replay_reads_v1_with_missing_context_gap
  假设 trace 目录只含合法 v1 ledger
  当 affected replay 查询
  那么 v1 lifecycle evidence 可读且返回 affected-trace-missing gap

场景: replay 不触发外部执行
  测试: test_affected_failure_replay_never_reruns_tools_or_models
  假设 provider 与 tool sentinel 在被调用时失败
  当 replay 从已保存 ledger 读取
  那么 查询成功且 sentinel 调用数为零

场景: trace graph 包含 affected authority 链
  测试: test_affected_trace_graph_contains_saved_code_and_authority_chain
  假设 trace v2 保存了 affected path、selector、worktree、VCS 与 quality outcome
  当 requirement trace-graph 渲染 Mermaid
  那么 图包含 requirement、work unit、scenario、test、code span、path、worktree、branch、VCS 与 quality verdict
