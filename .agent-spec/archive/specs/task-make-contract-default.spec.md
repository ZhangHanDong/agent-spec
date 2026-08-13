spec: task
name: "Task Contract 成为默认执行入口"
inherits: project
tags: [done, bootstrap, contract, gateway, cli]
---

## 意图

把 `TaskContract` 变成 `agent-spec` 的默认执行入口，
让 agent 在计划阶段默认消费 Contract，而不是历史遗留的简化 brief。

## 已定决策

- `SpecGateway::plan()` 返回默认执行用的 `TaskContract`
- `SpecGateway::brief()` 继续保留，但仅作为兼容层
- `agent-spec brief` 继续可用，但作为 `agent-spec contract` 的兼容别名

## 边界

### 允许修改
- src/spec_gateway/**
- src/**
- specs/**

### 禁止做
- 不要删除现有 `brief()` API
- 不要让 `brief` 与 `contract` 输出出现语义漂移
- 不要破坏现有 task spec 的解析与验证行为

## 完成条件

场景: Gateway 计划阶段返回 Task Contract
  测试: test_plan_returns_task_contract
  假设 某个任务级 spec 已被加载
  当 调用 `SpecGateway::plan()`
  那么 返回值是 `TaskContract`
  并且 输出使用 `Task Contract` 标题

场景: Brief 命令是 contract 兼容别名
  测试: test_brief_output_matches_contract_output
  假设 同一个任务级 spec
  当 CLI 分别渲染 `brief` 与 `contract`
  那么 两者输出保持一致
  并且 输出继续使用 `Task Contract` 结构
