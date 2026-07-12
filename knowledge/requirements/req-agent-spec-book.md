---
kind: requirement
id: REQ-AGENT-SPEC-BOOK
title: "agent-spec 1.0 Book"
status: accepted
liveness: auto
tags: [book, docs, mdbook]
---

# agent-spec 1.0 Book

## Problem

agent-spec 1.0 的能力散布在 README、skills 与 CHANGELOG 中，缺一本由浅入深、
以用法为主线、兼顾理念与架构的系统读物。本书用 mdbook 组织，中文先行，
Mermaid 图随文渲染，并以 spec 驱动写作：结构质量由绑定测试机械守卫，
而非口头承诺。

## Requirements

[REQ-AGENT-SPEC-BOOK-MDBOOK] 本书 MUST 以 mdbook 工程形式存在于 `book/`，`book.toml` 配置 `mdbook-mermaid` 预处理器使 Mermaid 在渲染页面中生效。

[REQ-AGENT-SPEC-BOOK-STRUCTURE] `book/src/SUMMARY.md` MUST 列出前言、五个部分的全部章节与附录，且每个被列出的章节文件 MUST 存在且非空。

[REQ-AGENT-SPEC-BOOK-ANCHOR] 每一章 MUST 以 `> **定位**` 锚开篇，并标注 baseline（agent-spec 1.0.0）。

[REQ-AGENT-SPEC-BOOK-MERMAID] 每一章 MUST 含至少一个 Mermaid 图（图文并茂是硬性要求，不是装饰）。

[REQ-AGENT-SPEC-BOOK-PREFACE] 前言 MUST 提供至少两条阅读路径与一幅全书知识地图（Mermaid）。

[REQ-AGENT-SPEC-BOOK-DOGFOOD] 附录 MUST 收录本书自身的需求与合同（自举展示），并说明结构测试如何守卫本书质量。

[REQ-AGENT-SPEC-BOOK-TRACES] 附录 MUST 含至少两条跨章 E2E 轨迹，每条覆盖三章以上并含时序/流程 Mermaid 图。

[REQ-AGENT-SPEC-BOOK-ZH-FIRST] 中文版 MUST 为第一版完整内容；英文版作为后续工作显式声明，不阻塞中文版交付。

## Scenarios

Scenario: 结构完整可渲染
  Given `book/` 下的 mdbook 工程
  When 结构守卫测试运行
  Then SUMMARY 中每个章节文件存在且非空，且 mermaid 预处理器已配置

Scenario: 章章有图有锚
  Given 全部章节文件
  When 逐章检查
  Then 每章含定位锚与至少一个 Mermaid 块

Scenario: 缺章即失败
  Given SUMMARY 引用了一个不存在的章节文件
  When 结构守卫测试运行
  Then 测试失败并指名缺失文件

## Dependencies

- REQ-KLL-WORK-UNITS

## Source Trace

- decision origin: 用户委托 2026-07-13 —— "侧重用法、理念和架构，由浅入深；spec 驱动写作；mdbook + mermaid；中英文，先中文"
- 写作规范来源: tech-writer skill v2.0.0 Book Writing Mode（定位锚、版本基线、E2E traces、前言阅读路径）
- staged contract: specs/task-agent-spec-book.spec.md
