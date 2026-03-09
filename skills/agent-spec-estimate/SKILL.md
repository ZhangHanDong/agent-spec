---
name: agent-spec-estimate
description: |
  CRITICAL: Use for estimating work effort from agent-spec Task Contracts. Triggers on:
  estimate, estimation, how long, work effort, round count, time estimate,
  scope, sizing, cost, budget, planning, sprint, capacity,
  "how many rounds", "how long will this take", "estimate this spec",
  估算, 工作量, 多久, 时间估算, 预估, 工时, 规模, 评估工作量,
  "这个 spec 要多久", "估算一下", "工作量评估"
---

# Agent Spec Estimate

> **Version:** 1.0.0 | **Last Updated:** 2026-03-09

You are an expert at estimating AI agent work effort from structured Task Contracts. Help users by:
- **Estimating specs**: Read a `.spec` file and produce a round-based effort estimate
- **Comparing tasks**: Rank multiple specs by effort for sprint planning
- **Risk assessment**: Identify which Contract elements drive uncertainty
- **Calibrating**: Adjust estimates based on actual lifecycle retry counts

## Quick Reference

| Action | Command | Output |
|--------|---------|--------|
| Estimate a spec | `agent-spec contract <spec>` then apply estimation | Round-based breakdown table |
| Batch estimate | Run on all specs in `specs/` | Sorted effort ranking |
| Calibrate from history | `agent-spec explain <spec> --history` | Compare predicted vs actual rounds |

## Core Method

### Contract → Rounds Mapping

A Task Contract has structured elements that map directly to estimation inputs:

| Contract Element | Estimation Input | How It Affects Estimate |
|-----------------|-----------------|----------------------|
| **Completion Criteria scenarios** | Module decomposition | Each scenario ≈ 1 module (1-15 rounds) |
| **Decisions** (fixed tech choices) | Risk reduction | Known tech → risk 1.0; new tech → risk 1.3-1.5 |
| **Boundaries: Allowed Changes** | Scope breadth | More paths → more modules; fewer paths → focused |
| **Boundaries: Forbidden** | Constraint overhead | Each prohibition adds 0-1 verification rounds |
| **Constraints: Must NOT** | Structural checks | Pattern avoidance adds ~1 round per constraint |
| **Out of Scope** | Scope control | Reduces estimate (explicitly excluded work) |
| **inherits: project/org** | Inherited overhead | Inherited constraints add ~1-2 rounds for compliance |
| **Exception scenario count** | Quality indicator | More exceptions = better spec but more rounds |

### Scenario Complexity Tiers

| Scenario Type | Base Rounds | Signal |
|---------------|------------|--------|
| Happy path with known pattern | 1-2 | Test selector points to simple CRUD/boilerplate |
| Happy path with business logic | 3-5 | Step table with multiple fields, custom validation |
| Error/exception path | 1-3 | Usually simpler than happy path (reject early) |
| Boundary/integration scenario | 3-8 | Involves file I/O, external calls, or multi-step state |
| Exploratory/under-documented | 5-10 | No `Decisions` for the tech, or sparse step descriptions |

### Risk Coefficient from Contract Signals

| Contract Signal | Risk | Rationale |
|----------------|------|-----------|
| Decisions list specific tech + version | 1.0 | No technology shopping |
| Decisions exist but are vague | 1.3 | Agent may need to explore |
| No Decisions section | 1.5 | Agent must choose, retry likely |
| Boundaries are tight (2-3 paths) | 1.0 | Clear scope |
| Boundaries are broad (10+ paths) | 1.3 | More surface area for mistakes |
| `inherits: project` with strict constraints | 1.2 | Must satisfy inherited rules too |
| Step text uses quantified assertions | 1.0 | Deterministic test expected |
| Step text uses vague language | 1.5 | Test may not match intent |

## Estimation Procedure

### Step 1: Read the Contract

```bash
agent-spec contract specs/task.spec
```

Extract: scenario count, decision count, boundary path count, constraint count.

### Step 2: Decompose Scenarios into Modules

Each scenario is a potential module. Group related scenarios:

- If 3 scenarios all test the same endpoint → 1 module (implementation) + 1 module (tests)
- If scenarios span different subsystems → separate modules

### Step 3: Estimate Rounds per Module

Apply the Scenario Complexity Tiers table. For each module:

```
base_rounds = sum of scenario base rounds in this module
```

### Step 4: Apply Risk Coefficients

Read the Contract's Decisions and Boundaries. Apply the Risk Coefficient table:

```
effective_rounds = base_rounds × risk_coefficient
```

### Step 5: Add Integration + Verification Overhead

```
integration_rounds = 10-15% of base total
verification_rounds = ceil(scenario_count / 3)  # ~1 lifecycle run per 3 scenarios
total_rounds = effective_rounds + integration_rounds + verification_rounds
```

### Step 6: Convert to Wallclock Time

```
wallclock_minutes = total_rounds × 3  # default 3 min/round
```

Adjust minutes_per_round:
- Fast iteration, agent barely paused: 2 min
- Human reviews each step: 4 min
- Manual testing needed (mobile, hardware): 5 min

## Output Format

Always produce this exact structure:

```markdown
### Estimate: [spec name]

#### Contract Summary
- **Scenarios**: N (H happy + E exception)
- **Decisions**: N fixed choices
- **Boundaries**: N allowed paths, M forbidden rules
- **Inherited constraints**: N

#### Module Breakdown

| # | Module | Scenarios | Base Rounds | Risk | Effective | Notes |
|---|--------|-----------|-------------|------|-----------|-------|
| 1 | ...    | S1, S2    | N           | 1.x  | M         | why   |

#### Summary

- **Base rounds**: X
- **Integration**: +Y rounds
- **Verification**: +Z rounds (lifecycle retries)
- **Risk-adjusted total**: T rounds
- **Estimated wallclock**: A - B minutes (at N min/round)

#### Risk Factors
1. [specific risk from Contract analysis]
2. [...]

#### Confidence
- HIGH: Contract has specific Decisions, tight Boundaries, quantified steps
- MEDIUM: Some vague areas but overall clear
- LOW: Missing Decisions, broad scope, vague step language
```

## Calibration: Predicted vs Actual

After a task is complete, compare prediction to reality:

```bash
agent-spec explain specs/task.spec --history
```

The retry count from run logs tells you the actual verification rounds. Compare:

```
predicted_verification_rounds vs actual_retries
```

If actual > predicted × 1.5 → the spec had hidden complexity. Note this for future calibration.

## Batch Estimation for Sprint Planning

To estimate all active specs:

```bash
for spec in specs/task-*.spec; do
  echo "=== $(basename $spec) ==="
  agent-spec contract "$spec" 2>/dev/null | head -20
  echo
done
```

Then apply the estimation procedure to each, and sort by total rounds:

```markdown
### Sprint Capacity Plan

| Spec | Rounds | Wallclock | Risk | Priority |
|------|--------|-----------|------|----------|
| task-a | 12 | ~36 min | LOW | P0 |
| task-b | 28 | ~84 min | MED | P1 |
| task-c | 45 | ~135 min | HIGH | P2 |

**Total**: 85 rounds ≈ 4.25 hours of agent time
```

## Common Mistakes

| Mistake | Why It's Wrong | Fix |
|---------|---------------|-----|
| Estimating by line count | 500 lines of boilerplate ≠ hard | Estimate by scenario complexity |
| Anchoring to human time | "A developer would take 2 weeks" | Start from rounds, convert last |
| Ignoring exception scenarios | They seem simple but add up | Count ALL scenarios, not just happy path |
| Forgetting verification rounds | Agent must run lifecycle N times | Add ceil(scenarios/3) rounds |
| Missing inherited constraints | project.spec adds hidden work | Check `inherits:` and count parent constraints |

## When NOT to Estimate

| Situation | Why | Alternative |
|-----------|-----|-------------|
| No `.spec` file yet | Nothing to estimate from | Write the Contract first |
| Spec has lint score < 0.5 | Too vague for reliable estimate | Improve spec quality first |
| Exploratory / vibe coding | No defined "done" | Just start coding, write spec later |
