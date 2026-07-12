spec: task
name: "关键场景门禁（Goal Gate）"
inherits: project
tags: [done, bootstrap, lifecycle, verify, phase7]
depends: []
estimate: 0.5d
---

## 意图

让 lifecycle 区分"普通失败"与"关键门禁被阻塞"，
给 agent 一个更强的未完成信号——critical 场景失败时输出 `gate_blocked`，
驱动 agent 优先解决门禁而非普通 failure。

灵感来源：Attractor 的 `goal_gate=true` 节点机制。

## 已定决策

- 通过场景 tags 中的 `critical` 标签标记门禁场景
- 也支持场景名称中的 `（critical）` / `(critical)` 后缀作为简写
- lifecycle JSON 输出新增 `gate_blocked` 布尔字段和 `blocked_gates` 数组
- 当 critical 场景 fail 时退出码为 `2`（区别于普通 fail 的 `1`）
- 无 critical 标签时行为完全不变（向后兼容）

## 边界

### 允许修改
- src/spec_core/**
- src/spec_gateway/**
- src/spec_report/**
- src/main.rs

### 禁止做
- 不要在无 critical 标签时改变现有退出码语义
- 不要强制所有 spec 必须有 critical 场景
- 不要修改 Verdict 枚举本身

## 完成条件

场景: critical 场景失败时报告 gate_blocked
  测试:
    包: agent-spec
    过滤: test_critical_scenario_fail_sets_gate_blocked
  假设 某个任务级 spec 有一个标记为 `critical` 的场景
  当 该场景 verdict 为 `fail`
  那么 lifecycle JSON 输出中 `gate_blocked` 为 `true`
  并且 `blocked_gates` 包含该场景名称

场景: critical 场景通过时不触发门禁
  测试:
    包: agent-spec
    过滤: test_critical_scenario_pass_no_gate_block
  假设 某个任务级 spec 有一个标记为 `critical` 的场景
  当 该场景 verdict 为 `pass`
  那么 lifecycle JSON 输出中 `gate_blocked` 为 `false`
  并且 `blocked_gates` 为空数组

场景: 无 critical 标签时行为不变
  测试:
    包: agent-spec
    过滤: test_no_critical_tag_preserves_existing_behavior
  假设 某个任务级 spec 没有任何 `critical` 标签
  当 lifecycle 输出 JSON 结果
  那么 输出中 `gate_blocked` 为 `false`
  并且 退出码语义与现有行为一致

场景: 场景名称后缀作为 critical 简写
  测试:
    包: agent-spec
    过滤: test_critical_suffix_in_scenario_name
  假设 某个场景名称为 "用户注册成功（critical）"
  当 parser 解析该场景
  那么 该场景被识别为 critical
  并且 场景名称中的 `（critical）` 后缀被移除后保留为显示名

场景: critical 失败的退出码为 2
  测试:
    包: agent-spec
    过滤: test_critical_fail_exit_code_is_2
  假设 某个任务级 spec 有 critical 场景且 verdict 为 `fail`
  当 lifecycle 命令执行完毕
  那么 退出码为 `2`
