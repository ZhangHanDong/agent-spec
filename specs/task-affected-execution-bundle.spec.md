spec: task
name: "Affected Execution Bundle"
tags: [atlas, intent-compiler, quality]
satisfies: [REQ-AFFECTED-EXECUTION-BUNDLE]
depends: [task-intent-aware-affected, task-quality-planning-bundles]
risk: A
---

## Intent

把 intent-impact report 编译成 Agent 可执行的检查、测试、gate、guidance 与 skill 清单，
同时保留每项选择理由和所有未闭合的证据缺口。

## Decisions

- 扩展现有 `quality.rs` execution-bundle 模型，不建立第二套 quality provider registry。
- risk 决定所需 QA evidence；fast check 来自 diagnostic/transformation role，acceptance gate
  来自 lifecycle、trace、显式 tests、adversarial review 与 required verification provider。
- risk A 要求 lifecycle、trace、targeted tests 与 adversarial review；risk B 要求 lifecycle
  与 trace；risk C 只要求 lifecycle。只有实际被选择的 provider 进入 `quality_profile`，并
  保留 executable、argv、cwd、timeout 与 output limit。
- heuristic test candidates 与 authoritative selectors 分字段保存。
- guidance 只按 affected source path 匹配；skill receipt 是 immutable provenance，不进入
  acceptance evidence。

## Boundaries

### Allowed Changes
- src/spec_knowledge/intent_impact.rs
- src/spec_knowledge/quality.rs
- src/spec_knowledge/guidance.rs
- src/spec_knowledge/mod.rs
- src/main.rs
- knowledge/requirements/req-affected-execution-bundle.md
- specs/task-affected-execution-bundle.spec.md
- docs/atlas-roadmap.md
- README.md
- AGENTS.md

### Forbidden
- 不执行 provider、quality tool、test 或 skill
- 不把 candidate test 提升为 authoritative
- 不把 skill receipt 计入通过证据
- 不使用 shell command string

## Out of Scope

- 实际调度 Agent
- 自动安装 skill 或 tool
- 修改 requirement risk

## Completion Criteria

场景: checks 与 gates 带稳定理由
  测试: test_affected_execution_bundle_selects_checks_and_gates_with_reasons
  假设 report 连接到 risk A contract 和 baseline quality profile
  当 bundle 构建
  那么 每个 fast check 与 acceptance gate 均含 provider role 和选择理由

场景: 只选择 authoritative tests
  测试: test_affected_execution_bundle_uses_only_authoritative_tests
  假设 report 同时含显式 selector 与 heuristic candidate
  当 tests 分类
  那么 authoritative_tests 只含显式 selector

场景: heuristic tests 单独保留
  测试: test_affected_execution_bundle_keeps_heuristic_test_candidates_separate
  假设 obligation 提供 candidate selector
  当 bundle 构建
  那么 candidate 只出现在 test_candidates 且不出现在 gates

场景: 缺失 selector 与 provider 不被掩盖
  测试: test_affected_execution_bundle_preserves_missing_selector_and_provider_gaps
  假设 intent report 含 selector-missing 与 provider-unavailable
  当 bundle 构建
  那么 两个 gaps 原样保留且不生成替代 evidence

场景: guidance 按 affected path 收敛
  测试: test_affected_execution_bundle_scopes_guidance_to_affected_paths
  假设两个 guidance 分别匹配和不匹配 affected source path
  当 bundle 收集 required skills
  那么 只选择匹配 guidance 的 skills 并解释 source

场景: skill receipt 与证据隔离
  测试: test_affected_execution_bundle_keeps_skill_receipts_separate_from_evidence
  假设 selected skill 文件可读取
  当 bundle 生成 receipt
  那么 receipt 含 hash 且 acceptance evidence 不含 receipt

场景: bundle 输出 byte stable
  测试: test_affected_execution_bundle_is_byte_stable
  假设相同 report、quality profile 与 guidance 输入
  当 bundle 连续渲染两次
  那么 JSON bytes 完全相同

场景: risk 策略改变可执行 gate
  测试: affected_execution_bundle_applies_distinct_risk_policies
  假设 同一 affected report 分别声明 risk A、B 与 C
  当 bundle 构建
  那么 三类 required evidence、quality profile、fast checks 与 acceptance gates 按策略不同且 provider 执行配置完整
