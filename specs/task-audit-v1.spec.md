spec: task
name: "agent-spec audit v1：spec 库健康度扫描"
inherits: project
tags: [audit, governance, phase8]
depends: [task-structural-check-v1]
estimate: 1d
---

## 意图

提供慢节奏的 spec 库体检:`agent-spec audit --spec-dir <dir>` 机械聚合整个 spec 库的健康指标
(spec 数、按 scope 的 Rule 数、未被 Example 证明的 Rule、无归属 scenario、未决 Discovery 问题、
非法 Rule id),帮助发现累积漂移。multi-run adversarial converge(caller-mode 多轮取共识)需要 AI,
留待后续;本期做纯机械的库级聚合。

## 已定决策

- 纯函数 `audit_specs(docs: &[SpecDocument]) -> AuditReport`,聚合:
  - `spec_count`、`rule_count`、`scenario_count`
  - `unproven_rules`:scenario_names 为空的 Rule 数(承诺但无 Example)
  - `ungrouped_scenarios`:rule 为 None 的 scenario 数
  - `open_questions`:未解决 Discovery 问题数(复用 Phase 4 的判定)
  - `malformed_rules`:非法 Rule id 数
- 新命令 `agent-spec audit --spec-dir <dir> [--format text|json]`:解析目录下所有 `.spec`/`.spec.md`,输出 AuditReport。
- audit 是 observability:**默认不 gate**(始终退出 0),只报告;是否据此失败由调用方决定。
- 不改 `is_passing` / verification / lint。

## 边界

### 允许修改

- src/spec_report/**（audit 模块)
- src/main.rs（audit 子命令)
- README.md、examples/**

### 禁止做

- 不要让 audit 默认改变退出码或 gate(纯报告)。
- 不要在本期做 multi-run caller-mode 共识(需 AI)。
- 不要改 `is_passing` / verification。

## 完成条件

### Rule: audit-aggregates-library-health — 机械聚合库级健康指标

场景: 聚合 spec/rule/scenario 计数
  测试:
    过滤: test_audit_counts_specs_rules_scenarios
  假设 两份 spec,共含 2 个 Rule 与 3 个 scenario
  当 调用 `audit_specs`
  那么 报告的 spec_count 为 2、rule_count 为 2、scenario_count 为 3

场景: 统计未被证明的 Rule
  测试:
    过滤: test_audit_counts_unproven_rules
  假设 一份 spec 含一个无 scenario 的 Rule 和一个有 scenario 的 Rule
  当 调用 `audit_specs`
  那么 报告的 unproven_rules 为 1

场景: 统计无归属 scenario
  测试:
    过滤: test_audit_counts_ungrouped_scenarios
  假设 一份 spec 含 2 个无 Rule 归属的 scenario
  当 调用 `audit_specs`
  那么 报告的 ungrouped_scenarios 为 2

场景: 统计未决 Discovery 问题
  测试:
    过滤: test_audit_counts_open_questions
  假设 一份 spec 的 `## Questions` 含一个未决问题
  当 调用 `audit_specs`
  那么 报告的 open_questions 为 1

场景: 统计非法 Rule id
  测试:
    过滤: test_audit_counts_malformed_rules
  假设 一份 spec 含一个非 kebab-case 的 `Rule:` 行
  当 调用 `audit_specs`
  那么 报告的 malformed_rules 为 1

### Rule: audit-is-non-gating — audit 只报告不改门禁

场景: 空库审计返回零计数不报错
  测试:
    过滤: test_audit_empty_library
  假设 没有任何 spec
  当 调用 `audit_specs`
  那么 所有计数为 0 且不 panic

场景: audit JSON 可机器解析
  测试:
    过滤: test_audit_json_serializes
  假设 一份非空审计报告
  当 序列化为 JSON
  那么 含 `spec_count`、`unproven_rules` 字段

## 排除范围

- multi-run caller-mode 共识(adversarial converge)—— 需 AI
- dead-rule 检测(需 capability ← task 反向引用,Phase 3.5 之后)
- 把 audit 接入 CI 门禁(本期纯报告)
- 跨 spec 重名/矛盾的深度分析
