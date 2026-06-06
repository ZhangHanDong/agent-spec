# Phase 6.5 Retrospective: Probe 抽象（证据来源统一）

> 填写时机:实施 + `lifecycle` / `guard` 通过之后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 6.5(`Probe` 抽象:把 scenario 的"如何取证"统一为一个枚举)
- **合约**: `specs/task-probe-abstraction-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)
- **最终 lifecycle verdict**: PASS — 6 passed / 0 failed / 0 skipped / 0 uncertain(6/6)
- **commit**: `f955a17`

## Before / After 关键指标

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 6 / 6 PASS |
| commit `+#[test]` | 6 |
| diff | 3 files, 231 insertions |
| 新增抽象 | `Probe::{ Test, Static, Benchmark, External, Inferential }` + `from_scenario` + `kind_label` |
| 对抗 hunt | 未执行(范围决策,见下) |

## Observations

- **Probe 把"证据来源"从"只有 cargo test"泛化为五类**:`Test(TestSelector)` / `Static(String)` / `Benchmark{runner,filter,threshold}` / `External{runner,args}` / `Inferential`。这回应了 Phase 2 复盘的 Refactor 建议——`test_found` 的语言耦合(只扫 Rust `fn`)需要被一个不绑死 cargo 的抽象取代。
- **`Probe::from_scenario(&Scenario)` 是分类入口**:从 scenario 的 selector/标记推导其证据类型,`kind_label()` 给出稳定标签供矩阵/报告展示。
- **本 phase 是脚手架(scaffolding)而非全实现**:Benchmark/External 的实际 runner 执行留待后续,v1 只落地**抽象 + 分类 + 标签**,让覆盖矩阵与报告能区分证据类型。`Inferential` 对应 AI 证据,与 Phase 2 的 `EvidenceProvenance::Inferential` 同义对齐。

## Main Takeaways

- **先立抽象、再接 runner,是控制风险的正确顺序**:Probe 把"证据有哪些种类"这件事先固化成类型,后续每接一种 runner(criterion / k6 / 跨语言)都是往已知枚举里填实现,而不是每次改动矩阵核心。
- **`Inferential` 必须与 provenance 通道一致**:AI 证据在 Probe 层叫 `Inferential`、在 verify 层叫 `EvidenceProvenance::Inferential`,两处命名同义是"机械 vs 推断"二分在全系统保持一致的前提。

## Refactor Recommendations

- **后续(非本路线图)**:接入真实 Benchmark runner(criterion)与 External runner(跨语言测试)时,`Probe::Benchmark.threshold` 的判定语义需补 scenario,且不得让 benchmark 的"未达阈值"默认 pass(沿用"LLM/外部证据永不默认 pass")。
- **Phase 9 discover** 当前只产 `Test` Probe 的草案;未来可按代码结构推断 `Benchmark`/`External` Probe(本期未做,Phase 9 已在排除范围注明)。

## Spec 库健康度快照

- 不产生新 Rule;不改 Rule scope 分布。
- 新增可区分维度:scenario 的**证据类型**(五类 Probe),为覆盖矩阵的 provenance 列提供更细的来源标签。

## 与原合约的偏离

- **仅脚手架,Benchmark/External 不执行**:v1 明确只做抽象 + 分类,runner 执行留后续——合约范围如此,非偏离,但在此显式记录"Probe 已立、runner 未接"的真实状态,避免误读为已支持跑 benchmark。
- **未执行对抗 hunt**(范围决策):纯类型 + 分类函数,无 I/O、无写文件,单测覆盖分类分支即足;未投入 hunt。

## 下一步

- 下一个 phase:Phase 7(机械分层结构检查 `check-structure`)。合约:`specs/task-structural-check-v1.spec.md`。
- 本复盘影响的 Phase 7 条目:无直接耦合;Probe 与 structural check 均为"扩展机械证据种类"的正交能力,Phase 7 是其中"静态结构约束"一类的具体落地。
