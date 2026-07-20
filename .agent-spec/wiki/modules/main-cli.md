---
title: "Main CLI"
type: module
source_files:
  - src/main.rs
tags:
  - cli
  - commands
status: draft
---

# Main CLI

## Role

Primary command dispatch and text/json formatting entrypoint. Atlas `flow`
always emits JSON and accepts no `--format` flag; disconnected results may
include bounded `runtime_boundaries` while preserving the existing flow state.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

Atlas D2 adds deterministic build flags for input identity, frontier size,
batch size, and working bytes; command dispatch ownership remains here.
