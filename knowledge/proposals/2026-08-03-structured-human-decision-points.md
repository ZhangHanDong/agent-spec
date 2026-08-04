---
kind: proposal
id: LEP-002
title: "Structured Human Decision Points"
status: accepted
liveness: n/a
tags: [knowledge, questions, interop, agent-ux, verification]
---

# Structured Human Decision Points

This proposal follows LEP-001's forward walk: it is the debate, and the
decision it produces will govern requirements of its own.

## Context

The pipeline stops for a human at three places, and at all three the stop is
free-form prose that every agent re-invents:

**1. Reverse interview (PRD/issue → REQ).** `requirements questions` already
emits a typed `ClarificationQuestion` carrying `id`, `target_id`,
`diagnostic_code`, `blocking`, `prompt`, `source` — **and `options:
Vec<String>`**. Both construction sites in `src/spec_knowledge/questions.rs`
hard-code `options: Vec::new()`. On this repository's own corpus that is 119
questions, none with a single candidate answer. Meanwhile the
`agent-spec-intent-compiler` skill instructs the agent to "Offer 2 or 3
concrete options only when the source supports them" and to "Preserve a
free-form answer path". The contract is written in prose for the model to
honor, while the field built to carry it ships empty.

**2. Proposal and decision choices.** A proposal's `## Unresolved Questions`
and a decision's `## Alternatives Considered` are the moments where a human
picks. LEP-001 shows the shape: "`LEP-NNN` (scaffold, KLL design) vs
`PROP-YYYY-MM-DD-SLUG` (current template)" is a single-choice question with
two concrete options and a recommendation — recorded as a sentence, resolved
by an agent writing prose into the produced ADR. Nothing machine-readable ever
represented the choice, so nothing could present it as a choice.

**3. Contract acceptance a machine cannot judge.** `resolve-ai` merges an
external decisions JSON (`AiDecision { model, confidence, verdict,
reasoning }`) into a verification report, replacing `uncertain` and
`pending_review` verdicts. There is no reverse direction: nothing emits "here
are the scenarios awaiting a judgment call, with the evidence." The human or
reviewing agent must hand-author the decisions file from a text report.

Modern agent harnesses render exactly this interaction natively — Claude Code's
`AskUserQuestion` presents 2–4 labelled options with descriptions, optional
multi-select, and an always-present free-text escape; Codex offers an
equivalent. The pipeline's decision points are already question-shaped. They
are simply not emitted in a shape a harness can render, so each agent
improvises the question, and the quality of a governance decision depends on
how well the model happened to phrase it.

## Motivation

A decision point that is not machine-readable cannot be routed, cannot be
audited, and cannot be presented consistently. This is the failure class
LEP-001 addressed for artifacts: the pipeline knew what should happen but only
said so in prose. Here the pipeline knows a human must choose, and only says so
in prose.

## Goals

- One question envelope, shared by all three emission points, that any harness
  renders without interpreting free text.
- Emitted options are grounded — derived from the diagnostic, the document, or
  the evidence — never invented at render time.
- A defined write-back path per question kind, so an answer changes the
  artifact rather than only the conversation.
- Shape constraints that keep the envelope renderable as-is by current
  harnesses (bounded option count, mandatory free-form path).

## Non-Goals

- Prescribing any harness's UI, or depending on one. The CLI emits and ingests;
  rendering stays outside (ADR-001, orchestrator-neutral core).
- Interactive prompting inside the CLI. No TTY questions, no wizard.
- Replacing human judgment with generated options. Options are candidates; the
  free-form path is never removed.
- Touching `## Open Questions` in requirements, which already blocks work-unit
  generation and needs no interaction change.

## Decision

Introduce a **decision-point envelope** — the existing `ClarificationQuestion`
shape, generalized and finally populated — and emit it from all three points.

**Envelope.** Existing fields keep their meaning; `options` carries structured
candidates (label, description, write-back value) rather than bare strings,
plus a `kind` discriminator naming which pipeline stage asked, and a
`multi_select` flag. The shape is constrained so it renders as-is: at most four
options, each with a short label and a one-sentence description, with a
free-form answer always implicitly available.

**Emission points.**

- `requirements questions` populates `options` from the diagnostic that raised
  the question — a compound clause offers its split candidates, a weak `Then`
  offers observable rewrites drawn from the clause's own nouns, an NFR without
  a measure offers the measure kinds the corpus already uses. When the
  diagnostic cannot ground a candidate, `options` stays empty and the question
  remains free-form; an empty list is a valid, honest answer.
- `knowledge questions <id>` extracts choices from a proposal's
  `## Unresolved Questions` and a decision's `## Alternatives Considered`,
  carrying the document's stated recommendation as such when it has one.
- `verify --emit-questions` turns `uncertain` and `pending_review` scenarios
  into judgment questions carrying scenario text, gathered evidence, and the
  verdict vocabulary as options — the inverse of `resolve-ai`, emitting what
  `resolve-ai` consumes.

**Write-back.** Each kind declares its apply path: a reverse-interview answer
edits the requirement clause or scenario it targets; a proposal or decision
answer resolves the question in the document and records the choice in the
produced artifact's `## Source Trace`; an acceptance answer becomes an
`AiDecision` entry that `resolve-ai` already ingests. Every applied answer is
attributable to a human decision, never to a model inference (the
intent-compiler rule stands).

## Compatibility

- CLI and public API: `requirements questions` gains populated `options` in
  existing JSON. Consumers reading `options` as bare strings need the
  structured form handled — this is the one breaking shape change and the
  produced decision must rule on it. `knowledge questions` and
  `verify --emit-questions` are new surfaces.
- File formats: no artifact format changes. Documents keep prose sections;
  extraction is read-only.
- Existing specs and KLL artifacts: unaffected until an answer is applied.

## Migration Plan

`options` is empty everywhere today, so populating it cannot regress a consumer
reading current output. If the structured option form is chosen over bare
strings, the questions JSON carries a version field so an old reader fails
loudly instead of silently misreading.

## Security Considerations

Questions carry source excerpts and evidence, so they surface file content to
whatever renders them. Emission respects the same trust boundaries as the rest
of the knowledge layer (no symlink escape, no reading outside declared roots),
and answers are inert data applied by explicit commands, never executed.

## Privacy Considerations

Envelopes embed source paths and excerpts. Nothing new leaves the machine, but
a rendered question is likelier to be pasted into a chat transcript than a lint
diagnostic is; excerpts stay minimal and bounded for that reason.

## Risks and Assumptions

### Assumptions

- Harnesses converge on roughly 2–4 options plus free text. Invalidated if a
  major harness requires unbounded or nested choices, which would need a richer
  envelope.
- Diagnostics carry enough structure to ground candidates. Invalidated if most
  lint messages prove prose-only, in which case option coverage stays low and
  the free-form path carries the work.

### Risks

- Generated options anchor the human toward a listed choice and away from a
  better unlisted one. Mitigation: the free-form path is mandatory, options are
  capped, and an option is emitted only when grounded.
- The envelope becomes a second diagnostic vocabulary competing with lint.
  Mitigation: questions derive from diagnostics rather than being authored
  separately, and a question always names the diagnostic that produced it.

## Consequences

Good, because the pipeline's human decision points become data: routable,
auditable, and renderable identically by any harness, instead of depending on
how each agent phrases them.

Good, because a field that has shipped empty since it was introduced finally
carries what the skill text already promises, closing a gap between documented
and actual behavior.

Good, because acceptance judgments gain a generated agenda, so the `resolve-ai`
path stops requiring a hand-authored JSON file.

Bad, because grounded option generation is real work per diagnostic class, and
a half-done version — options for the easy diagnostics only — reads as coverage
while most questions stay free-form.

Bad, because structured options invite anchoring: a human under time pressure
picks a listed option instead of thinking, precisely the failure mode LEP-001's
rationalization tables warn about.

Bad, because changing the option list shape later is a breaking change to a
published JSON surface, unlike the additive changes 1.x has favored.

## Alternatives Considered

- Leave it to the skills: instruct agents more precisely on how to ask.
  Rejected: this is the regime that produced 119 optionless questions and the
  original layer-jump incident. Prose guidance without a mechanical carrier
  decays, and LEP-001 already ruled on this pattern.
- Interactive TTY prompting in the CLI. Rejected: the primary caller is a
  non-interactive agent, and it would put orchestration inside the compiler
  core, contradicting ADR-001.
- One harness-specific output format, emitting `AskUserQuestion` payloads
  directly. Rejected: it names a vendor surface in the core CLI, the two-way
  vocabulary coupling ADR-001 rejected for orchestrator profiles. A neutral
  envelope that renders trivially into any of them is the edge projection.
- Questions as a separate authored artifact kind under `knowledge/`. Rejected:
  questions are derived state, not governed truth; persisting them creates a
  fourth thing to keep in sync with the documents that generate them.

## Prior Art

- `requirements questions` and the `ClarificationQuestion` type, which already
  chose this shape — including the unused `options` field.
- `resolve-ai` and `AiDecision`, the existing ingestion half of the acceptance
  loop.
- Claude Code `AskUserQuestion` and Codex's equivalent: bounded labelled
  options with descriptions, optional multi-select, and an always-available
  free-text answer.
- superpowers' scripted user gates, where a phase ends by presenting an exact
  enumerated menu rather than an open question.

## Unresolved Questions

- Where do options come from: mechanically derived by the CLI from diagnostic
  structure, drafted by the agent from source text and passed back for
  validation, or authored by the human in the document itself?
- Who applies an accepted answer: the CLI writing the artifact, or the agent
  editing and re-linting?
- Do acceptance judgments enter the provenance and audit chain as first-class
  evidence, or stay a pre-verification convenience?
- One `questions` namespace unifying all three emission points, or three
  stage-local surfaces (`requirements questions`, `knowledge questions`,
  `verify --emit-questions`)?

## Produces: ADR-003

Accepted 2026-08-04. All four unresolved questions above were put to the human
as structured multiple-choice — the interaction this proposal argues for — and
answered: agent-drafted options, agent write-back with CLI validation,
judgments as first-class provenance, and three stage-local surfaces. The
produced decision records those rulings, including the orchestrator-neutrality
tension the third one raises.
