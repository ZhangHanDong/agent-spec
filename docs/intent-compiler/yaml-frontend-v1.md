# Intent Compiler YAML Frontend v1

`agent-spec requirements import --from <file>.yaml` translates a
reference-style requirement tree into Requirement IR documents under
`knowledge/requirements/`. The IR and every downstream stage (`lint-knowledge`,
`requirements graph`, `work-units`, `plan`) stay frozen — this is a source
dialect, not a new pipeline. Routing is by file extension: `.yaml` / `.yml`
selects this frontend; anything else uses the Markdown marked-block intake.

## Accepted subset

The parser is hand-written and deliberately small. Accepted:

- two-space indentation, spaces only
- `key: value` scalar entries and `key:` block openers with known keys
- block lists (`- item`, `- key: value` map items)
- double-quoted scalars without escape sequences; unquoted scalars
- full-line `#` comments and blank lines

Rejected with a `yaml-unsupported-construct` diagnostic (whole import fails,
nothing is written):

- anchors (`&`), aliases (`*`), flow-style collections (`[...]`, `{...}`)
- block scalars (`|`, `>`), single-quoted scalars, escape sequences
- multi-document streams (`---`, `...`), tab indentation, odd indentation
- unknown map keys, duplicate keys, complex (`?`) keys

## Node schema

```yaml
requirements:
  - id: booking                # lowercase kebab, safe-id checked
    title: "Booking"
    type: FOLDER               # top-level nodes must be FOLDER
    status: accepted           # optional; proposed (default) | accepted
    description: "..."         # optional; becomes ## Problem
    children:
      - id: create-booking
        title: "Create a booking"
        type: ATOMIC           # children must be ATOMIC (no nesting)
        statement: "The system MUST ..."
        dependencies:          # optional; node ids
          - search-flights
        scenarios:             # optional
          - name: "Booking succeeds"
            given: "..."
            when: "..."
            then: "..."
```

## Mapping table

| YAML | Requirement IR |
|------|----------------|
| top-level FOLDER `id: booking` | `knowledge/requirements/req-booking.md` with `id: REQ-BOOKING` |
| FOLDER `description` | `## Problem` body (fallback: `Imported from <source>.`) |
| ATOMIC leaf `id: create-booking` + `statement` | clause `[REQ-BOOKING-CREATE-BOOKING] <statement>` |
| FOLDER `status` (optional, `proposed`/`accepted`) | frontmatter `status`; human acceptance lives in the YAML source — work units stay `informational` until accepted |
| leaf `scenarios[]` (`name`/`given`/`when`/`then`) | `Scenario:` blocks in a dedicated `## Scenarios` section (the requirement graph reads scenarios there; work units need them to become `ready`) |
| leaf `dependencies` to a node in another folder | `## Dependencies` entry on that folder's doc id |
| leaf `dependencies` inside the same folder | dropped (clause order carries it) |
| leaf `dependencies` to an unknown node id | emitted as `REQ-<ID>` so `requirements graph --gate` reports it |
| — | frontmatter `source: imported-yaml` provenance marker on every generated doc |

## Ownership and idempotence

- Import refuses to overwrite any existing file whose frontmatter lacks
  `source: imported-yaml`; the ownership check runs for every target before
  the first write (all-or-nothing).
- Re-importing an unchanged source regenerates byte-identical files
  (whole-file regeneration, no clause-level merging).
- `--check` compares the rendered output against the files on disk and exits
  non-zero on drift, mirroring the Markdown intake.

Contract: `specs/task-intent-compiler-yaml-frontend.spec.md`
(satisfies `REQ-INTENT-COMPILER-YAML-FRONTEND`).
