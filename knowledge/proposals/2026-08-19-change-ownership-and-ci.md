---
kind: proposal
id: LEP-005
title: "Change ownership and incremental CI"
status: proposed
liveness: n/a
tags: [guard, boundaries, ci, change-scope, ownership]
---

# Change ownership and incremental CI

## Context

`guard` 把**同一个变更集**交给 `--spec-dir` 下的**每一份** spec 做边界检查
（`main.rs cmd_guard`：一次 `resolve_guard_change_paths`，逐 spec
`verify_with_changes`）。任何 PR 都会让与之无关的 spec 报 "not covered by any
allowed boundary"——repo-wide guard 在有多份 task spec 的仓库里根本无法使用。
robrix2 的结论是"stock guard 不能用"（PR #324），自写了 `scripts/spec-guard.sh`：
改动的 spec 走完整 lifecycle（带变更集），其它 spec 只跑测试回归。

同时 `--change-scope` 只有 `none|staged|worktree|jj`，CI 对 PR base 做 diff 得自己
`git diff --name-only base...HEAD` 再逐个 `--change`（A4）；没有"每个 PR 只跑增量、
其余回归"的现成命令（C6）。

robrix2 反馈 A3（P0）、A4（P1）、C6（P1）。二次审查指出：不能简单按 "Allowed
Changes 与变更集有交集" 过滤——完全不相交恰恰可能是严重越界；spec 文件本身通常
不在自己的 Allowed Changes 里；capability / project spec 与 task spec 适用范围不同；
一个变更可能受多份 spec 共同治理。要先定义 ownership，再谈过滤。

## Motivation

- 让 stock `guard` 在多 spec 仓库可用，robrix2 的 4 步脚本能退役。
- 让"这个变更归哪份合约管"成为可计算的事实，并把"没人管的变更"显式暴露出来，
  而不是靠 spec 恰好挡住或恰好没挡住。
- CI 一条命令：base ref 的 diff → 改动的 spec 全量 → 其它 spec 回归。

## Goals

- 定义 **engaged**（被变更集触及）的机械规则。
- 边界层只对 engaged 的 task spec 生效；未 engaged 的 task spec 只跑绑定测试。
- 产出 **unowned changes** 报告：变更集中不属于任何 engaged spec 的 Allowed Changes
  的文件——先可见，不失败；由 project spec 决定是否 opt-in 成失败。
- `--change-scope base:<ref>`。
- `agent-spec ci [--base <ref>]` 把整条流水线固化。

## Non-Goals

- 不改变单个 spec 的边界语义（allow / forbid 的匹配规则见 REQ-BOUNDARY-PATH-EXPRESSIONS）。
- 不改变 skip≠pass；回归模式下的 skip 仍阻断（manual 场景见 LEP-004）。
- 不做 spec 之间的所有权冲突仲裁（多份 spec 同时 engaged 时全部检查）。

## Decision

1. **engaged 规则**：spec S 被变更集 C **engaged** 当且仅当以下任一成立：
   (a) C 包含 S 的 spec 文件本身；(b) C 中某文件命中 S 的任一 Allowed Changes 模式；
   (c) C 中某文件命中 S 的任一 `### Symbols` 所在文件（需 atlas 新鲜图；图不可用
   时忽略 (c)）。
2. **按层级区分**：
   - engaged 的 **task** spec：完整变更集做边界检查（现行语义）+ 全部场景。
   - 未 engaged 的 task spec：只跑绑定测试（回归），不做边界层。
   - **capability** spec：不做边界（长寿命，不"拥有"变更），始终全量场景（回归）。
   - **project** spec：Boundaries 是约束不是路径，不做变更集边界；lint 按 level。
3. **unowned changes**：C 中不被任何 engaged task spec 的 Allowed Changes 覆盖、也
   不匹配 project spec 声明的 `### Unowned Allow`（新可选子节，例如 `docs/**`、
   `Cargo.lock`）的文件，列入报告 `unowned_changes: [...]`。默认 Warning；project spec
   frontmatter `unowned_changes: fail` 时升为失败。
4. **CLI**：
   - `--change-scope base:<ref>`：`git diff --name-only <ref>...HEAD --diff-filter=ACMR`
     （jj 仓库：`jj diff --from <ref>`）。
   - `guard` 新增 `--boundary-scope engaged|all`，默认 `engaged`（旧行为 = `all`）。
   - `agent-spec ci [--base <ref>] [--spec-dir specs] [--code .]`：
     ① lint 改动的 spec（`--min-score` 可配）→ ② `check-structure`（若 project spec
     声明结构守卫）→ ③ capability spec 全量 + 其 `satisfies` 的 ADR `trace --gate`
     → ④ engaged task spec 全量（带边界）/ 其它 task spec 回归 → ⑤ unowned changes
     报告。JSON 与 text 两种输出；exit 2 表示门禁失败。
5. **不变量**：`--boundary-scope all` 保留旧行为；`lifecycle` 单 spec 语义不变。

## Compatibility

- CLI and public API: `guard` 默认行为改变（engaged）——是 breaking，需 minor 版本
  与 CHANGELOG 明示，并提供 `--boundary-scope all` 回退。
- File formats: guard / ci JSON 新增 `engaged`、`unowned_changes` 字段。
- Existing specs and KLL artifacts: 无需修改；`### Unowned Allow` 为可选。

## Migration Plan

- 发布后 robrix2 用 `agent-spec ci --base origin/main` 替换 `spec-guard.sh` 第 1、3、4
  步；第 2 步（结构守卫）待 C3 多规则支持后并入。
- 期望迁移期内先跑一次 `--boundary-scope all` 与 `engaged` 对比，确认没有 spec 依赖
  "全集边界"来挡住越界（若有，那份 spec 应把该路径写进 Forbidden）。

## Security Considerations

放宽默认边界范围会减少"误挡"，也可能减少"正挡"。缓解就是 unowned changes 报告：
所有不属于任何 engaged spec 的文件都被点名，project spec 可把它升级为失败。

## Privacy Considerations

无新数据采集；`base:<ref>` 只调用本地 VCS。

## Risks and Assumptions

### Assumptions

- Allowed Changes 足以表达"归属"。Invalidated if: 大量 spec 用 Symbols 而非路径
  表达范围——那时 (c) 必须依赖 atlas 图，图不新鲜会削弱 engaged 判定。
- 仓库有且只有一份 project spec 承担 unowned 策略。Invalidated if: 多 project spec。

### Risks

- engaged 判定漏掉本应检查的 spec。Mitigation：unowned 报告 + `--boundary-scope all` 可选。
- `ci` 子命令与各仓库自定义流程冲突。Mitigation：每一步都可单独跑；`ci` 只是固化顺序。

## Consequences

Good, because stock guard 在多 spec 仓库可用；"没人管的变更"第一次可见。
Good, because CI 一条命令，base ref 原生支持。
Bad, because guard 默认行为改变；文档、模板、CI workflow 都要更新。
Bad, because unowned 报告初期会很吵（docs、锁文件），需要 project spec 立即声明 `### Unowned Allow`。

## Alternatives Considered

- 只按 "spec 文件被改" 当 engaged（robrix2 脚本现状）——拒绝：改代码不改 spec 的
  PR 完全逃过边界层。
- 按 Allowed Changes 交集过滤但没有 unowned 报告——拒绝：不相交≠无关，越界会静默。
- 只加 `--change-scope base:<ref>`，不动 guard 语义——拒绝：A3 才是 P0。

## Prior Art

- robrix2 `scripts/spec-guard.sh`（流程参考，非语义参考）。
- CODEOWNERS：按路径声明归属并要求 owner 审批——engaged 规则是它的合约版。
- Bazel / Nx affected-graph：按变更集计算受影响目标。

## Unresolved Questions

- (c) Symbols 触及是否纳入 v1，还是先只用路径？
- unowned 默认 Warning 还是 Fail？robrix2 倾向"先可见"。
- `ci` 是否应包含 `check-structure`（依赖 C3 多规则），还是留给脚本？
- 多份 project spec 或 workspace 级 config（C5）时 unowned 策略从哪读？
- `--boundary-scope engaged` 作为默认是否太激进；是否先默认 `all` 一版再切换？
