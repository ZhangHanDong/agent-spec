---
kind: proposal
id: LEP-003
title: "Clause Coverage"
status: accepted
liveness: n/a
tags: [knowledge, lint, coverage, governance, liveness]
---

# Clause Coverage

## Context

A requirement's MUST clauses are the obligations the system promises to meet.
`liveness=Honored` is the toolchain's claim that those obligations hold. Today
nothing connects the two: a requirement reaches Honored when its satisfying
contract passes, and the contract passes when its scenarios pass — but no rule
requires those scenarios to correspond to the clauses.

The gap is measurable in this repository right now: **553 MUST clauses across
`knowledge/requirements/`, and 255 scenarios.** More than half the clauses
cannot have a scenario of their own even in principle. Scenarios never name a
clause either — `req-decision-point-emission.md` declares 12 clause ids, and
its `## Scenarios` section contains zero `REQ-` references. The two halves of
every requirement document are written side by side and linked by nothing.

The existing check is weaker than it reads. `requirement-must-needs-scenario`
fires only when a document with MUST clauses has **no scenarios at all**; one
scenario satisfies it no matter how many obligations sit above.

This is not hypothetical. During the 2026-08-07 review of the LEP-002 work,
`REQ-DECISION-POINT-EMISSION-MECHANICAL-MOAT` was found to have been
contradicted by the implementation — the clause said `resolve-ai` applies only
skip-scenario decisions "MUST remain unchanged", while the code had been
extended to let a human answer settle uncertain and pending_review as well.
The requirement still traced **Honored** throughout, because no scenario bound
that half of the clause. The contract was green, the gate was green, and the
governing obligation had been silently overturned. The clause-to-scenario
ratio says that failure mode is available to roughly half the corpus.

A precedent already exists one layer down. Task contracts have
`decision-coverage` and `observable-decision-coverage`, which extract keywords
from each `## Decisions` entry and require a matching scenario. The BDD dialect
also has explicit attribution: `Rule: <id> — <name>` groups scenarios under a
named rule, with `bdd-rule-id` enforcing id validity. Requirements have
neither.

## Motivation

Every mechanism this pipeline added in 1.3.0 — routing, scaffolds, integrity
gates, structured decision points — exists to make governance mechanical rather
than remembered. Clause coverage is the one place where the mechanism stops at
the door of the thing being governed: we check that a requirement *has* a
contract, that the contract's scenarios pass, that ids resolve and links point
both ways, and then we take the clause text itself entirely on faith.

## Goals

- A MUST clause without covering evidence is reported, not assumed satisfied.
- Coverage is attributable: a reader can see which scenario answers which
  clause, rather than inferring it from proximity.
- The existing corpus can adopt this incrementally, the way `orphan-spec`
  phased in, rather than turning 553 clauses red at once.
- `Honored` gains a defensible meaning: the obligations, not merely a contract,
  were exercised.

## Non-Goals

- One scenario per clause as a hard shape. A negative path and a happy path may
  both serve one clause; one scenario may exercise several.
- Semantic proof that a scenario actually verifies its clause. This buys
  attribution and presence, not correctness — a mis-attributed scenario is
  still possible and remains a review concern.
- Extending coverage checking to SHOULD or MAY in this proposal. MUST is what
  `Honored` claims about.
- Retrofitting the 553 existing clauses as part of the enabling change.

## Decision

Add clause coverage as a first-class relationship between a requirement's
`## Requirements` and `## Scenarios` sections, checked by a rule that phases in
by severity, and surfaced wherever `Honored` is reported.

**Attribution.** A scenario declares which clause ids it exercises. The
mechanism should follow the BDD dialect already in the codebase rather than
invent a second one — `Rule: <clause-id>` grouping scenarios is the closest
existing shape, and `bdd-rule-id` already validates ids of that form.

**The rule.** `clause-uncovered` reports a MUST clause with no attributed
scenario, naming the clause id and the document. Severity phases in exactly as
`orphan-spec` did: introduced at Info with a shrink-only baseline recording
today's uncovered clauses, rising in later releases once the baseline shrinks.

**Reporting.** Where `trace` reports `Honored`, it should be able to say how
much of the requirement that verdict rests on — a requirement whose contract
passes but whose clauses are half-uncovered is a different state from one whose
clauses are fully attributed, and today both print the same word.

**Fallback.** Where attribution is absent, keyword matching in the style of
`decision-coverage` can offer a weaker signal, but an inferred match must never
be presented as an explicit one.

## Compatibility

- CLI and public API: additive. A new diagnostic joins `lint-knowledge`; no
  existing rule changes meaning. If `trace` grows coverage reporting, its text
  output changes and its JSON gains fields.
- File formats: additive. Requirement documents without attribution keep
  parsing; attribution is new optional structure inside `## Scenarios`.
- Existing specs and KLL artifacts: unaffected until the baseline shrinks and
  severity rises.

## Migration Plan

The enabling change ships the rule at Info with every currently-uncovered
clause baselined, so no existing document breaks. Coverage is then added
document by document, and the baseline may only shrink — the same contract
`orphan-spec` made. A release that raises severity must state the baseline size
it expects to be at.

## Security Considerations

None beyond existing knowledge-layer trust boundaries: the rule reads
documents already parsed, adds no execution path, and emits no new file
content.

## Privacy Considerations

Diagnostics name clause ids and document paths, which are already present in
lint output. Clause text is quoted no more than existing requirement rules
quote it.

## Risks and Assumptions

### Assumptions

- Clause ids are stable enough to attribute against. Invalidated if documents
  routinely renumber clauses, which would make attribution churn.
- Requirement authors will attribute rather than route around the rule.
  Invalidated if the corpus grows a habit of one catch-all scenario claiming
  every clause id — coverage would read as satisfied while proving nothing.

### Risks

- Attribution theatre: scenarios labelled with clause ids they do not actually
  exercise. Mitigation: attribution is checkable, correctness is not, so this
  belongs in review — but a mis-attributed scenario is at least visible, where
  today the clause is invisible.
- 553 baselined clauses is a large debt to advertise. If the baseline never
  shrinks, the rule becomes decoration and the Info level its permanent home —
  the same failure `orphan-spec` risks, now twice.
- Splitting compound clauses to make them coverable interacts with
  `requirement-compound-clause`, which already asks for splits; authors may
  face two rules pulling on the same sentence.

## Consequences

Good, because `Honored` stops being a claim about contract mechanics and starts
being a claim about obligations — the word finally means what readers already
assume it means.

Good, because a clause that gets silently contradicted becomes visible: the
MECHANICAL-MOAT failure would have surfaced as an uncovered clause long before
a human happened to review the diff.

Good, because it completes the symmetry with task contracts, which have had
decision coverage all along; requirements have been the weaker layer without
anyone noticing.

Bad, because 553 baselined clauses is a debt that will be tempting to leave
alone, and a permanently-Info rule teaches that the pipeline's own diagnostics
are optional.

Bad, because attribution adds authoring weight to every requirement document,
and the payoff is invisible on the day it is written.

Bad, because coverage can be gamed with a catch-all scenario, so the rule
raises the floor without raising the ceiling.

## Alternatives Considered

- Strengthen `requirement-must-needs-scenario` to require scenario count ≥
  clause count — rejected: cheap to implement and cheap to game, since it
  never asks *which* clause a scenario serves, and it wrongly forbids one
  scenario legitimately covering two clauses.
- Keyword matching only, in the style of `decision-coverage` — rejected as the
  primary mechanism: it produces plausible-looking coverage with no author
  intent behind it, and this proposal exists because a plausible-looking green
  is exactly what hid the MECHANICAL-MOAT contradiction. Retained as a
  clearly-labelled fallback.
- Push coverage down to the contract instead, requiring each task spec scenario
  to name a clause — rejected for this round: the contract already has its own
  coverage rules, and the requirement document is where the obligation is
  written and where a reader looks for it.
- Do nothing and rely on review — rejected: this proposal was written because
  review is what caught it, once, by luck, after the fact.

## Prior Art

- `decision-coverage` / `observable-decision-coverage` in `spec_lint`: the same
  question one layer down, answered with keyword extraction.
- The BDD `Rule: <id> — <name>` dialect and `bdd-rule-id`: existing explicit
  attribution with id validation.
- `orphan-spec` (ADR-002): the phased severity plus shrink-only baseline this
  proposal copies wholesale.
- ISO/IEC/IEEE 29148 traceability, where each requirement carries verification
  method and coverage rather than existing as unverified prose.

## Unresolved Questions

- Attribution syntax: reuse `Rule: <clause-id>` grouping, add a per-scenario
  tag line, or extend frontmatter with a clause-to-scenario map?
- Should a clause with an attributed scenario that is itself `skip` count as
  covered, or does coverage require the scenario to actually run?
- Does `trace` report coverage alongside `Honored`, or does that belong in
  `requirements status` where the three axes already live?
- Should the keyword fallback ship at all, given that its weakness is the
  problem being solved — or is a labelled weak signal better than silence?
- Which release raises severity, and what baseline size is the precondition?

## Produces: ADR-004

Accepted 2026-08-08. Four of the five unresolved questions above were put to
the human as structured multiple-choice and answered: reuse the BDD `Rule:`
grouping for attribution, no keyword fallback, a skipped scenario does not
count as coverage. The produced decision records those rulings and settles the
remaining question about where coverage is reported.
