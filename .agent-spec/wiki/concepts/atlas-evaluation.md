---
title: "Atlas Evaluation And Adoption"
type: concept
source_files:
  - src/atlas_eval.rs
  - src/atlas_agent_eval.rs
  - src/main.rs
  - benchmarks/atlas/corpus.json
  - benchmarks/atlas/agent-ab-experiment-v1.json
  - benchmarks/atlas/agent-ab-plan-v1.json
  - benchmarks/atlas/serving-ab-experiment-v1.json
  - scripts/atlas-eval/run-agent-ab-opt-in.sh
  - scripts/atlas-eval/run-serving-ab-opt-in.sh
  - docs/atlas-evaluation.md
  - docs/atlas-agent-ab-gate.md
  - knowledge/requirements/req-atlas-agent-ab-gate.md
  - specs/task-atlas-agent-ab-gate.spec.md
tags:
  - atlas
  - evaluation
  - adoption
status: active
---

# Atlas Evaluation And Adoption

## Role

E0 and E3 provide deterministic corpus, query scoring, and receipt structure.
E1 is a separate adoption boundary for real execution. Agent A/B/C isolates
Atlas primitives from B5 context; a second direct/worker experiment isolates D4
serving. Both require complete strict receipts and retain failed runs.

Correctness, freshness, judge/session evidence, and query-metric completeness
gate before efficiency. Medium and large cases must improve beyond the matched
baseline MAD. Small cases retain an explicit tie zone. No external project's
percentage becomes an Atlas threshold.

## Authority

Checked-in manifests and plans are inputs, not benchmark results. Raw sessions
stay under ignored `.agent-spec/evaluation/` storage while their path and hash
remain in receipts. A machine pass is only a human-acceptance candidate and
does not change MCP, B5, or worker defaults.

