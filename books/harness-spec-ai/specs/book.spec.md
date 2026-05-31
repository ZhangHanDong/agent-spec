spec: task
name: "《Harness 工程之 Spec 驱动 AI 开发》全书写作合同"
tags: [book, harness, spec-driven, mdbook]
---

## Intent

建立一本极简中文技术书的可执行写作合同。全书用 mdBook 组织，以 Harness 工程和 Spec 驱动 AI 开发为主线，先完成每章 spec，再进入正文写作。

## Decisions

- 书稿目录固定为 `books/harness-spec-ai/`
- 全书采用 9 章结构，不加入 AI 简史或大模型原理泛论
- 核心公式固定为 `可靠 AI 开发 = Harness × Spec`
- 每章正文必须先满足对应章节 spec，再进入润色
- 事实性主张优先引用一手来源；厂商自报数字必须标明来源属性
- 图文预算固定记录在 `books/harness-spec-ai/specs/visual-budget.md`
- Mermaid 渲染使用 mdBook `additional-js` 加载本地 `mermaid.min.js` / `mermaid-init.js`

## Boundaries

### Allowed Changes
- books/harness-spec-ai/**

### Forbidden
- Do not rewrite existing agent-spec product specs while drafting the book
- Do not place manuscript chapters under the Rust crate `src/`
- Do not treat unverified market numbers as stable facts

## Completion Criteria

Rule: book-structure-is-mdbook - 全书结构可由 mdBook 渲染

Scenario: mdBook skeleton exists
  Review: human
  Test: book_overall_mdbook_skeleton_exists
  Given the book root is `books/harness-spec-ai`
  When the project is inspected
  Then `book.toml` and `src/SUMMARY.md` exist
  And SUMMARY lists the preface and 9 chapters

Rule: chapter-specs-precede-drafting - 每章正文先有章节合同

Scenario: every chapter has a matching spec
  Review: human
  Test: book_overall_every_chapter_has_spec
  Given SUMMARY lists 9 chapter pages
  When chapter specs are inspected
  Then every chapter has one matching `.spec.md` file under `books/harness-spec-ai/specs/`
  And each chapter spec contains Intent, Decisions, Boundaries, and Completion Criteria

Rule: source-discipline-is-explicit - 事实来源纪律进入全书合同

Scenario: source caveats are part of the writing contract
  Review: human
  Test: book_overall_source_caveats_are_explicit
  Given the book depends on 2025-2026 tools and market facts
  When drafting starts
  Then dynamic facts require fresh verification
  And speculative or vendor-reported claims are labeled as such

Rule: visual-budget-is-verifiable - 图文比例预算可验证

Scenario: visual budget is documented and checkable
  Review: human
  Test: book_overall_visual_budget_is_checkable
  Given the book has 9 planned chapters
  When visual planning is inspected
  Then `books/harness-spec-ai/specs/visual-budget.md` defines per-chapter character and visual-unit budgets
  And `books/harness-spec-ai/tools/check-visual-budget.sh` can verify those budgets after drafting

Rule: mermaid-rendering-is-supported - 渲染链路支持 Mermaid

Scenario: mdBook renders Mermaid diagrams
  Review: human
  Test: book_overall_mermaid_rendering_supported
  Given `books/harness-spec-ai/book.toml`
  When mdBook builds the book
  Then the generated HTML loads local Mermaid JavaScript assets
  And `mermaid-init.js` converts fenced Mermaid code blocks into rendered diagrams

## Out of Scope

- Writing full chapter prose in this phase
- Rendering final HTML with complete content
- Creating marketing copy or cover design
