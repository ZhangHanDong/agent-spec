spec: task
name: "Orphan Baseline Retirement"
tags: [knowledge, governance, migration, orphan]
satisfies: [REQ-ORPHAN-BASELINE-RETIREMENT]
depends: [task-pipeline-integrity-gate]
risk: A
---

## Intent

退休 1.3.0 为 36 份存量孤儿合约建立的过渡基线：用当前内容对应的全绿
lifecycle 证据归档已经完成的合约，删除已有等价归档副本的重复活动文件，把
baseline 清为不可复活的空列表，并按 ADR-002 将新孤儿诊断升级为 Warning。

## Decisions

- 36 份合约逐份跑 lifecycle；任何 Fail、Skip、Uncertain 或 stale evidence
  都阻止该份归档，不以 `done` 标签替代行为证据。
- 30 份尚未归档的合约添加 `done` 标签，在更新后重新跑带 run log 的
  lifecycle，再通过正式 `archive` 路径迁移。
- 6 份 Phase 1–6 合约已有只多一个 `done` 标签的等价 archive 副本，删除
  重复活动副本，不覆盖 archive 历史。
- `.agent-spec/orphan-baseline.json` 保留为空列表，方便既有 CLI 参数兼容；
  任意非空条目触发 `orphan-baseline-retired` Error 且不再提供豁免。
- `orphan-spec` 升为 Warning，但 `lint-knowledge --gate` 仍只因 Error 非零；
  Error 升级留给 ADR-002 规定的下一个 major。
- 新诊断只建议真实 `satisfies:` 或带当前全绿证据的归档，不再把 baseline
  当作修复动作；不自动猜 requirement，不自动写 baseline。

## Boundaries

### Allowed Changes
- src/main.rs
- .agent-spec/orphan-baseline.json
- .agent-spec/archive/specs/**
- knowledge/requirements/req-orphan-baseline-retirement.md
- knowledge/requirements/req-pipeline-integrity-gate.md
- knowledge/context/spec-archives.md
- specs/task-orphan-baseline-retirement.spec.md
- specs/task-pipeline-integrity-gate.spec.md
- specs/task-*.spec.md
- README.md
- docs/intent-compiler/architecture.md
- book/src/ch15-kll.md
- CHANGELOG.md

### Forbidden
- 不改动 36 份历史合约的行为、场景或测试选择器
- 不给历史合约猜测或批量伪造 satisfies 链接
- 不在 lifecycle 非全 Pass 或指纹不匹配时归档
- 不覆盖已经存在的 archive 文件
- 不把 orphan-spec 直接升级为 Error
- 不改动 clause-coverage 实现与 clause baseline

## Out of Scope

- clause baseline 的清理或严重级别升级
- 为已完成的历史功能倒填新的 requirement 语料
- 下一个 major 的 orphan-spec Error 升级
- archive summary 的通用格式或累计策略重构

## Completion Criteria

场景: 非空 baseline 被永久拒绝
  测试: test_non_empty_orphan_baseline_is_rejected_after_retirement
  假设 fixture baseline 列出一份没有 satisfies 的任务合约
  当 knowledge gate 收集诊断
  那么 输出 orphan-baseline-retired Error
  并且 同一任务仍输出 orphan-spec 而不被豁免

场景: 新孤儿升级为 Warning 且给出当前出口
  测试: test_orphan_spec_is_warning_with_current_remedies
  假设 需求语料非空且活动任务合约没有 satisfies
  当 knowledge gate 收集诊断
  那么 orphan-spec 为 Warning
  并且 消息包含 satisfies、passing lifecycle evidence 与 archive
  并且 消息不建议把路径加入已退休 baseline

场景: 仓库 baseline 清零
  测试: test_repository_orphan_baseline_is_empty
  假设 迁移后的仓库状态
  当 读取 .agent-spec/orphan-baseline.json
  那么 specs 是空列表

场景: 三十六份存量退出活动集并保留归档
  测试: test_repository_retired_orphans_have_archive_evidence
  假设 1.3.0 baseline 中原有 36 条路径
  当 检查 active specs、archive specs 与归档摘要
  那么 每份活动路径不存在、archive 文件存在且摘要指名该文件

场景: 活动任务合约全部有真实需求边
  测试: test_repository_active_task_specs_are_requirement_linked
  假设 当前 knowledge 与 specs 根目录
  当 构建 requirement plan
  那么 每个 task spec 的 satisfies 非空
  并且 不存在 dangling-spec-coverage

场景: 迁移不触碰 clause 工作
  测试: test_orphan_retirement_contract_excludes_clause_migration
  假设 orphan 与 clause 各有独立 baseline 和合约
  当 检查本合约边界与排除范围
  那么 clause baseline 清理与严重级别升级均明确排除
