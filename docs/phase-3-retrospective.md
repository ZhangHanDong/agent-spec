# Phase 3 Retrospective: capability 层 + promote（Rule keystone）

> 填写时机:实施 + 对抗 hunt + 修复完成后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 3（`SpecLevel::Capability` + `agent-spec promote` + Rule keystone）
- **合约**: `specs/task-capability-promote-v1.spec.md`
- **开始日期**: 2026-05-31
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8（实现)+ 对抗 hunt
- **最终 lifecycle verdict**: PASS — `lifecycle specs/task-capability-promote-v1.spec.md --code .` → 12 passed / 0 failed / 0 skipped / 0 uncertain（12/12)
- **实现 commit**: `244f3a0`;**hunt 修复 commit**: `b9dce07`

## Before / After 关键指标

只报实测量。`+#[test]` 为该 commit 新增的测试函数数;`cargo test` 总数只报路线图末端聚合值(未逐 commit 重测,见"与原合约的偏离")。

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 12 / 12 PASS |
| 实现 commit `+#[test]` | 12 |
| hunt 修复 commit `+#[test]` | 5(RED 先行) |
| 核心代码 diff | `main.rs +249`, `parser.rs +52`, `meta.rs +35`, `verify.rs +32`, `ast.rs +27` |
| 新增 CLI 命令 | `agent-spec promote` |
| 对抗 hunt | 9 confirmed → 5 fixed |

> 注:实现 commit `244f3a0` 全量 3506 insertions,其中绝大部分是 `books/harness-spec-ai/`(整本书)与 roadmap 文档,**与 Phase 3 代码无关**;Phase 3 真实代码即上表 src/* + 合约 160 行。如实记录以免后人误读 diff 体量。

## Observations

- **Rule keystone 落地**:`RuleKey = { scope, id }`,`id` 稳定 kebab-case、`name` 可变显示名;`RuleScope::{ Task(stem) | Capability(name) | Project }`。promote 只改 scope、**绝不改 id**——这是"Rule 身份不绑显示名"这一 Phase 1 用户纠偏的兑现点。
- **promote 门禁复用机械证据**:被提升的 Rule 其 Example 必须 found + pass(复用 `is_passing` + 覆盖矩阵),而非凭名义提升。
- **对抗 hunt:9 confirmed → 修 5**(`b9dce07`,5 个 RED 测试先行,+127/-14):
  - **空洞 promote 门禁**:Rule 无任何 Example 也能被提升 → 现要求 ≥1 example。
  - **HTML 注释泄漏进 Rule name**:`Rule:` 行尾 `<!-- -->` 注释被吞进显示名 → own-line 解析 + strip。
  - **`--to` 路径穿越**:capability 名未校验可写到任意路径 → `is_safe_capability_name` 在触碰文件系统前报错。
  - **`rule_id_of` 与解析端分歧**:id 派生算法两处不一致 → 统一为 leftmost separator。
  - **append 时缺 Completion Criteria section**:提升进已存在 capability spec 时未建 section。

## Main Takeaways

- **id 稳定性是 promote 全部价值的支点**:只要 id 在 promote 前后不变,task→capability 的引用链就不断。9 个 bug 里有 2 个(HTML 泄漏、rule_id 分歧)都在腐蚀 id/name 边界,印证这是高价值靶区。
- **"门禁"类逻辑天然是空洞默认的温床**:promote gate 初版"看起来在校验",实际 0 个 example 也放行。门禁必须有正向最小证据要求,不能只挡显式坏值。
- **写文件的命令必须先校验路径再触碰 FS**:`--to` 穿越是 capability 名直接拼路径的产物;凡接受用户名→落盘的命令都要过 `is_safe_*`。

## Refactor Recommendations

- **Phase 8 audit** 应能识别"AI 生成但未被 human affirm 的 Rule"——promote 时的 provenance event 是该识别的数据源(本 phase 已写入 provenance 通道)。
- **promote 的 capability 依赖图**(原 Phase 3 设想的 capability 依赖可视化)未在 v1 落地,移交 `agent-spec graph` 既有 DAG 能力按需扩展。

## Spec 库健康度快照

- 总 Rule 数(按 scope):本 phase 起 capability scope 首次可用;task scope 既有合约内若干。
- Dead Rules:0(promote 刚上线,尚无未引用的已提升 Rule)。
- 新增公开行为:`SpecLevel::Capability`、`RuleScope`/`RuleKey`、`agent-spec promote`(含 `promote_gate_ok` ≥1 example / `is_safe_capability_name` / `merge_ai_decisions`)。

## 与原合约的偏离

- **capability 依赖图未实现**:v1 范围收紧为"promote + id 稳定 + 门禁",依赖图移交既有 `graph` 命令,合约未承诺该项,属范围决策非偏离。
- **cargo test 累计总数未逐 commit 重测**:复盘只报该 commit `+#[test]` 与路线图末端聚合(HEAD:342 pass)。重测需逐 commit checkout + rebuild,成本不对等,故不报伪精确的逐期累计值。

## 对抗 bug 猎杀(post-implementation)

- 9 confirmed,修 5(最高价值的 5 个,`b9dce07`),遵守"无 repro 不修、无 failing test 不修",5 个 RED 测试先行。
- 与 P1/P2 对照:P1 hunt 5 confirmed(双语/Unicode 边界)、P2 hunt 8 confirmed(渲染注入/扫描器/verdict 通道),P3 hunt 9 confirmed(门禁空洞 + id/name 边界 + 路径穿越)。三期 hunt 共 22 个真 bug,全是 TDD 盲区。
- **P3 是 P3–P9 中唯一执行对抗 hunt 的 phase**:promote 涉及写文件 + 身份语义,属高风险,值得 hunt;P4–P9 多为机械只读/纯函数,未执行 hunt(各期"与原合约的偏离"已注明)。

## 下一步

- 下一个 phase:Phase 4(Discovery `## Questions` section + open-question lint)。合约:`specs/task-discovery-questions-v1.spec.md`。
- 本复盘影响的 Phase 4 条目:Discovery 阶段的 open question 是 promote 之前"未澄清意图"的拦截点;audit(Phase 8)将统计 open_questions,Phase 4 的 `question_is_resolved` 判定需与 audit 对齐。
