# ADR-0015: Warmth Is Operational and Sharing Is Separate

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

Keeping an API wrapper or host process alive does not guarantee provider cache retention,
valid authentication, a reusable native session, current configuration, or authority.
Environment lifetime and eligible reuse scope are different concerns.

## Decision

Warmth is a best-effort optimization owned by AgentHostSupervisor,
AgentSessionSupervisor, CapabilityHostSupervisor, EnvironmentManager, or the model
backend. Prewarm does not invoke a model or create an Attempt. Environment lifetime and
sharing scope are separately recorded; reuse creates fresh Task/Attempt/Trust/control
authority. V1 does not mount a USER_SHARED Environment across Workspace boundaries.

## Consequences

Warm state may disappear without affecting Task correctness. Every admission rechecks
freshness, Runtime incarnation, authentication, policy, and lease. Shared code writers
use isolated worktrees/overlays; shared browser input has one control-lease owner.
