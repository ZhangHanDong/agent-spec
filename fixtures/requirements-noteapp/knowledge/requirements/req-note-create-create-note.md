---
kind: requirement
id: REQ-NOTE-CREATE
title: "Create Note"
status: accepted
liveness: auto
tags: [example, rust]
---

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


## Source Trace

- prd:noteapp
## Open Questions

None.
