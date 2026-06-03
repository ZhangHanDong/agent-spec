# Phase 6 Retrospective: 单源多工具 integration 生成

> 填写时机:实施 + `lifecycle` / `guard` 通过之后。模板:`docs/phase-retrospective-template.md`。
> 路线图:`docs/comparison-openspec-speckit.md` §13。

## 元数据

- **Phase 编号**: 6(`agent-spec gen-integrations`:单一来源生成 agents / cursor / claude 集成文件)
- **合约**: `specs/task-gen-integrations-v1.spec.md`
- **完成日期**: 2026-06-01
- **作者 / Reviewer**: AlexZ + Claude Opus 4.8(实现)
- **最终 lifecycle verdict**: PASS — 7 passed / 0 failed / 0 skipped / 0 uncertain(7/7)
- **commit**: `4fcacca`

## Before / After 关键指标

| 指标 | 值 |
|------|----|
| 合约 scenarios 自验证 | 7 / 7 PASS |
| commit `+#[test]` | 7 |
| diff | 4 files, 303 insertions |
| 新增 CLI 命令 | `agent-spec gen-integrations`(`--target` / `--out` / `--check`) |
| 新增能力 | `integration_body()`、`render_target`/`render_named`、`has_drifted` |
| 对抗 hunt | 未执行(范围决策,见下) |

## Observations

- **单一来源(`integration_body()`)→ 多目标渲染**:agents / cursor / claude 三种工具的集成文件由同一 body 渲染,避免三份各自漂移。这是吸收 Spec Kit "scaffolding/governance" 能力的 BDD-spine 版本——但**不是脚手架一堆模板,而是单源 + drift 检测**。
- **`--check` 模式 = drift 守门**:`has_drifted` 比对磁盘现状与应生成内容,漂移时非零退出。这让"生成物是否被手改偏离"可进 CI,而非靠人肉巡检。
- **生成 vs 检查同源**:写入路径与检查路径共用 `render_*`,保证"check 通过"严格等价于"write 不会改动",杜绝两条路径语义分叉(这正是 Phase 2 hunt C8 那类"两条路径语义对不上"的预防性设计)。

## Main Takeaways

- **单源 + drift 检测 > 多模板脚手架**:Spec Kit 式脚手架的长期成本是 N 份模板各自漂移;把它压成"一个 body + N 个渲染器 + 一个 `--check`",治理成本从"维护 N 份"降到"维护 1 份 + 自动验漂移"。
- **凡是"生成"必须配"检查同源"**:write 与 check 共用渲染器,是避免"生成器升级了、检查器没跟上"这类系统性盲区的结构性手段。

## Refactor Recommendations

- **通用**:`--check` 的 drift 退出码模式可推广到任何"agent-spec 产出落盘文件"的命令(如未来的 spec 导出),作为统一的 CI 守门约定。
- **无下游 phase 直接依赖**:Phase 6.5(Probe)与本 phase 解耦,各自独立。

## Spec 库健康度快照

- 不产生新 Rule;不改 Rule scope 分布。
- 新增对外产物:三类工具集成文件的单源生成 + drift 检测能力。

## 与原合约的偏离

- **目标集合固定为 agents / cursor / claude 三者**(`--target all` 展开为此三者);未做插件式任意目标注册——v1 范围内,合约未承诺可扩展目标注册,非偏离。
- **未执行对抗 hunt**(范围决策):生成与检查同源、由单测覆盖等价性,攻击面集中在路径/转义,已由 scenario 覆盖;未单独投入 hunt。如实记录。

## 下一步

- 下一个 phase:Phase 6.5(Probe 抽象:统一 Test / Static / Benchmark / External / Inferential 证据来源)。合约:`specs/task-probe-abstraction-v1.spec.md`。
- 本复盘影响的 Phase 6.5 条目:无直接耦合;两者均为"扩展证据/产物来源"的正交能力。
