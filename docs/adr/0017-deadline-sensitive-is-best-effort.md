# ADR-0017: Deadline-Sensitive Work Is Best-Effort

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

Cloud models, networks, external sites, local devices, and authentication can become
unavailable without warning. Calling a workflow “realtime” would imply guarantees the
system cannot meet. Computer-use fallback may also change the semantics or risk of an
operation.

## Decision

`DEADLINE_SENSITIVE` requests prioritized placement and preflight. Required dependencies,
freshness, budget, authority, and fallback are checked before effects. Select structured
API, structured browser, accessibility browser, or screen interaction only when it can
perform the same authorized operation. ActionBatch keeps each consequential operation's
Effect/Evidence identity. Hard realtime is not a product contract.

## Consequences

The UI can report readiness and blockers honestly, but cannot promise completion by a
deadline. Human authorization boundaries remain explicit, and an unsuitable fallback
stops for review.
