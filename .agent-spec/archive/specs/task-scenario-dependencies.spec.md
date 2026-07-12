spec: task
name: "场景依赖与拓扑排序执行"
inherits: project
tags: [done, bootstrap, parser, lifecycle, lint, phase9]
depends: [task-checkpoint-resume, task-history-summary]
estimate: 1d
---

## 意图

让场景可以声明对其他场景的前置依赖，
lifecycle 按拓扑序执行场景，前置失败时自动跳过依赖场景。
这为多阶段 spec 提供确定性执行顺序。

灵感来源：Attractor 的 5 步确定性 edge selection 算法。

## 已定决策

- 场景新增 `前置:` / `Depends:` 字段，值为其他场景名称
- lifecycle 在执行前按依赖关系拓扑排序
- 前置场景 fail 时，依赖场景自动标记为 `skip`，evidence 记录跳过原因
- 循环依赖在 lint 阶段报错
- 无依赖声明的场景保持原有执行顺序

## 边界

### 允许修改
- src/spec_core/**
- src/spec_parser/**
- src/spec_gateway/**
- src/spec_lint/**

### 禁止做
- 不要在无依赖声明时改变场景执行顺序
- 不要允许循环依赖通过 lint
- 不要让依赖跳过的场景 verdict 为 `fail`（应为 `skip`）

## 完成条件

场景: 前置失败时依赖场景被跳过
  测试:
    包: agent-spec
    过滤: test_dependency_skip_on_prerequisite_fail
  假设 场景 B 声明 `前置: 场景 A`，场景 A verdict 为 `fail`
  当 lifecycle 执行
  那么 场景 B verdict 为 `skip`
  并且 evidence 记录 "前置场景 '场景 A' 失败"

场景: 循环依赖被 lint 检测
  测试:
    包: agent-spec
    过滤: test_lint_detects_circular_dependency
  假设 场景 A 依赖场景 B，场景 B 依赖场景 A
  当 lint 检查该 spec
  那么 报告包含循环依赖错误

场景: 拓扑排序保证执行顺序
  测试:
    包: agent-spec
    过滤: test_topological_sort_execution_order
  假设 场景 C 依赖场景 B，场景 B 依赖场景 A
  当 lifecycle 执行
  那么 执行顺序为 A → B → C

场景: parser 正确解析前置字段
  测试:
    包: agent-spec
    过滤: test_parse_depends_field_in_scenario
  假设 某个场景声明 `前置: 用户注册`
  当 parser 解析该场景
  那么 AST 中 `depends_on` 包含 "用户注册"

场景: 无依赖声明时执行顺序不变
  测试:
    包: agent-spec
    过滤: test_no_dependency_preserves_original_order
  假设 某个 spec 的所有场景均无 `前置:` 声明
  当 lifecycle 执行
  那么 执行顺序与场景在 spec 中的书写顺序一致
