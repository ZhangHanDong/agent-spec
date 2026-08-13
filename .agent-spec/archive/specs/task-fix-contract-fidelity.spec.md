spec: task
name: "修正 Contract 保真度"
inherits: project
tags: [done, bootstrap, contract, phase0]
---

## 意图

在继续扩展 review flow 之前，
先修正 `agent-spec` 的合同面失真问题，让 Agent 读取到的 Task Contract 更接近真实规格。

## 已定决策

- 本轮只补最小 Phase 0：祖先 `Constraints` 与 `Decisions` 的继承
- `TaskContract` 应区分 `Must`、`Must Not` 与 `Decisions`
- 默认文本 `contract` 输出应保留 step table 与结构化 `测试:` selector

## 边界

### 允许修改
- src/spec_core/**
- src/spec_parser/**
- src/spec_gateway/**
- src/**
- specs/**

### 禁止做
- 不要把 `Must` 再次回填进 `Decisions`
- 不要把这轮范围膨胀成完整 `Boundaries` 继承
- 不要只修 JSON 输出而忽略默认文本 `contract`

## 完成条件

场景: 继承链保留项目级约束与已定决策
  测试:
    过滤: test_load_resolves_full_project_contract_from_spec_directory
  假设 `project.spec` 声明了 Constraints 与 Decisions
  当 task spec 通过默认继承链加载
  那么 计划阶段的 Task Contract 包含这些继承得到的规则与已定决策
  并且 不要求用户手工提供额外搜索路径

场景: Task Contract 区分 Must 与 Decisions
  测试:
    过滤: test_task_contract_keeps_must_must_not_and_decisions_distinct
  假设 某个 task spec 同时声明 Must、Must Not 与已定决策
  当 gateway 构造 Task Contract
  那么 输出中保留这三类不同语义
  并且 不再把 Must 合并进 Decisions

场景: contract 输出保留结构化验收信息
  测试:
    包: agent-spec
    过滤: test_contract_output_preserves_step_tables_and_test_selectors
  假设 某个 Completion Criteria 场景带有 step table 与结构化 `测试:` selector
  当 CLI 渲染 `agent-spec contract`
  那么 默认输出保留这些结构化信息
  并且 Claude Code 的 tool-first 路径不再丢失关键验收上下文

## 排除范围

- 完整 `Boundaries` 继承
- `agent-spec explain`
- run log 与真实 AI backend
