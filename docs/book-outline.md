# agent-spec 1.0 Book — 大纲

> Baseline: agent-spec 1.0.0（兼容性承诺生效版）。中文先行，英文版后续。
> 组织：mdbook + mdbook-mermaid，源码在 `book/`，每章 ≥1 Mermaid 图。
> 写作方式：spec 驱动 —— 本书自身有 KLL 需求（REQ-AGENT-SPEC-BOOK）与任务合同
> （specs/task-agent-spec-book.spec.md），结构质量由绑定测试机械守卫。

## 前言

阅读准备、前置知识、两条阅读路径（速通实践者 / 深读架构师）、全书知识地图（Mermaid）、标记约定。

## 第一部分 入门（由浅入深的起点）

- ch01 意图编译器是什么 —— 审查点位移、一图速览
- ch02 安装与第一个合同 —— install / init / lint / lifecycle 五分钟跑通
- ch03 七步工作流 —— 全景流程与每步命令

## 第二部分 合同（The Contract，核心用法）

- ch04 合同四要素 —— Intent / Decisions / Boundaries / Completion Criteria
- ch05 场景 DSL 与测试绑定 —— BDD 关键字、Test selector、表格、标签
- ch06 质量门 lint —— linter 家族、lint-ack、Rule 分组
- ch07 验证与重试 lifecycle —— 四层验证、五种 verdict、重试协议
- ch08 边界、守卫与符号 —— BoundariesVerifier、guard、### Symbols（Linker 入口）
- ch09 验收与追溯 —— explain / stamp / matrix / archive

## 第三部分 意图编译（The Intent Compiler）

- ch10 从 PRD 到需求 IR —— intake 标记块、requirements import、YAML 方言
- ch11 治理与三轴状态 —— 状态机、transition/supersede、status、machine JSON
- ch12 计划与工作单元 —— graph、work-units、plan --gate、draft-specs
- ch13 溯源与重放 —— traceability、provenance v1/v2、verify-run
- ch14 编译束与代码绑定 —— compile（agent-spec-v1/arc-v1）、bind、bundle

## 第四部分 知识与生态

- ch15 KLL 与 liveness —— knowledge/、四类文档、trace --gate、lint-knowledge
- ch16 Rust Atlas —— 图构建/查询/新鲜度、MCP atlas 工具
- ch17 Live Wiki 与 MCP —— wiki 家族、只读 MCP server

## 第五部分 理念与架构

- ch18 设计哲学 —— 确定性优先、skip≠pass、derived-never-stored、ADR-001
- ch19 架构全景 —— 双 IR、Intent-Code Linker、五交付边界、schema 家族、1.0 承诺

## 附录

- A 命令速查表
- B 场景 DSL 参考卡
- C 本书的 Spec（自举）—— 本书需求与合同全文及其验证结果
- D 端到端轨迹（E2E Traces）—— ①一条需求从 PRD 到 honored；②一次合同验证之旅
