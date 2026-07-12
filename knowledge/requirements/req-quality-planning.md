---
kind: requirement
id: REQ-QUALITY-PLANNING
title: "Quality Planning and Execution Bundles"
status: accepted
liveness: auto
tags: [intent-compiler, quality, execution-bundle, providers]
---

# Quality Planning and Execution Bundles

## Problem

Code intelligence and quality verification are different capabilities: Atlas
describes structure while Clippy, cargo test, and similar tools produce
diagnostics or verdicts. Nothing today resolves which tools, skills, and
policies one work unit needs, so agents assemble that context ad hoc. The
target architecture requires typed quality providers and an Execution Bundle
that packages contract, bindings, profile, skills, fast checks, and acceptance
gates for one work unit.

## Requirements

[REQ-QUALITY-PLANNING-PROVIDER-ROLES] Provider interfaces MUST be typed by role: code intelligence, diagnostic, verification, transformation, and agent guidance.

[REQ-QUALITY-PLANNING-NORMALIZED] Normalized quality outcomes MUST distinguish pass, fail, unavailable, error, and policy-authorized skip; a required provider that is unavailable, errors, or skips MUST NOT contribute passing evidence.

[REQ-QUALITY-PLANNING-CONFIG] Provider configuration MUST use executable and argument arrays, explicit working directories, timeouts, and output limits rather than interpolated shell commands.

[REQ-QUALITY-PLANNING-BUNDLE] An Execution Bundle MUST package the work unit, contract path, code bindings, quality profile, required skills, fast checks, and acceptance gates.

[REQ-QUALITY-PLANNING-SKILL-RECEIPT] A skill receipt MAY record resolved skill id, version, source, and content hash, but an agent's claim of having read a skill MUST NOT count as acceptance evidence.

[REQ-QUALITY-PLANNING-NEGATIVE] Satisfying specs MUST include negative scenarios covering unavailable required providers and malformed bundle requests.

## Dependencies

- REQ-CODE-GRAPH-IR

## Source Trace

- target architecture: docs/intent-compiler/architecture.md (Quality Planning And Execution Bundles; delivery boundary 4)
- staged contract: specs/roadmap/task-quality-planning-bundles.spec.md
