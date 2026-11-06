# Data model - Automations and Recurring Host

**Feature:** 010 | **Status:** DRAFT

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

## Migration

None in this revision. Blockers in SPEC section 5 must be closed before a migration is designed.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]
## External workflow references (provider-owned)

Feature 010 does not create Workflow, WorkflowNode, WorkflowEdge, WorkflowRun or WorkflowStep tables. A future qualified adapter may define a provider-scoped workflow reference and normalized input/output mapping for a Routine; no canonical WorkflowRef machine type or production provider adapter currently exists in this checkout. The external graph, revision, node state and provider run identity remain provider-owned; runtime-only external run handles are protected as provider-private data. The LiteCowork CapabilityInvocation and Task/Attempt receipts remain the local durable authority.

No migration is proposed for this reference comparison. A new external authoring capability requires a separate owner decision and provider contract.
