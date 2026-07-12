spec: task
name: "Counter"
satisfies: [REQ-COUNTER]
---

## Intent

Provide a small counter for wiki fixture generation.

## Completion Criteria

Scenario: Increment counter
  Test: counter_increment_adds_one
  Given a new counter
  When it is incremented
  Then its value is 1
