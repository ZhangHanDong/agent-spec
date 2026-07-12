---
kind: requirement
id: REQ-PARITY-BOOKING
title: "Parity Booking"
status: accepted
liveness: auto
tags: [parity, booking]
---

# Parity Booking

## Problem

A reference-shaped booking requirement used to pin the compile bundle
layouts: one accepted requirement with scenarios and two normative clauses.

## Requirements

[REQ-PARITY-BOOKING-RESERVE] The system MUST reserve an available slot exactly once.

[REQ-PARITY-BOOKING-CONFLICT] The system MUST reject a booking for an already-reserved slot.

## Scenarios

Scenario: booking succeeds
  Given an available time slot
  When a visitor books the slot
  Then the slot is reserved and confirmed

Scenario: double booking is rejected
  Given a slot that is already reserved
  When a second visitor books the same slot
  Then the booking is rejected with a conflict
