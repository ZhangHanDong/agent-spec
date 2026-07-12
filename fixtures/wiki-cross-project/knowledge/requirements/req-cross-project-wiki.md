---
kind: requirement
id: REQ-CROSS-PROJECT-WIKI
title: "Cross-Project Wiki Fixture"
status: accepted
liveness: auto
tags: [wiki, fixture]
---

# Cross-Project Wiki Fixture

## Problem

The fixture needs a real requirement target for its flow trace.

## Requirements

[REQ-CROSS-PROJECT-WIKI] The fixture MUST map its two project articles.

## Scenarios

Scenario: Map fixture projects
  Given two project articles
  When the project map builds
  Then both projects and their flow are present
