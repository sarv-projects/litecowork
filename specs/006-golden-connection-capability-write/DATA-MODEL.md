# Data model - Golden Connection and One Capability Write

**Feature:** 006 | **Status:** IN_REVIEW - revision 1.1 records USER-DELEGATED device-flow direction; formal domain-owner ratification remains pending.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Connection with opaque id.
- Secret handle, never token bytes in projections.
- CapabilityGrant, Invocation, Effect, Evidence.
- Approval bound to plan digest.
- AuthAttempt bound to Workspace, principal, provider, purpose (`DEVICE_AUTH | TOKEN_REFRESH`), optional Connection ID, idempotency key, expected version, expiry and lifecycle state. It stores no device code, user code, access token, refresh token, client secret, App private key or PKCE verifier. During token finalization it may hold only preallocated opaque access/refresh SecretRef IDs in private storage so startup recovery can remove orphan credentials; those IDs are never public event/projection/log fields.
- The GitHub device code exists only in bounded daemon memory for its short provider-defined lifetime. Restart expires the pending AuthAttempt and requires a fresh owner-initiated flow; it never resumes by reconstructing a device code from SQLite or logs.
- Device user code is an explicitly user-visible, one-time value delivered only to the authenticated settings UI response. It is excluded from Conversation messages, agent context, events and logs. Access/refresh tokens are OS SecretStore values referenced by opaque SecretRef IDs.
- Before any token bytes are written, the AuthAttempt durably records the two preallocated SecretRef IDs and enters `TOKEN_CUSTODY_PENDING`. SecretStore `write_at` is create-only and cannot overwrite an existing credential; an uncertain/duplicate create becomes cleanup-and-reauthorize, never silent replacement. One SQLite transaction then commits the Connection and transitions AuthAttempt to `SUCCEEDED`, transferring refs to the Connection. Until that transaction commits, startup recovery removes staged refs through idempotent `remove`; if removal fails, AuthAttempt remains `CLEANUP_PENDING` and startup retries. Recovery never removes refs owned by a committed Connection.
- The connected identity's usable repository set is the intersection of the user's GitHub access and the GitHub App's permission/installation access. CapabilityGrant remains a separate exact repository + operation + expiry authority owned by LiteCowork.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

None shipped yet. The `connections` table already exists in `docs/schemas/sqlite-v1.sql` (status CHECK without `REVOKED`, `secret_refs_json`) and has no Rust store. T01 designs a forward-only migration (provisionally allocated v41 in `docs/LITECOWORK-BUILD-TASKS.md`) that rebuilds it with `REVOKED` and a remote-revocation field and adds `auth_attempts` with purpose, optional Connection identity, bounded lifecycle state, expiry, version, non-secret correlation metadata and private staged SecretRef IDs. Device/user codes and token bytes never enter SQLite; only opaque SecretRef IDs do. T03 provides addressable create-only `write_at` and idempotent `remove` operations so restart can find and clean staged keyring entries without overwriting existing credentials. T14 reconciles staged refs against committed Connection records before new provider work. T07 replaces the `sqlite-v15.sql` Invocation creation and DISPATCHED triggers with guards that admit only the combined dispatch transaction.

## Current implementation boundary (2026-11-06)

- DiscoverCatalog/DiscoverItem projections and Runtime capability-host observations are not Connection records or grants. `authoritative=false` is an enforced projection property.
- The current SecretStore primitive writes an opaque SecretRef to the OS keychain and exposes no general token-read projection. No account Connection aggregate/service uses it in the active checkout.
- Approval is a separate exact-action decision aggregate. `approval.resolved.v1` records an owner decision; it is not an Invocation, CapabilityGrant, Effect dispatch receipt or provider result.
- The active CapabilityInvocation read model can show persisted Invocation observations/status. The current Operator cannot create or dispatch an Invocation; the SQLite transition store rejects DISPATCHED. No external provider request/response is implied by an Invocation history row.
- The served grant create path currently produces only a Task-planning Resource-read grant. Connection-backed read/write grants, their provider binding and SecretRef association remain target-only.
- Connection token bytes, device codes, provider credentials and native provider response bodies must never be placed in ordinary Conversation messages, agent prompts, domain events, logs or public UI projections. The device user code is returned only to the authenticated settings surface that owns the active attempt.
