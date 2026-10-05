# ADR-0013: Host Delegation Uses Accepted Plan Steps

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

An unconstrained agent tool that can create arbitrary child work would bypass PlanRevision
immutability, Task admission, budgets, isolation, authority checks, and recovery limits.
Native subagents already provide agent-owned internal delegation.

## Decision

LiteCowork creates child Attempts only for READY Steps in the current accepted immutable
PlanRevision. The lead proposes a PlanRevision through the existing Task path when work is
missing. Each host-delegated child receives a new AgentSession and lease, budget
reservation, freshly evaluated scoped grants, and an Environment attachment admitted
under its sharing/isolation contract. Parent grants, approvals,
SecretLeases, and native credentials never flow to the child.

## Consequences

Child work is auditable and recoverable without giving LiteCowork ownership of native
subagents. Delegation admission may require plan revision before execution. Retries and
escalations create new Attempts; worker identity is never mutated in place.
