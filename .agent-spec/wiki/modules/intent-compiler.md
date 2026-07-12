---
title: "Intent Compiler"
type: module
source_files:
  - src/spec_knowledge/parser.rs
  - src/spec_knowledge/requirement_graph.rs
  - src/spec_knowledge/work_units.rs
  - src/spec_knowledge/requirement_plan.rs
  - knowledge/requirements/req-kll-work-units.md
  - knowledge/requirements/req-requirements-compiler-plan-dag.md
  - specs/task-requirements-compiler-plan-dag.spec.md
tags:
  - kll
  - requirements
  - compiler
  - dag
---

# Intent Compiler

## Role

The intent compiler lowers long-lived KLL requirement artifacts into
implementation planning surfaces. Its core chain is:

```text
knowledge/requirements/*.md
  -> RequirementGraph
  -> WorkUnitSet
  -> RequirementPlan
  -> specs/task-*.spec.md
  -> lifecycle / trace evidence
```

The CLI remains deterministic. AI may help draft requirements or ask reverse
interview questions, but the compiler itself operates on checked Markdown,
parsed specs, and recorded verification evidence.

## Requirement Documents

Requirement documents are parsed from frontmatter plus level-2 sections. The
frontmatter `id` is canonical and normalized to uppercase. Input is strict:
frontmatter starts on line one, malformed or duplicate keys fail parsing, ids
must be safe hyphenated identifiers, directory and `kind` must agree, and
recursive collection rejects symlinks. A missing knowledge or specs root is a
blocking compiler diagnostic, not an empty graph. `title` is used by the
requirement graph; if it is missing, the graph falls back to the id and emits a
`missing-title` diagnostic.

Important sections are:

- `## Problem`
- `## Requirements`
- `## Scenarios`
- `## Dependencies`
- `## Child Requirements`
- `## Source Trace`
- `## Open Questions`

Open questions are blocking input, not commentary. A requirement with real open
questions becomes a blocked work unit.

## Graph And Work Units

`RequirementGraph` collects requirement nodes, dependencies, children,
scenarios, source trace, tags, diagnostics, and parse errors. Validation checks
duplicate ids, dangling dependency or child ids, missing scenarios on leaf
requirements, blocked open questions, and graph cycles.

`WorkUnitSet` classifies each requirement:

- `leaf_full`: has scenarios and no children; ready for implementation.
- `parent_scenario`: has scenarios and children; ready, but still structural.
- `grouping_only`: has children but no scenarios; informational only.
- `blocked_questions`: has open questions; blocked.
- `missing_scenarios`: no children and no scenarios; blocked.

## Plan DAG

`RequirementPlan` combines requirement graph data, work units, and spec
coverage from `satisfies: [REQ-*]` frontmatter. It emits:

- requirement nodes with status and mode
- first-class work-unit and spec nodes
- dependency, child, work-unit, satisfies, and spec-dependency edges
- deterministic execution batches for ready requirements
- coverage records from requirements to satisfying specs
- diagnostics and parse errors

The plan gate treats ready requirements without satisfying specs as hard
coverage failures. It ignores `specs/roadmap/` and archive directories when it
builds the active spec set, and superseded requirements remain informational
history rather than uncovered work. This is the bridge from KLL truth to
executable task contracts.

## Evidence And QA

Lifecycle trace records use explicit requirement-to-scenario ownership. A spec
with one `satisfies` id assigns all authored scenarios to that requirement. A
multi-requirement spec must match KLL scenario names without ambiguity; the
ledger never emits a requirement-by-scenario Cartesian product. Replay returns
all scenarios from the latest run.

An explicit `risk: A|B|C` enables lifecycle QA evidence gates. Class C requires
a passing lifecycle, Class B also requires trace evidence, and Class A also
requires targeted test evidence plus explicit adversarial-review assertion.
