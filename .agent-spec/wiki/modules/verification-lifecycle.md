---
title: "Verification Lifecycle"
type: module
source_files:
  - src/spec_verify/mod.rs
  - src/spec_verify/boundaries.rs
  - src/spec_verify/test_verifier.rs
  - src/main.rs
  - AGENTS.md
  - skills/agent-spec-tool-first/SKILL.md
tags:
  - lifecycle
  - verification
  - contract
---

# Verification Lifecycle

## Role

The lifecycle pipeline is the main quality gate for a Task Contract. It combines
spec linting with mechanical verification and returns scenario verdicts that an
agent can act on without changing the contract.

The core runtime verifier model is small:

- `VerificationContext` carries code paths, changed paths, AI mode, and the
  resolved spec.
- Each verifier returns scenario results.
- `run_verification` keeps the first result for each scenario.
- Mechanical verifiers stamp computational provenance; the AI verifier stamps
  inferential provenance.
- Any scenario not covered by a verifier becomes `skip`, not `pass`.

## Mechanical Verifiers

The boundary verifier runs only when explicit changed paths are available. It
collects allowed and forbidden path boundaries from the task spec and compares
them with normalized changed files. A changed file outside the allowed set, or
inside a forbidden boundary, fails the boundary scenario.

The test verifier looks for explicit scenario `Test:` selectors first, then
falls back to legacy comment bindings. It runs Cargo tests with the selected
filter and converts test output to scenario evidence. Successful tests produce
`pass`, unless the scenario is human-review mode, which becomes
`pending_review`.

## Agent Loop

The intended loop is:

```bash
agent-spec contract specs/task.spec.md
agent-spec lifecycle specs/task.spec.md --code . --format json
```

When lifecycle fails, fix the implementation or tests that are bound to the
contract. Do not weaken the spec to make verification pass. `skip`, `fail`,
`uncertain`, and `pending_review` are distinct states and should remain distinct
in reports, trace records, and failure explanation.

Contracts with Atlas symbols require a rebuilt graph whose `atlas status`
identity and layer states support authority. A borrowed worktree graph or stale
available semantic layer blocks the definitive symbol evidence used by
lifecycle. `atlas affected` may locate impacted code, but lifecycle binds only
the explicit selectors declared by scenarios; filenames never become inferred
test evidence.
Atlas runtime-boundary candidates are also excluded from lifecycle authority:
their query-hint classification can direct investigation but cannot prove a
Contract symbol or scenario.

An `atlas context` receipt similarly proves only bounded selection over a
pinned graph. It can expose stale source and missing evidence for investigation,
but cannot become a pass verdict, infer a selector, or satisfy a binding. The
Contract and lifecycle verifier remain authoritative.

The F1 provider lifecycle selectors invoke only the checked-in, explicitly
enabled local fixture. A passing conformance receipt verifies transport,
projection, freshness, cancellation, and publication behavior; it cannot prove
production language accuracy or auto-enable a provider.

D4 worker receipts preserve typed execution state and can support selectors
that explicitly test concurrency behavior. They do not become pass verdicts by
themselves. The D4 contract binds 19 scenarios to computational tests,
including default/direct parity, overload, cancellation, transport liveness,
worktree isolation, and the checked-in matrix gate.

## Trace Implication

Lifecycle results become useful only when they remain tied to:

- requirement ids from `satisfies`
- work units from the requirement plan
- spec scenarios
- test selectors and test evidence
- changed code paths or declared test targets
- worktree, branch, and VCS reference when available

That chain is what lets agent-spec answer which requirement, work unit, scenario,
test, code path, worktree, and commit failed.

## Maintenance

Atlas D2 was reviewed here; lifecycle consumes one pinned graph generation and
does not treat generation publication as scenario evidence by itself.

Atlas D3 was reviewed here; a reader lease protects that pinned generation,
while watcher/daemon state remains diagnostic rather than scenario evidence.

Atlas B5 was reviewed here; context compilation can prepare Agent working input
but does not change lifecycle verdict semantics or trace provenance.

Atlas D4 was reviewed here; concurrent scheduling changes execution mechanics,
not lifecycle verdict semantics or evidence provenance.

Atlas E1's own Contract lifecycle verifies the harness selectors. Real Agent
and serving gate receipts remain separate adoption evidence and cannot replace
scenario test evidence.

Reviewed for the 1.2 release: release metadata does not weaken any Atlas
selector. Provider, Agent A/B, and concurrent-serving contracts must still pass
against a fresh worktree-bound graph before publication.
