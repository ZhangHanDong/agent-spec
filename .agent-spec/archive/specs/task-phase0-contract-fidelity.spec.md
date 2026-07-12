spec: task
name: "Phase 0：Contract 保真度修正"
inherits: project
tags: [done, roadmap, planned, phase0, contract]
---

## 意图

在继续扩展 review 流程、run log 或 AI 能力之前，
先把 `agent-spec` 的合同面修到足够保真，避免后续功能建立在失真的 Task Contract 上。

## 已定决策

- 最小 Phase 0 先补齐祖先 `Constraints` 与 `Decisions` 的继承
- `TaskContract` 应把 `Must`、`Must Not`、`Decisions` 区分为一等语义
- `contract` 的主输出应保留 Completion Criteria 里的 step table 与 test selector

## 边界

### 允许修改
- src/spec_core/**
- src/spec_parser/**
- src/spec_gateway/**
- src/**
- specs/**
- README.md

### 禁止做
- 不要继续把 `Must` 当成 `Decisions`
- 不要只修 JSON 输出而忽略默认文本 `contract` 输出
- 不要在保真度修正完成前优先实现 `stamp`、run log 或真实 AI backend

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

- `agent-spec explain`
- run log 与执行历史
- 真实 AI backend
