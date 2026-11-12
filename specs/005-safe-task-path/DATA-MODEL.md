# Data model - Safe Task Path

**Feature:** 005 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Task and TaskSpecRevision.
- PlanRevision and Steps.
- Attempt, ExecutionLease, Environment reference, scoped grant/use, Effect, Evidence and VerificationRun.
- Idempotency receipt.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

The B-18 write proposal must pin one confined target, base digest, exact bounded new-content digest, accepted plan action, approval and Effect. The content's durable source/reference is open at A-87. The target `FileWriteGrant` request field is not defined as either a requested scope or a durable Trust grant; caller-supplied grant data cannot establish authority (A-90). Private `IsolationAttestation` values remain in an incarnation-scoped Runtime store; only the owner-approved digest/reference may appear in a local admission record, never the attestation body or native handle.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

None shipped yet. T01 must first resolve A-70/A-87/A-88/A-90 with the owner contracts. The migration may add only the resulting owner-approved planner/Attempt Environment relationship, durable payload reference, grant/Effect/ApprovalUse relationships, Attempt-to-Environment link or attestation digest reference that those decisions require; it must not pre-commit to unresolved fields or store host paths/native handles. Conversation-origin Tasks reuse the existing `task_spec_revisions.source_message_refs_json` column and the Task `conversation_id`; storage currently refuses them in `validate_task_create_commit`.

## Current implementation boundary (cross-checked 2026-11-12)

- Task and TaskSpecRevision are the only records committed by the current Task-create flow. The Task envelope is not a persisted Plan, Step, Attempt, Environment, Grant, Effect or Evidence record.
- PlanRevision/Step, Attempt/ExecutionLease, CapabilityGrant, Effect and Evidence records are canonical target owner records and have storage/domain code, but the production Task admission/Execution pipeline is not connected in the daemon.
- The current planning-readiness projection is derived, read-only and not persisted as a Task lifecycle transition. It is not an approval, PlanRevision, session reservation or Attempt receipt.
- The current Task-planning Resource-read grant is scoped to exact Resource refs and `workspace.resource.read`; it is not a write grant or Environment execution authority.
- Runtime-local Environment handles, process identities, isolation evidence and quiescence proofs must remain private/runtime-local and must not be copied into ordinary replicated Task events.
- Current UI projections may show Task/spec/Step/Artifact/readiness records only as committed owner data; they must not infer Attempt liveness, Environment security or verified write success from those records.
- The Environment provider probe is discovery evidence only. On Windows it reports no Task-capable `LocalWorkspace` class or Task lifetime; on Linux a trusted Bubblewrap candidate is necessary for the capability observation but does not provision an Environment or produce an attestation.
- `IsolationAttestation` is Runtime-local private evidence and cannot be accepted from a UI, Operator, Agent, or provider request. The accepted `AdmitFileWriteAttempt` target shape has been corrected accordingly.
- Planner readiness and Attempt admission each require current-incarnation Environment evidence, but its identity/scope, reuse policy and recovery relationship remain unresolved across Feature 005 and 015 (A-70).
- The durable source and replay path for B-18 replacement bytes, the exact mapping between Attempt/lease admission and Effect/Trust/ApprovalUse dispatch, and the authority meaning of the proposed FileWriteGrant field remain unresolved (A-87/A-88/A-90). No file-write payload is currently represented as a committed Task record or receipt.
