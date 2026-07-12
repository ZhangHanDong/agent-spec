---
kind: requirement
id: REQ-NOTE-LIST
title: "List Notes"
status: accepted
liveness: auto
tags: [example, rust]
---

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


## Source Trace

- prd:noteapp
## Open Questions

None.
