spec: task
name: "强化 rewrite/parity 合同写作"
inherits: project
tags: [done, contract-quality, skills, templates, parity, phase-next]
---

## 意图

让 `agent-spec` 在 rewrite/parity 类任务上更容易写出真正绑定外部行为的合同。
本任务聚焦于 skill、模板和参考示例，把“未绑定的可观察行为”检查内建进 authoring 工作流。

## 已定决策

- 本轮不新增 DSL 语法，只通过 skills、模板和参考 spec 提升写作质量
- `agent-spec-authoring` 必须新增 `Behavior Surface Checklist`
- `agent-spec-tool-first` 必须新增 `Unbound Observable Behavior` 审查步骤
- 仓库应新增一份 rewrite/parity 示例 task spec，演示行为矩阵写法
- checklist 默认覆盖 stdout/stderr、`--json`、`-o/--output`、cold start、cache miss、fallback、local/remote、partial failure

## 边界

### 允许修改
- skills/agent-spec-authoring/**
- skills/agent-spec-tool-first/**
- .claude/skills/**
- README.md
- docs/**
- specs/**

### 禁止做
- 不要在 skill 中重新定义一套与 parser 不兼容的 spec 语法
- 不要只改 README 而忽略 skill 主体
- 不要把 rewrite/parity 指南写成抽象原则而没有具体检查清单

## 完成条件

场景: authoring skill 包含行为面检查清单
  测试:
    过滤: test_authoring_skill_includes_behavior_surface_checklist
  假设 用户正在为 CLI 或 MCP 工具编写 task spec
  当 查看 `agent-spec-authoring` skill
  那么 skill 明确要求检查 stdout/stderr、`--json`、`-o/--output`、fallback、cold start 和文件副作用
  并且 把这些行为作为合同写作前的必查项

场景: tool-first skill 包含未绑定可观察行为审查步骤
  测试:
    过滤: test_tool_first_skill_mentions_unbound_observable_behavior_review_step
  假设 用户已经完成 `agent-spec parse` 和 `agent-spec lint`
  当 查看 `agent-spec-tool-first` skill
  那么 skill 额外要求审查还有哪些 stdout、stderr、文件、网络和持久化行为未被场景绑定
  并且 说明这一步特别适用于 rewrite/parity 任务

场景: 仓库提供 rewrite/parity 示例合同
  测试:
    过滤: test_rewrite_parity_example_spec_exists_and_covers_behavior_matrix
  假设 仓库提供参考 spec
  当 用户查看 rewrite/parity 示例 task spec
  那么 示例覆盖 command x output mode、local x remote、warm cache x cold start 等行为矩阵
  并且 示例中的每个矩阵维度都绑定了显式测试选择器

场景: README 说明 rewrite/parity 合同的写法与普通功能合同不同
  测试:
    过滤: test_readme_documents_rewrite_parity_contract_authoring_guidance
  假设 用户阅读项目文档
  当 查找 rewrite 或 parity 相关说明
  那么 README 提到这类任务需要先绑定可观察行为矩阵
  并且 指向对应的 skill 或示例 spec

场景: skill 明确指出遗漏行为矩阵时合同不应交付
  测试:
    过滤: test_skill_guidance_rejects_parity_contracts_missing_behavior_matrix
  假设 某个 rewrite task spec 只描述主流程功能，没有覆盖 stdout、`--json`、fallback 或 cold start
  当 用户按照 skill 自检该合同
  那么 skill 明确要求补齐这些未绑定的可观察行为
  并且 不把该合同视为可直接交付给 agent

场景: skill 不会把普通功能合同误判为 parity 合同
  测试:
    过滤: test_skill_guidance_does_not_require_behavior_matrix_for_non_parity_tasks
  假设 某个普通增量功能 task spec 不以 rewrite、migration 或 parity 为目标
  当 用户按照 skill 自检该合同
  那么 skill 不要求补齐 rewrite/parity 行为矩阵
  并且 不把普通功能合同错误标记为缺失 parity 覆盖

## 排除范围

- parser 或 AST 的语法扩展
- 新增 lint 规则本身
- AI verifier 对 rewrite/parity 合同的专门推理
