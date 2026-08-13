---
kind: decision
id: ADR-004
title: "Clause Coverage Attribution"
status: accepted
tags: [knowledge, lint, coverage, governance, liveness]
---

# Clause Coverage Attribution

## Context

LEP-003（accepted 2026-08-08）指出：`liveness=Honored` 声称需求的义务成立，
但没有任何东西把 MUST 条款与场景连起来。本仓库 553 条 MUST 条款对 255 个
场景，过半条款连原则上都不可能有自己的场景；场景也从不引用条款 id
（`req-decision-point-emission.md` 声明 12 个条款 id，其 `## Scenarios`
段落里 `REQ-` 出现零次）。既有的 `requirement-must-needs-scenario` 只在
「一个场景都没有」时才报，一个场景就能让十二条义务过关。

实证不是假想：2026-08-07 的审查发现
`REQ-DECISION-POINT-EMISSION-MECHANICAL-MOAT` 被实现推翻，而需求全程显示
Honored——因为没有场景绑住那半句条款。合约绿、门禁绿，被推翻的治理义务
无声无息。

LEP-003 留下五个未决问题，其中四个以结构化多选向人提出并当场裁决。

## Decision

采纳 LEP-003 的条款覆盖，并裁决其未决问题：

- **归属语法：复用 BDD 的 `Rule: <条款 id>` 分组。** 场景归属到 Rule 之下，
  Rule id 即条款 id。不发明第二套语法：`bdd-rule-id` 已在校验此形态 id 的
  合法性，spec 层也已用这套分组，需求层沿用同一方言。
- **不做关键词兜底。** 只认显式归属。本提案存在的理由正是「看上去绿」掩盖了
  真问题；推断出来的覆盖会重蹈覆辙，诚实地报未覆盖优于制造似是而非的绿。
  这一点推翻了 LEP-003 草案中「保留标注为弱信号的兜底」的倾向。
- **skip 的场景不算覆盖。** 覆盖要求场景真正跑过，与 liveness 既有语义一致
  （skip 不算通过），并杜绝「写个空场景占位」刷覆盖率。
- **覆盖报在 `requirements status`，不进 `trace`。** 三条独立状态轴已经在
  status 命令里，覆盖率是治理面的度量而非 liveness 本身；`trace` 保持只答
  liveness，避免把两个概念混在同一行输出里。
- **规则与分阶段：** 新规则 `clause-uncovered` 指名条款 id 与文档，引入版本
  为 Info，配只准缩小的基线文件记录当下未覆盖的条款；升级严重级别的版本与
  基线规模前提由后续决策定，沿用 ADR-002 对 `orphan-spec` 的模式。

## Consequences

Good, because `Honored` 从「合约机械通过」变成「义务被检验过」，这个词终于
等于读者本来以为的意思。

Good, because 被悄悄推翻的条款会显形：MECHANICAL-MOAT 那次会以「未覆盖条款」
的形式早早出现，而不是靠人碰巧审查发现。

Good, because 与任务合约层的 `decision-coverage` 形成对称——需求层一直是更弱
的那层，此前无人察觉。

Bad, because 553 条待覆盖条款是笔醒目的债，若基线从不缩小，规则沦为装饰、
Info 成为永久归宿——`orphan-spec` 已有的风险，现在有两处。

Bad, because 归属给每份需求文档增加了写作负担，而回报在写下的当天不可见。

Bad, because 覆盖可以被「一个场景认领全部条款」刷掉，规则抬高地板但不抬高
天花板；归属正确性属于评审范畴，机器只保证归属可见。

## Alternatives Considered

- 加强 `requirement-must-needs-scenario` 要求场景数不少于条款数 —— 被否：
  实现便宜、绕过也便宜，它从不问某个场景服务哪一条，还会错误地禁止一个场景
  合法地覆盖两条条款。
- 关键词匹配作为主机制或弱信号兜底 —— 被否（本轮裁决推翻了草案倾向）：
  推断出的覆盖没有作者意图支撑，而「看上去绿」正是本提案要消除的东西。
- 新增每场景一行 `Covers:` 标签 —— 被否：语义等价于 Rule 分组，但要新增一种
  语法并再写一套校验，与既有 BDD 方言重复。
- frontmatter 里写条款到场景的映射表 —— 被否：声明与场景相隔很远，改场景时
  极易忘改映射，是漂移的温床。
- 把覆盖下推到合约层，要求每个 spec 场景指名条款 —— 被否（本轮）：合约已有
  自己的覆盖规则，而义务写在需求文档里，读者也在那里找它。
- 不做，靠人工审查 —— 被否：本提案的起因正是人工审查在事后、靠运气抓到了
  一次。

## Source Trace

- proposal: LEP-003
  (knowledge/proposals/2026-08-08-clause-coverage.md, accepted 2026-08-08)
- 四项裁决来源: 2026-08-08 结构化多选交互，对应 LEP-003 的
  `## Unresolved Questions` 前四项；第五项（升级版本与基线前提）由后续决策定
- 实测证据: knowledge/requirements/ 下 553 条 MUST 条款对 255 个场景；
  req-decision-point-emission.md 的 Scenarios 段零 `REQ-` 引用
- 失效实证: REQ-DECISION-POINT-EMISSION-MECHANICAL-MOAT 被实现推翻期间
  需求持续显示 Honored（2026-08-07 审查发现）
- 先例: spec_lint 的 decision-coverage / observable-decision-coverage；
  BDD `Rule: <id> — <name>` 方言与 bdd-rule-id；ADR-002 的 orphan-spec
  分阶段与只准缩小基线
- governed requirements: REQ-CLAUSE-COVERAGE
