spec: task
name: "Phase 6：Advanced Verification"
inherits: project
tags: [done, roadmap, planned, phase6, verification]
---

## 意图

把验证金字塔做完整，但保持它是探索性、显式启用、成本可见的高级能力，
而不是把高成本验证塞进每次默认 lifecycle。

## 已定决策

- 验证层级以显式 `layers` 开关暴露
- 成本报告按层输出 token、时长与命中场景
- Contract 确定性度量保持实验性质，不进入默认 CI

## 边界

### 允许修改
- src/**
- src/spec_core/**
- src/spec_gateway/**
- src/spec_report/**
- README.md
- specs/**

### 禁止做
- 不要让高成本层默认开启
- 不要把实验性确定性度量写进基础质量门槛
- 不要模糊各验证层的成本边界

## 完成条件

场景: lifecycle 支持显式验证层选择
  测试:
    包: agent-spec
    过滤: test_lifecycle_layers_flag_selects_verification_stack
  假设 用户只想运行部分验证层
  当 lifecycle 接收 `--layers lint,boundary,test`
  那么 只运行指定层
  并且 报告中保留每层的独立结果

场景: 成本报告按层输出
  测试:
    过滤: test_cost_report_breaks_down_tokens_time_and_layers
  假设 某次生命周期执行同时使用了 test 与 AI 层
  当 用户请求成本报告
  那么 输出包含每层的 token、时间与汇总成本
  并且 用户能看到高成本层是否值得开启

场景: 确定性度量保持实验功能
  测试:
    包: agent-spec
    过滤: test_measure_determinism_is_explicitly_experimental
  假设 用户希望评估 Contract 方差
  当 用户查看 `measure-determinism`
  那么 命令被标注为实验性
  并且 不会进入默认的 lifecycle 或 guard

## 排除范围

- 默认启用多 Agent 实现比较
- 把成本报告变成强制门禁
