spec: task
name: "Knowledge Prefix Registry"
tags: [knowledge, governance, standards]
satisfies: [REQ-KNOWLEDGE-PREFIX-REGISTRY]
risk: B
---

## Intent

把知识层 id 前缀注册表落成单一真相：`standards/operational` 下的注册表
文档、与之一致的 proposal 模板（`LEP-NNN`、含 lint 必需三节）、以及固定
「模板实例化即合规」性质的测试。这是 ADR-002 四个工作流的第一步，后续
的路由表与 lint 规则都引用它。

## Decisions

- 注册表文档路径：`knowledge/standards/operational/id-registry.md`，映射
  `LEP-` → proposals、`ADR-` → decisions、`REQ-` → requirements、
  `task-` → specs。
- `proposal-template.md` 改为 `LEP-NNN` 方案，section 结构以
  `Context/Decision/Consequences` 为必备骨架，原 Summary/Motivation 等节
  作为可选扩展保留在骨架之后。
- `scaffold.rs` 的 `LEP_TEMPLATE` 与仓库内模板文件逐字节一致，由测试
  `test_repo_templates_match_scaffold` 固定。
- 模板实例化测试：把每个 `*-template.md` 的占位 id 替换为合法 id 后走
  `lint_doc`，断言零 Error。

## Boundaries

### Allowed Changes
- knowledge/standards/operational/**
- knowledge/proposals/proposal-template.md
- src/spec_knowledge/scaffold.rs
- src/spec_knowledge/mod.rs
- specs/task-knowledge-prefix-registry.spec.md

### Forbidden
- 不改动任何 lint 规则的语义
- 不改动既有知识文档的 id
- 不引入新依赖

## Out of Scope

- 新 lint 规则（W2 合约负责）
- `knowledge new` 命令（W3 合约负责）
- skill 文本变更（W3/W4 合约负责）

## Completion Criteria

场景: 注册表文档存在且映射齐全
  测试: test_id_registry_doc_lists_all_prefixes
  假设 仓库处于合约完成状态
  当 读取 knowledge/standards/operational/id-registry.md
  那么 四条前缀到目录的映射逐条存在

场景: proposal 模板实例化零 Error
  测试: test_proposal_template_instantiation_lints_clean
  假设 proposal-template.md 的占位 id 被替换为 LEP-999
  当 lint_doc 以 kind proposal 扫描该实例
  那么 诊断中零 Error

场景: PROP 前缀回归被测试拦截
  测试: test_no_prop_prefix_remains
  假设 仓库任一模板或脚手架常量被改回 PROP- 前缀
  当 前缀一致性测试运行
  那么 测试失败并指名冲突文件路径

场景: 模板与脚手架漂移被拦截
  测试: test_repo_templates_match_scaffold
  假设 knowledge/proposals/proposal-template.md 与 scaffold 常量内容不一致
  当 模板一致性测试运行
  那么 测试失败并输出首个差异行号
