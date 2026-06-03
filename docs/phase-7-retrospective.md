# Phase 7 Retrospective: 机械分层结构检查（dependency-cruiser-lite）

> 填写时机:实施 + `lifecycle` / `guard` 通过之后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 7（`agent-spec check-structure`:在文件 glob 范围内禁止某引用)
- **合约**: `specs/task-structural-check-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)
- **最终 lifecycle verdict**: PASS — 5 passed / 0 failed / 0 skipped / 0 uncertain(5/5)
- **commit**: `8d6a56f`

## Before / After 关键指标

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 5 / 5 PASS |
| commit `+#[test]` | 5 |
| diff | 4 files, 270 insertions |
| 新增 CLI 命令 | `agent-spec check-structure`(`--code` / `--forbid` / `--in`) |
| 新增能力 | `structural_violations(code_paths, forbidden, glob)` + `glob_matches` |
| 对抗 hunt | 未执行(范围决策,见下) |

## Observations

- **StructuralRule = "在某 glob 范围内禁止出现某引用"**:如 `--forbid crate::services --in clients/**`,违反即列出文件并非零退出。这是把架构分层约束变成**机械可判的传感器**,而非 code review 口头规矩。
- **glob 自实现(`glob_matches`)**:`**` 跨目录、`*` 单段,范围限定到文件子树。违规是"匹配 glob 的文件里含禁止子串"。
- **观测性优先、可进 CI**:非零退出码让分层约束像测试一样守门,补上了 agent-spec 此前只有"测试/边界/AI"三类证据、缺"结构约束"一类的空缺。

## Main Takeaways

- **架构不变量必须机械化才能持久**:"clients 层不许直接调 services"这类规矩,只要靠人记就会腐蚀;变成 `check-structure` 一行命令 + CI 守门后,它和单测一样不可绕过。
- **glob 范围是结构检查的精度关键**:禁止项必须带作用域(`--in`),否则"全仓禁止 X"过宽、误伤合法用法;作用域让约束精确到"哪层不许碰什么"。

## Refactor Recommendations

- **Phase 8 audit** 与本 phase 解耦,但二者同属"机械观测性"family;未来可把 structural 违规数纳入库级健康度快照。
- **多规则批量**:v1 一次一条 `--forbid`;后续可从 spec 的 Decisions 段批量提取结构规则一次性检查(本期未做)。

## Spec 库健康度快照

- 不产生新 Rule;不改 Rule scope 分布。
- 新增机械证据类别:**结构/分层约束**(此前缺位)。

## 与原合约的偏离

- **单规则、命令行驱动**:v1 一次校验一条 `--forbid`/`--in`,未做 spec 内声明式多规则——合约范围如此,非偏离;显式记录"声明式批量"为未实现项。
- **未执行对抗 hunt**(范围决策):只读扫描 + 子串/glob 匹配,无写文件;边界(glob `**` vs `*`、子串误匹配)由 scenario 覆盖。注:Phase 2 hunt 曾揭示"宽松子串匹配是 false 信号温床",本 phase 的 `--forbid` 是**显式用户输入的子串**、语义上就是"出现即违规",与 P2 那种"扫 `#[test]` 却误匹配注释"不同类,故未单列 hunt;此差异如实记录备查。

## 下一步

- 下一个 phase:Phase 8(spec 库健康度 audit)。合约:`specs/task-audit-v1.spec.md`。
- 本复盘影响的 Phase 8 条目:audit 是"库级机械观测性"的聚合器,与 check-structure 的"单仓结构观测"互补;二者共同构成"verification 之外的传感器层"。
