# ADR-0019: Separate Policy, Approval, Credentials, Egress, and Audit

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

Agents need to use authenticated external services, but providing raw credentials to an
agent or allowing a network path around LiteCowork's Effect and fencing checks would make
Core authorization claims unreliable.

## Decision

TrustService remains the application boundary and composes policy, approval, credential,
egress, and audit responsibilities. Credentials are brokered at the egress boundary and
remain unavailable to agent context where the provider supports that pattern. Required
authorization may be preflighted, but consequential approval binds to the exact action at
execution. Direct paths are allowed only when equivalent provider guarantees hold.

## Consequences

Credential bytes, cookies, and private tokens stay out of domain events, TaskPackets,
ordinary Artifacts, logs, and backups. Providers that cannot enforce the required
authorization/effect/fencing contract must use the LiteCowork Gateway or remain outside
Core guarantees.
