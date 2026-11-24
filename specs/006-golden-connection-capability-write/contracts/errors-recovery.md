# Errors and recovery - Golden Connection and One Capability Write

**Status:** IN_REVIEW - revision 1.1 records USER-DELEGATED direction; formal domain-owner ratification pending. Rules below remain TARGET.

## Target recovery rules

- Device authorization is started only by an explicit owner action. `device_code` stays in daemon memory with the provider expiry; `user_code` is returned only to the authorized settings UI and is never included in agent context, Conversation, events or logs. Cancel stops polling and fences late poll responses. The provider-supplied interval is a minimum; `slow_down` increases the next allowed poll time by the provider-defined increment. No parallel poll is admitted for the same AuthAttempt.
- A daemon restart expires pending device AuthAttempts. It never reconstructs or retries a device code from persistence. The owner starts a new flow; a late token response from the old attempt is discarded and cannot create/replace a Connection. Device flow uses no callback/state/PKCE verifier, so callback replay checks are replaced by AuthAttempt identity/version, terminal-state single flight and expiry checks.
- If a token response arrives after cancellation or after the AuthAttempt version/expiry changed, it cannot write a Connection. Preallocated refs and durable AuthAttempt state make partial keyring writes discoverable for cleanup after a crash; no separate cleanup-tombstone schema is needed beyond the AuthAttempt lifecycle defined by T01. Cleanup retries never reopen the Connection.
- Refresh uses the fixed GitHub token endpoint, serializes per Connection, and atomically replaces both rotated access and refresh SecretRefs. A lost/ambiguous refresh response must not retry the consumed refresh token; mark the Connection `REAUTH_REQUIRED` and require a new explicit device flow unless provider reconciliation proves the replacement pair.
- GitHub App authorization revocation is a separate user action in GitHub account Settings → Applications → Authorized GitHub Apps. Local Disconnect immediately revokes LiteCowork grants and deletes/fences local credentials; it reports remote revocation as `OWNER_ACTION_REQUIRED` until the user completes that step or an authenticated revocation observation exists.

- Device-flow cancel, denial, expiry or stale AuthAttempt leaves no usable Connection. A terminal attempt wins once; duplicate or late poll responses are refused. Token custody is write-only and provider tokens/device codes never enter an event, log, agent prompt or ordinary UI response.
- Token finalization first persists a non-secret cleanup intent with preallocated SecretRef IDs in the private AuthAttempt row; only then may ConnectionService write token bytes to those deterministic SecretStore slots. One SQLite transaction commits the Connection and transfers the refs from AuthAttempt to Connection. If that transaction fails or the daemon restarts first, startup removes staged slots idempotently. Failed removal retains `CLEANUP_PENDING` for retry; committed Connection refs are never removed by the orphan sweep. A lost response replays the original idempotent receipt rather than creating another secret.
- Per-Connection refresh is serialized. `invalid_grant` or provider revocation becomes REAUTH_REQUIRED; transient refresh failure becomes DEGRADED. Neither silently deletes historical Effects or starts a write retry.
- A revoked Connection or revoked/expired CapabilityGrant fences future Invocations. Historical Approval, Invocation, Effect and Evidence records remain readable within their owner retention rules.
- Approval is bound to exact action/plan digest, target, scope, expiry and owner identity. APPROVE cannot be accepted with a stale version or different digest. Decision commit is idempotent and does not itself run the action.
- Provider dispatch requires one atomic authorization boundary rechecking owner, Runtime/lease, Connection, grant, activation, Trust and exact `ApprovalUse`. External provider calls remain outside SQLite.
- Timeout before dispatch can safely fail without claiming a provider effect. Timeout after dispatch is AMBIGUOUS; the system must reconcile exact provider identity/target/request digest before authorizing a new dispatch. Never infer success from an HTTP timeout or an agent assertion.
- Provider redirect destinations are revalidated against the allowlist and SSRF policy. TLS downgrade, private/loopback/link-local/multicast/cloud-metadata destinations and unbounded response bodies fail closed.

## Current implementation gaps

- Discover's `connect-or-install` returns `503 DEPENDENCY_UNAVAILABLE` because LiteSPM package service/contract is not qualified. The `test` route returns UNKNOWN without contacting any provider.
- Connection list/get/disconnect are target-only; no account Connection device-auth begin/poll/cancel/refresh route or Connection UI exists.
- The mounted AgentProfile credential-slot route (Feature 002) now has CODE/SERVED v36 durable replay and idempotent SecretStore behavior. Current Feature 002 V1 modules declare no LiteCowork-owned slot, so they remain unavailable for credential submission; actual daemon-restart/native-keyring qualification is open. This is not an account Connection lifecycle.
- Approval list/get/resolve works and is exact-digest-bound, but approval is not connected to provider Invocation dispatch.
- CapabilityInvocation Operator is read-only. SQLite refuses DISPATCHED admission; Trust is compiled but not wired; no combined Trust/Effect/ApprovalUse transaction or live provider reconciler exists.
- The served grant endpoint is a narrow Task-planning Resource-read grant, not connector authority.

## Accepted V1 additions (revision 1.0)

- Keystore unavailable: Connect fails closed; no plaintext fallback.
- Revoke: REVOKED and grant revocation commit first, then keystore delete; a delete failure is reported and does not reopen the Connection. Remote revocation is CONFIRMED or OWNER_ACTION_REQUIRED, never assumed.
- Restart: pending AuthAttempts become EXPIRED; keystore entries with no committed Connection are removed; DISPATCHED or AMBIGUOUS Invocations go to reconciliation, never to a second POST.
- `connect-or-install` keeps its 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` until package 013 qualifies.

## Canonical error contract

Use errors in `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`, including `DEPENDENCY_UNAVAILABLE`, `CAPABILITY_UNAVAILABLE`, `CAPABILITY_UNHEALTHY`, `SECRET_UNAVAILABLE`, `APPROVAL_REQUIRED`, `POLICY_DENIED`, `SENSITIVE_INPUT_UNSUPPORTED`, `PROVIDER_INPUT_UNSUPPORTED`, `INVOCATION_AMBIGUOUS`, `EFFECT_AMBIGUOUS`, `EFFECT_RECONCILIATION_REQUIRED`, `TIMEOUT`, `CONFLICT` and `IDEMPOTENCY_CONFLICT` where their canonical conditions apply. This package adds no new error codes.

## Qualification gate

Exercise owner/Workspace mismatch, malformed provider device/token responses, duplicate/stale poll, cancel/denial/expiry/slow_down, failed SecretStore write, failed Connection commit, refresh concurrency/rotation/expiry, revoke races, stale grants, exact-action approval/digest mismatch, idempotent replay, provider timeout before/after dispatch, redirects/SSRF, AMBIGUOUS reconciliation, crash/restart and post-revoke denial against the exact golden provider and OS. No end-to-end provider invocation was executed in this audit.
