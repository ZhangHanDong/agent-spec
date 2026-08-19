---
title: "Spec Parser"
type: module
source_files:
  - src/spec_parser
tags:
  - parser
  - contract
status: draft
---

# Spec Parser

## Role

Task Contract parsing, frontmatter parsing, and inheritance resolution.

## Maintenance

Update this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.

## List sections

`group_list_lines` is the single entry for the five bullet-list sections
(Decisions, Constraints, Boundaries, Out of Scope, Questions): indented
continuation lines join their bullet (space between Latin, none between CJK),
deeper-indented sub-bullets stay inside the parent as `\n  - ` fragments,
and blank lines / `###` headers / HTML comments / unindented prose close the
item. Item spans point at the bullet line. Acceptance Criteria parsing is
separate and unchanged.
