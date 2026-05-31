spec: task
name: "第 9 章: 从代码到长文 - Spec 驱动的内容创作"
inherits: book
tags: [book, chapter, writing, synthesis]
---

## Intent

把本书方法论从代码生成扩展到长文本创作：复杂写作同样需要 constitution、outline、scene cards、draft tasks 和 review verdict。读者应理解写软件和写书共享同一种 spec-driven harness 思维。

## Decisions

- Sudowrite Story Bible 作为结构化写作类比，NovelAI Lorebook 作为自由度更高的对照
- 类比服务于方法论，不写成写作软件评测
- 本书自身的 mdBook + chapter specs 作为 meta-case
- 结尾回到全书公式 `可靠 AI 开发 = Harness × Spec`

## Boundaries

### Allowed Changes
- books/harness-spec-ai/src/ch09-spec-driven-writing.md

### Forbidden
- Do not turn the chapter into a fiction-writing tutorial
- Do not overclaim that writing and coding have identical verification strength
- Do not introduce new tools without mapping them to the core model

## Completion Criteria

Rule: writing-code-isomorphism-is-careful - 代码与写作同构但不混同

Scenario: isomorphism and limits are both stated
  Review: human
  Test: ch09_isomorphism_and_limits_are_stated
  Given the chapter thesis
  When code and writing workflows are compared
  Then their shared artifact chain is shown
  And the text states where prose review remains human judgment

Rule: story-bible-maps-to-spec-chain - 写作工具映射到 spec 链

Scenario: Story Bible workflow maps to Spec Kit flow
  Review: human
  Test: ch09_story_bible_maps_to_spec_chain
  Given the writing tool section
  When Story Bible, Outline, Beat, and Draft are introduced
  Then they map to constitution, plan, tasks, and implement

Rule: book-meta-case-closes-the-loop - 本书自身作为案例收束

Scenario: final section uses this mdBook project as meta-case
  Review: human
  Test: ch09_book_meta_case_closes_loop
  Given the final section
  When the book production workflow is summarized
  Then mdBook, ascent-research, and chapter specs are shown as the book's own harness
  And the reader gets a reusable process for their own long-form project

## Out of Scope

- Full novel planning template
- Product comparison matrix
- Claims about automated literary quality

