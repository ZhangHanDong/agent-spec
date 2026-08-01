---
kind: decision
id: ADR-002
title: "Forward-Walking Pipeline Enforcement"
status: accepted
tags: [knowledge, governance, lint, scaffold, skills]
---

# Forward-Walking Pipeline Enforcement

## Context

LEP-001（accepted 2026-08-01）指出：LEP → ADR → REQ → spec 的向前走流水线
只存在于设计文档里，工具链的地图（skill、模板）、路牌（错误信息）、关卡
（lint）都不阻止——有时甚至教唆——跳层。实证包括：本仓库 74 个 spec 中 37 个
无 `satisfies:`；`proposal-template.md` 的 id 方案与 `init --workspace` 脚手架
冲突且其 section 结构过不了 `proposal-required-section` lint（模板被扫描豁免
所以陷阱不可见）；知识图完整性检查散在三套验证器里而 `lint-knowledge --gate`
只跑其一；唯一面向知识层的 skill 无版本头且不提 proposals/decisions。

## Decision

采纳 LEP-001 的四个工作流，并裁决其未决问题：

- **前缀注册表定案**：proposal 的 id 方案为 `LEP-NNN`，废弃
  `proposal-template.md` 中的 `PROP-YYYY-MM-DD-SLUG`。注册表
  （`LEP-` → proposals、`ADR-` → decisions、`REQ-` → requirements、
  `task-` → specs）落盘在 `knowledge/standards/operational/`，文件名仍按
  `YYYY-MM-DD-slug.md`，稳定 id 只存在于 frontmatter。
- **门禁合流路径定案**：graph/plan 两套验证器的诊断并入现有
  `lint-knowledge --gate`，不新建 `knowledge` 命令空间；命令空间重组是
  独立的 CLI 表面变更，需另立提案。
- **orphan-spec 分阶段升级定案**：引入时为 Info；下一个 minor 升 Warning；
  下一个 major 升 Error。存量豁免记录在仓库内基线文件，基线只准缩小。
- 四个工作流各由一份需求文档治理，每份需求由一份带 `satisfies:` 的任务
  合约落地（见 Source Trace）。

## Consequences

Good, because 跳层从主观判断变成机械诊断：对人和 agent 同一套关卡，
`trace` 能沿注册表把 LEP → ADR → REQ → spec 走成数据。

Good, because 三套验证器合流后语料只有一套诊断词汇表，既有的
dangling 检查进入它们本该在的门禁路径。

Bad, because 分阶段严重级别意味着一个孤儿 spec 仍然放行的窗口期；基线
必须逐版本缩小，否则过渡态固化为现状。

Bad, because `lint-knowledge --gate` 吸收 graph/plan 检查会拉长门禁耗时，
且改动集中在已经过大的 `main.rs` 命令接线上，落地必须拆成多份合约。

## Alternatives Considered

- 保留 `PROP-YYYY-MM-DD-SLUG` —— 被否：脚手架与 KLL 设计文档均已使用
  `LEP-`，且日期入 id 与文件名标准重复；两处只能有一个真相。
- 双前缀并存（LEP 与 PROP 都合法）—— 被否：路由表的价值就在于「一类
  东西一个去处」，双前缀让每条 lint 和每张路由表都要写两遍。
- 立即新建 `knowledge` 命令空间并吸收 `lint-knowledge`/`trace` —— 被否
  （推迟）：那是破坏性 CLI 表面变更，与本决策的门禁目标正交，不应
  捆绑落地。
- orphan-spec 直接 Error —— 被否：半数存量 spec 一夜之间打断 `guard`，
  惩罚最早采用者；带基线的分阶段升级到达同一终点。

## Source Trace

- proposal: LEP-001
  (knowledge/proposals/2026-08-01-forward-walking-knowledge-pipeline.md,
  accepted 2026-08-01)
- governed requirements: REQ-KNOWLEDGE-PREFIX-REGISTRY,
  REQ-PIPELINE-INTEGRITY-GATE, REQ-KNOWLEDGE-SCAFFOLD,
  REQ-SKILL-GUIDANCE-GOVERNANCE
- staged contracts: specs/roadmap/task-knowledge-prefix-registry.spec.md,
  specs/roadmap/task-pipeline-integrity-gate.spec.md,
  specs/roadmap/task-knowledge-scaffold.spec.md,
  specs/roadmap/task-skill-guidance-governance.spec.md
- field incident: agent-chat workspace session post-mortem, 2026-08-01
