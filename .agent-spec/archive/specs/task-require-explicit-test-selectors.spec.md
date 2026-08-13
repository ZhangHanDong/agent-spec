spec: task
name: "缺少显式测试绑定时阻止通过"
inherits: project
tags: [done, bootstrap, lint, verify, quality-gate]
---

## 意图

把显式 `测试:` selector 从推荐做法提升为默认质量门槛，
避免任务合约仍然依赖隐式场景名匹配或遗留注释映射。

## 已定决策

- 缺少显式 selector 的任务级场景会产生 `error` 级 lint
- `quality_gate` 在存在 `error` 级 lint 时直接失败
- 旧版 `// @spec:` 只作为 verifier 兼容层，不再满足 lint 要求

## 边界

### 允许修改
- src/spec_core/**
- src/spec_lint/**
- src/spec_gateway/**
- src/**
- specs/**

### 禁止做
- 不要把缺少 selector 只当成 info 级提示
- 不要让 `quality_gate(0.0)` 绕过 error 级 lint
- 不要移除旧版 `// @spec:` verifier fallback

## 完成条件

场景: 缺少显式绑定的任务场景触发 lint 错误
  测试: test_explicit_test_binding_linter_requires_task_scenario_selectors
  假设 某个任务级 spec 的场景没有声明 `测试:` selector
  当 lint pipeline 检查该 spec
  那么 产生 `explicit-test-binding` 规则
  并且 诊断级别为 `error`

场景: 显式绑定的任务场景通过 lint
  测试: test_explicit_test_binding_linter_accepts_explicit_selector
  假设 某个任务级 spec 的场景声明了 `测试:` selector
  当 lint pipeline 检查该 spec
  那么 不会产生 `explicit-test-binding` 错误

场景: error 级 lint 阻止质量闸门通过
  测试: test_quality_gate_fails_on_error_lint_issue
  假设 某个任务级 spec 缺少显式测试绑定
  当 gateway 运行 `quality_gate(0.0)`
  那么 质量闸门仍然失败
  并且 失败原因说明存在 error 级 lint
