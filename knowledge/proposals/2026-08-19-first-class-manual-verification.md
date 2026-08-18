---
kind: proposal
id: LEP-004
title: "First-class manual verification"
status: proposed
liveness: n/a
tags: [verification, manual, human-judgment, provenance, migration]
---

# First-class manual verification

## Context

agent-spec 的 verdict 词汇是 `pass / fail / skip / uncertain / pending_review`，
且 **skip ≠ pass**（AGENTS.md:518）：`is_passing` 要求 failed、skipped、uncertain 全为零。
这条规则是护城河——它让"没有测试的场景"永远无法伪装成绿。

但它没有给"本来就不该由机器验证的场景"留位置。robrix2 的 45 份 spec 里，
几乎每一份都有 UI / homeserver 场景，只能绑到不存在的 `manual_test_*` 选择器，
verifier 报 `skip`（`test_verifier.rs:231`：选择器零匹配 → Skip）。结果是
`guard` 一跑 35/45 份"失败"，全部是 skip；`stamp` 永远 `Spec-Passing: false`；
人工验收只能写进 commit message。robrix2 的 `scripts/spec-guard.sh` 于是自己
把 skip 容忍掉了——这恰恰是 skip≠pass 想防止的事，只不过发生在工具外面。

现有的两块拼图都差一步：

- `Review: human`（`ReviewMode::Human`）要求测试**真实运行并通过**后才给
  `pending_review`（`test_verifier.rs:233`）；零匹配仍是 skip。它是"机器先证明、
  人再签字"，不是"只能由人证明"。
- `verify --emit-questions` → 人答 envelope → `resolve-ai --decisions` 已经能把人
  的判定结算 `uncertain` / `pending_review`（ADR-003），并以
  `HumanJudgment { source, verdict, reasoning, scenario_id, evidence_digest }`
  进入 trace 证据链（REQ-HUMAN-JUDGMENT-PROVENANCE）。但它**不结算 skip**
  （机械 skip 的场景不发问题），也不写 `.agent-spec/runs`，`explain --history`
  和 `stamp` 看不见。

robrix2 反馈 A1（P0）与 A2（P0）；两轮审查确定：A2 依赖 A1，不能单独作为小
补丁；不得提供把普通 skip 降级为通过的开关；判定记录按 ADR-001 不带身份。

## Motivation

- 让"这条场景由人验证"成为合约里能声明、工具能识别、门禁能区分的事实，
  而不是靠命名约定和 grep。
- 让人工验收成为一等证据：写进 run log、绑定 spec 指纹、在 explain / stamp /
  matrix 可见，spec 一改就失效。
- 让 robrix2 这类有大量不可自动化场景的仓库能用 stock `guard`，不必自写门禁。

## Goals

- 一等的 manual 场景声明，与 `Test:` 互斥或并存皆可（见决策）。
- 独立的 verdict：manual 场景在无人工结算时不是 `skip`，是 `manual_pending`。
- 人工结算复用 ADR-003 的 envelope 链路，结果进 run log 并绑定 `spec_fingerprint`。
- 普通 skip 的语义与门禁完全不变。
- 迁移路径：lint 提示 + codemod，让存量 `manual_test_*` 仓库能一次性迁到新语法。

## Non-Goals

- 不改变 `Review: human` 的语义（它仍是"机器证明后人签字"）。
- 不提供 `--fail-on fail` 或任何把 skip / manual_pending 视为通过的全局开关。
- 不在核心记录里保存审批人身份、时间戳以外的任何身份声明。
- 不解决 A3/A4/C6（变更集与 spec 的相关性、CI 增量），那是 LEP-005。

## Decision

1. **语法**：场景级字段 `Verification: manual` / `验证: manual`。带此字段的场景
   不参与 TestVerifier；`Test:` 可缺省。若同时给出 `Test:`，机器结果仅作辅助证据，
   verdict 仍由人工结算决定。
2. **verdict**：新增 `Verdict::ManualPending`。未结算的 manual 场景 verdict 为
   `manual_pending`；`is_passing` 把它与 skip 同等对待（**阻断**）。`--review-mode`
   不影响它。
3. **结算**：`verify --emit-questions` 为 `manual_pending` 场景发出判定问题（kind
   `manual-verification`），人以 answered envelope 回答，`resolve-ai` 把它结算为
   `pass` / `fail`，并在 `HumanJudgment` 上追加 `spec_fingerprint` 与可选的
   `external_reference`（PR review URL、commit sha 等不可解释的外部引用）。
   `resolve-ai` 同时写 run log（今天它不写）。
4. **失效**：结算记录绑定当时的 `spec_fingerprint`；lifecycle 读到指纹不匹配的
   结算时忽略它并给 `manual_diagnostic`（形状同 `checkpoint_diagnostic`）。
5. **可见性**：`explain` / `stamp --dry-run` / `matrix` 显示 manual 场景的结算状态
   （`manual: settled pass (fingerprint …)` / `manual_pending`）；`stamp` 增
   `Spec-Manual: N settled / M pending` trailer。
6. **迁移**：新 lint `manual-selector-hint`（Warning）：选择器零匹配且名字含
   `manual`（不区分大小写）的场景，提示改用 `Verification: manual`；新命令
   `agent-spec migrate manual --prefix manual_test_ [--write]` 做 codemod。
7. **不变量**：普通 skip 继续阻断；`Review: human` 不变；`HumanJudgment` 不新增
   ADR-001 禁止的字段（`forbidden_identity_fields` 守卫扩展到新字段）。

## Compatibility

- CLI and public API: 新字段、新 verdict、新 lint、新子命令、新 trailer；既有输出
  在没有 manual 场景时逐字节不变。`Verdict` enum 新增变体是 JSON 消费者可见的
  变化，需在 CHANGELOG 明示。
- File formats: run log 条目新增可选 `manual_settlements`；envelope kind 新增值。
- Existing specs and KLL artifacts: 无 manual 场景的 spec 不受影响。

## Migration Plan

1. 发布带 lint 提示的版本；robrix2 跑 `agent-spec migrate manual --prefix manual_test_`
   预览，人工确认后 `--write`。
2. 迁移后 `guard` 对这些 spec 的报告从 "35 failed (skip)" 变为 "manual_pending N"，
   仍阻断，直到人工结算。
3. robrix2 的 `spec-guard.sh` 可以退役 skip 容忍逻辑。

## Security Considerations

人工结算是把"通过"交给人；风险是绕过机器。缓解：结算绑定 spec 指纹（改 spec
即失效）、进入 run log 与 trace 证据链（可审计）、`explain --history` 可见、
不在核心存身份但存外部引用（审计走外部系统）。

## Privacy Considerations

`external_reference` 是用户主动写入的 URL / sha，不采集任何用户数据；不新增
身份字段（ADR-001）。

## Risks and Assumptions

### Assumptions

- 需要人工验证的场景在真实仓库里占比可观（robrix2：约 1/3 spec）。Invalidated if:
  绝大多数所谓 manual 场景其实能自动化——那时应投资测试而非结算。
- ADR-003 的 envelope 足以承载 manual 结算。Invalidated if: 结算需要附件（截图等），
  那需要新的证据形状。

### Risks

- 团队把一切难写测试的场景都标 manual。Mitigation：`audit` 报 manual 占比；
  project spec 可设上限（后续）。
- 新 verdict 破坏 JSON 消费者。Mitigation：CHANGELOG + 无 manual 场景时输出不变。

## Consequences

Good, because 人工验证有了合法位置，skip≠pass 反而更干净——skip 只剩"本该
自动化却没有"一种含义。
Good, because 结算是有指纹、有 run log、可审计的证据，不再是 commit message 里的一句话。
Bad, because verdict 词汇从五个变六个，所有渲染与消费方要认识新值。
Bad, because 多一条人工路径就多一条被滥用的路径；只能靠可见性与审计约束。

## Alternatives Considered

- `--fail-on fail`（把 skip 全局视为通过）——拒绝：直接违反 skip≠pass。
- 复用 `Review: human` 表示 manual——拒绝：它要求测试先真实运行，语义相反。
- 只用命名约定 `manual_test_*` + 文档——拒绝：这正是 robrix2 今天的状态，工具无法区分。
- 让 `resolve-ai` 直接结算 skip——拒绝：会让"忘了写测试"与"本就该人验"混在一起。

## Prior Art

- ADR-003 结构化人工决策点；REQ-HUMAN-JUDGMENT-PROVENANCE。
- REQ-CHECKPOINT-SPEC-FINGERPRINT（本轮）：指纹失效模式的直接先例。
- Cucumber `@manual` 标签与 Xray / Zephyr 的 manual test 状态机。

## Unresolved Questions

- 字段名用 `Verification: manual` 还是 `Test: manual`？前者语义更清楚，后者改动更小。
- manual 场景带 `Test:` 时机器结果是否应能把 verdict 判成 fail（人无法把红的判绿）？
- 结算是否需要过期时间（例如随 release 失效），还是只随 spec 指纹失效？
- `Spec-Manual` trailer 是否值得，还是并入 `Spec-Summary`？
- codemod 是否也该处理 robrix2 那种 "Property — …" 标题约定（A7），还是留给单独提案？
