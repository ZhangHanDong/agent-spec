# Id Registry

The single authority mapping artifact id prefixes to their home directory.
"What am I holding, where does it go" is answered here and nowhere else;
routing tables in skills and scaffolds copy this table, never fork it.

| You hold | Directory | Id prefix |
|---|---|---|
| A debate: should we do this, and why | `knowledge/proposals/` | `LEP-` |
| A settled architectural ruling | `knowledge/decisions/` | `ADR-` |
| An obligation the system must satisfy | `knowledge/requirements/` | `REQ-` |
| An executable, verifiable task contract | `specs/` | `task-` |

## Rules

- File names use `YYYY-MM-DD-slug.md` for proposals and lowercase kebab-case
  elsewhere; the stable id lives in frontmatter, never in the file name.
- `PROP-*` ids are retired (ADR-002). Proposals use `LEP-NNN`.
- Guidance docs use `G-NNN` under `knowledge/guidance/`; context docs are
  free-form and unregistered.
- Each layer has one exit: a proposal exits via `## Produces: ADR-NNN`, a
  decision exits via a governed requirement, a requirement exits via
  `requirements draft-specs`, a spec exits via `satisfies:`-bound
  implementation.
