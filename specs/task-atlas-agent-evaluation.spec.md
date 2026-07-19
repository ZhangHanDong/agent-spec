spec: task
name: "Atlas Agent Evaluation Baseline"
tags: [atlas, evaluation, benchmark, dogfood]
satisfies: [REQ-ATLAS-AGENT-EVALUATION]
depends: [task-rust-atlas-code-graph]
estimate: 3d
---

## Intent

建立 Rust Atlas 的可复现 Agent A/B 基线：用版本化 corpus、确定性 run plan、typed receipt
和 summary 把“答案是否正确、是否减少 Read/Grep 与总工具调用”变成可审计工件。真实模型
执行保持 opt-in，默认测试只验证 corpus、计划和汇总逻辑，不访问网络。

<!-- lint-ack: bdd-rule-grouping — benchmark 编译、汇总和 opt-in 执行是同一条线性验收链 -->

## Decisions

- corpus 使用 `benchmarks/atlas/corpus.json`，schema id 为
  `agent-spec/atlas-eval/corpus-v1`；case size 固定为 `small|medium|large`，task class 固定为
  `symbol|flow|impact|implementation|stale|scip-unavailable|compile-failing|worktree`。
- 新模块 `src/atlas_eval.rs` 定义 typed corpus、run plan、receipt、summary；不得使用 ad-hoc
  字符串解析 JSON。
- CLI 使用 `agent-spec atlas benchmark validate|plan|summarize`；所有输出为 JSON，验证或
  汇总失败时 stdout 为空、stderr 返回可执行 diagnostic、进程非零退出。
- plan 为每个 case 生成 `atlas` 与 `baseline` 两个 arm，每个 arm 至少三次 trial，并把
  model、prompt、revision、permissions、cache condition 固化到每个 run。
- summary 先报告 correctness failure，再计算 read、graph call、tool call、duration、
  context 与可选 cost 的 median 和 median absolute deviation；不得用平均值隐藏失败。
- `scripts/atlas-eval/run-opt-in.sh` 只消费 run plan 和显式 `ATLAS_EVAL_AGENT_COMMAND`，未设置
  时拒绝执行；默认 `cargo test` 永远不启动真实 Agent。

## Boundaries

### Allowed Changes
- benchmarks/atlas/**
- scripts/atlas-eval/**
- src/atlas_eval.rs
- src/main.rs
- docs/atlas-evaluation.md
- knowledge/requirements/req-atlas-agent-evaluation.md
- specs/task-atlas-agent-evaluation.spec.md
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Forbidden
- 不在默认测试、build script 或 CI 中发起网络请求或真实模型调用
- 不复制 codegraph 的百分比作为 Atlas pass threshold
- 不允许少于三次 trial 的 case 进入 run plan
- 不把缺失 correctness 的 receipt 当作可汇总结果

## Out of Scope

- 改变默认 MCP 工具暴露面
- 实现 `atlas explore`、`flow`、`impact`
- 选择或安装特定 Agent 产品

## Completion Criteria

场景: 合法 corpus 生成配对 run plan
  测试: test_atlas_eval_plan_pairs_arms_and_trials
  假设 corpus 覆盖 small、medium、large 且每个 case 请求三次 trial
  当 `atlas benchmark plan` 编译 corpus
  那么 每个 case 生成参数一致的 `atlas` 与 `baseline` arm
  并且 每个 arm 恰好包含三次 trial

场景: 重复 case id 被拒绝
  测试: test_atlas_eval_rejects_duplicate_case_ids
  假设 corpus 中两个 case 使用相同 id
  当 validator 读取 corpus
  那么 返回 `atlas-eval-duplicate-case` diagnostic
  并且 不生成 run plan

场景: 少于三次 trial 被拒绝
  测试: test_atlas_eval_rejects_too_few_trials
  假设 一个 case 配置 `trials_per_arm=2`
  当 `atlas benchmark plan` 编译 corpus
  那么 返回 `atlas-eval-trials` diagnostic
  并且 stdout 为空

场景: 缺失 correctness 的 receipt 阻塞汇总
  测试: test_atlas_eval_summary_rejects_missing_correctness
  假设 run receipt 没有 correctness verdict
  当 `atlas benchmark summarize` 读取 receipts
  那么 返回 `atlas-eval-receipt` diagnostic
  并且 不产生 aggregate summary

场景: run plan 文件输出是原子且安静的
  测试: test_atlas_eval_plan_writes_atomic_output
  假设 一个合法 corpus 与一个不存在的 `--out` 目标文件
  当 `atlas benchmark plan` 生成 run plan
  那么 目标文件包含完整且可解析的 run plan JSON
  并且 stdout 为空且不存在临时文件残留

场景: 默认测试不执行真实 Agent
  测试: test_atlas_eval_opt_in_runner_requires_command
  假设 未设置 `ATLAS_EVAL_AGENT_COMMAND`
  当 opt-in runner 被调用
  那么 runner 在启动子进程前失败
  并且 diagnostic 说明如何显式配置 command
