spec: task
name: "代码质量门禁（Complexity Gate）"
inherits: project
tags: [done, bootstrap, verify, phase8]
depends: [task-goal-gate]
estimate: 1d
---

## 意图

在 lifecycle 的验证阶段增加可选的代码质量检查，
防止 agent 写出"所有测试通过但极其臃肿"的实现。
质量约束从 spec 的 Constraints 中提取，支持行数比和复杂度指标。

灵感来源：Autoresearch 的简洁性标准——"0.001 改善 + 20 行丑代码 = 不值得"。

## 已定决策

- 新增 `ComplexityVerifier`，与现有 4 个 verifier 并列
- 质量约束从 Constraints 的 `Must` 类别中识别特定关键词
- `--layers` 增加 `complexity` 选项
- 不引入外部工具依赖，使用 git diff 统计行数变化
- 产生隐式场景 `[complexity] code quality gate`

## 边界

### 允许修改
- src/spec_verify/**
- src/spec_core/**
- src/spec_gateway/**
- src/main.rs

### 禁止做
- 不要让 complexity verifier 在无质量约束时产生任何 verdict
- 不要强制依赖 clippy 或其他外部 lint 工具
- 不要修改现有 verifier 的行为

## 完成条件

场景: 行数比超标时 fail
  测试:
    包: agent-spec
    过滤: test_complexity_verifier_fails_on_line_ratio_exceeded
  假设 某个 spec 声明"新增行数不超过删除行数的 3 倍"
  当 变更集净增 100 行、删除 10 行
  那么 `[complexity]` 场景 verdict 为 `fail`
  并且 evidence 包含实际行数比

场景: 无质量约束时无 verdict
  测试:
    包: agent-spec
    过滤: test_complexity_verifier_silent_without_constraints
  假设 某个 spec 的 Constraints 中没有质量相关关键词
  当 lifecycle 执行 complexity 层
  那么 不产生任何额外场景或 verdict

场景: 行数比达标时 pass
  测试:
    包: agent-spec
    过滤: test_complexity_verifier_passes_on_acceptable_ratio
  假设 某个 spec 声明"新增行数不超过删除行数的 3 倍"
  当 变更集净增 20 行、删除 10 行
  那么 `[complexity]` 场景 verdict 为 `pass`

场景: 使用 git diff 统计行数变化
  测试:
    包: agent-spec
    过滤: test_complexity_verifier_uses_git_diff_stats
  假设 某个 spec 声明行数比约束
  当 ComplexityVerifier 计算变更统计
  那么 统计来源为 git diff 的 `--stat` 输出
  并且 不依赖外部 lint 工具
