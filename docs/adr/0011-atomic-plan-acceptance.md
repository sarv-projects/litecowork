# ADR-0011: Accept Plans and Materialize Steps Atomically

- **Status:** Accepted for vNext
- **Date:** 2026-10-03

## Context

A plan is the source for durable Steps, and Task.current_plan_revision identifies the
accepted plan. If promotion and Step creation are separate committed commands, a crash can
leave the Task pointing at a plan with no executable Steps. Retrying then risks duplicate
Steps or an ambiguous current plan.

## Decision

TaskService accepts a structurally valid plan in one StateStore transaction. The command
appends the immutable PlanRevision, advances the Task pointer, materializes the plan's Step
records, supersedes obsolete unstarted Steps, and appends all corresponding domain events.
The command is idempotent and returns a PlanAcceptance containing the revision, resulting
Steps, and Task version. Step materialization is an internal helper, not a separately
observable operation.

A plan based on a stale TaskSpecRevision is rejected in v1. Active Attempts retain their
Attempt identity and lease until they settle or are safely cancelled; plan replacement
does not silently take execution authority away from them.

## Consequences

The Task cannot commit a current plan without its Steps. Plan acceptance has one atomic
recovery boundary. Agent reasoning remains external; TaskService validates structure and
ownership only.
