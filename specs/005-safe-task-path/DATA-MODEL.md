# Data model - Safe Task Path

**Feature:** 005 | **Status:** DRAFT

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Task and TaskSpecRevision.
- PlanRevision and Steps.
- Attempt, ExecutionLease, CapabilityGrant, Effect, Evidence.
- Idempotency receipt.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

None in this revision. Blockers in SPEC section 5 must be closed before a migration is designed.

## Current implementation boundary (2026-11-06)

- Task and TaskSpecRevision are the only records committed by the current Task-create flow. The Task envelope is not a persisted Plan, Step, Attempt, Environment, Grant, Effect or Evidence record.
- PlanRevision/Step, Attempt/ExecutionLease, CapabilityGrant, Effect and Evidence records are canonical target owner records and have storage/domain code, but the production Task admission/Execution pipeline is not connected in the daemon.
- The current planning-readiness projection is derived, read-only and not persisted as a Task lifecycle transition. It is not an approval, PlanRevision, session reservation or Attempt receipt.
- The current Task-planning Resource-read grant is scoped to exact Resource refs and `workspace.resource.read`; it is not a write grant or Environment execution authority.
- Runtime-local Environment handles, process identities, isolation evidence and quiescence proofs must remain private/runtime-local and must not be copied into ordinary replicated Task events.
- Current UI projections may show Task/spec/Step/Artifact/readiness records only as committed owner data; they must not infer Attempt liveness, Environment security or verified write success from those records.