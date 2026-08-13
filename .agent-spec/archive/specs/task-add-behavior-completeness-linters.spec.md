spec: task
name: "新增行为完整性 lint"
inherits: project
tags: [done, contract-quality, lint, behavior, phase-next]
---

## 意图

让 `agent-spec` 不只检查规格是否“写得像合同”，
还要能发现“合同结构完整但关键可观察行为没被绑定”的问题。
本任务聚焦在新增一组 warning-first 的 lint，直接针对这类遗漏给出反馈。

## 已定决策

- 首批新增 4 个 lint：`observable-decision-coverage`、`output-mode-coverage`、`precedence-fallback-coverage`、`external-io-error-strength`
- 第一阶段全部以 warning 落地，先观察误报率，再决定是否升级为 error
- 新 lint 只针对 task spec 生效，不改变 project spec 的现有通过标准
- `observable-decision-coverage` 重点关注可观察行为关键词，而不是普通实现细节
- `external-io-error-strength` 先做启发式检查，不要求工具精确证明测试一定命中真实生产路径

## 边界

### 允许修改
- src/spec_lint/**
- src/spec_core/**
- src/**
- specs/**
- README.md

### 禁止做
- 不要在这一轮修改 parser DSL
- 不要把 warning 直接升级成默认 error
- 不要为了降低误报率而把规则做成几乎不触发

## 完成条件

场景: 可观察行为决策缺少场景时给出提示
  测试:
    过滤: test_observable_decision_coverage_warns_when_behavioral_decisions_lack_scenarios
  假设 某个 task spec 的 Decisions 提到 `--json`、stdout 和 fallback 顺序
  当 运行 `agent-spec lint`
  那么 lint 输出包含 `observable-decision-coverage` warning
  并且 warning 指出缺少对应场景覆盖这些可观察行为

场景: 输出模式未覆盖时给出提示
  测试:
    过滤: test_output_mode_coverage_warns_when_json_or_output_flags_are_uncovered
  假设 某个 task spec 提到 `--json` 和 `-o/--output`
  当 该 spec 的场景只覆盖默认 human 输出
  那么 lint 输出包含 `output-mode-coverage` warning
  并且 warning 指出未覆盖的输出模式

场景: 优先级和回退顺序未验证时给出提示
  测试:
    过滤: test_precedence_fallback_coverage_warns_when_ordered_behavior_has_no_scenario
  假设 某个 task spec 的 Decisions 包含 `local -> cache -> remote` 的读取顺序
  当 运行 `agent-spec lint`
  那么 lint 输出包含 `precedence-fallback-coverage` warning
  并且 warning 指出该顺序尚未被场景验证

场景: 外部 I/O 错误场景的测试强度过弱时给出提示
  测试:
    过滤: test_external_io_error_strength_warns_on_weak_mock_only_http_scenarios
  假设 某个场景描述 HTTP 4xx/5xx 处理，但测试选择器和步骤只体现纯注入 mock
  当 运行 `agent-spec lint`
  那么 lint 输出包含 `external-io-error-strength` warning
  并且 warning 提示该场景没有表达真实 I/O 边界覆盖

场景: 非行为性普通决策不会被新规则大量误报
  测试:
    过滤: test_behavior_completeness_linters_do_not_flag_plain_implementation_choices
  假设 某个 task spec 的 Decisions 只声明库选择和目录结构
  当 运行 `agent-spec lint`
  那么 上述新 lint 不会仅因这些普通实现决策而触发 warning

## 排除范围

- 把高风险 warning 升级为默认 error
- 针对 project spec 设计另一套行为 lint
- 设计完整的语义推理或 AI 参与的 lint
