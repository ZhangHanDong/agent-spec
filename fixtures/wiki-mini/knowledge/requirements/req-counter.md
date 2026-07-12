---
kind: requirement
id: REQ-COUNTER
title: "Counter"
liveness: auto
---

# Counter

## Problem

The fixture needs one small requirement for wiki trace demonstration.

## Requirements

[REQ-COUNTER] The counter MUST increment by one.

## Scenarios

Scenario: Increment counter
  Given a new counter
  When it is incremented
  Then its value is 1

## Source Trace

- fixture

## Open Questions

None.
