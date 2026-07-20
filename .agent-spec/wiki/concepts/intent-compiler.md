---
title: "Intent Compiler"
type: concept
source_files:
  - knowledge/requirements/req-requirements-compiler-plan-dag.md
  - src/spec_knowledge/requirement_plan.rs
  - src/spec_knowledge/trace_ledger.rs
  - src/spec_knowledge/test_obligations.rs
  - src/spec_knowledge/worktrees.rs
  - crates/rust-atlas/src/context.rs
  - src/main.rs
tags:
  - requirements
  - compiler
  - dag
status: draft
---

# Intent Compiler

## Role

The intent compiler lowers stable KLL requirements into executable,
verifiable, and traceable work:

```text
PRD/issue -> knowledge/requirements/*.md -> requirements graph/plan
          -> work_units.json / worktrees.json / test_obligations.json
          -> specs/task-*.spec.md -> lifecycle -> trace ledger
```

`requirements plan` is the central DAG surface. It joins requirement nodes,
dependency edges, execution batches, and satisfying specs. `requirements
test-obligations` emits spec-derived test obligations without scanning
implementation code. `requirements worktrees` maps ready work units to
deterministic branch/path entries for parallel agents.

`requirements trace`, `requirements replay`, `requirements explain-failure`,
and `requirements trace-graph` read stored trace ledgers. A failure can be
walked back to requirement id, work unit, spec path, scenario, test selector,
code target, worktree/branch, VCS reference, and related wiki articles when the
wiki source trace covers those paths.

The CLI remains deterministic. AI may help draft candidate KLL requirements or
ask reverse-interview questions from `requirements questions`, but reviewed
`knowledge/requirements/*.md` artifacts remain the source of truth.

Atlas runtime-boundary hints may explain where a static flow stopped, but the
intent compiler does not treat those heuristic candidates as code bindings or
acceptance evidence. Only governed graph facts and explicit Contract selectors
enter plans, lifecycle, and trace ledgers.

The B5 query context compiler is an optional code-grounding aid for an Agent
executing or reviewing a work unit. It does not lower PRD text, infer
requirements, change requirement status, create `satisfies` links, or turn
nearby tests into test obligations. Those remain deterministic intent-compiler
and governance operations.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 was reviewed here; graph publication changed, but governed bindings
and explicit Contract selectors remain the intent compiler boundary.

Atlas D3 was reviewed here; watcher and daemon state can trigger refresh but
cannot transition a requirement or create execution evidence.

Atlas B5 was reviewed here; bounded code context remains downstream working
input and does not become intent-compiler truth or acceptance evidence.

Atlas E1 was reviewed here; its plans consume governed corpus inputs and its
receipts measure Agent behavior. Neither artifact compiles or transitions a
requirement.
