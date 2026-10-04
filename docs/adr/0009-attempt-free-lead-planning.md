# ADR-0009: Initial Planning Does Not Require an Attempt

- **Status:** Accepted for vNext
- **Date:** 2026-10-02

## Context

A lead agent must be able to propose the first PlanRevision before Steps exist. Requiring an execution Attempt to author that plan creates a circular dependency: Attempts execute Steps, but the initial plan creates the Steps.

## Decision

Authorize a Task-scoped TASK_PLANNING AgentSession without an Attempt, lease, or writable Environment. It may clarify intent and propose a plan. TaskService validates and promotes the plan, then materializes Steps; execution Attempts begin afterward. Planning sessions cannot invoke consequential capabilities or publish artifacts.

## Consequences

Task and AgentSession lifecycles represent the initial planning phase honestly while preserving the invariant that every Attempt executes one Step.
