# Phase 8 Retrospective: spec 库健康度 audit

> 填写时机:实施 + `lifecycle` / `guard` 通过之后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 8（`agent-spec audit`:跨整个 spec 库的机械健康度聚合)
- **合约**: `specs/task-audit-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)
- **最终 lifecycle verdict**: PASS — 7 passed / 0 failed / 0 skipped / 0 uncertain(7/7)
- **commit**: `9d12343`

## Before / After 关键指标

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 7 / 7 PASS |
| commit `+#[test]` | 7 |
| diff | 4 files, 275 insertions |
| 新增 CLI 命令 | `agent-spec audit`(`--spec-dir` / `--format text\|json`) |
| 新增结构 | `AuditReport` + `audit_specs(docs)` |
| 对抗 hunt | 未执行(范围决策,见下) |

## Observations

- **audit 是"观测性,永不 gate"**:聚合 `spec_count` / `rule_count` / `scenario_count` / `unproven_rules` / `ungrouped_scenarios` / `open_questions` / `malformed_rules`,只报告不判失败。这把前面各 phase 埋下的健康度信号(P3 Rule、P4 open question、P1 malformed rule)收口成**一张库级快照**。
- **复用而非重算**:`unproven_rules`(Rule 无 Example)、`ungrouped_scenarios`(scenario 无 Rule 归属)、`open_questions`(复用 Phase 4 的 `question_is_resolved`)、`malformed_rules`(复用 Phase 1 的 id 校验)——audit 不引入新判定,只聚合既有语义,避免"两套判定分叉"。
- **JSON 输出可被工具消费**:`AuditReport` derive `Serialize`,`--format json` 让 CI / dashboard 能消费库健康趋势。

## Main Takeaways

- **健康度信号必须在各 phase 埋点、在 audit 收口**:如果 audit 自己重新定义"什么是 unproven / open / malformed",就会和 lint/parser 的判定漂移;正确做法是 audit 纯聚合、判定留在源头。这是"单一真相来源"在观测层的体现。
- **observability ≠ gate**:audit 刻意不影响退出码——库的健康是给人看的趋势,不是阻断提交的硬门;硬门留给 lifecycle/guard 的机械 verdict。混淆两者会让"健康度"变成噪声门禁。

## Refactor Recommendations

- **可增维度**:Phase 5 的 `acknowledged` lint 数、各维度告警分布;Phase 3 的"AI 生成未 affirm 的 Rule"数(provenance 通道已具备数据)。这些是 audit 的自然下一批字段,本期未纳入,记录为后续增强点。
- **`--dry-run` 快照**:复盘模板假设的 `audit --dry-run` 库快照,当前由 `audit --format json` 等价提供;模板措辞可在后续统一。

## Spec 库健康度快照(本 phase 落地后,HEAD 实测)

- `agent-spec guard --spec-dir specs`:**37 / 37 specs passed**。
- 总 Rule 数(按 scope):task 若干 + capability(P3 起可用);具体分布由 `agent-spec audit --spec-dir specs --format json` 实时给出。
- Orphan Examples(无 Rule 归属):由 `ungrouped_scenarios` 实时计。
- Open questions:由 `open_questions` 实时计。
- Malformed rules:由 `malformed_rules` 实时计。
- **本 phase 起,以上不再靠 grep+jq 手工汇总,改由 `audit` 一条命令产出**——这正是复盘模板 §"Spec 库健康度快照"所等的自动化落点。

## 与原合约的偏离

- **维度集合为 v1 七项**,未含 acknowledged / AI-unaffirmed Rule 等——合约范围如此,非偏离;列为后续增强。
- **实现期撞上重复 `collect_spec_files`**:我曾新增一个重复函数,既有版本(返回 `Result`)已存在于 ~line 3006;发现后删除重复、改用 `?` 传播。如实记录这次自纠。
- **未执行对抗 hunt**(范围决策):纯只读聚合、判定全部复用既有源头逻辑,无新攻击面;单测覆盖各计数分支即足。

## 下一步

- 下一个 phase:Phase 9(`discover --from-codebase`:从测试反向生成 spec 骨架)。合约:`specs/task-discover-from-codebase-v1.spec.md`。
- 本复盘影响的 Phase 9 条目:Phase 9 草案 seed 的 `## Questions` 会被 audit 计入 `open_questions`——冷启动 spec 天然带 open question,正是 audit 健康度"已知不完整"的诚实来源。
