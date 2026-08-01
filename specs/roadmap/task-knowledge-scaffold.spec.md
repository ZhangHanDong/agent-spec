spec: task
name: "Knowledge Scaffold Command"
tags: [knowledge, cli, scaffold, skills]
satisfies: [REQ-KNOWLEDGE-SCAFFOLD]
depends: [task-knowledge-prefix-registry]
risk: B
---

## Intent

新增 `agent-spec knowledge new <kind> <id>` 单件脚手架命令，产出 lint 干净、
预填合法枚举值、末尾带唯一出口指引的知识层骨架；并把「持有物 → 去处 →
前缀 → 命令」路由表写进 authoring 与 intent-compiler 两个 skill，使正确的
下一份产物比跳层捷径更便宜。

## Decisions

- 命令面：`knowledge new <proposal|decision|requirement> <id>`，kind 用
  clap 枚举，id 为位置参数；生成文件名按 `YYYY-MM-DD-slug.md`（proposal）
  与 `<lower-id>-<slug>.md`（decision/requirement）惯例，slug 取自
  `--title` 或 id。
- 骨架内容复用 `scaffold.rs` 模板常量，占位 id 替换为实参 id，frontmatter
  预填：proposal `status: proposed` + `liveness: n/a`；decision
  `status: proposed`；requirement `status: proposed` + `liveness: auto`。
- 前缀校验查注册表映射：错配时退出码 2，错误信息列出该 kind 合法前缀。
- 已存在目标文件时退出码 2 并打印已存在路径，不写任何字节。
- 路由表以同一段 Markdown 表格写入两个 skill，表格内容与
  `id-registry.md` 的映射一致，由 CI 文本包含检查固定。

## Boundaries

### Allowed Changes
- src/main.rs
- src/spec_knowledge/scaffold.rs
- src/spec_knowledge/mod.rs
- skills/agent-spec-authoring/SKILL.md
- skills/agent-spec-intent-compiler/SKILL.md
- fixtures/**
- specs/roadmap/task-knowledge-scaffold.spec.md

### Forbidden
- 不覆盖任何已存在文件
- 不改动 lint 规则
- 不引入交互式提问流程

## Out of Scope

- guidance/context 两类文档的脚手架
- skill 版本头与 CI 校验（W4 合约负责）
- `init --workspace` 行为变更

## Completion Criteria

场景: 生成的提案骨架即刻合规
  测试: test_knowledge_new_proposal_lints_clean
  假设 工作区已有 knowledge/ 目录树
  当 运行 knowledge new proposal LEP-002
  那么 knowledge/proposals/ 下生成骨架且 lint_doc 对其零 Error

场景: 骨架末尾只有一个出口
  测试: test_knowledge_new_requirement_has_single_exit
  假设 由 knowledge new requirement REQ-X 生成的骨架
  当 解析骨架末节
  那么 出口文本指向 requirements draft-specs 且无第二出口

场景: 前缀错配被拒绝并列出合法前缀
  测试: test_knowledge_new_rejects_prefix_mismatch
  假设 任意工作区
  当 运行 knowledge new decision LEP-9
  那么 退出码为 2 且 stderr 列出 decision 的合法前缀 ADR-

场景: 已存在文件不被覆盖
  测试: test_knowledge_new_refuses_to_clobber
  假设 目标路径已存在同名文档
  当 再次运行相同 knowledge new 命令
  那么 退出码为 2 且文件内容逐字节不变

场景: 目录树缺失时报可自愈错误
  测试: test_knowledge_new_without_workspace_names_init
  假设 工作区无 knowledge/ 目录
  当 运行 knowledge new proposal LEP-002
  那么 退出码为 2 且错误信息指名 init --workspace
