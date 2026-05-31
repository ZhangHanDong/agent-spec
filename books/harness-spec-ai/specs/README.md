# Chapter Specs

These files are writing contracts for the mdBook project, not product implementation specs.

Current validation target:

- every chapter has a dedicated `.spec.md`
- every chapter declares Intent, Decisions, Boundaries, and Completion Criteria
- every scenario has an explicit `Test:` selector for traceability
- scenarios use `Review: human` because manuscript quality requires editorial review

Known non-blocking lint warnings:

- `error-path`: chapter writing contracts mostly describe positive editorial acceptance paths.
- `decision-coverage`: some editorial decisions are chapter-level constraints rather than individual scenario assertions.
- behavior linters may occasionally trigger on words such as "output" when the text is about publishing artifacts, not CLI behavior.

Before drafting a chapter, use its spec as the acceptance checklist. Before finalizing a chapter, replace or supplement human-review selectors with concrete manuscript QA checks where practical.

Visual budget and Mermaid rendering are governed by `visual-budget.md`.
