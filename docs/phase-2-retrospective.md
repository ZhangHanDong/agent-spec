# Phase 2 Retrospective: 机械覆盖矩阵

> 填写时机:实施 + 对抗 hunt + 修复完成后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 2(机械覆盖矩阵 Rule × Scenario × Test × Verdict × Provenance)
- **合约**: `specs/task-coverage-matrix-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)+ Codex(合约 code review)+ 对抗 hunt(22 agents)
- **最终 lifecycle verdict**: PASS — `lifecycle specs/task-coverage-matrix-v1.spec.md --code .` → 15 passed / 0 failed / 0 skipped / 0 uncertain(15/15)
- **guard**: 29 specs passed

## Before / After 关键指标

| 指标 | Phase 1 末 | Phase 2 实现后 | hunt 修复后 | Δ |
|------|-----------:|--------------:|-----------:|---|
| `cargo test` 通过 | 253 | 274 | **280** | +27 |
| 合约 scenarios 自验证 | — | 15/15 | 15/15 | — |
| guard | 28 | 29 | 29 | +1(本合约提升进 specs/) |
| clippy 警告 | 1(pre-existing) | 1 | 1 | 0 |
| 新增 CLI 命令 | — | `agent-spec matrix` | — | +1 |

## Observations

- 合约阶段(实施前)Codex code review 抓到 **4 个语义口子**(test_found 用 cargo 子串语义不稳、boundary_relevant 无定义、provenance 漏 caller-mode、CLI 运行语义未锁),全部在 commit 前修订入合约。**合约层的对抗审查与代码层一样有效**——4 个口子都是"合约看起来对、但和现有代码语义对不上"的盲区。
- 实施分 3 组 TDD(provenance 基础 / 矩阵核心 / 渲染+CLI),各组先 RED 后 GREEN,无回退。
- `provenance` additive 字段再次触发"所有 `ScenarioResult` 字面量必须补字段"的税——这次是 **5 个 verifier 生产字面量 + 10+ 测试夹具**;sed/perl 批量补字段时误伤了 `spec_report` 的 `CostEntry`(也有 `duration_ms`),靠 `git checkout` 回退 + lookahead 正则修正。**教训:批量字段插入要按结构而非按字段名。**
- 对抗 hunt(22 agents,17 候选,~7.5 min,~76 万 tokens):**8 confirmed / 0 uncertain / 9 refuted**。0 误报,验证器用 negative control(改一个字符看行为是否变)隔离因果。
- 8 个确认 bug 全部集中在"看起来对"的边界:
  - **渲染注入**(1):markdown/text 单元格未转义 `|` → 行被撑破(scenario 名、selector 都中招)。
  - **scanner 状态机**(4):`tokio::test` 子串匹配注释/字符串字面量 → false found;块注释里的 `#[test]` → false found;单行 `#[test] fn` → false missing。test-fn 索引是 `test_found` 的根,这些直接腐蚀矩阵核心价值。
  - **verdict 通道完整性**(2):`merge_ai_decisions` 覆盖了机械 pass/fail(不只 Skip)→ caller AI 能推翻机械证据(违反"机械是护城河");boundary synthetic scenario 不进矩阵 → 边界 FAIL 静默丢失(违反合约自己的 §note)。

## Main Takeaways

- **C7(AI 覆盖机械 verdict)是最重要的一个**:它直接违背 BDD-spine 的核心命题"机械执行是护城河、LLM 判断永不默认 pass"。代码注释本就写着"replace Skip verdicts",实现却没守住——**注释承诺与代码行为脱节**正是对抗审查的高价值靶区。
- **scanner 这类"宽松字符串匹配"是 false 信号的温床**:`contains("tokio::test")` 看似无害,实际匹配注释/字符串。机械索引必须用结构化判断(`#[` 前缀 + attr 名),不能用子串。
- **合约自己的 §note 也要被验证**:C8 是实现没兑现合约里"boundary 作为 row 出现"的承诺。合约写了不等于实现做了——这正是 agent-spec 存在的理由,递归适用于它自己。
- **对抗审查是 TDD 的正交补充**(Phase 1 已得此结论,Phase 2 再次印证):TDD 证明"我想到的对",hunt 找"我没想到的输入"。两期共 13 个真 bug 全是 TDD 盲区。

## Refactor Recommendations

- **Phase 5**:加 `bdd-duplicate-scenario-name` lint —— C6(重名 scenario 共用首个 result 的 verdict)是 run_verification/test-binding/checkpoint 全系统对"scenario 名唯一"的假设,应在 lint 层显式拦截,而不是在矩阵层粉饰。
- **Phase 6.5(Probe)**:`test_found` 的精确函数名扫描器将被 Probe 抽象取代/扩展(Test/Static/Benchmark/External)。届时 scanner 的语言耦合(只扫 Rust `fn`)需要泛化。
- **通用**:additive 字段的构造点批量补全应有一个工具化做法(按 AST/结构匹配,而非 `duration_ms:` 行正则),避免每期重复误伤。

## Spec 库健康度快照

- 总 Rule 数(按 scope):task = 两份 v1 合约内若干;capability = 0(Phase 3);project = 0。
- Dead Rules:0(promote 未上线)。
- 新增公开 API:`spec_report::coverage`(`CoverageMatrix` / `build_coverage_matrix` / `collect_test_function_names`);`spec_core::EvidenceProvenance` + `ScenarioResult.provenance`。
- 已知限制(记录在案):跨 crate package 限定的精确测试定位未做(v1 按函数名);`test_found` 用名存在性而非 cargo runner 语义;重名 scenario 未拦截(待 Phase 5 lint)。

## 与原合约的偏离

- **provenance additive 字段触碰 5 个 verifier 生产字面量**:合约 carve-out 已预先允许(仅补 `provenance: None`,不改 verdict 逻辑)。实际还顺带需要在 `lifecycle.rs` / `main.rs` 的 `#[cfg(test)]` 夹具补字段(沿用 Phase 1 carve-out)。
- **C8 修复扩展了 build_coverage_matrix 行为**:除按 `all_scenarios` 出行外,新增"追加无匹配 scenario 的 report 结果(orphan rows)"。这是兑现合约 §note 的必要行为,不是偏离——合约本就要求 boundary 作为 row 出现。
- **C6 明确不修(范围决策)**:重名 scenario 是全系统假设,Phase 2 不在矩阵层处理,移交 Phase 5 lint。

## 对抗 bug 猎杀(post-implementation)

- 规模:22 agents,17 候选,**8 confirmed / 0 uncertain / 9 refuted**,~7.5 min,~76 万 subagent tokens。
- 5 lens:render-injection / test-fn-scanner / matrix-assembly / provenance-resolve / cli-semantics。
- 0 误报(验证器实证 + negative control)。
- 修复 6(C1–C5,C7,C8),遵守"无 repro 不修、无 failing test 不修",6 个 RED 测试先行;延后 1(C6)。
- 与 Phase 1 对照:Phase 1 hunt 5 confirmed(全在双语/Unicode 边界);Phase 2 hunt 8 confirmed(渲染注入 + 扫描器宽松匹配 + verdict 通道完整性)。两期 hunt 共 13 个真 bug,均为 TDD 未覆盖的边界,印证对抗审查作为收尾流程的价值。

## 下一步

- 下一个 phase:Phase 3(capability 层 + promote + Rule provenance event log + capability 依赖图)。合约草案待写 `specs/task-capability-promote-v1.spec.md`。
- 本复盘影响的 Phase 3 条目:覆盖矩阵的 orphan-row 机制 + provenance 通道是 capability 层"系统当前行为真相 × 验证证据"的展示底座;promote 的前置门禁应复用 `is_passing` + 覆盖矩阵(确保被提升的 Rule 的 Example 都 found+pass)。
