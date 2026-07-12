spec: task
name: "List Notes"
tags: [example, rust]
satisfies: [REQ-NOTE-LIST]
depends: [task-req-note-create]
risk: C
---

## Intent

Implement note listing for the example Rust note store.

## Decisions

- Return notes in creation order.
- Return cloned notes so callers cannot mutate store internals.

## Boundaries

### Allowed Changes
- src/lib.rs
- tests/noteapp_contract.rs

### Forbidden
- Do not add persistence.
- Do not add external dependencies.

## Completion Criteria

Scenario: List notes
  Test: note_list_returns_created_notes
  Given a note store with two created notes
  When notes are listed
  Then the returned list contains both notes in creation order
