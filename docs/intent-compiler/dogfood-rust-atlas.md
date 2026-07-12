# Cold-Start Dogfood: rust-atlas Through the Intent Compiler

Date: 2026-07-12. This is the repository's cold-start proof for the intent
compiler: the real rust-atlas Phase 1 requirement was expressed as a raw YAML
tree and driven through the entire pipeline — import, gates, work units, plan,
draft contract — with every gate deterministic and every step reproducible.

## Ownership: the flow this exercise settled

The canonical flow for requirements born from conversation is:

1. Humans and AI talk requirements; the confirmed natural-language artifact is
   human-owned. When the human writes structured clauses directly, that
   artifact IS `knowledge/requirements/req-*.md` — hand-authored IR, the main
   practice in this repository.
2. The intent compiler's downstream stages (`lint-knowledge`, `graph`,
   `work-units`, `plan`, `draft-specs`) consume it.
3. YAML is a **compatibility projection**: an alternative *input* dialect when
   requirements arrive already structured from external tools (the shipped
   frontend), and a future *export* target from confirmed requirements.

During this exercise the ownership was temporarily inverted — the confirmed
requirement was regenerated from a YAML source — which was wrong for a
human-confirmed requirement and was reverted after review:
`knowledge/requirements/req-rust-atlas.md` is the human-owned canonical
document (no provenance marker, so the frontend's overwrite protection now
guards it), and the temporary `docs/rust-atlas-requirements.yaml` was removed.
The exercise itself remains fully reproducible: the same tree shape lives in
`fixtures/requirements-yaml/requirements.yaml`, and the run below can be
replayed from any YAML source.

## The run (as exercised)

```bash
agent-spec requirements import --from <requirements>.yaml --out knowledge/requirements
agent-spec requirements import --from <requirements>.yaml --out knowledge/requirements --check
agent-spec lint-knowledge --gate
agent-spec requirements graph --knowledge knowledge --format json --gate
agent-spec requirements work-units --knowledge knowledge --out .agent-spec/work_units.json
agent-spec requirements plan --knowledge knowledge --specs specs --format json --gate
agent-spec requirements draft-specs --knowledge knowledge --out docs/intent-compiler/dogfood
agent-spec trace REQ-RUST-ATLAS --code .
```

All gates exited 0 during the exercise. `trace REQ-RUST-ATLAS` reports
`Unproven` with the staged Phase 1 contract as `[Skip]` — the correct
pre-implementation state.

## What the dogfood caught (and fixed)

The run was not a rubber stamp; it exposed two real frontend gaps, both fixed
contract-first with TDD in `specs/task-intent-compiler-yaml-frontend.spec.md`:

1. **No home for human acceptance.** Generated docs hardcoded
   `status: proposed`, and `work-units` correctly demoted the requirement to
   `informational` — the governance gate refused to schedule unaccepted work.
   Fix: the YAML dialect gained an optional FOLDER `status: proposed|accepted`
   field; acceptance lives in the human-owned source file.
2. **Scenarios in the wrong section.** The renderer inlined `Scenario:` blocks
   under `## Requirements`, where the requirement graph does not read them, so
   the work unit was `blocked: missing_scenarios`. Fix: scenarios render into a
   dedicated `## Scenarios` section.

After both fixes: `WU-REQ-RUST-ATLAS` is `ready / leaf_full`, and
`draft-specs` generated `task-req-rust-atlas-rust-atlas-code-graph.spec.md`
(kept in this directory as evidence; it parses with 22 clause-derived
decisions and pending test selectors).

3. **Acceptance means scheduling, and the plan gate enforces it.** With the
   folder `accepted`, `requirements plan --gate` failed with
   `requirement-uncovered: REQ-RUST-ATLAS is ready but has no satisfying spec`
   — the atlas contract is still staged in `specs/roadmap/`, and the plan gate
   only recognizes active contracts. That is the roadmap promotion rule,
   mechanically enforced: accepting a requirement obliges promoting its
   contract to `specs/` (where `guard` then demands green tests). Since Phase 1
   is not starting yet, the canonical requirement stays `status: proposed`;
   Phase 1 kickoff = set it to `accepted` and promote the roadmap contract to
   `specs/` in the same change. The accepted→ready→draft path was exercised
   during this run and is covered by the frontend's fixture corpus.

## Draft vs hand-authored contract

The generated draft is a review artifact: correct `satisfies: [REQ-RUST-ATLAS]`
edge, Intent from the Problem statement, one decision per clause, scenario
skeletons with `pending_*` selectors, and placeholder boundaries (`src/**`).
The hand-authored `specs/roadmap/task-rust-atlas-code-graph.spec.md` remains
the executable contract — it adds what drafts cannot invent: real boundaries,
14 bound test selectors, Rule grouping, and resolved design questions. That
division of labor is by design: the compiler produces the skeleton and the
traceability, humans produce the judgment.

## Conclusion for the 1.0 checklist

The cold-start precondition is met: raw intent → IR → gates → ready work unit
→ draft contract, exercised on a real feature in this repository and
reproducible from the frontend's fixture corpus
(`fixtures/requirements-yaml/requirements.yaml`). The exercise also settled the
ownership rule: human-confirmed requirements are hand-owned canonical IR, and
YAML is a side-entrance input dialect (plus a future export target), never the
source of truth for a confirmed requirement. The remaining atlas work
(implementing Phase 1 until `trace REQ-RUST-ATLAS` reports `Honored`) is
scheduled work, not a pipeline gap.
