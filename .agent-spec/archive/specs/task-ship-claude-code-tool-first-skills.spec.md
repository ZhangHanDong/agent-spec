spec: task
name: "交付 Claude Code tool-first skills"
inherits: project
tags: [done, bootstrap, skills, claude-code, tool-first, phase5]
---

## 意图

把 `agent-spec` 真正接到 Claude Code 的日常工作流里，
通过项目级 skills 让 Claude Code 默认走 tool-first 路径，而不是只停留在库接口或概念描述。

## 已定决策

- 在仓库内提供 project-local Claude Code skills
- skills 至少分成 `tool-first` 与 `authoring` 两条路径
- `tool-first` skill 作为主路径，明确使用 `contract`、`lifecycle`、`guard`

## 边界

### 允许修改
- .claude/skills/**
- src/**
- specs/**
- README.md

### 禁止做
- 不要把 skill 的主叙事改回嵌入式 API
- 不要让 authoring skill 替代 tool-first skill 成为默认执行路径
- 不要只创建 skill 文件而不写明具体命令和工作流

## 完成条件

场景: tool-first skill 指向核心 CLI 工作流
  测试:
    包: agent-spec
    过滤: test_claude_code_tool_first_skill_exists_and_mentions_contract_lifecycle_guard
  假设 仓库内提供 Claude Code project-local skills
  当 用户查看 `agent-spec-tool-first` skill
  那么 skill 内容包含 `contract`、`lifecycle`、`guard`
  并且 明确说明优先使用 CLI 工具路径

场景: authoring skill 指向 Task Contract 写作
  测试:
    包: agent-spec
    过滤: test_claude_code_authoring_skill_exists_and_mentions_task_contract_sections
  假设 仓库内提供 `agent-spec-authoring` skill
  当 用户查看该 skill
  那么 skill 内容包含 `Intent`、`Decisions`、`Boundaries`、`Completion Criteria`
  并且 明确要求显式 `Test:` / `测试:` selector

场景: README 说明 Claude Code skills 的使用方式
  测试:
    包: agent-spec
    过滤: test_readme_documents_claude_code_tool_first_skills
  假设 仓库根目录提供使用文档
  当 用户阅读 README
  那么 README 提到 Claude Code project-local skills
  并且 说明 `tool-first` 是默认集成路径
