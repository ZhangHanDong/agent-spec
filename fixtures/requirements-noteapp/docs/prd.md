# Note App PRD

The example app demonstrates the agent-spec intent compiler workflow on a small Rust library.

<!-- agent-spec:requirement id=REQ-NOTE-CREATE title="Create Note" tags=example,rust source=prd:noteapp -->
## Problem

Users need to create short notes and receive the created note back from the store.

## Requirements

[REQ-NOTE-CREATE] The note store MUST create a note containing stable id, title, body fields.

## Scenarios

Scenario: Create note
  Given an empty note store
  When a note with title "compiler notes" and body "capture requirement decisions" is created
  Then the returned note has id 1 and appears in the store list

## Dependencies

None.

## Open Questions

None.
<!-- /agent-spec:requirement -->

<!-- agent-spec:requirement id=REQ-NOTE-LIST title="List Notes" tags=example,rust source=prd:noteapp -->
## Problem

Users need to list notes in creation order.

## Requirements

[REQ-NOTE-LIST] The note store MUST list created notes in creation order.

## Scenarios

Scenario: List notes
  Given a note store with two created notes
  When notes are listed
  Then the returned list contains both notes in creation order

## Dependencies

- REQ-NOTE-CREATE

## Open Questions

None.
<!-- /agent-spec:requirement -->
