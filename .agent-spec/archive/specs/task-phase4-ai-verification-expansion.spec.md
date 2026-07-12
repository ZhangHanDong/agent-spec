spec: task
name: "Phase 4：AI Verification Expansion"
inherits: project
tags: [done, roadmap, planned, phase4, ai]
---

## 意图

在保持确定性验证优先的前提下，
把 AI 验证从 stub 提升成真正可用的辅助层，用来处理机械验证无法覆盖的剩余场景。

## 已定决策

- provider 选择与鉴权继续留在宿主或适配层，不把 provider 配置塞回核心 Contract 模型
- 先增强 `AiRequest` 的上下文保真度，再接真实 backend
- sycophancy-aware lint 先于 adversarial 模式交付
- adversarial 模式保持显式 opt-in，且位于普通 AI 验证之后

## 边界

### 允许修改
- src/spec_core/**
- src/spec_lint/**
- src/spec_verify/**
- src/spec_gateway/**
- src/**
- README.md
- specs/**

### 禁止做
- 不要让 AI 层改变默认的确定性验证路径
- 不要把 provider/model/auth 配置写死进 Task Contract
- 不要在 prompt 中用“必须找出 bug”这类诱导性表达

## 完成条件

场景: AI request 打包完整验证上下文
  测试:
    过滤: test_build_ai_request_includes_contract_change_set_and_evidence_context
  假设 某个场景需要 AI 验证
  当 verifier 构造 `AiRequest`
  那么 请求包含 Contract、change set 与相关证据上下文
  并且 不只剩下场景名和裸步骤文本

场景: lint 检测 sycophancy 风险
  测试:
    过滤: test_sycophancy_linter_flags_bug_finding_bias
  假设 某个 Spec 使用“找出所有 bug”这类诱导性语句
  当 用户运行 lint
  那么 输出指出 sycophancy 风险
  并且 给出中性重写建议

场景: adversarial 验证保持显式 opt-in
  测试:
    包: agent-spec
    过滤: test_adversarial_verification_is_disabled_by_default
  假设 用户仅启用普通 AI 验证
  当 lifecycle 或 verify 执行
  那么 不会自动触发多 Agent 对抗流程
  并且 对抗性验证只在显式参数下运行

## 排除范围

- 把 provider 配置作为核心 CLI 契约的一部分
- 默认开启 adversarial 模式
