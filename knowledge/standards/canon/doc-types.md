# Documentation Types

This standard adapts Lore's documentation engineering practice for agent-spec.
It governs human-facing `docs/` material. It does not replace KLL artifacts in
`knowledge/`, task contracts in `specs/`, or lifecycle evidence.

## Type Families

- Product docs: Tutorial, How-To, Reference, Explanation, and Landing pages.
- Contributor docs: Internals, ADR, Code-Standard, and contributor How-To pages.
- KLL docs: decision, requirement, guidance, and proposal artifacts under
  `knowledge/`. KLL artifacts are machine-consumable project truth and remain
  governed by `lint-knowledge`, `trace`, and lifecycle gates.

## Routing Rules

- Tutorial: learning by doing from a known starting point to a verifiable result.
- How-To: a short recipe for a reader who already knows the domain.
- Reference: exhaustive facts about a command, API, file format, or config key.
- Explanation: why a concept, architecture, or trade-off exists.
- Internals: implementation details contributors need to reason about code.
- ADR: durable decision and trade-off record.
- Code-Standard: rules for future code, paired with rationale and examples.
- Landing: folder index that routes readers to child pages.

## Agent-Spec Rule

Use Lore-style doc types for reader-facing documentation quality. Use KLL
requirements, decisions, and proposals when the content must be traceable,
linted, and connected to code through `satisfies: [REQ-*|ADR-*]`.
