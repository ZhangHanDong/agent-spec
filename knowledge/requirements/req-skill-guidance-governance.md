---
kind: requirement
id: REQ-SKILL-GUIDANCE-GOVERNANCE
title: "Skill Guidance Governance"
status: accepted
liveness: auto
tags: [skills, governance, ci, errors, dx]
---

# Skill Guidance Governance

## Problem

引导面会无声腐烂：2026-08-01 的现场事故中，agent 加载的是 3.2.0 版
authoring skill（仓库已捆 3.5.0），照过时地图走了旧路；五个捆绑 skill 里
唯一面向知识层的 `agent-spec-intent-compiler` 连版本头都没有；frontmatter
解析错误不列合法值集（`parser.rs` 的 `unknown status '{other}'`），作者
只能翻源码自救——而同仓库 `transitions.rs` 的同类错误早已列出合法值。
ADR-002 要求：版本头进 CI、错误信息自愈、引导文本按 superpowers 方式
对抗测试。

## Requirements

[REQ-SKILL-GUIDANCE-GOVERNANCE-VERSION-HEADER] `skills/` 下每个 SKILL.md MUST 含版本头，至少包括 Version、Last Updated 与 `Tracks:`（声明其对应的 agent-spec 版本）。

[REQ-SKILL-GUIDANCE-GOVERNANCE-CI-CHECK] CI MUST 校验每个 SKILL.md 的 `Tracks:` 与当前 crate 版本一致，不一致时构建失败并指名过期的 skill 文件。

[REQ-SKILL-GUIDANCE-GOVERNANCE-ERROR-ENUM] `spec_knowledge` frontmatter 解析对 kind、status、liveness 的未知取值 MUST 在错误信息中列出完整合法值集，风格与 `transitions.rs` 既有先例一致。

[REQ-SKILL-GUIDANCE-GOVERNANCE-ADVERSARIAL] 仓库 MUST 含至少一个对抗路由测试用例：模拟「跳过形式，直接写 spec」的指令场景，断言 skill 文本仍指向 knowledge 层路由表与 orphan-spec 关卡；用例随 skill 文本变更一起维护。

## Scenarios

Scenario: 过期 skill 拦在 CI
  Given 某 SKILL.md 的 Tracks 落后于 crate 版本
  When CI 的 skill 版本校验运行
  Then 校验以非零退出且输出指名该 SKILL.md 路径

Scenario: 未知 status 错误可自愈
  Given 一份 frontmatter 写有 status: drafted 的知识文档
  When lint-knowledge 解析该文档
  Then 错误信息列出 proposed、accepted、superseded、deprecated、rejected 全集

Scenario: 缺版本头的 skill 被指名
  Given skills/ 下存在无版本头的 SKILL.md
  When CI 的 skill 版本校验运行
  Then 校验以非零退出且指名缺失 Tracks 的文件

Scenario: 对抗路由用例常绿
  Given 「跳过形式直接写 spec」的固定对抗指令文本
  When 对抗路由测试运行
  Then 断言命中 skill 中的路由表与硬关卡文本，测试通过

## Dependencies

- REQ-KNOWLEDGE-SCAFFOLD

## Source Trace

- decision: ADR-002（四工作流采纳，2026-08-01）
- proposal: LEP-001
- 现场事故: agent-chat 工作区 2026-08-01——加载 3.2.0 skill 的 agent 产出
  满分孤儿合约；本仓库 .claude/skills 仅装 5 个捆绑 skill 中的 2 个
- 风格先例: transitions.rs 的 unknown governance status 错误列出合法值全集
- 方法论: superpowers writing-skills 的压力场景测试与 skip-formalities 用例
- staged contract: specs/roadmap/task-skill-guidance-governance.spec.md
