spec: task
name: "Verification Diagnostics"
tags: [verification, lifecycle, guard, diagnostics, bugfix]
satisfies: [REQ-VERIFICATION-DIAGNOSTICS]
risk: B
estimate: 1d
---

## Intent

三处诊断质量修复：`Package:` 指向非 workspace 成员时明确指出而不是伪装成编译
失败；验证汇总把真实场景计数与合成层（boundaries / atlas-symbols / complexity）
分开呈现，同时保持顶层门禁计数与 JSON 字段顺序不变以兼容既有消费者；`guard
--change-scope` 的 help 列全 `none, staged, worktree, jj`。来源是 robrix2 反馈
C2 / A5 / A4（队列 2）。

## Decisions

- `test_verifier.rs` 新增 `cargo_workspace_members(root) -> Option<Vec<String>>`：在
  workspace root 跑 `cargo metadata --no-deps --format-version 1`，取 `packages[].name`；
  失败返回 None。每次 verify 最多调用一次并缓存。
- 绑定带 `Package: P` 且成员名单可得而 P 不在其中 → 不跑 cargo test，verdict Uncertain，
  reason 为 "package `P` is not a member of the cargo workspace at <root> (members: a, b, …)"；
  成员名单不可得 → 现有行为不变。
- `VerificationSummary` 追加两个字段（声明在既有字段之后）：
  `scenarios: ScenarioCounts`（只统计名字不以 `[` 开头的结果）与
  `layers: Vec<LayerVerdict { name, verdict }>`（`[name]` 前缀即层名）；顶层
  `total/passed/failed/skipped/uncertain/pending_review` 取值不变，struct 直接序列化时
  新字段位于既有字段之后（经 `serde_json::Value` 输出时为字母序，顶层 `failed` 仍先于嵌套）。
- 文本报告与 run log 的汇总行改为 `"{p}/{t} scenarios passed, {f} failed, {s} skipped, {u} uncertain"`
  后跟各层 `" · layers: boundaries=pass, atlas-symbols=fail"`（无层时省略后半段）。
- guard 的 `--change-scope` help 改为 "Auto-detect changes from VCS: none, staged, worktree, jj"。

## Boundaries

### Allowed Changes
- src/spec_verify/test_verifier.rs
- src/spec_core/verify.rs
- src/spec_gateway/lifecycle.rs
- src/spec_report/mod.rs
- src/main.rs
- src/spec_knowledge/liveness.rs — test fixture gains ..Default::default() only
- specs/task-verification-diagnostics.spec.md
- knowledge/requirements/req-verification-diagnostics.md
- .agent-spec/wiki/**
- CHANGELOG.md
- skills/agent-spec-tool-first/references/commands.md
- book/src/**

### Forbidden
- 不改变顶层 summary 计数与 `is_passing` 门禁语义
- 不改变 JSON 中既有字段的顺序
- 不新增 `Manifest:` / `Dir:` 选择器

## Out of Scope

- `Manifest:` / `Dir:` 选择器（在子目录跑 cargo）
- `--boundary-scope` 与 changed-spec 相关性过滤（队列 4）
- lint 与 boundary 层的独立 pass/fail 汇总（lint 不在 verification report 内）

## Completion Criteria

<!-- lint-ack: verification-metadata-suggestion — 本合约场景为进程内单元测试与 clap help 断言，不涉及外部 I/O -->

### Rule: package-membership — 非成员包被指名

场景: 非成员包得到明确 reason 且不跑测试
  测试: test_package_selector_not_in_workspace_names_members
  假设 一个 `Package: not-a-crate` 的场景与本仓库 workspace
  当 用 `cargo_workspace_members` 校验后运行 verifier
  那么 verdict 为 uncertain 且 reason 含 "not a member of the cargo" 与 "agent-spec" 且 evidence stdout 为空

场景: 成员包照常运行
  测试: test_package_selector_member_still_runs
  假设 一个 `Package: agent-spec` 且 Filter 指向真实测试的场景
  当 运行 verifier
  那么 verdict 为 pass

场景: 成员名单不可得时行为不变
  测试: test_package_reason_falls_back_without_metadata
  假设 成员名单为 None 且 cargo 以非零退出零测试
  当 计算 reason
  那么 文本等于既有的 "cargo exited before any test ran (build/toolchain failure)" 形式

### Rule: summary — 场景与层分开

场景: 场景计数不含层且顶层计数不变
  测试: test_summary_splits_scenarios_and_layers
  假设 10 个 pass 场景与 1 个 `[boundaries]` fail 结果
  当 调用 `from_results`
  那么 scenarios.total 为 10 且 layers 含 boundaries=fail 且顶层 total 为 11 且 failed 为 1

场景: 无层时 layers 为空且不序列化
  测试: test_summary_without_layers_serializes_like_before
  假设 只有真实场景的结果
  当 序列化 summary
  那么 JSON 不含 "layers" 键

场景: 新字段在旧字段之后
  测试: test_summary_json_keeps_legacy_field_order
  假设 含层结果的 summary
  当 以 struct 直接序列化为 JSON
  那么 "failed" 首次出现的位置在 "scenarios" 之前

场景: 文本汇总分行
  测试: test_text_summary_line_separates_scenarios_and_layers
  假设 10 个 pass 场景与 1 个 boundaries pass 层
  当 渲染汇总行
  那么 输出含 "10/10 scenarios passed" 与 "boundaries=pass"

### Rule: guard-help — help 列全

场景: guard help 列全 change-scope 取值
  测试: test_guard_change_scope_help_lists_all_values
  假设 guard 子命令
  当 渲染 --help
  那么 输出含 "none, staged, worktree, jj"
