---
name: agent-spec-intent-compiler
description: Use when converting PRD or issue prose into KLL requirements, running the intent compiler plan, or reverse-interviewing a human to resolve requirement ambiguity before task spec generation.
---

# Agent Spec Intent Compiler

Use this skill when a user wants to move from raw PRD/issue prose to governed KLL requirements and task specs.

## Rules

- The CLI remains deterministic and model-free.
- Raw source belongs in `docs/`.
- Machine-consumable truth belongs in `knowledge/requirements/*.md`.
- Do not silently invent missing requirements. Emit or ask clarification questions.
- Do not treat a model inference as accepted; only human-confirmed answers may change KLL truth.
- Every generated requirement must include `## Source Trace`.
- Open questions stay in `## Open Questions` until answered by the human.
- For agent-spec's own development, dogfood this workflow on the repository's own KLL requirement and task spec before presenting fixtures as sufficient proof.
- Treat KLL roots as compiler trust boundaries: do not bypass unsafe-id, strict-frontmatter, kind-directory, missing-root, or symlink diagnostics.
- Read the plan as a cross-layer DAG: requirement nodes lower to work units, work units satisfy spec nodes, and spec dependencies remain explicit edges.

## Workflow

1. Read the raw PRD or issue.
2. Draft Candidate Requirement Block entries using the PRD Intake Output Contract below.
3. Ask for human confirmation before importing candidate blocks into KLL.
4. Run `agent-spec requirements import --from docs/prd.md --out knowledge/requirements` only after the candidate blocks are accepted.
5. Run `agent-spec lint-knowledge --knowledge knowledge --gate`.
6. Run `agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate`.
7. Run `agent-spec requirements test-obligations --knowledge knowledge --specs specs --format json --out .agent-spec/test_obligations.json`.
8. Run `agent-spec requirements worktrees --knowledge knowledge --specs specs --base main --path-prefix ../agent-spec-worktrees --out .agent-spec/worktrees.json`.
9. After code grounding or a change, run `agent-spec requirements affected` and save the provider-neutral intent-impact report.
10. Run `agent-spec requirements affected-bundle` to apply the risk A/B/C evidence policy and select executable provider configs, justified checks, explicit tests, gates, guidance, and skill receipts; generated selector slugs remain non-authoritative candidates.
11. After lifecycle and quality execution, run `agent-spec requirements affected-record` with the same stable `run_id` to store the report, optional bundle, and normalized outcomes in trace ledger v2.
12. When debugging, run `agent-spec requirements replay REQ-*`, `requirements explain-failure REQ-*`, or `requirements trace-graph REQ-*`; all three read stored lifecycle and affected evidence only and never rerun providers, tools, skills, or models.
13. Run `agent-spec requirements questions --knowledge knowledge --specs specs --format json`.
14. Use the Reverse Interview Loop below to ask only the emitted blocking questions, grouped by requirement id.
15. Use Answer Integration below to write accepted answers back into KLL requirements as requirement clauses, scenarios, source trace entries, or resolved open questions.
16. Generate task specs with `satisfies: [REQ-*]`.
17. Run `agent-spec lifecycle`, `agent-spec guard`, and `agent-spec trace`.
18. For agent-spec itself, confirm dogfood evidence with `requirements replay`, `requirements explain-failure`, and `requirements trace-graph` for the repository requirement id.

The skill text must preserve these exact terms for documentation tests: QA class, state-machine, reverse interview, active specs, dogfood.

## PRD Intake Output Contract

Natural-language PRD intake is an AI-assisted drafting step, not a CLI parser mode. The output is a set of Candidate Requirement Block entries that a human can review and then import.

Each Candidate Requirement Block must include:

- `id`: stable `REQ-*` id proposed by the agent.
- `title`: short title from the user-facing requirement, not a filename slug.
- `tags`: domain tags when the source text supports them.
- `source`: original PRD, issue, ticket, or document path.
- `source excerpt`: the smallest quoted or paraphrased source span that justifies the requirement.
- `confidence`: `high`, `medium`, or `low`, based on how directly the source states the requirement.
- `## Problem`: why the requirement exists.
- `## Requirements`: one or more normative clauses using MUST, MUST NOT, SHOULD, or MAY.
- `## Scenarios`: at least one executable scenario for each implementation-bearing leaf.
- `## Dependencies`: explicit requirement ids, or `None.`
- `## Source Trace`: source path plus source excerpt reference.
- `## Open Questions`: `None.` only when there is no unresolved ambiguity.

Candidate blocks should use the import marker format:

```md
<!-- agent-spec:requirement id=REQ-EXAMPLE title="Example" tags=domain source=docs/prd.md -->
## Problem

...

## Requirements

[REQ-EXAMPLE] The system MUST ...

## Scenarios

Scenario: Observable behavior
  Given ...
  When ...
  Then ...

## Dependencies

None.

## Source Trace

- docs/prd.md#section: source excerpt ...

## Open Questions

- Should ...?
<!-- /agent-spec:requirement -->
```

Low-confidence blocks must keep the uncertainty in `## Open Questions`; do not hide it in prose.

## Reverse Interview Loop

Run `requirements questions --format json` after importing or editing KLL requirements. Treat the JSON as the agenda for the reverse interview.

For each blocking question:

- Show the requirement id.
- Explain which diagnostic produced the question.
- Show the exact question text.
- Include the source excerpt when available.
- Offer 2 or 3 concrete options only when the source supports them.
- Preserve a free-form answer path.

Do not ask non-blocking warning questions unless the user explicitly wants quality cleanup. Do not treat a model inference as accepted.

## Reverse Interview Format

For each question, present:

- Requirement id
- Why the ambiguity blocks execution
- The exact question
- 2 or 3 concrete answer choices when the source text supports them
- A free-form option when none of the choices is correct

Never treat an inferred answer as accepted unless the human confirms it.

## Answer Integration

After the human answers:

- Mark the answer as human-confirmed in the working notes or source trace.
- Convert the answer into a concrete requirement clause, scenario step, dependency, QA class, or source trace entry.
- Remove or rewrite the corresponding `## Open Questions` item only after the answer is represented in the requirement body.
- Re-run `lint-knowledge --gate`, `requirements plan --gate`, and `requirements questions`.
- If questions remain blocking, continue the loop before generating task specs.
