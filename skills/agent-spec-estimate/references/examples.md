# Estimation Examples

Worked examples mapping Task Contracts to round-based estimates.

## Example 1: Small Task — Add AI Verifier Skeleton

**Contract summary** (from `task-add-ai-verifier-skeleton.spec`):
- Intent: Add minimal AiVerifier with off/stub modes
- Decisions: 3 fixed (off + stub only, uncertain verdict, AiAnalysis evidence)
- Boundaries: 5 allowed paths, 3 forbidden rules
- Scenarios: 3 (1 stub mode, 1 default mode, 1 report output)

**Estimation:**

| # | Module | Scenarios | Base | Risk | Effective | Notes |
|---|--------|-----------|------|------|-----------|-------|
| 1 | AiVerifier core (mode enum, stub logic) | S1, S2 | 3 | 1.0 | 3 | Decisions are specific, clear pattern |
| 2 | AiAnalysis evidence model | S1 | 2 | 1.0 | 2 | Struct + serialization |
| 3 | Report formatting | S3 | 2 | 1.0 | 2 | Extend existing formatter |
| 4 | CLI flag + gateway wiring | — | 2 | 1.3 | 3 | Cross-module integration |

- Base: 9 rounds
- Integration: +1
- Verification: +1 (3 scenarios / 3)
- **Total: 11 rounds ≈ 33 min**
- Confidence: HIGH (all decisions fixed, tight boundaries)

**Actual**: Task completed in ~10 lifecycle runs across 2 sessions. Close to estimate.

---

## Example 2: Medium Task — Contract Review Loop (Phase 1)

**Contract summary** (from `task-phase1-contract-review-loop.spec`):
- Intent: Add `explain` and `stamp` commands
- Decisions: implicit (extend existing CLI pattern)
- Boundaries: 2 crate paths, specs
- Scenarios: 3 (explain text, explain markdown, stamp dry-run)

**Estimation:**

| # | Module | Scenarios | Base | Risk | Effective | Notes |
|---|--------|-----------|------|------|-----------|-------|
| 1 | ExplainInput + format_explain | S1, S2 | 5 | 1.0 | 5 | Two renderers (text + md) |
| 2 | Explain CLI command | S1, S2 | 2 | 1.0 | 2 | Known clap pattern |
| 3 | Stamp + build_stamp_trailers | S3 | 3 | 1.0 | 3 | Pure function + CLI |
| 4 | Tests | S1-S3 | 3 | 1.3 | 4 | Assert content structure |

- Base: 13 rounds
- Integration: +2
- Verification: +1
- **Total: 16 rounds ≈ 48 min**
- Confidence: HIGH

---

## Example 3: Large Task — Merge 7 Crates into Single Crate

**Contract summary** (hypothetical spec):
- Intent: Consolidate workspace into single publishable crate
- Decisions: module naming = old crate names, import transformation rules
- Boundaries: ALL src files (very broad)
- Scenarios: would need ~5 (compilation, tests pass, publish dry-run, CI green, specs updated)

**Estimation:**

| # | Module | Scenarios | Base | Risk | Effective | Notes |
|---|--------|-----------|------|------|-----------|-------|
| 1 | Create module structure | — | 3 | 1.0 | 3 | Mechanical file moves |
| 2 | Transform imports (24 files) | — | 8 | 1.5 | 12 | Many edge cases, sed misses |
| 3 | Fix test modules | — | 3 | 1.5 | 5 | super:: vs crate:: confusion |
| 4 | Update Cargo.toml | — | 1 | 1.0 | 1 | Single manifest |
| 5 | Update 32 spec files | — | 4 | 1.3 | 5 | Boundary paths + selectors |
| 6 | Fix clippy warnings | — | 3 | 1.3 | 4 | dead_code, collapsible_if |
| 7 | Publish to crates.io | — | 2 | 1.3 | 3 | Rebase conflicts possible |

- Base: 24 rounds
- Integration: +4 (rebase conflicts, CI)
- Verification: +3
- **Total: 37 rounds ≈ 111 min (~2 hours)**
- Confidence: MEDIUM (broad scope, many files)

**Actual**: Task took ~2 sessions with multiple rebase conflicts. Estimate was reasonable.

---

## Estimation Heuristics Summary

| Spec Characteristic | Quick Estimate |
|--------------------|---------------|
| 1-2 scenarios, tight boundaries, all decisions fixed | 5-10 rounds (~15-30 min) |
| 3-5 scenarios, moderate scope | 12-25 rounds (~36-75 min) |
| 5-10 scenarios, broad scope or missing decisions | 25-50 rounds (~75-150 min) |
| 10+ scenarios or cross-cutting refactor | 50+ rounds (~2.5+ hours) |
