# Phase 1 Retrospective: BDD Semantics v1

> Filled after implementation; `lifecycle` + `guard` green. Template: `docs/phase-retrospective-template.md`.
> Roadmap: `docs/comparison-openspec-speckit.md` §13.

## 元数据

- **Phase 编号**: 1 (Formulation — Rule → Example)
- **合约**: `specs/task-bdd-semantics-v1.spec.md`
- **完成日期**: 2026-05-28
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8 (1M context)
- **最终 lifecycle verdict**: PASS — `agent-spec lifecycle specs/task-bdd-semantics-v1.spec.md --code .` → 19 passed / 0 failed / 0 skipped / 0 uncertain / 0 pending_review (19/19)
- **guard**: `agent-spec guard --spec-dir specs --code .` → 28 specs passed

## Before / After 关键指标

| 指标 | Before | After | Δ |
|------|-------:|------:|---|
| Rust 测试标记 (`#[test]`/`#[tokio::test]`) | 239 | 261 | +22 |
| `cargo test` 通过 | 241 | 253 | +12* |
| v1 合约 scenarios 自验证 | n/a | 19/19 pass | — |
| `guard --spec-dir specs` | 28 pass | 28 pass | 0 (无回归) |
| 源码改动 | — | 11 files, +1252 / -51 | — |
| clippy 警告 | 1 (pre-existing) | 1 (same) | 0 |

\* 253 计 `cargo test` 结果数;261 计源码内 `#[test]` 标记(含 helper 模块差异)。

## Observations

- v1 在现有 `specs/` + `examples/`(共 ~25 份 spec)上跑出 **94 条 `bdd-rule-grouping`** 与 **26 条 `bdd-implementation-detail-step`** 诊断,全部 Info/Warning,**0 条 Error**——没有任何现有 spec 被新 lint 卡住。
- 94 条 rule-grouping 几乎全是 Info(3+ 无 Rule 场景),符合预期:现有 spec 早于 Rule 基元,默认扁平。
- 26 条 impl-detail 中,中文 UI 动词(点击/输入/访问…)与英文(click/type/visit…)都命中——双语关键词列表确实需要,英文列表直接翻译不够。
- 自我纠正指南的四要素强制(`test_new_bdd_lints_emit_self_correction_guidance`)在实现时**抓到一个真实缺口**:`bdd-scenario-shape` 的诊断最初缺少 lint-ack forward-reference,被测试挡下后补齐。说明"四要素"作为机械断言有效,不是装饰。
- 新增 AST 字段(`Scenario.rule` / `AcceptanceCriteria.rules` / `.malformed_rules`)additive + serde-default,旧 JSON 消费方零感知:`test_json_output_additive_only` 验证旧 spec 不产出 `rules`/`rule` 键。

## Main Takeaways

- **自我纠正指南是 lint 价值的核心,不是规则本身。** 四要素机械断言值得在 Phase 5 完整 lint-ack 落地时保留。
- **双语关键词必须独立维护。** `bdd-implementation-detail-step` 的中英文列表不能互译,Phase 4/5 扩展时分别维护。
- **additive AST 字段会强制触碰所有构造点。** 这是下条偏离的根因;Phase 2 加 `evidence_provenance` 时会再次遇到——提前规划测试夹具改动。
- **scenario 的 rule 必须在场景开始时捕获,不能在 flush 时。** 否则跨 Rule 边界会把上一条 Rule 的尾场景错误归入下一条 Rule。这是实现中最易错的一点,已由 `parse_scenarios` 的 `current_scenario_rule` 处理 + 测试覆盖。

## Refactor Recommendations

- **Phase 2**:覆盖矩阵的 verdict 列应区分 computational / inferential(`ScenarioResult.evidence_provenance` 或 `verification_source`,字段名见用户反馈,采多值证据而非单 kind)。它会再次触碰 `ScenarioResult` 全部构造点——按本期经验,先列清单。
- **Phase 5**:完整 lint-ack 机制(`<!-- lint-ack: <code> — <reason> -->` 的诊断身份 / 注释保留 / 匹配 / 过期策略 / `acknowledged_warnings` 字段 / explain 展示)。v1 已在所有 bdd-* 诊断文案预留该语法引用。
- **未来**:`bdd-rule-grouping` 当前对 markdown `### 普通标题` 下的场景也算"未分组"(因为它们不是 `Rule:` 行)。v1 合约自身就触发了这一点(它用 `### Rule 与 Parser 行为` 这种普通小标题分节)。若噪声偏高,Phase 5 可考虑区分"装饰性小标题"与"真未分组"。

## Spec 库健康度快照

- 总 Rule 数(按 scope):task = v1 合约内若干(其余现有 spec 为 0,均未用 Rule);capability = 0(Phase 3);project = 0(Phase 3)。
- Dead Rules:0(promote 未上线)。
- Orphan Examples(无 Rule 归属):现有 spec 绝大多数场景(预期——Rule 是新基元,旧 spec 未迁移)。
- AI-generated 未 affirm 的 Rules:n/a(provenance 是 Phase 3)。

## 与原合约的偏离

- **`spec_verify` 测试夹具的机械字段补全。** 合约原 `禁止做` 完全禁止改 `src/spec_verify/*.rs`。但新增 additive 字段 `Scenario.rule` / `AcceptanceCriteria.rules` / `.malformed_rules` 后,Rust 要求所有 struct 字面量补全字段,包括这些文件 `#[cfg(test)]` 夹具内的 `Scenario {...}` / `Section::AcceptanceCriteria {...}`。实施中据此把合约边界修订为精确 carve-out:**仅允许在这些文件已有的 `#[cfg(test)]` 夹具内机械补 `rule: None` / `rules: vec![]` / `malformed_rules: vec![]`,不改任何验证逻辑或非测试代码**。验证通过语义未变(`test_existing_specs_pass_lifecycle_after_v1_changes` + 全量 `cargo test` 佐证)。
- **`lifecycle.rs` / `main.rs` 的测试追加**:合约已预留"仅限已有 `#[cfg(test)] mod tests` 追加测试"的 carve-out;本期在 `lifecycle.rs` 追加了 2 个回归测试,未触碰 `is_passing` 函数体或主路径。
- **`bdd-implementation-detail-step` 关键词扩充**:合约示例只列了英文关键词;实现按合约决策的"中英文双语"要求补了中文 UI 动词(点击/输入/访问/填写/选择/打开页面/查看页面/拖拽)。属合约意图内,非偏离,记此备查。

## 下一步

- 下一个 phase:Phase 2(机械覆盖矩阵),合约草案待写 `specs/task-coverage-matrix-v1.spec.md`。
- 本复盘影响的 Phase 2 合约条目:覆盖矩阵需带 `evidence_provenance` 列;矩阵数据结构(Rule × Scenario × Test × Verdict)作为 Phase 3 capability / Phase 4 discovery 的共同底座。
