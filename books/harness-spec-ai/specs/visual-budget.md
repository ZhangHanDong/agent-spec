# Visual Budget and Verification

This book should read like an engineering field guide, not a wall of prose. Visuals must carry structure: diagrams, matrices, workflow charts, annotated code/spec blocks, and compact comparison tables.

## Global Ratio

- Target manuscript size: 51,000-59,000 Chinese characters before appendices.
- Target visual units: 38-45.
- Target density: about 1 visual unit per 1,200-1,600 Chinese characters.
- Hard floor: every chapter must have at least 3 visual units after drafting.
- Long-text guard: no chapter section should run beyond about 1,500 Chinese characters without a visual, table, code/spec block, or checklist.

## What Counts as a Visual Unit

- Mermaid diagram: 1 unit.
- Raster/vector image or screenshot: 1 unit.
- Comparison table with at least 3 rows: 1 unit.
- Annotated code/spec block with commentary: 1 unit.
- Process checklist or acceptance matrix: 1 unit.

Do not inflate the count with decorative images. A visual unit must help the reader understand structure, sequence, contrast, or verification.

## Chapter Budget

| Chapter | Target characters | Visual units | Mermaid floor | Required visual anchors |
|---|---:|---:|---:|---|
| 1. Environment bottleneck | 4,000-5,000 | 3-4 | 1 | paradigm shift, Harness x Spec formula, book map |
| 2. Harness engineering | 6,000-7,000 | 5-6 | 2 | five-layer stack, feedback loop, long-running handoff |
| 3. Spec-driven development | 6,000-7,000 | 5-6 | 1 | Spec -> Plan -> Tasks -> Implement, artifact role matrix |
| 4. BDD spine | 6,000-7,000 | 5-6 | 2 | Discovery/Formulation/Automation loop, Rule -> Example -> Verdict |
| 5. Agent-spec standards | 5,000-6,000 | 4-5 | 1 | context loading pyramid, AGENTS/SKILL/MCP/task spec matrix |
| 6. Spec to Verdict | 6,000-7,000 | 5-6 | 2 | Task Contract anatomy, lifecycle sequence, coverage matrix |
| 7. codex-rs Rust contracts | 6,000-7,000 | 4-5 | 1 | Rust rule map, feedback loop, anti-pattern table |
| 8. Rust agent project | 7,000-8,000 | 6-7 | 2 | repo architecture, spec-first sequence, provider/tool flow |
| 9. Spec-driven writing | 5,000-6,000 | 4-5 | 1 | writing artifact chain, book-as-harness meta-flow |

## Verification

Use the checker after each chapter draft:

```bash
books/harness-spec-ai/tools/check-visual-budget.sh
```

During the skeleton phase, placeholders can be skipped:

```bash
books/harness-spec-ai/tools/check-visual-budget.sh --allow-placeholders
```

Rendering verification:

```bash
mdbook build books/harness-spec-ai
books/harness-spec-ai/tools/check-rendering.sh
```

The build must load local `mermaid.min.js` plus `mermaid-init.js`. Mermaid rendering is runtime-based: fenced `mermaid` code blocks remain in Markdown, and `mermaid-init.js` converts them into Mermaid diagrams in the generated HTML.
