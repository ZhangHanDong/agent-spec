## Roadmap Specs

`specs/roadmap/` contains staged self-hosting task specs for future `agent-spec` work.

These files are real task contracts, but they are not part of the default top-level
`agent-spec guard --spec-dir specs --code .` run until they are promoted into
the top-level `specs/` directory.

Promotion rule:

- draft or future-phase contracts stay in `specs/roadmap/`
- active implementation contracts move to top-level `specs/`
- implemented contracts whose lifecycle passes are tagged `done` and moved out via
  `agent-spec archive --spec-dir specs/roadmap --run-log-dir .` (archived copies live in
  `.agent-spec/archive/specs/`, summary in `knowledge/context/spec-archives.md`)

Nested roadmap specs still inherit the top-level [`project.spec`](../project.spec.md).

## Current Status (triaged 2026-07-12)

16 legacy roadmap specs (phase0–6, plan-command, goal-gate, checkpoint-resume,
complexity-gate, human-review, optimize-scenario-mode, scenario-dependencies,
spec-dependency-graph, support-scenario-verification-metadata) verified passing
and were archived — the features shipped long ago.

Remaining staged contracts:

| Spec | State | Gap |
|------|-------|-----|
| `task-atlas-mir-layer.spec.md` | planned (Phase 2) | depends on Phase 1 |
| `task-atlas-kll-integration.spec.md` | planned (Phase 3) | depends on Phase 1 |
| `task-context-fidelity.spec.md` | partial (2/3, 1 skip) | one bound test not found in the codebase |
| `task-history-summary.spec.md` | not started (0/4 skip) | no bound tests exist yet |
| `task-status-file-contract.spec.md` | partial (3/4, 1 skip) | one selector unbound |
| `task-strengthen-rewrite-contract-authoring.spec.md` | partial (4/6, 2 skip) | two selectors unbound |

## Target-Architecture Delivery Boundaries

`docs/intent-compiler/architecture.md` stages the target intent-compiler
architecture as five contracts:

| Boundary | Contract |
|----------|----------|
| 1. Requirement governance gate + explicit transitions | `specs/task-requirement-governance-transitions.spec.md` (active) |
| 2. Provider-neutral Code Graph IR + typed code bindings | to be staged (builds on `specs/task-rust-atlas-code-graph.spec.md`, now active) |
| 3. Rust Atlas through the Intent-Code Linker | `specs/roadmap/task-atlas-kll-integration.spec.md` |
| 4. Quality Providers, profiles, Execution Bundles | to be staged |
| 5. Aggregate status/evidence queries + full dogfood | to be staged |

Use:

```bash
agent-spec contract specs/roadmap/task-rust-atlas-code-graph.spec.md
```

when you want to inspect or refine a staged roadmap contract before promotion.
