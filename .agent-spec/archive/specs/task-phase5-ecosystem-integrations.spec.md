spec: task
name: "Phase 5：Ecosystem Integrations"
inherits: project
tags: [done, roadmap, planned, phase5, ecosystem]
---

## 意图

让更多 Agent 工具和编排系统可以复用 `agent-spec`，
同时保持核心定位不变：CLI-first、tool-first、agent-agnostic。

## 已定决策

- 新的工具集成优先通过 skill / rule / convention 文件交付
- 首批扩展目标是 Codex CLI、Cursor 与 Aider
- `lifecycle` 与 `guard` 的 JSON 输出继续作为编排接口
- `checkpoint` 能力保持可选，且按 VCS 能力渐进增强
- Entire / Symphony 这类深度集成放在技能模板和接口稳定之后

## 边界

### 允许修改
- .claude/**
- AGENTS.md
- README.md
- src/**
- src/spec_report/**
- specs/**

### 禁止做
- 不要为了单一 Agent 工具改变核心 CLI 契约
- 不要让生态集成抢在 explain / run log / org.spec 之前成为主工作面
- 不要让 checkpoint 成为所有用户的默认依赖

## 完成条件

场景: 提供更多 Agent 工具的集成模板
  测试:
    包: agent-spec
    过滤: test_additional_agent_integration_templates_exist
  假设 仓库需要支持多种 Agent 工具
  当 用户查看集成模板目录
  那么 能找到 Claude Code 之外的模板
  并且 它们继续遵循 `contract -> lifecycle -> guard` 主路径

场景: JSON 输出适合作为编排接口
  测试:
    过滤: test_report_json_exposes_contract_and_verification_summary_for_orchestrators
  假设 外部编排系统读取 lifecycle 或 guard 结果
  当 用户选择 JSON 输出
  那么 输出包含结构化 Contract 与 verification summary
  并且 不要求外部系统解析人类文本

场景: checkpoint 能力保持可选
  测试:
    包: agent-spec
    过滤: test_checkpoint_commands_are_optional_and_vcs_aware
  假设 当前仓库可能是 Git、jj 或无 VCS
  当 用户查看 checkpoint 能力
  那么 命令按 VCS 能力渐进增强
  并且 不会把 checkpoint 强制注入默认 lifecycle

## 排除范围

- 把 Entire API 变成硬依赖
- 把编排系统逻辑写进验证核心
