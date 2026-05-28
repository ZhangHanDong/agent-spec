# Phase {N} Retrospective: {phase-name}

> 填写时机:**该 phase 完成实施、`lifecycle` / `guard` 通过之后**。
> 不要留空字段, 不要把"待补充"作为最终状态——复盘要么数据完整, 要么不写。
> 本模板对应路线图见 `docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: {N}
- **合约**: `specs/task-{phase-slug}.spec.md`
- **开始日期**: {YYYY-MM-DD}
- **完成日期**: {YYYY-MM-DD}
- **作者 / Reviewer**: {人 + agent 型号}
- **最终 lifecycle verdict**: PASS / FAIL ({summary 计数: passed / failed / skipped / uncertain / pending_review})

## Before / After 关键指标

具体数字, 不写定性描述。空着不如不报。

| 指标 | Before | After | Δ |
|------|--------|-------|---|
| spec 文件数 | | | |
| scenarios 平均每 spec | | | |
| 有 Rule 归属的 scenario 比例 | | | |
| lint 触发率(per spec) | | | |
| `guard` 平均运行时长(s) | | | |
| `cargo test` 通过/失败/总数 | | | |
| {phase-specific 指标 1} | | | |
| {phase-specific 指标 2} | | | |

## Observations

按事实陈述, 不做归因或建议。每条一行, 必要时附数据。

- {例: "v1 上线后, 52 个现有 spec 中 23 个触发 `bdd-rule-grouping`, 19 个被 agent 主动改写, 4 个被 acknowledged 为合理例外"}
- {例: "中文 spec 触发 `bdd-implementation-detail-step` 的频率比英文 spec 高 3.2 倍, 集中在'点击/输入'类 UI 动词"}
- {例: "agent 在自我纠正指南指引下, 主动修复 lint 的比例从 0% 升至 64%"}

## Main Takeaways

从 Observations 抽出的、能被下一个 Phase 使用的判断。每条要可付诸行动。

- {例: "自我纠正指南是 lint 有效性的核心, 不是规则本身——Phase 5 完整 lint-ack 时必须保留指南格式"}
- {例: "中文社区需要独立维护的关键词列表, 不能用英文列表直接翻译"}
- {例: "Rule provenance 应在 promote 时强制写入, 否则 audit 无法识别 AI 生成且未 affirm 的 Rule"}

## Refactor Recommendations

哪些事实暗示需要修改下一个 phase 的计划? 直接给出对路线图的具体改动建议。

- {例: "Phase 2 覆盖矩阵的 verdict 列应增加 `evidence_provenance` 区分 computational vs inferential, 字段在本 phase 已就位"}
- {例: "Phase 4 Discovery 的 `## Questions` lint 阈值应取 …"}

## Spec 库健康度快照

本 phase 落地后, 用 `agent-spec audit --dry-run` 跑出来的快照(audit 命令 Phase 8 落地后可用; Phase 1-7 期间手工汇总, 用 grep + jq 拼即可):

- 总 Rule 数(按 scope): task = {n}, capability = {n}, project = {n}
- Dead Rules(未被任何 task 引用): {n}
- Orphan Examples(无 Rule 归属): {n}
- AI-generated 未被 human affirm 的 Rules: {n}
- 平均 Rule 重用次数(被多少 task / capability 引用): {n}
- {本 phase 引入的健康度新维度}

## 与原合约的偏离

实施过程中**没有按合约走的**地方, 必须如实列出。这不是认错——这是 BDD-spine 路线对自身的诚实纪律。

- {例: "合约要求 `Rule:` 行 em dash 分隔 id 与 name, 实际实现接受了 ` -- ` 与 ` — ` 两种; 已在 v1.0.1 patch 中收紧"}
- {例: "合约场景 `test_X` 在最终代码中拆分为 `test_X_a` + `test_X_b`, 因为单测覆盖颗粒度需求"}

## 下一步

完成本复盘之后, 直接开 next phase 的 task contract 草案。

- 下一个 phase: Phase {N+1}, 合约草案路径 `specs/task-{next-phase-slug}.spec.md`
- 本复盘影响的合约条目: {引用 next-phase 合约里需要根据本复盘调整的具体段落}
