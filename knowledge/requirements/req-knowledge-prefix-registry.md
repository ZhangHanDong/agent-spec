---
kind: requirement
id: REQ-KNOWLEDGE-PREFIX-REGISTRY
title: "Knowledge Artifact Prefix Registry"
status: accepted
liveness: auto
tags: [knowledge, governance, standards, template]
---

# Knowledge Artifact Prefix Registry

## Problem

「我手里的东西属于哪一层」目前没有任何一处权威回答：`proposal-template.md`
用 `PROP-YYYY-MM-DD-SLUG`，`init --workspace` 脚手架用 `LEP-NNN`，KLL 设计
文档说注册表应在 `standards/operational` 却并不存在。更糟的是模板自身的
section 结构（Summary/Motivation/…）缺少 `Context/Decision/Consequences`，
照抄模板写出的提案必吃三个 `proposal-required-section` Error，而模板文件
被扫描豁免（`governance.rs` 排除 `*-template.md`），陷阱在有人踩之前不可见。
ADR-002 已裁决 `LEP-NNN` 胜出；本需求把注册表与模板落成一致的、可被后续
lint 引用的单一真相。

## Requirements

[REQ-KNOWLEDGE-PREFIX-REGISTRY-DOC] `knowledge/standards/operational/` 下 MUST 存在注册表文档，逐条列出 id 前缀到目录的映射：`LEP-` → proposals、`ADR-` → decisions、`REQ-` → requirements、`task-` → specs，并声明文件名与 id 分离（文件名 `YYYY-MM-DD-slug.md` / 稳定 id 在 frontmatter）。

[REQ-KNOWLEDGE-PREFIX-REGISTRY-TEMPLATE-ID] `knowledge/proposals/proposal-template.md` 与 `init --workspace` 写出的 `lep-template.md` MUST 使用 `LEP-NNN` id 方案，仓库内 MUST NOT 残留 `PROP-` 前缀的模板或脚手架常量。

[REQ-KNOWLEDGE-PREFIX-REGISTRY-TEMPLATE-LINT] 每个 `*-template.md` 按其 kind 实例化（替换占位 id 后）MUST 通过该 kind 的 required-section lint 且零 Error；此性质 MUST 由测试固定，防止模板与 lint 再次漂移。

[REQ-KNOWLEDGE-PREFIX-REGISTRY-SCAFFOLD-SYNC] 仓库根下 `knowledge/` 的模板与 `scaffold.rs` 中的脚手架常量 MUST 内容一致，一致性 MUST 由测试固定。

## Scenarios

Scenario: 注册表回答路由问题
  Given 一位作者持有一篇尚在辩论的治理提案
  When 查阅 knowledge/standards/operational/ 下的注册表文档
  Then 得到唯一去处 knowledge/proposals/ 与唯一前缀 LEP-

Scenario: 模板实例化即合规
  Given 任一 *-template.md 按占位说明填入合法 id
  When lint-knowledge 扫描该实例
  Then 零 Error

Scenario: 模板与脚手架漂移被测试拦截
  Given proposal-template.md 被改回 PROP- 前缀
  When 仓库测试套件运行
  Then 模板一致性测试失败并指名冲突文件

## Source Trace

- decision: ADR-002（前缀注册表定案 LEP-NNN，2026-08-01）
- proposal: LEP-001
- 实测证据: proposal-template.md 无 Context/Decision/Consequences，与
  proposal.rs REQUIRED 三节冲突；governance.rs 模板豁免使冲突不可见
- staged contract: specs/task-knowledge-prefix-registry.spec.md
