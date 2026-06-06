spec: task
name: "单源多工具生成 v1"
inherits: project
tags: [ecosystem, integrations, phase6]
depends: [task-lint-ack-dimensions-v1]
estimate: 1d
---

## 意图

用单一来源生成各 Agent 工具的集成指令文件,替代手工维护 `AGENTS.md` / `.cursorrules` /
Claude skill 之间必然漂移的多份副本。一份 `integration_body()` 渲染成多个目标格式;
`--check` 模式检测现有文件是否与单源漂移,作为 CI 守卫。

## 已定决策

- 单一来源 `integration_body() -> String`:tool-first 的核心指令(用 `agent-spec` 的 contract → plan → lifecycle → guard → explain)。
- `render_target(target, body) -> String`:把同一 body 包装成目标格式。targets:`agents`(AGENTS.md)、`cursor`(.cursorrules)、`claude`(skill markdown,带 frontmatter)。
- 所有 target 的正文都来自同一个 `integration_body()`,不各自维护副本(单源保证)。
- 新命令 `agent-spec gen-integrations --target <agents|cursor|claude|all> [--out <dir>] [--check]`:
  - 默认:把渲染结果写到 out 目录下对应文件。
  - `--check`:不写文件;若现有文件内容与渲染结果不同,以非零状态报告漂移(CI 守卫)。
- 纯函数 `integration_body` / `render_target` 可单测;命令只做 IO。
- 不改 verification / lint / is_passing。

## 边界

### 允许修改

- src/spec_report/**（integration 渲染模块)
- src/main.rs（gen-integrations 子命令)
- README.md、examples/**

### 禁止做

- 不要为每个 target 维护独立正文(必须单源)。
- 不要在本期复刻 Spec Kit 那种 30+ integration 注册/manifest 机制(只做 3 个核心 target)。
- 不要改 verification / lint / is_passing。
- `--check` 不得写文件。

## 完成条件

### Rule: single-source-of-truth — 所有 target 共享同一正文

场景: 三个 target 都包含同一核心正文
  测试:
    过滤: test_all_targets_share_integration_body
  假设 调用 `integration_body()` 得到核心正文
  当 分别渲染 agents / cursor / claude 三个 target
  那么 三者的输出都包含该核心正文的关键指令(`lifecycle`、`guard`)

场景: 正文提及 tool-first 工作流
  测试:
    过滤: test_integration_body_is_tool_first
  假设 调用 `integration_body()`
  当 检查内容
  那么 包含 `contract`、`lifecycle`、`guard` 三个命令名

### Rule: per-target-format — 每个 target 有正确的格式外壳

场景: claude target 带 skill frontmatter
  测试:
    过滤: test_claude_target_has_frontmatter
  假设 渲染 claude target
  那么 输出以 YAML frontmatter(`---`)开头,含 `name:` 字段

场景: agents target 是纯 markdown 无 frontmatter
  测试:
    过滤: test_agents_target_is_plain_markdown
  假设 渲染 agents target
  那么 输出不以 `---` frontmatter 开头
  并且 含一级标题

场景: 未知 target 报错
  测试:
    过滤: test_unknown_target_errors
  假设 一个未知 target 名 `vim`
  当 调用 render 入口
  那么 返回错误而不 panic

### Rule: check-mode-detects-drift — check 模式检测漂移且不写文件

场景: 内容一致时 check 通过
  测试:
    过滤: test_check_passes_when_content_matches
  假设 现有文件内容与渲染结果完全一致
  当 以 check 模式比较
  那么 报告"无漂移"(一致)

场景: 内容不同时 check 报告漂移
  测试:
    过滤: test_check_reports_drift_when_different
  假设 现有文件内容与渲染结果不同
  当 以 check 模式比较
  那么 报告"漂移"(不一致)

## 排除范围

- Spec Kit 式的 integration 注册表 / manifest / 文件哈希卸载机制(只做 3 个核心 target)
- 超过 agents/cursor/claude 之外的工具目标(后续按需增加)
- 把生成纳入 lifecycle/guard 门禁
