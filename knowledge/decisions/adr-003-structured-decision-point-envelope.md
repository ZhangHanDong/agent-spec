---
kind: decision
id: ADR-003
title: "Structured Decision Point Envelope"
status: accepted
tags: [knowledge, questions, interop, agent-ux, verification, provenance]
---

# Structured Decision Point Envelope

## Context

LEP-002（accepted 2026-08-04）指出流水线有三处停下来等人拍板——逆向访谈、
提案与决策选型、机器判不了的合约验收——而三处都是自由散文，每个 agent 各问
各的。实证：`ClarificationQuestion` 早就有 `options: Vec<String>` 字段，
`questions.rs` 的两个构造点却全部硬编码 `Vec::new()`，本仓库 119 个问题
零候选；与此同时 intent-compiler skill 白纸黑字要求「Offer 2 or 3 concrete
options」。字段备好了从未接线，承诺写在 prose 里靠模型自觉。

LEP-002 留下四个未决问题。它们本身就是选择题，因此以结构化多选形式向人提出
并当场裁决——这既是本决策的输入，也是它主张的交互的第一次实地演练。

## Decision

采纳 LEP-002 的决策点信封，并裁决其四个未决问题：

- **选项来源：agent 从源文本起草，CLI 只校验形状。** CLI 发出问题骨架
  （id、target、diagnostic_code、prompt、source），候选由 agent 读源文本
  生成；CLI 校验至多四项、每项有标签与一句描述、自由作答路径始终存在。
  CLI 自身不生成候选，intent-compiler 的「模型推断不等于已接受」规则照旧：
  候选是待选项，不是事实。
- **回填：agent 改文件，CLI 只验证。** 不新增写文档的 CLI 子命令；agent
  编辑产物后跑既有 lint、gate 与 verify 证明改动合规。唯一例外是验收判定
  产出的 decisions JSON，它本就是 `resolve-ai` 的既有输入格式。
- **人工判定进 provenance，且必须绕开 ADR-001 的身份禁令。** 判定作为一等
  证据进入 run log 与 trace 证据链，可被 `requirements replay` 与
  `explain-failure` 看到——但记录的是判定的类别与内容（`source: human`、
  verdict、reasoning、被判定 scenario 与证据的 digest），不是身份。
  `actor`、`authority`、`approval`、`policy` 字段仍被机械禁止；外部编排
  系统仍按 digest 在自己的存储里绑定审批人。一句话：core 记录「此处由人
  裁决，裁决内容为 X，绑定证据摘要为 Y」，永不记录「某人批的」。
- **命令面：三个阶段局部表面，不建统一 `questions` 命名空间。** 沿用
  `requirements questions`（填充 options），新增 `knowledge questions <id>`
  与 `verify --emit-questions`。信封形状由共享类型保证一致，不靠命令聚合。

## Consequences

Good, because 人类决策点变成可路由、可审计、可被任何 harness 同样渲染的
数据，不再取决于某个 agent 当时怎么措辞。

Good, because 判定进证据链后「这条 scenario 是人判过的」成为可重放事实，
而按类别而非身份记录使其与 ADR-001 的编排中立性并存。

Good, because 回填交给 agent、CLI 只验证，意味着不必在核心里造一个结构化
Markdown 改写器；已有的 lint 与 gate 就是回填正确性的判据。

Bad, because 候选由模型生成，质量随模型波动，且引入锚定风险：人在压力下会
选列出的选项而非思考。自由作答路径是唯一的机械缓解。

Bad, because 「agent 写、CLI 验」使「答案到文本」这一步不可重放：同一份答案
两次落笔可能得到不同措辞，只能保证结果合规，不能保证字节一致。

Bad, because 三个局部表面各自维护信封一致性，没有单一聚合点，靠共享类型与
测试防漂移；若将来出现第四个发射点，一致性成本线性增长。

## Alternatives Considered

- CLI 从诊断机械推导候选 —— 被否（本轮）：确定性与可测试性更好，但每类诊断
  都要写生成器，覆盖率爬升慢；本轮选择用 agent 换覆盖率，代价是候选质量随
  模型波动。此路径未来可作为高频诊断的补充，与本决策不冲突。
- 人在文档里手写选项 —— 被否：零推断最诚实，但对已存在的 119 个 lint 派生
  问题完全无效，它们没有作者会回头补选项。
- 直接输出 harness 专有格式（如 AskUserQuestion payload）—— 被否：核心 CLI
  里出现厂商表面即是 ADR-001 拒绝过的双向词汇耦合；中立信封加边缘投影是同
  一套理由。
- 统一 `questions` 顶层命名空间 —— 被否（本轮）：学习成本更低，但要给
  `requirements questions` 做别名兼容，且与现有按阶段分层的命令面不一致。
- 判定只做验证前便利设施、不进证据链 —— 被否：那样审计时看不出哪些通过是人
  拍的，与三条独立状态轴的可追溯目标相悖。
- 判定进证据链并记录 actor 与 authority —— 被否：直接违反 ADR-001；CLI 无法
  证明谁批准，记录身份是它证明不了的断言。

## Source Trace

- proposal: LEP-002
  (knowledge/proposals/2026-08-03-structured-human-decision-points.md,
  accepted 2026-08-04)
- 四项裁决来源: 2026-08-04 结构化多选交互，逐条对应 LEP-002 的
  `## Unresolved Questions`
- 上游约束: ADR-001（orchestrator-neutral core；`actor`/`authority`/
  `approval`/`policy` 字段机械禁止，外部系统按 digest 绑定审批）
- 实测证据: src/spec_knowledge/questions.rs 两处构造点硬编码
  `options: Vec::new()`；`requirements questions --format json` 在本仓库
  产出 119 个问题、0 个带候选
- governed requirements: REQ-DECISION-POINT-ENVELOPE,
  REQ-DECISION-POINT-EMISSION, REQ-HUMAN-JUDGMENT-PROVENANCE
