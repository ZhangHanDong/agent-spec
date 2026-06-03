# Phase 4 Retrospective: Discovery Questions（`## Questions` + open-question lint）

> 填写时机:实施 + `lifecycle` / `guard` 通过之后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 4（Discovery 阶段:`## Questions` section + `OpenQuestionLinter`)
- **合约**: `specs/task-discovery-questions-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)
- **最终 lifecycle verdict**: PASS — 7 passed / 0 failed / 0 skipped / 0 uncertain(7/7)
- **commit**: `23a5355`

## Before / After 关键指标

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 7 / 7 PASS |
| commit `+#[test]` | 7 |
| diff | 8 files, 306 insertions |
| 新增 section 类型 | `Section::Questions { items, span }` |
| 新增 lint | `open-question`(`OpenQuestionLinter`) |
| 对抗 hunt | 未执行(范围决策,见下) |

## Observations

- **`## Questions / 问题 / 待澄清` 三种 header 均解析**为 `Section::Questions`,item 为 bullet 列表。这是 BDD-spine "Discovery → Formulation → Automation" 三段里 **Discovery 的唯一结构化落点**。
- **open question 不阻断验证**:`OpenQuestionLinter` 只发 Warning/Info,不进 Error 通道,不影响 `is_passing`——遵守路线图"lint 是传感器、不是门禁,verification 语义不变"。
- **resolved 判定**与 Phase 8 audit 的 `question_is_resolved` 同源:`[x]` / `[X]` / `[已解决]` / `RESOLVED` / `已解决` 视为已决。
- **Discovery 是 Phase 9 reverse-engineer 的下游接口**:Phase 9 生成的草案 seed 一个 `## Questions` 标注"自动草拟需人工细化",正是消费本 phase 的结构。

## Main Takeaways

- **Discovery 必须是结构化 section 而非散文**:把"未澄清"做成可被 lint/audit 统计的 `Section::Questions`,才能让 open question 数量进入 Phase 8 健康度快照,否则它只是注释。
- **lint 的"非阻断"边界要在 section 引入时就守住**:open question 是"提醒补全",不是"判失败";一旦它能让 verdict 变 fail,就破坏了 Discovery 的探索属性。

## Refactor Recommendations

- **Phase 8 audit** 的 `open_questions` 计数直接复用本 phase 的 `Section::Questions` + resolved 判定(已对齐,无需改动)。
- **Phase 9 discover** 的草案应 seed `## Questions`(已落地),作为冷启动 spec "已知不完整"的诚实标记。

## Spec 库健康度快照

- 新增可统计维度:**open questions**(未解决的 Discovery 问题数),Phase 8 audit 聚合。
- 本 phase 不产生新 Rule;不影响 Rule scope 分布。

## 与原合约的偏离

- **无 AI 充实**:本 phase 只做 `## Questions` 的解析 + lint,不做"用 AI 自动回答 question"——与合约范围一致,非偏离。
- **未执行对抗 hunt**(范围决策):本 phase 是只读解析 + 非阻断 lint,无写文件 / 无身份语义 / 无外部 runner,不属高风险靶面;hunt 预算留给 P3 这类写文件 phase。如实记录该取舍。

## 下一步

- 下一个 phase:Phase 5(lint-ack 机制 + 五维分类)。合约:`specs/task-lint-ack-dimensions-v1.spec.md`。
- 本复盘影响的 Phase 5 条目:`open-question` 应归入 Phase 5 的 lint 维度分类(`dimension_of`),并可被 lint-ack 机制 acknowledge 为"本 spec 暂不澄清"的合理例外。
