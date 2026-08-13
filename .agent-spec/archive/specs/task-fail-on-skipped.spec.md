spec: task
name: "跳过场景不得判定为通过"
inherits: project
tags: [done, bootstrap, verify, lifecycle]
---

## 意图

把 `agent-spec` 的最终判定从“零失败即可通过”改成“关键场景被正向验证后才通过”，
避免把 `skip` 误报成绿色结果。

## 已定决策

- 最终决策在出现 `skip` 或 `uncertain` 时返回非通过结果
- 验证报告继续保留每个场景原始的 `pass`、`fail`、`skip`、`uncertain` verdict
- 结构化约束检查继续产生 `pass` 或 `fail`

## 边界

### 允许修改
- src/spec_gateway/**
- src/**
- src/spec_core/verify.rs

### 禁止做
- 不要在存在 `skip` 的情况下返回 `passed: true`
- 不要把 `skip` 重写成 `pass`
- 不要隐藏导致非通过的 `skip` 或 `uncertain`

## 完成条件

场景: 单个跳过场景导致非通过
  测试: test_skip_is_not_passing
  假设 某个任务级 spec 只产生一个 `skip` 场景
  当 lifecycle 输出 JSON 结果
  那么 最终字段 `passed` 为 `false`
  并且 场景 verdict 仍然是 `skip`

场景: 结构通过加跳过仍然非通过
  测试: test_pass_plus_skip_is_not_passing
  假设 验证报告同时包含一个结构化 `pass` 和一个验收场景 `skip`
  当 gateway 计算最终决策
  那么 结构化 `pass` 继续保留
  并且 最终决策仍然为非通过
