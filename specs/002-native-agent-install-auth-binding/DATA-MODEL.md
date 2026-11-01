# Data model - Native Agent Install, Auth and Binding

**Feature:** 002 | **Status:** DRAFT

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- AgentRegistryEntry — cache, not authority.
- AgentInstallationObservation — Runtime-local.
- AgentControlDescriptor — time-bounded, non-secret.
- AgentBinding — Workspace-scoped durable eligibility, not a session handle.

## Relations

AgentBinding is the Workspace-scoped aggregate. The registry cache, installation observations, control descriptors, and lifecycle operations are Runtime-local and do not carry a Workspace id. Runtime-local secrets, handles, and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Install and update begin an idempotent lifecycle operation, perform the module call outside SQLite, then persist the Runtime-local observation. Binding configuration and enable or disable commit their receipt inside the transaction. Neither receipt means authentication success or session readiness.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

None in this revision. Blockers in SPEC section 5 must be closed before a migration is designed.
