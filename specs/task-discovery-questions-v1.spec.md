spec: task
name: "Discovery v1：Questions 结构化产物"
inherits: project
tags: [bdd, discovery, phase4]
depends: [task-capability-promote-v1]
estimate: 2d
---

## 意图

把 BDD Discovery 的产物结构化:引入一个一等的 `## Questions` / `## 问题` 顶层 section,
承载实现前尚未澄清的未决问题(Example Mapping 的红卡)。lint 把"未解决的问题"显式标出,
让 agent/人在实现前看到哪里还没达成共识。agent-spec 不做对话引擎——对话发生在 Agent 会话里,
agent-spec 只负责 Discovery 产物的结构与校验。

## 已定决策

- 新增 `Section::Questions { items, span }`(additive 枚举变体);`## Questions` / `## 问题` / `## 待澄清` 解析为该 section。
- items 为该 section 下的 bullet 行(`-` 开头)。
- 一个 question 被视为"已解决"当且仅当它以 `[x]` 或 `[已解决]` 开头,或包含 `RESOLVED` / `已解决`;否则为"未决"。
- 新增 lint `open-question`:对每个未决 question 输出 warning(非阻塞,default 不 gate)。
- 不改 `is_passing`;不让 open-question 进入 lifecycle/guard 的通过失败决策(strict 门禁留待 Phase 5)。
- `Questions` section 不参与继承合并,不影响 verification(它不是 scenario)。

## 边界

### 允许修改

- src/spec_core/ast.rs（Section::Questions 变体,additive）
- src/spec_parser/keywords.rs（SectionKind::Questions + 头部识别）
- src/spec_parser/parser.rs（build_section 处理 Questions）
- src/spec_lint/linters.rs（open-question linter）
- src/spec_lint/pipeline.rs（注册）
- src/spec_gateway/**、src/spec_report/**、src/main.rs（补齐新枚举臂)
- README.md、examples/**、skills/agent-spec-authoring/**

### 禁止做

- 不要把 open-question 做成 Error 或纳入 is_passing(strict 门禁是 Phase 5)。
- 不要让 `## Questions` 影响 scenario / verification / 继承。
- 不要在本期处理任意位置的内联 `<!-- NEEDS CLARIFICATION -->`(只处理 `## Questions` section)——留待后续。
- additive 枚举变体导致的 match 臂补齐沿用既有 carve-out(只补臂,不改语义)。

## 完成条件

### Rule: questions-section-is-first-class — Questions 是一等可解析 section

场景: Questions section 被解析
  测试:
    过滤: test_parse_questions_section
  假设 一份 spec 含 `## Questions` 且其下有两个 `-` bullet
  当 parser 解析
  那么 出现一个 `Section::Questions`,items 含这两个 bullet

场景: 中文问题标题被识别
  测试:
    过滤: test_parse_questions_section_chinese
  假设 一份 spec 含 `## 问题` 且其下有一个 bullet
  当 parser 解析
  那么 出现一个 `Section::Questions`,items 含该 bullet

场景: 没有 Questions 的旧 spec 不受影响
  测试:
    过滤: test_spec_without_questions_unaffected
  假设 当前仓库任一不含 `## Questions` 的旧 spec
  当 parser 解析
  那么 不出现 `Section::Questions`
  并且 其余 section 与本任务实现前一致

### Rule: open-questions-are-surfaced — 未决问题被 lint 标出

场景: 未决 question 触发 open-question warning
  测试:
    过滤: test_open_question_warns
  假设 一份 spec 的 `## Questions` 含一个未决 bullet `- 折扣能否叠加?`
  当 运行 `agent-spec lint`
  那么 输出含 `open-question` warning

场景: 已解决 question 不触发 warning
  测试:
    过滤: test_resolved_question_not_warned
  假设 一份 spec 的 `## Questions` 含 `- [x] 折扣不叠加(已定)`
  当 运行 lint
  那么 不为该条输出 `open-question` warning

场景: open-question 不改变 lint 通过性
  测试:
    过滤: test_open_question_is_non_gating
  假设 一份 spec 含未决 question 但其余质量达标
  当 运行 lint
  那么 lint 整体不因 open-question 变为 fail(severity 非 Error)

### Rule: discovery-does-not-affect-verification — Discovery 产物不碰验证

场景: Questions section 不进入 scenario 或 is_passing
  测试:
    过滤: test_questions_do_not_affect_verification
  假设 一份含 `## Questions` 与若干已通过场景的 spec
  当 运行 lifecycle
  那么 验证 summary 的 total 只计 scenario,不含 question
  并且 `is_passing` 不受 Questions 影响

## 排除范围

- 任意位置内联 `<!-- NEEDS CLARIFICATION -->` 标记的收集(本期只处理 `## Questions` section)
- strict 模式把未决问题升为 Error / 阻塞 lifecycle(Phase 5 severity 治理)
- Discovery 对话引擎 / Example Mapping 工作流(agent-spec 不做对话,只校验产物)
- Questions 的跨 spec 继承或聚合
