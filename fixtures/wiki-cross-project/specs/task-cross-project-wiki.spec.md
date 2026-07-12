spec: task
name: "Cross-Project Wiki Fixture"
tags: [wiki, fixture]
satisfies: [REQ-CROSS-PROJECT-WIKI]
---

## Intent

Bind the fixture flow to a parseable Task Contract.

## Completion Criteria

Scenario: Map fixture projects
  Test: test_cross_project_wiki_fixture_builds_project_map
  Given two project articles
  When the project map builds
  Then both projects and their flow are present
