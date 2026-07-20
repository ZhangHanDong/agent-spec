spec: task
name: "Atlas Real Agent A/B Gate"
tags: [atlas, evaluation, benchmark, adoption, dogfood]
satisfies: [REQ-ATLAS-AGENT-AB-GATE]
depends: [task-atlas-agent-evaluation, task-atlas-query-context-compiler, task-atlas-concurrent-query-serving]
estimate: 4d
---

## Intent

把 E1 从离线指标扩展成真实 Agent 可执行但默认不运行的晋升门。三臂实验分别证明 Atlas
基础查询面和 B5 context compiler 的增量价值；独立 burst 实验判断 D4 worker，任何默认值
变化都必须等待完整 receipt、correctness-first gate 和人工接受。

<!-- lint-ack: bdd-rule-grouping — Agent surface 和 concurrent serving 共享同一 E1 adoption authority，但使用独立 plan/receipt schema -->

## Decisions

- 新模块 `src/atlas_agent_eval.rs` 定义严格 typed manifest、Agent plan/receipt/gate、concurrent
  plan/receipt/gate；schema 分别使用 `agent-spec/atlas-eval/agent-experiment-v1`、
  `agent-plan-v1`、`agent-receipts-v1`、`agent-gate-v1`、`serving-plan-v1`、
  `serving-receipts-v1` 和 `serving-gate-v1`。
- Agent arms 固定为 `baseline`、`atlas-primitives`、`atlas-context`；baseline 必须包含
  `read` 和 `grep`，B 必须是 A 加 Atlas primitive/explore，C 必须恰好是 B 加
  `atlas-context`。
- prompt hook、MCP config、user skill 和 tool instruction 使用 `disabled` 或带 64 位小写
  hex fingerprint 的 `pinned` 配置；同一配置写入所有 matched runs，arm 不得覆盖。
- correctness、judge version、rubric fingerprint、raw session path/hash、tool trace hash、完整
  query metrics 和所有失败状态都进入严格 receipt；`/tmp` 不得作为 canonical session store。
- medium/large benefit 和 small tie zone 由 matched baseline trial 的 median/MAD 计算；候选
  correctness 或 freshness 失败时不计算为通过，零样本和 legacy metrics 都不能改善结果。
- B 对 A 只控制 Atlas primitive/default-surface candidate；C 对 B 只控制 B5 profile candidate。
- concurrent plan 固定四个 B5 load profile、direct/worker 两臂和每臂至少三次 burst；worker
  promotion 要求语义 digest 与 snapshot 匹配、所有 logical query 正确、无 timeout/stale、
  heartbeat 在 manifest budget 内，且 batch duration 改善超过 direct-arm MAD。
- `scripts/atlas-eval/run-agent-ab-opt-in.sh` 与 `run-serving-ab-opt-in.sh` 只执行显式单一
  executable；默认 test、CI、build script 和 CLI validation 不启动 Agent 或访问网络。
- checked-in manifest 和 derived plans 不是结果；没有真实 receipts 时 roadmap 保持 pending，
  默认 MCP discovery、B5 profile 和 worker mode保持不变。

## Boundaries

### Allowed Changes
- benchmarks/atlas/**
- scripts/atlas-eval/**
- src/atlas_agent_eval.rs
- src/main.rs
- docs/atlas-agent-ab-gate.md
- docs/atlas-evaluation.md
- docs/atlas-roadmap.md
- knowledge/requirements/req-atlas-agent-ab-gate.md
- specs/task-atlas-agent-ab-gate.spec.md
- docs/superpowers/specs/2026-07-21-atlas-agent-ab-gate-design.md
- docs/superpowers/plans/2026-07-21-atlas-agent-ab-gate.md
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- .agent-spec/wiki/**
- CHANGELOG.md

### Symbols
- agent-spec: agent_spec::atlas_agent_eval

### Forbidden
- 不在默认测试、build script 或 CI 中启动真实 Agent、模型或网络访问
- 不删除、忽略或重编号失败 run 来改善 aggregate
- 不从其他项目复制 benchmark percentage 作为 gate threshold
- 不在 E1 receipt 和人工接受前改变默认 MCP、B5 或 worker 配置
- 不把 fixture 或 synthetic receipt 描述为真实 Agent evidence

## Out of Scope

- 选择、安装或绑定特定 Agent、模型供应商或 orchestration runtime
- 在 agent-spec 内置 LLM judge
- 上传或长期托管原始 session 内容
- 自动修改默认产品配置

## Completion Criteria

场景: 三臂计划保持所有环境变量对称
  测试: test_agent_ab_plan_builds_three_symmetric_arms
  假设 合法 corpus、固定 environment 和三个合法 tool surface
  当编译 Agent plan
  那么 每个 case 和 trial 生成 A/B/C 三个匹配 run
  并且 除 surface 和 arm 外所有控制字段与 fingerprint 相同

场景: 非法 surface ablation 被拒绝
  测试: test_agent_ab_plan_rejects_asymmetric_surface
  假设 B 缺少 A 的 tool 或 C 除 atlas-context 外还改变其他 tool
  当验证 experiment manifest
  那么 返回 atlas-agent-ab-surface diagnostic
  并且 不生成 plan

场景: Agent 与 serving 每臂少于三次被拒绝
  测试: test_agent_ab_plan_requires_three_trials
  假设 corpus case 或 serving profile 只请求两次 trial
  当编译对应 plan
  那么 返回 atlas-agent-ab-trials diagnostic

场景: 缺失重复和未知 receipt 被拒绝
  测试:
    过滤: test_agent_ab_gate_requires_exact_planned_runs
    层级: integration
  假设 receipt bundle 缺少、重复或增加一个 run id
  当执行 Agent gate
  那么 返回 atlas-agent-ab-completeness diagnostic
  并且 不产生 aggregate

场景: 失败 run 被保留并阻塞候选
  测试: test_agent_ab_gate_retains_failed_runs
  假设 一个 candidate run 状态为 failed 且带 session evidence
  当执行 Agent gate
  那么 output 保留该 run 的 failure diagnostic
  并且 candidate comparison 为 blocked

场景: legacy 或部分 query metrics 被拒绝
  测试: test_agent_ab_gate_rejects_legacy_query_metrics
  假设 receipt 缺失 query metric schema 或任一 query metric
  当反序列化或 gate receipt
  那么 返回 atlas-agent-ab-receipt diagnostic

场景: correctness 和 stale 优先阻塞效率收益
  测试: test_agent_ab_gate_blocks_correctness_and_stale_regression
  假设 candidate 的 tool calls 更少但答案错误或 stale_as_fresh 为 true
  当执行 Agent gate
  那么 comparison 为 blocked
  并且 diagnostic 不把效率改善描述为通过

场景: baseline variance 派生 medium large benefit
  测试: test_agent_ab_gate_derives_benefit_from_baseline_mad
  假设 medium 和 large case 有三个 baseline trial 及匹配 candidate trial
  当候选 median 改善超过 baseline MAD
  那么对应 Read/Grep、round trip 和 total tool call metric 通过

场景: small repository tie zone 不隐藏 overhead
  测试: test_agent_ab_gate_keeps_small_tie_zone_visible
  假设 small case candidate overhead 位于或超出 baseline MAD tie zone
  当执行 Agent gate
  那么 output 分别报告 tie 或 blocked 而不并入 improvement

场景: B 对 A 和 C 对 B 的结论互不替代
  测试: test_agent_ab_gate_scopes_surface_promotions
  假设 B 对 A 通过而 C 对 B 未通过
  当生成 gate receipt
  那么 Atlas primitive candidate 为 passed
  并且 B5 candidate 保持 blocked

场景: session judge 和 trace evidence 必须完整
  测试:
    过滤: test_agent_ab_gate_validates_session_evidence
    层级: integration
  假设 run 使用空 judge version、临时 session path 或非法 hash
  当解析 receipt
  那么 返回 atlas-agent-ab-evidence diagnostic

场景: concurrent plan 固定四类负载和 matched burst
  测试: test_agent_ab_serving_plan_builds_matched_profiles
  假设 合法 non-fixture pinned repository 和每臂三次 trial
  当编译 serving plan
  那么 light、traversal、source-heavy、mixed 各生成 direct/worker matched runs

场景: worker correctness snapshot 或 queue 回退阻塞晋升
  测试:
    过滤: test_agent_ab_serving_gate_blocks_correctness_snapshot_and_timeout_regression
    层级: integration
  假设 worker 丢失 logical query、产生 stale、snapshot 不同或 queue timeout
  当执行 serving gate
  那么 worker candidate 为 blocked

场景: fixture burst 不能冒充真实 E1 evidence
  测试:
    过滤: test_agent_ab_serving_plan_rejects_fixture_repository
    层级: integration
  假设 serving experiment 指向 fixtures 路径或非完整 Git revision
  当编译 serving plan
  那么 返回 atlas-agent-ab-real-repository diagnostic

场景: opt-in runners 要求显式 executable
  测试:
    过滤: test_agent_ab_opt_in_runners_require_explicit_commands
    层级: integration
    命中: scripts/atlas-eval/run-agent-ab-opt-in.sh, scripts/atlas-eval/run-serving-ab-opt-in.sh
  假设 `scripts/atlas-eval/run-agent-ab-opt-in.sh` 未设置 Agent command
  并且 `scripts/atlas-eval/run-serving-ab-opt-in.sh` 未设置 serving command
  当分别运行两个 opt-in runner
  那么 在启动子进程前以 diagnostic 和非零状态退出

场景: CLI 文件输出原子且 stdout 安静
  测试:
    过滤: test_agent_ab_cli_writes_atomic_outputs
    层级: integration
  假设 合法 Agent experiment、corpus 和不存在的 out 文件
  当 agent-plan 使用 --out 写计划
  那么 out 文件是完整可解析 JSON
  并且 stdout 为空且没有临时文件残留
