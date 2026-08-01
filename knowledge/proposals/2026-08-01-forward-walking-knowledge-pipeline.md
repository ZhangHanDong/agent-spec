---
kind: proposal
id: LEP-001
title: "Forward-Walking Knowledge Pipeline"
status: accepted
liveness: n/a
tags: [knowledge, governance, lint, scaffold, skills]
---

# Forward-Walking Knowledge Pipeline

## Context

The intended authoring flow is a forward walk through the knowledge layers:

```
knowledge/proposals/    LEP-NNN   the debate: should we do this, and why
    │  ## Produces: ADR-NNN            (edge minted on acceptance)
knowledge/decisions/    ADR-NNN   the ruling: what we decided, alternatives
    │  Source Trace back-links
knowledge/requirements/ REQ-*     the obligations: MUST clauses + scenarios
    │  satisfies: [REQ-*]
specs/                  task-*    the contract: decisions + scenarios + tests
```

In practice authors — human and agent — enter at the bottom and retrofit
upward, or never retrofit at all. A field incident (2026-08-01, agent-chat
workspace) showed the full failure chain: an agent asked to land a *proposal*
with agent-spec, with the authoring skill loaded, produced a task contract
directly in `specs/`, scored 100 on `lint`, and skipped the proposal,
decision, and requirement layers entirely. Nothing in the toolchain routed it
to `knowledge/proposals/` and nothing stopped the jump.

The audit of this repository shows the same forces acting locally:

- 37 of 74 specs under `specs/` declare no `satisfies:` at all, and no lint
  rule flags an orphan contract.
- Knowledge-graph integrity is split across three validators with disjoint
  vocabularies — `lint_corpus` (`spec_knowledge/governance.rs`),
  `build_requirement_graph` (`requirement_graph.rs`),
  `build_requirement_plan` (`requirement_plan.rs`) — and only the first runs
  under `lint-knowledge --gate`. `{spec} satisfies missing requirement` exists
  as a check but sits outside the gate path.
- `knowledge/proposals/proposal-template.md` uses id scheme
  `PROP-YYYY-MM-DD-SLUG`; the `init --workspace` scaffold uses `LEP-NNN`; the
  prefix registry the KLL design says lives in `standards/operational` is not
  there.
- The same template's sections (Summary/Motivation/Goals/…) do not include
  `Context`, `Decision`, or `Consequences`, so any proposal copied from it
  fails `proposal-required-section` three times. Templates are exempt from
  scanning (`governance.rs:121`), so the trap is invisible until an author
  falls into it.
- Frontmatter parse errors do not enumerate valid values (`unknown status
  '{other}'`, `parser.rs:169`), while transition errors do
  (`transitions.rs:50`); an author who guesses wrong must read source to
  recover.
- Of the five bundled skills, the only knowledge-layer-facing one
  (`agent-spec-intent-compiler`) carries no version header and does not
  mention proposals, decisions, or the ADR/REQ relationship. There is no
  `agent-spec knowledge` namespace and no per-artifact scaffold command; the
  knowledge surface is scattered across `lint-knowledge`, `trace`,
  `requirements`, and `mcp`.

When several independent authors fall into the same hole, the hole belongs to
the tool. The map (skills, templates), the signposts (error messages), and
the gates (lints) all currently permit — and sometimes teach — the layer jump.

## Decision

Make the forward walk the path of least resistance, in four workstreams,
ordered so each unlocks the next:

**W1 — One prefix registry.** Ratify the artifact id registry (`LEP-` for
proposals, `ADR-` for decisions, `REQ-` for requirements, task ids for specs)
in `knowledge/standards/operational/`, fix `proposal-template.md` to match
both the registry and the `proposal-required-section` lint, and align the
scaffold templates. Every later routing table and lint keys off this
registry.

**W2 — Pipeline-integrity gates.** Fold the graph and plan validators into
the `lint-knowledge --gate` path (or a single `knowledge gate` entry point),
then add three rules:

- `orphan-spec` — a task spec with no `satisfies:` while
  `knowledge/requirements/` is non-empty. Phased severity (Info → Warning →
  Error with a recorded baseline) so the 37 existing orphans do not break the
  build on day one.
- `dependency-kind-mismatch` — an `ADR-*`/`LEP-*` id inside a requirement's
  `## Dependencies`; the diagnostic names the fix ("move to `## Source
  Trace`").
- `produces-link-integrity` — an accepted proposal must have a resolvable
  `## Produces` target, and the produced decision's Source Trace must link
  back.

**W3 — Forward-walking scaffold and routing.** Add
`agent-spec knowledge new proposal|decision|requirement <id>` to scaffold a
lint-clean artifact with valid enum values pre-filled, and put one routing
table ("what you hold → where it goes → id prefix") at the top of the
authoring and intent-compiler skills. Each stage's template ends with exactly
one exit: a proposal exits via `## Produces`, a decision exits via a governed
requirement, a requirement exits via `requirements draft-specs`. Skills gain
a hard gate stated as a bright line: no task contract without `satisfies:`
when a requirements corpus exists.

**W4 — Guidance that cannot silently rot.** Every bundled skill carries a
version header with a `Tracks:` field checked in CI against the crate
version; frontmatter parse errors enumerate their valid value sets; skill
changes are tested the way superpowers tests process docs — baseline the
failure without the guidance, counter the recorded rationalizations, and keep
an adversarial routing case ("skip the formalities, just write the spec")
green.

## Consequences

Good, because the layer jump stops being a judgment call: the scaffold makes
the right next artifact cheaper than the wrong one, and the gate catches the
jump mechanically for humans and agents alike.

Good, because merging the three validators gives the corpus one diagnostic
vocabulary and puts existing dangling-reference checks onto the gate path
where they were designed to act.

Good, because the registry plus back-link lint turns the LEP → ADR → REQ →
spec chain into data that `trace` can follow end-to-end, instead of a
convention that lives in a design doc.

Bad, because phased severities mean a window in which orphan specs still
pass; the baseline must shrink release by release or the phase-in becomes the
status quo.

Bad, because a `knowledge` namespace and merged gate touch `main.rs` command
wiring that is already the largest file in the repo; the work should land as
several contracts, not one.

Bad, because CI-checked skill version headers add release friction: shipping
a CLI change now fails CI until the affected skill's `Tracks:` line moves.

## Alternatives Considered

- Fix only the skills and docs, no tooling change — rejected: the field
  incident happened *with* a current authoring skill loaded; guidance without
  gates decays, and the 37 local orphans were produced under this regime.
- Hard-error on orphan specs immediately — rejected: half of `specs/` would
  fail `guard` overnight, punishing exactly the repositories that adopted
  specs earliest; phased severity with a baseline reaches the same end state
  without the flag day.
- An interactive wizard (`agent-spec init --interactive` walking all four
  layers) — rejected: the primary authors are agents in non-interactive
  sessions; lintable artifacts and scaffolds compose with any harness, a
  wizard composes with none.
- Adopt the superpowers skill suite wholesale as the routing layer without
  CLI changes — rejected: skills are per-harness and per-install (this repo
  already shows a 5-bundled/2-installed drift); the CLI gate is the only
  surface every consumer shares. Its *mechanisms* — trigger-only routing,
  bright-line gates, phased enforcement, tested guidance — are adopted in
  W2–W4; its distribution model is not.

## Prior Art

- superpowers (obra/superpowers): iron-law bright lines, rationalization
  tables harvested from observed failures, artifact-as-contract phase
  handoffs with a single exit, "no placeholders" word lists enforced
  mechanically, and TDD applied to process documentation.
- Lore LEP process: proposal → decision handoff this repo's `## Produces`
  edge already encodes (`spec_knowledge/proposal.rs`).
- MADR: the Context/Decision/Consequences shape the decision and proposal
  lints already require.
- ISO/IEC/IEEE 29148 and EARS: already the basis of the requirement clause
  lints; W2 extends the same philosophy — machine-checkable authoring rules —
  from clause quality to pipeline integrity.

## Unresolved Questions

- Prefix ratification: `LEP-NNN` (scaffold, KLL design) vs
  `PROP-YYYY-MM-DD-SLUG` (current template). This proposal recommends
  `LEP-NNN` and uses it; the produced decision must ratify or overturn.
- Orphan-spec phase-in schedule: which release turns Warning into Error, and
  where the baseline file lives.
- Whether the merged gate ships as `lint-knowledge --gate` growing graph/plan
  checks, or as a new `knowledge` namespace absorbing `lint-knowledge`,
  `trace`, and the gate — the latter is cleaner and the larger change.

## Source Trace

- field incident: agent-chat workspace session post-mortem, 2026-08-01
  (agent produced an orphan contract with the authoring skill loaded; four
  pre-existing requirement docs in that workspace carry ADR ids in
  `## Dependencies`)
- repository audit: 2026-08-01 sweep of `src/spec_knowledge/`,
  `src/spec_lint/`, `specs/` (37/74 orphans), bundled `skills/`
- design lineage: docs/superpowers/specs/2026-06-23-knowledge-liveness-layer-design.md
  (§6.3 proposals, §7 satisfies edge, prefix registry note)

## Produces: ADR-002

- Accepted 2026-08-01; the produced decision ratifies the prefix registry and
  the four workstreams, and governs one requirement document per workstream.
