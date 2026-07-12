# External Consumers (Record, Non-Normative)

agent-spec's integration boundary is one-way (ADR-001,
`knowledge/decisions/adr-001-orchestrator-neutral-core.md`): orchestrator
adapters depend on agent-spec's stable CLI/JSON/digest surfaces; agent-spec
depends on no orchestrator. This file records what known external consumers
plan to build against those surfaces. Nothing here is a task, a dependency,
or a promise of this repository — if none of it ever happens, agent-spec's
roadmap and 1.0 are unaffected.

## First consumer: an orchestrated software factory

A software-factory stack (chat cockpit for requirement conversations, an
authority service owning approved requirement versions and freezes, a
provenance/certification layer) intends to use agent-spec as its spec-side
compiler. Work that lives on that side, in their repositories:

- An authority-contract ADR ratifying system roles; agent-spec appears there
  only as a pluggable deterministic compiler.
- A requirements-to-spec adapter invoking the public agent-spec CLI
  (`requirements import/transition/plan/compile/traceability/verify-run`)
  and assembling consumer-shaped bundles from `--layout arc-v1` output.
- Approval storage, approval envelopes bound to agent-spec's reported
  digests, human identity/permission mapping, and the approval UI.

## What they may rely on

The surfaces those consumers build against are the ones agent-spec already
promises to stabilize (README "Stability and the road to 1.0") plus the
staged integration arc: machine-readable governance actions, the
requirement-traceability projection, v2 provenance manifests with replay,
and the compile bundle layouts (`agent-spec-v1`, `arc-v1`).

## What they may not expect

Approval identity, authority, workflow state, or policy semantics inside
agent-spec — mechanically forbidden by the integration-arc contracts and
recorded as a README non-goal.
