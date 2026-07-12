---
kind: decision
id: ADR-001
title: "Orchestrator-Neutral Compiler Core"
status: accepted
tags: [intent-compiler, boundary, integration]
---

# Orchestrator-Neutral Compiler Core

## Context

agent-spec serves as the spec side of software-factory orchestrators: humans
discuss requirements in a chat cockpit, an authority system records approved
requirement versions, and the intent compiler turns them into deterministic,
verifiable artifacts. Integration reviews (2026-07-12) surfaced the coupling
question: should stage-approval protocols, approval envelopes, and
factory-shaped bundle commands live inside the compiler so one orchestrator
can consume it directly?

## Decision

agent-spec provides deterministic compilation artifacts, stable machine
formats, digests, and replay. Any orchestrator may insert human approval
between compiler commands — but approval identity, authority, and workflow
never enter the compiler core.

Concretely:

- The dependency is one-way: orchestrator adapters depend on agent-spec's
  stable CLI/JSON/digest surfaces; agent-spec depends on no orchestrator.
- Core outputs and schemas are mechanically forbidden from carrying `actor`,
  `authority`, `approval`, or `policy` fields; external systems bind
  approvals to reported digests in their own stores.
- The compiler pipeline stays a set of independently invokable commands so
  approval checkpoints are an orchestrator concern, not a compiler mode.
- External-format compatibility is an edge projection: the default layout is
  provider-neutral (`agent-spec-v1`); reference-compiler compatibility is the
  named `arc-v1` layout, like SARIF or SCIP support — usable by that entire
  ecosystem, owned by no single orchestrator.

## Consequences

Good, because the 1.0 stability freeze contains only self-owned concepts:
orchestrator architectures can change without touching the compiler, and any
factory — not one — can consume the same surfaces.

Good, because approval semantics stay where identity can actually be
attested; a CLI cannot prove who approved, so it reports facts and digests
instead.

Bad, because there is no turnkey integration in this repository: each
orchestrator must build its own adapter and approval store, and cross-system
end-to-end acceptance lives outside this repo's CI.

Bad, because reference-compiler compatibility is pinned at schema/consumer
level by parity goldens, not at byte level with the reference renderer.

## Alternatives Considered

- Stage Approval Protocol in the compiler core (configurable approval
  checkpoints, approval-envelope references in manifests) — rejected because
  it imports another system's domain concepts into the 1.0 freeze and couples
  the compiler to an architecture that is itself still a proposal.
- Orchestrator-specific bundle command in core (`--profile openfab-v1` /
  `--arc-bundle`) — rejected because a provider name in the core CLI creates
  two-way vocabulary coupling; the neutral bundle plus a named edge layout
  serves the same consumers.
- Dual canonical-ownership profiles (hand-written Markdown canonical in
  native mode, `requirements.yaml` canonical in factory mode) — rejected
  because no file format should be promoted to cross-system truth; the
  orchestrator's approved snapshot is the enterprise authority, the KLL
  Markdown is the compiler's internal IR, and YAML stays an exchange
  projection.

## Source Trace

- decision ratified: orchestrator-neutral compiler integration review
  2026-07-12 (user decision; cross-review by the live reference-architecture
  session drove the protocol-vs-decoupling correction)
- mechanical enforcement: specs/roadmap/task-compiler-machine-surface.spec.md
  (Forbidden), specs/roadmap/task-provenance-run-hardening.spec.md
  (Forbidden), specs/roadmap/task-reference-compiler-parity.spec.md
  (vocabulary check scenario)
- governed requirements: REQ-COMPILER-MACHINE-SURFACE,
  REQ-PROVENANCE-RUN-HARDENING, REQ-REFERENCE-COMPILER-PARITY
