# Phase 5 Retrospective: lint-ack 机制 + 五维分类

> 填写时机:实施 + `lifecycle` / `guard` 通过之后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 5(lint acknowledgement + lint 维度分类)
- **合约**: `specs/task-lint-ack-dimensions-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)
- **最终 lifecycle verdict**: PASS — 8 passed / 0 failed / 0 skipped / 0 uncertain(8/8)
- **commit**: `4754d07`

## Before / After 关键指标

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 8 / 8 PASS |
| commit `+#[test]` | 8 |
| diff | 10 files, 397 insertions |
| 新增结构 | `LintAck { code, reason }`、`SpecDocument.lint_acks`、`Dimension` enum |
| 新增 lint 能力 | `dimension_of(rule_code)`、`LintReport::dimension_counts()` |
| 对抗 hunt | 未执行(范围决策,见下) |

## Observations

- **lint-ack 是"传感器可被知情豁免"的机制**:`<!-- lint-ack: CODE 理由 -->` 让 spec 作者显式 acknowledge 某条 Warning/Info,带**强制理由**。被 ack 的 lint 从报告中过滤,但**计入 `acknowledged` 计数**——豁免可见、可审计,不是静默吞掉。
- **关键安全边界:lint-ack 只能过滤 Warning/Info,永不过滤 Error**(`pipeline.rs::run()` 强制)。Error 是"机械可判的硬失败",不容 acknowledge。
- **五维分类**(`Dimension`)把零散 lint code 归类为可聚合的维度,让"这个 spec 在哪一类质量上弱"成为可统计量,而非逐条 code。
- **理由是 ack 的一等公民**:无理由的 ack 不成立——这把"豁免"从"关掉告警"提升为"留下决策记录"。

## Main Takeaways

- **可豁免性是 lint 长期可用的前提**:没有 ack,作者面对误报只能改 spec 迁就 linter 或整体关 lint;有了带理由的 ack,传感器既保持开启又不绑架作者。这是 Fowler "sensors" 模型落到 spec 工具的关键一环。
- **豁免必须留痕**:`acknowledged` 计数让 audit 能回答"这个库里有多少被知情豁免的告警",豁免本身成为健康度信号而非盲区。
- **Error 不可 ack 是不可动摇的红线**:一旦 Error 可豁免,机械护城河就被作者侧绕过,违背 BDD-spine 核心命题。

## Refactor Recommendations

- **Phase 8 audit** 可增加"acknowledged lint 数 / 各维度告警分布"作为库级健康度维度(本 phase 的 `dimension_counts` + `acknowledged` 已就位)。
- **Phase 4 `open-question`** 应纳入维度分类并可被 ack(已在本 phase 的 `dimension_of` 覆盖范围内对齐)。

## Spec 库健康度快照

- 新增可统计维度:**acknowledged lint 数**、**五维告警分布**。
- 不产生新 Rule;不改 Rule scope 分布。

## 与原合约的偏离

- **维度集合按既有 lint code 实际归类**,未预先固化为对外稳定枚举的全集——`Dimension` 随 lint 增长可扩展,合约未承诺冻结维度全集,非偏离。
- **未执行对抗 hunt**(范围决策):本 phase 是 lint 过滤逻辑 + 分类映射,核心安全边界(Error 不可 ack)由单测直接覆盖,无外部攻击面;hunt 预算未投入。如实记录。

## 下一步

- 下一个 phase:Phase 6(单源多工具 integration 生成)。合约:`specs/task-gen-integrations-v1.spec.md`。
- 本复盘影响的 Phase 6 条目:无直接耦合;Phase 6 起转入"对外产物生成"类 phase,lint-ack 的"带理由豁免"模式可作为后续任何"生成物 drift 容忍"设计的参照。
