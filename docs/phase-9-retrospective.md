# Phase 9 Retrospective: discover --from-codebase（冷启动反向生成 spec 骨架）

> 填写时机:实施 + `lifecycle` / `guard` 通过之后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 9（`agent-spec discover --from-codebase`:从既有测试机械反推 spec 草案)
- **合约**: `specs/task-discover-from-codebase-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)
- **最终 lifecycle verdict**: PASS — 5 passed / 0 failed / 0 skipped / 0 uncertain(5/5)
- **commit**: `f3a8da6`

## Before / After 关键指标

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 5 / 5 PASS |
| commit `+#[test]` | 5 |
| diff | 5 files, 230 insertions |
| 新增 CLI 命令 | `agent-spec discover --from-codebase`(`--code` / `--name` / `--out`) |
| 新增能力 | `draft_spec_from_tests(test_names, spec_name) -> String` |
| E2E 实测 | 对 `src/spec_report` 扫出 47 测试 → 47 scenario,草案回解析成功 |
| 对抗 hunt | 未执行(范围决策,见下) |

## Observations

- **缓解 §9.4 冷启动代价**:项目已有测试但没有 spec 时,把每个测试函数机械反推为一个绑定该测试(`测试: <fn>`)的 scenario 草案,产出一份**可被 agent-spec 验证**的 task spec 骨架。
- **草案必带 `## Questions` 种子**:标注"这些 scenario 由 discover 自动草拟、需人工细化意图与 Given/When/Then"。这是诚实纪律——**不假装草案是完整契约**,而是消费 Phase 4 的 Discovery 结构标记"已知不完整"。
- **纯机械、无 AI**:占位 `当/那么` 步骤,语义充实留待人工/后续。复用 Phase 2 的 `collect_test_function_names`(排序后输出,稳定可复现)。
- **空测试集仍产可解析占位草案**:`test_draft_empty_tests_is_parseable` 锁定这一边界,避免空输入产出不可解析的残骸。

## Main Takeaways

- **冷启动工具的价值在"可验证的起点",不在"完整的终点"**:discover 产的不是成品契约,而是一个能立刻进 lint/verify 流水线的骨架 + 一份明确的待办(Questions)。把"反向生成"定位成 Discovery 的入口而非 Automation 的输出,是它不越界、不制造虚假完整感的关键。
- **生成物必须自我验证**:草案"能被 parser 解析"是硬约束(`test_draft_is_parseable`)——生成一个 agent-spec 自己都解析不了的 spec 是自相矛盾。递归适用:agent-spec 的产物也要过 agent-spec 的关。

## Refactor Recommendations

- **后续**:按代码结构推断 Rule 分组 / capability、用 AI 充实 Given/When/Then——均在本期排除范围,是 discover 的自然演进方向。
- **Probe 泛化**:当前只产 `Test` Probe 草案;接入 Phase 6.5 的 Probe 抽象后,可对 benchmark/external 测试产对应 Probe 草案。

## Spec 库健康度快照

- 不产生新 Rule;不改 Rule scope 分布。
- discover 草案 seed 的 `## Questions` 会被 Phase 8 audit 计入 `open_questions`——冷启动 spec 天然"已知不完整",这是健康度的诚实来源而非缺陷。
- HEAD 实测:`cargo test` 342 passed;`guard` 37/37;clippy 1(`ReviewMode` derivable_impls,历史遗留)。

## 与原合约的偏离

- **未走严格 RED-first TDD**:本 phase 的 `discover.rs` 实现 + 5 个测试在一次 Write 中写成,首跑即绿,**未先观测到 RED**。测试是真实解析 `draft_spec_from_tests` 的输出(非 mock),可信度高,但按 TDD 纪律这不是合格的"先红后绿"循环——如实记录,不粉饰为干净 TDD。这是 P3–P9 中唯一一次未先看红的 phase。
- **`name` 未转义**:`name: "{spec_name}"` 若名字含 `"` 会破坏 frontmatter。本期范围是"草案工具、name 由用户控制、仅解析",未做转义;记录为已知小限制,非合约要求项。
- **未执行对抗 hunt**(范围决策):纯字符串构造函数 + 复用既有扫描器,无写入逻辑分支(仅 `--out` 落盘),边界由 5 个 scenario 覆盖;未投入 hunt。

## 下一步

- **本 phase 是 BDD-spine 路线图 P1–P9 的最后一环**。无下一 phase。
- 路线图收口状态:P1–P9 全部实现、自托管合约通过、已 commit;分支 `bdd-spine-direction` 未推送,等用户决定 merge/push。
- 后续演进候选(均非本路线图承诺):discover 的 AI 语义充实、Probe runner 接入(criterion/k6/跨语言)、audit 维度扩展(acknowledged / AI-unaffirmed Rule)、声明式批量 structural 规则。
