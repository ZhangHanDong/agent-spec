spec: task
name: "宿主注入 AI backend"
inherits: project
tags: [done, bootstrap, ai, gateway, embed, phase4]
---

## 意图

让 `agent-spec` 保持 provider-agnostic，
由嵌入它的宿主 agent 注入自己的 AI backend，而不是在 `agent-spec` 内部配置 provider。

## 已定决策

- `spec-gateway` 暴露接受 `AiBackend` 的验证入口
- `agent-spec` CLI 继续只保留 `off` / `stub`，不承载 provider 配置
- provider、模型、鉴权和超时等配置由宿主 agent 负责

## 边界

### 允许修改
- src/spec_gateway/**
- src/spec_verify/**
- specs/**
- README.md

### 禁止做
- 不要在 `agent-spec` 内部引入 provider 配置模型
- 不要要求宿主 agent 先把 provider 转换成 CLI 参数再调用 gateway
- 不要破坏现有 `stub` 模式和默认 `off` 行为

## 完成条件

场景: gateway 支持注入自定义 AI backend
  测试:
    过滤: test_verify_with_injected_ai_backend_uses_host_backend
  假设 某个宿主 agent 提供自定义 `AiBackend`
  当 gateway 使用该 backend 执行验证
  那么 验证结果来自该 backend
  并且 `AiAnalysis` 证据保留 backend 返回的 model 与 reasoning

场景: 默认 gateway 入口仍不依赖外部 provider
  测试:
    过滤: test_verify_default_keeps_uncovered_scenarios_skipped
  假设 某个未被覆盖的场景
  当 gateway 使用默认验证入口
  那么 场景仍然是 `skip`
  并且 不要求传入 provider 配置
