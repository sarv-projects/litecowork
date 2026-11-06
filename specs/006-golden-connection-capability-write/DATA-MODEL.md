# Data model - Golden Connection and One Capability Write

**Feature:** 006 | **Status:** DRAFT

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Connection with opaque id.
- Secret handle, never token bytes in projections.
- CapabilityGrant, Invocation, Effect, Evidence.
- Approval bound to plan digest.

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

- DiscoverCatalog/DiscoverItem projections and Runtime capability-host observations are not Connection records or grants. `authoritative=false` is an enforced projection property.
- The current SecretStore primitive writes an opaque SecretRef to the OS keychain and exposes no general token-read projection. No account Connection aggregate/service uses it in the active checkout.
- Approval is a separate exact-action decision aggregate. `approval.resolved.v1` records an owner decision; it is not an Invocation, CapabilityGrant, Effect dispatch receipt or provider result.
- The active CapabilityInvocation read model can show persisted Invocation observations/status. The current Operator cannot create or dispatch an Invocation; the SQLite transition store rejects DISPATCHED. No external provider request/response is implied by an Invocation history row.
- The served grant create path currently produces only a Task-planning Resource-read grant. Connection-backed read/write grants, their provider binding and SecretRef association remain target-only.
- Connection token bytes, OAuth callback payloads, provider credentials and native provider response bodies must never be placed in ordinary Conversation messages, agent prompts, domain events, logs or UI projections.