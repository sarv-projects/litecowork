# Errors and recovery - Golden Connection and One Capability Write

## Target recovery rules

- OAuth cancellation or callback/state mismatch leaves no usable Connection. The first valid terminal callback wins; replayed `state` is refused. Token custody is write-only and the provider token never enters an event, log, agent prompt or ordinary UI response.
- A failed durable Connection commit after SecretStore write purges the newly created secret reference. A lost response replays the original idempotent receipt rather than creating another secret.
- Per-Connection refresh is serialized. `invalid_grant` or provider revocation becomes REAUTH_REQUIRED; transient refresh failure becomes DEGRADED. Neither silently deletes historical Effects or starts a write retry.
- A revoked Connection or revoked/expired CapabilityGrant fences future Invocations. Historical Approval, Invocation, Effect and Evidence records remain readable within their owner retention rules.
- Approval is bound to exact action/plan digest, target, scope, expiry and owner identity. APPROVE cannot be accepted with a stale version or different digest. Decision commit is idempotent and does not itself run the action.
- Provider dispatch requires one atomic authorization boundary rechecking owner, Runtime/lease, Connection, grant, activation, Trust and exact `ApprovalUse`. External provider calls remain outside SQLite.
- Timeout before dispatch can safely fail without claiming a provider effect. Timeout after dispatch is AMBIGUOUS; the system must reconcile exact provider identity/target/request digest before authorizing a new dispatch. Never infer success from an HTTP timeout or an agent assertion.
- Provider redirect destinations are revalidated against the allowlist and SSRF policy. TLS downgrade, private/loopback/link-local/multicast/cloud-metadata destinations and unbounded response bodies fail closed.

## Current implementation gaps

- Discover's `connect-or-install` returns `503 DEPENDENCY_UNAVAILABLE` because LiteSPM package service/contract is not qualified. The `test` route returns UNKNOWN without contacting any provider.
- Connection list/get/disconnect are target-only; no account Connection create/OAuth/callback/refresh route or Connection UI exists.
- The SecretStore primitive exists, but the mounted AgentProfile credential-slot route (Feature 002) currently returns 503 before the write pending durable replay protection. It is not an account Connection lifecycle.
- Approval list/get/resolve works and is exact-digest-bound, but approval is not connected to provider Invocation dispatch.
- CapabilityInvocation Operator is read-only. SQLite refuses DISPATCHED admission; Trust is compiled but not wired; no combined Trust/Effect/ApprovalUse transaction or live provider reconciler exists.
- The served grant endpoint is a narrow Task-planning Resource-read grant, not connector authority.

## Canonical error contract

Use errors in `docs/SCHEMAS.md` and `docs/schemas/error-codes.schema.json`, including `DEPENDENCY_UNAVAILABLE`, `CAPABILITY_UNAVAILABLE`, `CAPABILITY_UNHEALTHY`, `SECRET_UNAVAILABLE`, `APPROVAL_REQUIRED`, `POLICY_DENIED`, `SENSITIVE_INPUT_UNSUPPORTED`, `PROVIDER_INPUT_UNSUPPORTED`, `INVOCATION_AMBIGUOUS`, `EFFECT_AMBIGUOUS`, `EFFECT_RECONCILIATION_REQUIRED`, `TIMEOUT`, `CONFLICT` and `IDEMPOTENCY_CONFLICT` where their canonical conditions apply. This package adds no new error codes.

## Qualification gate

Exercise owner/Workspace mismatch, malformed callback, state replay, cancel, failed SecretStore write, failed Connection commit, refresh concurrency, invalid_grant, revoke races, stale grants, exact-action approval/digest mismatch, idempotent replay, provider timeout before/after dispatch, redirects/SSRF, AMBIGUOUS reconciliation, crash/restart and post-revoke denial against the exact golden provider and OS. No end-to-end provider invocation was executed in this audit.