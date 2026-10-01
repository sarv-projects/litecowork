# ADR-0003: Keep LitePSM Independent

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

Package discovery, verification, installation, updates, provider supervision, and
removal form a separate ecosystem lifecycle from task-specific authorization and use.

## Decision

LitePSM owns package/ecosystem truth. AgentCowork owns CapabilityRefs, Offers, Grants,
and Activations scoped to a Task/Attempt, and integrates through a LitePSM client.

## Consequences

Catalog discovery and package installation grant no execution authority. Active Tasks
pin exact versions/digests. AgentCowork does not build a second marketplace.
