---
title: "Knowledge Versus Docs"
type: decision
source_files:
  - skills/agent-spec-wiki/SKILL.md
  - AGENTS.md
  - docs/atlas-query-context.md
  - docs/atlas-concurrent-query-serving.md
  - knowledge/requirements/req-atlas-query-context-compiler.md
  - specs/task-atlas-query-context-compiler.spec.md
  - knowledge/requirements/req-atlas-concurrent-query-serving.md
  - specs/task-atlas-concurrent-query-serving.spec.md
tags:
  - knowledge
  - docs
  - wiki
status: draft
---

# Knowledge Versus Docs

## Role

Durable truth belongs in knowledge/, executable contracts in specs/, human docs in docs/, and agent working memory in .agent-spec/wiki/.

Atlas graph shards, query indexes, and code bindings are derived working data.
Their freshness and authority gates affect verification, but they do not replace
accepted KLL requirements or decisions.

Reader-facing Atlas commands and budgets belong in README and agent guidance;
the accepted requirement and Task Contract remain the normative statement of
why those query surfaces and honesty boundaries exist.
The wiki may summarize runtime-boundary behavior for agent navigation, while
the KLL requirement owns its normative limits and the dedicated docs page owns
the user-facing command contract.

The same routing applies to B5: `docs/atlas-query-context.md` explains usage,
the accepted KLL requirement and Task Contract govern behavior, and emitted
context JSON is disposable runtime evidence rather than documentation truth.

D4 follows the same routing. Its reader guide belongs in `docs/`, normative
queue and outcome rules remain in KLL and the Task Contract, the checked-in
fixture receipt is regression evidence, and live worker receipts remain
derived runtime data.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 was reviewed here; the reader guide remains in `docs/`, the normative
requirement remains in `knowledge/`, and generated graph data remains derived.

Atlas D3 follows the same routing: `docs/atlas-live-runtime.md` is the reader
guide, `REQ-ATLAS-LIVE-RUNTIME` is normative, and `.runtime/` state is derived.

Atlas B5 was reviewed here; context receipts may be cited by the wiki but do not
move normative content out of `knowledge/` or executable criteria out of
`specs/`.

Atlas D4 was reviewed here; measured latency, CPU, RSS, and heartbeat values do
not become requirements merely because the wiki or docs summarize them.

Atlas E1 follows the same routing: the accepted requirement and Task Contract
are normative, `docs/atlas-agent-ab-gate.md` is reader guidance, and real
session/gate files remain ignored runtime evidence.
