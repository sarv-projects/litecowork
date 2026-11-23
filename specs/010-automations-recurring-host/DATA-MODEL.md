# Data model - Automations and Recurring Host

**Feature:** 010 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Automation revision.
- Occurrence id and cursor.
- Task link.
- Notification status.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration and existing schema

No migration has shipped for this package; the current migration chain ends at v35. The canonical SQLite v1 schema already defines `notification_deliveries` with every listed status (`PENDING`, `SENDING`, `SENT`, `FAILED`, `AMBIGUOUS`, `SUPPRESSED`), attempt Runtime/host epoch/count, `next_attempt_at`, `last_error_code` and `settled_at`. It also defines the Runtime-local encrypted `automation_trigger_bindings` table for opaque provider cursors. Current source does not yet serve notification claim/settle or CONNECTOR_EVENT hosting, so these existing fields do not count as implemented behavior.

The v45 reservation is provisional. A delivery version/CAS token is absent from the current row shape, while the proposed settle route requires a version fence. T01 must identify the exact owner-approved additive DDL, or release v45 if existing attempt/epoch fields can satisfy the approved contract. Do not re-add existing status or cursor fields. This unresolved contract is tracked as A-98 in the global alignment register.

The accepted quiet-hours target also needs one canonical persistence contract before a class-selection UI is implemented: current `QuietHours` is a timezone/weekdays/start/end value on one versioned `NotificationPreference` per event class, while SPEC describes a Workspace-level enabled flag and suppressed-class set. A-97 tracks the required owner resolution; do not infer a new table or parallel preference schema here.

Suppressed delivery persistence is also unresolved: this accepted package's AC-007 requires a visible `SUPPRESSED` delivery row, while the higher-authority global `docs/DATA-MODEL.md` says suppressed decisions write no delivery row, matching current producer behavior. A-99 tracks owner resolution; do not implement either interpretation against the other contract without updating both owners together.


## External workflow references (provider-owned)

Feature 010 does not create Workflow, WorkflowNode, WorkflowEdge, WorkflowRun or WorkflowStep tables. A future qualified adapter may define a provider-scoped workflow reference and normalized input/output mapping for a Routine; no canonical WorkflowRef machine type or production provider adapter currently exists in this checkout. The external graph, revision, node state and provider run identity remain provider-owned; runtime-only external run handles are protected as provider-private data. The LiteCowork CapabilityInvocation and Task/Attempt receipts remain the local durable authority.

No migration is proposed for this reference comparison. A new external authoring capability requires a separate owner decision and provider contract.
