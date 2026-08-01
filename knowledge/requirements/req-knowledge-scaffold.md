---
kind: requirement
id: REQ-KNOWLEDGE-SCAFFOLD
title: "Forward-Walking Knowledge Scaffold"
status: accepted
liveness: auto
tags: [knowledge, cli, scaffold, routing, skills]
---

# Forward-Walking Knowledge Scaffold

## Problem

现在从零写一份知识层文档的路径是：找模板（可能过不了 lint）、猜枚举值
（错误信息不列合法值）、猜目录。而写一份任务 spec 的路径短得多——这个
成本差正是跳层的经济学根源。`init --workspace` 只建目录树，没有单件
脚手架命令。ADR-002 要求让「正确的下一份产物」比「错误的捷径」更便宜：
一条 `knowledge new` 命令产出 lint 干净的骨架，skill 顶部一张路由表回答
「手里的东西该去哪」。

## Requirements

[REQ-KNOWLEDGE-SCAFFOLD-NEW] `agent-spec knowledge new <proposal|decision|requirement> <id>` MUST 在对应目录按注册表命名规则创建骨架文档，frontmatter 预填该 kind 的合法枚举值，产物经 lint-knowledge 扫描零 Error。

[REQ-KNOWLEDGE-SCAFFOLD-PREFIX-CHECK] `knowledge new` MUST 拒绝 id 前缀与 kind 不匹配的调用（如 `knowledge new decision LEP-9`），错误信息 MUST 列出该 kind 的合法前缀。

[REQ-KNOWLEDGE-SCAFFOLD-NO-CLOBBER] 目标文件已存在时 `knowledge new` MUST 拒绝执行并指名已存在路径，MUST NOT 覆盖。

[REQ-KNOWLEDGE-SCAFFOLD-EXIT-POINTER] 每个骨架末尾 MUST 写明该层的唯一出口：proposal 出口为 `## Produces: ADR-NNN`，decision 出口为治理需求文档，requirement 出口为 `requirements draft-specs`。

[REQ-KNOWLEDGE-SCAFFOLD-ROUTING-TABLE] `agent-spec-authoring` 与 `agent-spec-intent-compiler` 两个 skill 的正文 MUST 含同一张路由表（持有物类型 → 目标目录 → id 前缀 → 脚手架命令），表内容 MUST 与注册表文档一致。

## Scenarios

Scenario: 一条命令得到合规提案骨架
  Given 工作区已有 knowledge/ 目录树
  When 运行 agent-spec knowledge new proposal LEP-002
  Then knowledge/proposals/ 下生成骨架且 lint-knowledge 对其零 Error

Scenario: 前缀错配被拒绝并教路
  Given 任意工作区
  When 运行 agent-spec knowledge new decision LEP-9
  Then 命令以非零退出且错误信息列出 decision 的合法前缀 ADR-

Scenario: 已存在文件不被覆盖
  Given knowledge/decisions/ 下已有 ADR-002 文档
  When 运行 agent-spec knowledge new decision ADR-002
  Then 命令以非零退出且指名已存在路径，文件内容不变

Scenario: 骨架带唯一出口
  Given 由 knowledge new requirement REQ-X 生成的骨架
  When 阅读骨架末尾
  Then 出口指向 requirements draft-specs 且无第二出口

## Dependencies

- REQ-KNOWLEDGE-PREFIX-REGISTRY

## Source Trace

- decision: ADR-002（四工作流采纳，2026-08-01）
- proposal: LEP-001
- 实测证据: 现状仅 init --workspace 建目录树；frontmatter 枚举猜错时
  parser.rs 报 unknown status 不列合法值
- staged contract: specs/task-knowledge-scaffold.spec.md
