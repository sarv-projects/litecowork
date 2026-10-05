# ADR-0016: Coworkers, Goals, and Suggestions Do Not Own Execution

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

Persistent identity and intent improve continuity, but they can become a second hidden
execution authority if they own schedules, Task state, approvals, or automatic planning.
The proposed generic `AuthorityCeiling` would duplicate scoped CapabilityGrants,
Approval, and Effect risk policy without replacing their precise checks.

## Decision

Coworkers hold user-facing identity/preferences; Goals hold user-authored desired
outcomes; Suggestions hold expiring proposals. Tasks remain execution truth. Goals never
schedule or complete work automatically. Suggestions require explicit user action. Trust
continues to use scoped grants, exact approvals, Effect risk, and human handoff; no broad
AuthorityCeiling enum is added.

## Consequences

Identity and proactive UX can survive lead/model replacement without changing Task
ownership. Progress is derived from accepted outcomes and Evidence. Future authority
controls must refine existing Trust scopes or be justified by a concrete enforcement gap.
