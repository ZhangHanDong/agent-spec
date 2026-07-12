spec: task
name: "Create Note"
tags: [example, rust]
satisfies: [REQ-NOTE-CREATE]
risk: C
---

## Intent

Implement note creation for the example Rust note store.

## Decisions

- Use an in-memory `NoteStore`.
- Assign ids starting at 1 and increment by 1 for each created note.

## Boundaries

### Allowed Changes
- src/lib.rs
- tests/noteapp_contract.rs

### Forbidden
- Do not add external dependencies.

## Completion Criteria

Scenario: Create note
  Test: note_create_adds_note
  Given an empty note store
  When a note with title "compiler notes" and body "capture requirement decisions" is created
  Then the returned note has id 1 and appears in the store list
